//! Podcasts. Shows come from the catalog API (`/api/v2/podcasts`: credits, artwork);
//! episode lists from `/api/v1`, where each episode carries the listener's position and
//! whether it was played, so one call fills a show page with progress.
//!
//! An episode the server has not stored yet answers its stream with 409: the server is
//! asked to fetch it first, as the other own.audio apps do, so the box itself never
//! contacts the podcast's host.

use std::{collections::HashMap, sync::Arc, time::Duration};

use anyhow::bail;
use reqwest::{Method, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::{sync::Semaphore, task::JoinSet};

use crate::{
    library::{Names, Track, V2Credit, V2Page, encode},
    session::Auth,
};

/// How long the server may take to fetch an episode before playing it.
const FETCH_TIMEOUT: Duration = Duration::from_secs(180);

#[derive(Debug, Clone, Serialize)]
pub struct Show {
    pub id: String,
    pub title: String,
    pub host: String,
    pub image: Option<String>,
    pub description: Option<String>,
    pub unplayed: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Episode {
    pub id: String,
    pub show_id: String,
    pub show_title: String,
    pub title: String,
    pub description: Option<String>,
    pub published_at: Option<String>,
    pub duration_secs: f64,
    pub image: Option<String>,
    pub progress_secs: f64,
    pub completed: bool,
}

// --- the server's shapes ------------------------------------------------------------

#[derive(Deserialize)]
struct V2Podcast {
    id: String,
    title: String,
    #[serde(default)]
    credits: Vec<V2Credit>,
    description: Option<String>,
    image: Option<String>,
}

#[derive(Deserialize)]
struct V1Unplayed {
    feed_id: String,
    unplayed: i64,
}

#[derive(Deserialize)]
struct V1Episode {
    id: String,
    feed_id: String,
    title: String,
    description: Option<String>,
    published_at: Option<String>,
    duration_secs: Option<f64>,
    image_url: Option<String>,
    #[serde(default)]
    progress_secs: Option<f64>,
    #[serde(default)]
    completed: bool,
}

#[derive(Deserialize)]
struct V1Continue {
    kind: String,
    episode_id: Option<String>,
    feed_id: Option<String>,
    episode_title: Option<String>,
    feed_title: Option<String>,
    position_secs: Option<f64>,
    duration_secs: Option<f64>,
}

#[derive(Deserialize)]
struct StreamUrl {
    url: String,
}

/// Feed descriptions are often HTML; the screen shows text. Tags go, paragraph breaks stay,
/// and the common entities are decoded.
fn plain(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut tag = String::new();
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => {
                in_tag = true;
                tag.clear();
            }
            '>' if in_tag => {
                in_tag = false;
                let name = tag.trim_start_matches('/').split_whitespace().next().unwrap_or("").to_ascii_lowercase();
                if matches!(name.as_str(), "p" | "br" | "br/" | "div" | "li") && !out.ends_with('\n') {
                    out.push('\n');
                }
            }
            _ if in_tag => tag.push(ch),
            _ => out.push(ch),
        }
    }
    let out = out
        .replace("&nbsp;", " ")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&");
    out.trim().to_owned()
}

fn episode(e: V1Episode, show: &Show) -> Episode {
    Episode {
        id: e.id,
        show_id: e.feed_id,
        show_title: show.title.clone(),
        title: e.title,
        description: e.description.as_deref().map(plain),
        published_at: e.published_at,
        duration_secs: e.duration_secs.unwrap_or(0.0),
        // Episode artwork when the feed has its own, the show's otherwise.
        image: e.image_url.or_else(|| show.image.clone()),
        progress_secs: e.progress_secs.unwrap_or(0.0),
        completed: e.completed,
    }
}

// --- what the screen asks for -------------------------------------------------------

pub async fn shows(auth: &Auth) -> anyhow::Result<Vec<Show>> {
    let (page, unplayed) = tokio::join!(
        auth.get::<V2Page<V2Podcast>>("/api/v2/podcasts?limit=100&include=contributors"),
        auth.get::<Vec<V1Unplayed>>("/api/v1/podcasts/unplayed-counts"),
    );
    let page = page?;
    let unplayed: HashMap<String, i64> =
        unplayed.unwrap_or_default().into_iter().map(|u| (u.feed_id, u.unplayed)).collect();
    let names = Names::from(&page.included);
    Ok(page
        .items
        .into_iter()
        .map(|p| Show {
            host: names.role_line(&p.credits, "host"),
            unplayed: unplayed.get(&p.id).copied().unwrap_or(0),
            id: p.id,
            title: p.title,
            image: p.image,
            description: p.description,
        })
        .collect())
}

async fn show(auth: &Auth, show_id: &str) -> anyhow::Result<Show> {
    shows(auth).await?.into_iter().find(|s| s.id == show_id).ok_or_else(|| anyhow::anyhow!("no such podcast"))
}

/// A show and a page of its episodes, newest first.
pub async fn episodes(auth: &Auth, show_id: &str, offset: usize) -> anyhow::Result<(Show, Vec<Episode>)> {
    let path = format!("/api/v1/podcasts/{}/episodes?limit=50&offset={offset}", encode(show_id));
    let (show, list) = tokio::join!(show(auth, show_id), auth.get::<Vec<V1Episode>>(&path));
    let show = show?;
    let list = list?.into_iter().map(|e| episode(e, &show)).collect();
    Ok((show, list))
}

