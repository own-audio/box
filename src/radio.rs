//! Internet radio: stations from radio-browser.info (an open, community-kept directory),
//! favourites kept on the box. The box asks the directory for a station by its id and
//! plays the address the directory gives; a page never hands the player an address of its
//! own, so a phone cannot make mpv open a file on the box.

use std::time::Duration;

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};

/// The directory's mirrors, tried in turn (its own advice is to pick one of its servers).
const MIRRORS: [&str; 3] = [
    "https://de1.api.radio-browser.info",
    "https://fi1.api.radio-browser.info",
    "https://de2.api.radio-browser.info",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Station {
    /// radio-browser's `stationuuid`.
    pub id: String,
    pub name: String,
    /// The stream, as resolved by the directory (playlists unwrapped).
    pub url: String,
    #[serde(default)]
    pub logo: Option<String>,
    #[serde(default)]
    pub country: Option<String>,
    #[serde(default)]
    pub codec: Option<String>,
    #[serde(default)]
    pub bitrate: Option<u32>,
}

#[derive(Deserialize)]
struct Raw {
    stationuuid: String,
    name: String,
    url_resolved: String,
    #[serde(default)]
    favicon: String,
    #[serde(default)]
    countrycode: String,
    #[serde(default)]
    codec: String,
    #[serde(default)]
    bitrate: u32,
}

fn web(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://")
}

impl Raw {
    fn station(self) -> Option<Station> {
        let name = self.name.trim().to_owned();
        if name.is_empty() || !web(&self.url_resolved) {
            return None;
        }
        Some(Station {
            id: self.stationuuid,
            name,
            url: self.url_resolved,
            logo: Some(self.favicon).filter(|f| f.starts_with("https://")),
            country: Some(self.countrycode).filter(|c| !c.is_empty()),
            codec: Some(self.codec).filter(|c| !c.is_empty()),
            bitrate: Some(self.bitrate).filter(|b| *b > 0),
        })
    }
}

async fn ask(http: &reqwest::Client, path_and_query: &str) -> anyhow::Result<Vec<Station>> {
    let mut last = None;
    for mirror in MIRRORS {
        let result = http
            .get(format!("{mirror}{path_and_query}"))
            .timeout(Duration::from_secs(8))
            .send()
            .await
            .and_then(reqwest::Response::error_for_status);
        match result {
            Ok(r) => {
                let raw: Vec<Raw> = r.json().await.context("the radio directory's answer")?;
                return Ok(raw.into_iter().filter_map(Raw::station).collect());
            }
            Err(err) => last = Some(err),
        }
    }
    Err(last.map_or_else(|| anyhow::anyhow!("no radio directory"), Into::into))
}

/// Stations by name (empty: the most liked), in a country (`CZ`) or anywhere. The
/// directory matches without accents ("slager" finds "Sláger FM").
pub async fn search(http: &reqwest::Client, query: &str, country: Option<&str>) -> anyhow::Result<Vec<Station>> {
    let mut q = format!(
        "/json/stations/search?hidebroken=true&order=votes&reverse=true&limit=60&name={}",
        crate::library::encode(query.trim())
    );
    if let Some(c) = country.filter(|c| c.len() == 2 && c.chars().all(|ch| ch.is_ascii_alphabetic())) {
        q.push_str(&format!("&countrycode={}", c.to_ascii_uppercase()));
    }
    ask(http, &q).await
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Country {
    /// ISO 3166-1 alpha-2 (`HU`).
    #[serde(rename(deserialize = "iso_3166_1"))]
    pub code: String,
    pub stationcount: u32,
}

/// Countries with stations, the most stations first.
pub async fn countries(http: &reqwest::Client) -> anyhow::Result<Vec<Country>> {
    let mut last = None;
    for mirror in MIRRORS {
        match http.get(format!("{mirror}/json/countries")).timeout(Duration::from_secs(8)).send().await.and_then(reqwest::Response::error_for_status) {
            Ok(r) => {
                let mut list: Vec<Country> = r.json().await.context("the radio directory's countries")?;
                list.retain(|c| c.code.len() == 2 && c.stationcount >= 5);
                list.sort_by_key(|c| std::cmp::Reverse(c.stationcount));
                return Ok(list);
            }
            Err(err) => last = Some(err),
        }
    }
    Err(last.map_or_else(|| anyhow::anyhow!("no radio directory"), Into::into))
}

/// One station by its id, as the directory has it now.
pub async fn by_id(http: &reqwest::Client, id: &str) -> anyhow::Result<Station> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_hexdigit() || c == '-') {
        bail!("not a station id");
    }
    ask(http, &format!("/json/stations/byuuid/{id}")).await?.into_iter().next().context("no such station")
}

/// How many stations "Recently played" keeps.
pub const RECENT: usize = 12;

/// `station` first in the list of stations played lately, once.
pub fn remember(recent: &mut Vec<Station>, station: &Station) {
    recent.retain(|r| r.id != station.id);
    recent.insert(0, station.clone());
    recent.truncate(RECENT);
}

/// A station as the player's queue keeps it: no length, no album; the address in `stream`.
pub fn track(s: &Station) -> crate::library::Track {
    crate::library::Track {
        id: s.id.clone(),
        title: s.name.clone(),
        artist: String::new(),
        album: None,
        album_id: None,
        image: s.logo.clone(),
        duration_secs: 0.0,
        disc: None,
        number: None,
        book_id: None,
        podcast_id: None,
        stream: Some(s.url.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_latest_station_comes_first_once() {
        let st = |id: &str| Station { id: id.into(), name: id.into(), url: "https://a/b".into(), logo: None, country: None, codec: None, bitrate: None };
        let mut recent = vec![st("a"), st("b")];
        remember(&mut recent, &st("b"));
        assert_eq!(recent.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(), ["b", "a"]);
        for i in 0..20 {
            remember(&mut recent, &st(&i.to_string()));
        }
        assert_eq!(recent.len(), RECENT);
        assert_eq!(recent[0].id, "19");
    }

    #[test]
    fn a_station_needs_a_name_and_a_web_address() {
        let raw = |name: &str, url: &str, favicon: &str| Raw {
            stationuuid: "9617a958-0601-11e8-ae97-52543be04c81".into(),
            name: name.into(),
            url_resolved: url.into(),
            favicon: favicon.into(),
            countrycode: "CZ".into(),
            codec: "MP3".into(),
            bitrate: 128,
        };
        let s = raw(" ČRo Radiožurnál ", "https://rozhlas.stream/radiozurnal.mp3", "https://example.cz/logo.png").station().unwrap();
        assert_eq!(s.name, "ČRo Radiožurnál");
        assert_eq!(s.logo.as_deref(), Some("https://example.cz/logo.png"));
        assert!(raw("x", "file:///etc/passwd", "").station().is_none(), "only web addresses play");
        assert!(raw("", "https://a/b", "").station().is_none());
        assert_eq!(raw("x", "https://a/b", "http://insecure/logo.png").station().unwrap().logo, None);
    }
}