/// Episodes started and not finished, most recent first.
pub async fn in_progress(auth: &Auth) -> anyhow::Result<Vec<Episode>> {
    let (items, shows) = tokio::join!(auth.get::<Vec<V1Continue>>("/api/v1/library/continue"), shows(auth));
    let images: HashMap<String, Option<String>> =
        shows.unwrap_or_default().into_iter().map(|s| (s.id, s.image)).collect();
    Ok(items?
        .into_iter()
        .filter(|i| i.kind.eq_ignore_ascii_case("episode"))
        .filter_map(|i| {
            let show_id = i.feed_id?;
            Some(Episode {
                id: i.episode_id?,
                image: images.get(&show_id).cloned().flatten(),
                show_title: i.feed_title.unwrap_or_default(),
                show_id,
                title: i.episode_title.unwrap_or_default(),
                description: None,
                published_at: None,
                duration_secs: i.duration_secs.unwrap_or(0.0),
                progress_secs: i.position_secs.unwrap_or(0.0),
                completed: false,
            })
        })
        .take(20)
        .collect())
}

/// The newest episodes across all shows, unplayed ones only.
pub async fn latest(auth: &Arc<Auth>) -> anyhow::Result<Vec<Episode>> {
    let shows = shows(auth).await?;
    let limit = Arc::new(Semaphore::new(4));
    let mut lookups = JoinSet::new();
    for show in shows.into_iter().take(30) {
        let (auth, limit) = (Arc::clone(auth), Arc::clone(&limit));
        lookups.spawn(async move {
            let _slot = limit.acquire_owned().await;
            let path = format!("/api/v1/podcasts/{}/episodes?limit=10&offset=0", encode(&show.id));
            let list: Vec<V1Episode> = auth.get(&path).await.unwrap_or_default();
            list.into_iter().map(|e| episode(e, &show)).collect::<Vec<_>>()
        });
    }
    let mut all = Vec::new();
    while let Some(done) = lookups.join_next().await {
        all.extend(done.unwrap_or_default());
    }
    all.retain(|e| !e.completed);
    // RFC 3339 dates sort as text.
    all.sort_by(|a, b| b.published_at.cmp(&a.published_at));
    all.truncate(30);
    Ok(all)
}

pub async fn mark(auth: &Auth, ids: &[String], completed: bool) -> anyhow::Result<()> {
    auth.send_json(Method::POST, "/api/v1/playback/episodes/progress/bulk", &json!({ "episode_ids": ids, "completed": completed }))
        .await
}

/// Episodes as a play queue: the chosen one and, after it, the unplayed ones that follow
/// in the list (so a show plays on), starting at the saved second.
pub fn queue(list: &[Episode], start: usize) -> (Vec<Track>, f64) {
    let tracks: Vec<Track> = list
        .iter()
        .enumerate()
        .skip(start)
        .filter(|(i, e)| *i == start || !e.completed)
        .take(20)
        .map(|(_, e)| Track {
            id: e.id.clone(),
            title: e.title.clone(),
            artist: e.show_title.clone(),
            album: Some(e.show_title.clone()),
            album_id: None,
            image: e.image.clone(),
            duration_secs: e.duration_secs,
            disc: None,
            number: None,
            book_id: None,
            podcast_id: Some(e.show_id.clone()),
            stream: None,
        })
        .collect();
    let first = &list[start];
    let secs = if first.completed { 0.0 } else { first.progress_secs };
    (tracks, secs)
}

pub async fn stream_url(auth: &Auth, show_id: &str, episode_id: &str) -> anyhow::Result<String> {
    let path = format!("/api/v1/podcasts/{}/episodes/{}/stream", encode(show_id), encode(episode_id));
    let (status, body) = auth.get_with_status(&path).await?;
    let body = if status == StatusCode::CONFLICT {
        // Not stored on the server yet: have it fetched, then ask again.
        auth.post_slow(&format!("/api/v1/podcasts/{}/episodes/{}/download", encode(show_id), encode(episode_id)), FETCH_TIMEOUT)
            .await?;
        let (status, body) = auth.get_with_status(&path).await?;
        if !status.is_success() {
            bail!("the episode could not be fetched (HTTP {})", status.as_u16());
        }
        body
    } else if status.is_success() {
        body
    } else {
        bail!("server error (HTTP {})", status.as_u16());
    };
    Ok(serde_json::from_value::<StreamUrl>(body)?.url)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ep(id: &str, completed: bool, progress: f64) -> Episode {
        Episode {
            id: id.into(),
            show_id: "s".into(),
            show_title: "Show".into(),
            title: id.into(),
            description: None,
            published_at: None,
            duration_secs: 600.0,
            image: None,
            progress_secs: progress,
            completed,
        }
    }

    #[test]
    fn plays_on_through_the_unplayed_ones_after_the_chosen_episode() {
        let list = [ep("a", false, 0.0), ep("b", false, 42.0), ep("c", true, 0.0), ep("d", false, 0.0)];
        let (tracks, secs) = queue(&list, 1);
        let ids: Vec<&str> = tracks.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, ["b", "d"]);
        assert_eq!(secs, 42.0);
        assert_eq!(tracks[0].podcast_id.as_deref(), Some("s"));
    }

    #[test]
    fn descriptions_lose_their_markup() {
        assert_eq!(plain("<p>Rock &amp; roll</p><p>Part&nbsp;2<br/>end</p>"), "Rock & roll\nPart 2\nend");
        assert_eq!(plain("no markup"), "no markup");
    }

    #[test]
    fn a_played_episode_chosen_again_starts_over() {
        let list = [ep("a", true, 600.0)];
        let (tracks, secs) = queue(&list, 0);
        assert_eq!((tracks.len(), secs), (1, 0.0));
    }
}
