//! Browsing the music library. The server's catalog API (`/api/v2`) is paged and keeps names
//! apart from ids; this turns it into the few flat shapes the screen needs.

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use anyhow::Context;
use serde::{Deserialize, Serialize};
use tokio::{sync::Semaphore, task::JoinSet};

use crate::session::Auth;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub album_id: Option<String>,
    /// A signed address the browser can load as it is.
    pub image: Option<String>,
    pub duration_secs: f64,
    pub disc: Option<i32>,
    pub number: Option<i32>,
    /// Set when this is one file of an audiobook: streams and progress go through the book.
    #[serde(default)]
    pub book_id: Option<String>,
    /// Set when this is a podcast episode: the show it belongs to.
    #[serde(default)]
    pub podcast_id: Option<String>,
    /// A radio station's stream (see `radio.rs`). Only the box sets it: a page's tracks
    /// come without it.
    #[serde(default)]
    pub stream: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Album {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub image: Option<String>,
    pub year: Option<String>,
    pub track_count: i64,
    pub duration_secs: f64,
}

#[derive(Debug, Serialize)]
pub struct Artist {
    pub id: String,
    pub name: String,
    pub image: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Playlist {
    pub id: String,
    pub name: String,
    pub track_count: i64,
    /// The account's own: it may add to it, take out of it and delete it. A playlist
    /// shared by someone else in the family is only played.
    pub mine: bool,
    /// A smart playlist: a rule the server fills anew at each opening; never changed here.
    pub smart: bool,
    /// Its cover, or its first track's; a signed address the browser can load.
    pub image: Option<String>,
    /// When this box last played it (unix seconds), for the home page's order.
    pub played_at: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub next: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AlbumDetail {
    pub album: Album,
    pub tracks: Vec<Track>,
}

#[derive(Debug, Serialize)]
pub struct SearchResults {
    pub artists: Vec<Artist>,
    pub albums: Vec<Album>,
    pub tracks: Vec<Track>,
}

// --- the server's shapes ------------------------------------------------------------

#[derive(Deserialize)]
pub(crate) struct V2Page<T> {
    pub(crate) items: Vec<T>,
    pub(crate) next: Option<String>,
    #[serde(default)]
    pub(crate) included: Option<Included>,
}

#[derive(Deserialize, Default)]
pub(crate) struct Included {
    #[serde(default)]
    pub(crate) contributors: Vec<V2Contributor>,
    #[serde(default)]
    albums: Vec<V2Album>,
}

#[derive(Deserialize)]
pub(crate) struct V2Credit {
    contributor_id: String,
    role: String,
    position: i16,
    #[serde(default)]
    credited_as: Option<String>,
    #[serde(default)]
    join_phrase: String,
}

#[derive(Deserialize)]
pub(crate) struct V2Contributor {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) image: Option<String>,
}

#[derive(Deserialize)]
struct V2Album {
    id: String,
    title: String,
    #[serde(default)]
    credits: Vec<V2Credit>,
    release_date: Option<String>,
    track_count: i64,
    duration_ms: i64,
    image: Option<String>,
}

#[derive(Deserialize)]
struct V2AlbumAnswer {
    #[serde(flatten)]
    album: V2Album,
    #[serde(default)]
    included: Option<Included>,
}

#[derive(Deserialize)]
struct V2Track {
    id: String,
    title: String,
    album_id: Option<String>,
    #[serde(default)]
    credits: Vec<V2Credit>,
    disc_number: Option<i32>,
    track_number: Option<i32>,
    duration_ms: Option<i64>,
    image: Option<String>,
}

#[derive(Deserialize)]
struct V2TrackAnswer {
    #[serde(flatten)]
    track: V2Track,
    #[serde(default)]
    included: Option<Included>,
}

#[derive(Deserialize)]
struct V2Playlist {
    id: String,
    name: String,
    track_count: i64,
    owner_id: String,
    #[serde(default)]
    image: Option<String>,
}

#[derive(Deserialize)]
struct V2Smart {
    id: String,
    name: String,
    owner_id: String,
}

#[derive(Deserialize)]
struct V2Entry {
    id: String,
    track: V2EntryTrack,
}

#[derive(Deserialize)]
struct V2EntryTrack {
    id: String,
}

#[derive(Deserialize)]
struct V1PlaylistEntry {
    track: V1Track,
}

#[derive(Deserialize)]
struct V1Track {
    id: String,
    title: String,
    #[serde(default)]
    artist: Option<String>,
    album: Option<String>,
    album_id: Option<String>,
    duration_secs: Option<f64>,
    disc_number: Option<i32>,
    track_number: Option<i32>,
}

/// Names by contributor id, from a page's `included`.
pub(crate) struct Names(HashMap<String, String>);

impl Names {
    pub(crate) fn from(included: &Option<Included>) -> Self {
        Self(
            included
                .iter()
                .flat_map(|i| i.contributors.iter())
                .map(|c| (c.id.clone(), c.name.clone()))
                .collect(),
        )
    }

    /// "A feat. B": the performing credits in order, with the server's own join phrases.
    fn credit_line(&self, credits: &[V2Credit]) -> String {
        let mut performing: Vec<&V2Credit> =
            credits.iter().filter(|c| c.role == "artist" || c.role == "featured").collect();
        performing.sort_by_key(|c| c.position);
        let mut line = String::new();
        for (n, c) in performing.iter().enumerate() {
            let name = c.credited_as.clone().or_else(|| self.0.get(&c.contributor_id).cloned());
            line.push_str(name.as_deref().unwrap_or("?"));
            if n + 1 < performing.len() {
                line.push_str(if c.join_phrase.is_empty() { ", " } else { &c.join_phrase });
            }
        }
        line
    }

    /// Everyone credited in `role` ("author", "narrator"), in order, comma-separated.
    pub(crate) fn role_line(&self, credits: &[V2Credit], role: &str) -> String {
        let mut these: Vec<&V2Credit> = credits.iter().filter(|c| c.role == role).collect();
        these.sort_by_key(|c| c.position);
        these
            .iter()
            .map(|c| c.credited_as.clone().or_else(|| self.0.get(&c.contributor_id).cloned()).unwrap_or_else(|| "?".into()))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn year(date: &Option<String>) -> Option<String> {
    date.as_deref().and_then(|d| d.get(..4)).map(str::to_owned)
}

fn album(a: V2Album, names: &Names) -> Album {
    Album {
        artist: names.credit_line(&a.credits),
        year: year(&a.release_date),
        duration_secs: a.duration_ms as f64 / 1000.0,
        id: a.id,
        title: a.title,
        image: a.image,
        track_count: a.track_count,
    }
}

fn track(t: V2Track, names: &Names, albums: &HashMap<String, String>) -> Track {
    Track {
        artist: names.credit_line(&t.credits),
        album: t.album_id.as_ref().and_then(|id| albums.get(id).cloned()),
        duration_secs: t.duration_ms.unwrap_or(0) as f64 / 1000.0,
        id: t.id,
        title: t.title,
        album_id: t.album_id,
        image: t.image,
        disc: t.disc_number,
        number: t.track_number,
        book_id: None,
        podcast_id: None,
        stream: None,
    }
}

pub(crate) fn cursor(path: String, cursor: Option<&str>) -> String {
    match cursor {
        Some(c) if !c.is_empty() => format!("{path}&cursor={}", encode(c)),
        _ => path,
    }
}

pub(crate) fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

// --- what the screen asks for -------------------------------------------------------

pub async fn albums(auth: &Auth, sort: &str, after: Option<&str>) -> anyhow::Result<Page<Album>> {
    let sort = if sort == "added" { "added" } else { "title" };
    let path = cursor(format!("/api/v2/albums?sort={sort}&limit=40&include=contributors"), after);
    let page: V2Page<V2Album> = auth.get(&path).await?;
    let names = Names::from(&page.included);
    Ok(Page { items: page.items.into_iter().map(|a| album(a, &names)).collect(), next: page.next })
}

pub async fn artists(auth: &Auth, after: Option<&str>) -> anyhow::Result<Page<Artist>> {
    let path = cursor("/api/v2/contributors?role=artist&limit=60".to_owned(), after);
    let page: V2Page<V2Contributor> = auth.get(&path).await?;
    Ok(Page {
        items: page.items.into_iter().map(|c| Artist { id: c.id, name: c.name, image: c.image }).collect(),
        next: page.next,
    })
}

pub async fn artist_albums(auth: &Auth, artist_id: &str) -> anyhow::Result<Vec<Album>> {
    let path = format!("/api/v2/albums?contributor_id={}&limit=200&include=contributors", encode(artist_id));
    let page: V2Page<V2Album> = auth.get(&path).await?;
    let names = Names::from(&page.included);
    Ok(page.items.into_iter().map(|a| album(a, &names)).collect())
}

/// Every track on the artist's albums, album by album in the order the artist's page shows
/// them, each album in its own order.
pub async fn artist_tracks(auth: &Arc<Auth>, artist_id: &str) -> anyhow::Result<Vec<Track>> {
    let albums = artist_albums(auth, artist_id).await?;
    let limit = Arc::new(Semaphore::new(4));
    let mut lookups = JoinSet::new();
    for (i, album) in albums.into_iter().enumerate() {
        let (auth, limit) = (Arc::clone(auth), Arc::clone(&limit));
        lookups.spawn(async move {
            let _turn = limit.acquire().await;
            (i, album_detail(&auth, &album.id).await)
        });
    }
    let mut found = Vec::new();
    while let Some(done) = lookups.join_next().await {
        let (i, detail) = done?;
        found.push((i, detail?.tracks));
    }
    found.sort_by_key(|(i, _)| *i);
    let tracks: Vec<Track> = found.into_iter().flat_map(|(_, t)| t).collect();
    anyhow::ensure!(!tracks.is_empty(), "no tracks by this artist");
    Ok(tracks)
}

#[derive(Deserialize)]
struct V2Played {
    id: String,
}

/// The albums the account played last, newest first (its own history, from any device).
/// The server lists ids only; each album is looked up, six at a time.
pub async fn recent_albums(auth: &Arc<Auth>) -> anyhow::Result<Vec<Album>> {
    let played: V2Page<V2Played> = auth.get("/api/v2/me/played?kind=album&order=recent&limit=24").await?;
    let limit = Arc::new(Semaphore::new(6));
    let mut lookups = JoinSet::new();
    for (i, p) in played.items.into_iter().enumerate() {
        let (auth, limit) = (Arc::clone(auth), Arc::clone(&limit));
        lookups.spawn(async move {
            let _turn = limit.acquire().await;
            let answer: anyhow::Result<V2AlbumAnswer> = auth.get(&format!("/api/v2/albums/{}?include=contributors", encode(&p.id))).await;
            (i, answer)
        });
    }
    let mut found = Vec::new();
    while let Some(done) = lookups.join_next().await {
        // An album gone since it was played is left out, not an error.
        if let (i, Ok(answer)) = done? {
            let names = Names::from(&answer.included);
            found.push((i, album(answer.album, &names)));
        }
    }
    found.sort_by_key(|(i, _)| *i);
    Ok(found.into_iter().map(|(_, a)| a).collect())
}

pub async fn album_detail(auth: &Auth, album_id: &str) -> anyhow::Result<AlbumDetail> {
    let id = encode(album_id);
    let answer: V2AlbumAnswer = auth.get(&format!("/api/v2/albums/{}?include=contributors", encode(&id))).await?;
    let names = Names::from(&answer.included);
    let album = album(answer.album, &names);

    let mut tracks = Vec::new();
    let mut after: Option<String> = None;
    loop {
        let path = cursor(format!("/api/v2/tracks?album_id={}&limit=200&include=contributors", encode(&id)), after.as_deref());
        let page: V2Page<V2Track> = auth.get(&path).await?;
        let names = Names::from(&page.included);
        let titles = HashMap::from([(album.id.clone(), album.title.clone())]);
        tracks.extend(page.items.into_iter().map(|t| track(t, &names, &titles)));
        match page.next {
            Some(n) => after = Some(n),
            None => break,
        }
    }
    Ok(AlbumDetail { album, tracks })
}

/// v2's playlists, which say whose each one is (v1's do not).
pub async fn playlists(auth: &Auth) -> anyhow::Result<Vec<Playlist>> {
    let me = auth.user_id().await.unwrap_or_default();
    let mut out = Vec::new();
    let mut after: Option<String> = None;
    loop {
        let page: V2Page<V2Playlist> = auth.get(&cursor("/api/v2/playlists?limit=100".to_owned(), after.as_deref())).await?;
        out.extend(page.items.into_iter().map(|p| playlist_of(p, &me)));
        match page.next {
            Some(n) => after = Some(n),
            None => break,
        }
    }
    // Smart playlists first, as the apps show them. A server from before them answers 404.
    let smart: Vec<Playlist> = match auth.get::<V2Page<V2Smart>>("/api/v2/smart-playlists").await {
        Ok(page) => page
            .items
            .into_iter()
            .map(|p| Playlist {
                mine: !me.is_empty() && p.owner_id == me,
                id: p.id,
                name: p.name,
                track_count: -1,
                smart: true,
                image: None,
                played_at: None,
            })
            .collect(),
        Err(err) => {
            tracing::warn!(%err, "reading smart playlists");
            Vec::new()
        }
    };
    Ok(smart.into_iter().chain(out).collect())
}

/// What a smart playlist holds now, in the order to play it: a new draw each time.
pub async fn smart_tracks(auth: &Auth, id: &str) -> anyhow::Result<Vec<Track>> {
    let path = format!("/api/v2/smart-playlists/{}/resolve?include=contributors,albums", encode(id));
    resolved(auth, &path, &serde_json::json!({})).await
}

/// The server's ready-made rules (forgotten favourites, a genre station…), passed on as
/// they come: the screen names them and sends a rule back to be drawn.
pub async fn smart_presets(auth: &Auth) -> anyhow::Result<serde_json::Value> {
    auth.get("/api/v2/smart-playlists/presets").await
}

/// Draws a rule that is not saved: a preset, or a genre station with its genre filled in.
pub async fn resolve_rule(auth: &Auth, rule: &serde_json::Value) -> anyhow::Result<Vec<Track>> {
    resolved(auth, "/api/v2/smart-playlists/resolve?include=contributors,albums", &serde_json::json!({ "rule": rule })).await
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GenreCount {
    pub genre: String,
    pub track_count: i64,
}

/// The library's genres with their track counts (v1 counts them; v2 does not).
pub async fn genres(auth: &Auth) -> anyhow::Result<Vec<GenreCount>> {
    auth.get("/api/v1/music/genres").await
}

async fn resolved(auth: &Auth, path: &str, body: &serde_json::Value) -> anyhow::Result<Vec<Track>> {
    let resolved: V2Page<V2Track> = auth.send_json_for(reqwest::Method::POST, path, body).await?;
    let names = Names::from(&resolved.included);
    let titles: HashMap<String, String> =
        resolved.included.iter().flat_map(|i| i.albums.iter()).map(|a| (a.id.clone(), a.title.clone())).collect();
    // Most tracks carry no picture of their own: their album's, which the answer includes
    // (asking for each album instead took 6 s for 50 tracks on the radio).
    let covers: HashMap<String, String> = resolved
        .included
        .iter()
        .flat_map(|i| i.albums.iter())
        .filter_map(|a| a.image.clone().map(|img| (a.id.clone(), img)))
        .collect();
    let mut tracks: Vec<Track> = resolved.items.into_iter().map(|t| track(t, &names, &titles)).collect();
    for t in tracks.iter_mut().filter(|t| t.image.is_none()) {
        t.image = t.album_id.as_ref().and_then(|id| covers.get(id).cloned());
    }
    Ok(tracks)
}

fn playlist_of(p: V2Playlist, me: &str) -> Playlist {
    Playlist {
        mine: !me.is_empty() && p.owner_id == me,
        id: p.id,
        name: p.name,
        track_count: p.track_count,
        smart: false,
        image: p.image,
        played_at: None,
    }
}

pub async fn create_playlist(auth: &Auth, name: &str, track_ids: &[String]) -> anyhow::Result<Playlist> {
    let name = name.trim();
    anyhow::ensure!(!name.is_empty(), "the playlist needs a name");
    let body = serde_json::json!({ "name": name, "track_ids": track_ids });
    let made: V2Playlist = auth.send_json_for(reqwest::Method::POST, "/api/v2/playlists", &body).await?;
    Ok(playlist_of(made, &auth.user_id().await.unwrap_or_default()))
}

pub async fn add_to_playlist(auth: &Auth, playlist_id: &str, track_ids: &[String]) -> anyhow::Result<Playlist> {
    anyhow::ensure!(!track_ids.is_empty(), "nothing to add");
    let path = format!("/api/v2/playlists/{}/entries", encode(playlist_id));
    let now: V2Playlist = auth.send_json_for(reqwest::Method::POST, &path, &serde_json::json!({ "track_ids": track_ids })).await?;
    Ok(playlist_of(now, &auth.user_id().await.unwrap_or_default()))
}

/// Takes out the entry at `index` (as the screen lists them, from 0), after checking it is
/// still `track_id`: the playlist may have changed in an app since the screen read it.
pub async fn remove_from_playlist(auth: &Auth, playlist_id: &str, index: usize, track_id: &str) -> anyhow::Result<()> {
    let mut entries: Vec<V2Entry> = Vec::new();
    let mut after: Option<String> = None;
    while entries.len() <= index {
        let path = cursor(format!("/api/v2/playlists/{}/entries?limit=200", encode(playlist_id)), after.as_deref());
        let page: V2Page<V2Entry> = auth.get(&path).await?;
        entries.extend(page.items);
        match page.next {
            Some(n) => after = Some(n),
            None => break,
        }
    }
    let entry = entries.get(index).filter(|e| e.track.id == track_id).context("the playlist has changed; open it again")?;
    auth.delete(&format!("/api/v2/playlists/{}/entries/{}", encode(playlist_id), encode(&entry.id))).await
}

pub async fn delete_playlist(auth: &Auth, playlist_id: &str) -> anyhow::Result<()> {
    auth.delete(&format!("/api/v2/playlists/{}", encode(playlist_id))).await
}

pub async fn playlist_tracks(auth: &Arc<Auth>, playlist_id: &str) -> anyhow::Result<Vec<Track>> {
    let entries: Vec<V1PlaylistEntry> =
        auth.get(&format!("/api/v1/music/playlists/{}/tracks", encode(playlist_id))).await?;
    let mut tracks: Vec<Track> = entries
        .into_iter()
        .map(|e| {
            let t = e.track;
            Track {
                id: t.id,
                title: t.title,
                artist: t.artist.unwrap_or_default(),
                album: t.album,
                album_id: t.album_id,
                image: None,
                duration_secs: t.duration_secs.unwrap_or(0.0),
                disc: t.disc_number,
                number: t.track_number,
                book_id: None,
                podcast_id: None,
                stream: None,
            }
        })
        .collect();

    // v1's `cover_url` needs the session, which the screen's browser can't send; v2's album
    // images are signed addresses it can load. One lookup per album, not per track.
    let albums: HashSet<String> = tracks.iter().filter_map(|t| t.album_id.clone()).collect();
    let images = album_images(auth, albums).await;
    for t in &mut tracks {
        t.image = t.album_id.as_ref().and_then(|id| images.get(id).cloned().flatten());
    }
    Ok(tracks)
}

async fn album_images(auth: &Arc<Auth>, ids: HashSet<String>) -> HashMap<String, Option<String>> {
    let limit = Arc::new(Semaphore::new(6));
    let mut lookups = JoinSet::new();
    for id in ids {
        let (auth, limit) = (Arc::clone(auth), Arc::clone(&limit));
        lookups.spawn(async move {
            let _slot = limit.acquire_owned().await;
            let answer: anyhow::Result<V2AlbumAnswer> = auth.get(&format!("/api/v2/albums/{}", encode(&id))).await;
            (id, answer.ok().and_then(|a| a.album.image))
        });
    }
    let mut images = HashMap::new();
    while let Some(done) = lookups.join_next().await {
        if let Ok((id, image)) = done {
            images.insert(id, image);
        }
    }
    images
}

pub async fn search(auth: &Auth, query: &str) -> anyhow::Result<SearchResults> {
    let q = encode(query);
    let artists: V2Page<V2Contributor> = auth.get(&format!("/api/v2/contributors?role=artist&q={q}&limit=12")).await?;
    let albums: V2Page<V2Album> = auth.get(&format!("/api/v2/albums?q={q}&limit=12&include=contributors")).await?;
    let tracks: V2Page<V2Track> =
        auth.get(&format!("/api/v2/tracks?q={q}&limit=30&include=contributors,albums")).await?;

    let album_names = Names::from(&albums.included);
    let track_names = Names::from(&tracks.included);
    let titles: HashMap<String, String> = tracks
        .included
        .iter()
        .flat_map(|i| i.albums.iter())
        .map(|a| (a.id.clone(), a.title.clone()))
        .collect();
    Ok(SearchResults {
        artists: artists.items.into_iter().map(|c| Artist { id: c.id, name: c.name, image: c.image }).collect(),
        albums: albums.items.into_iter().map(|a| album(a, &album_names)).collect(),
        tracks: tracks.items.into_iter().map(|t| track(t, &track_names, &titles)).collect(),
    })
}

/// The songs marked as favourites, in the server's order. The server lists only their ids,
/// so each is looked up (a few at a time); the list is capped for the screen.
pub async fn favourites(auth: &Arc<Auth>) -> anyhow::Result<Vec<Track>> {
    const MAX: usize = 100;
    let ids: Vec<String> = auth.get("/api/v1/music/starred").await?;
    let limit = Arc::new(Semaphore::new(6));
    let mut lookups = JoinSet::new();
    for (n, id) in ids.into_iter().take(MAX).enumerate() {
        let (auth, limit) = (Arc::clone(auth), Arc::clone(&limit));
        lookups.spawn(async move {
            let _slot = limit.acquire_owned().await;
            let path = format!("/api/v2/tracks/{}?include=contributors,albums", encode(&id));
            let answer: anyhow::Result<V2TrackAnswer> = auth.get(&path).await;
            (n, answer.ok())
        });
    }
    let mut found = Vec::new();
    while let Some(done) = lookups.join_next().await {
        // A favourite that has since been deleted is left out, not an error.
        if let Ok((n, Some(answer))) = done {
            let names = Names::from(&answer.included);
            let titles: HashMap<String, String> = answer
                .included
                .iter()
                .flat_map(|i| i.albums.iter())
                .map(|a| (a.id.clone(), a.title.clone()))
                .collect();
            found.push((n, track(answer.track, &names, &titles)));
        }
    }
    found.sort_by_key(|(n, _)| *n);
    Ok(found.into_iter().map(|(_, t)| t).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn credit(id: &str, role: &str, position: i16, join: &str) -> V2Credit {
        V2Credit {
            contributor_id: id.into(),
            role: role.into(),
            position,
            credited_as: None,
            join_phrase: join.into(),
        }
    }

    #[test]
    fn credit_line_follows_positions_and_join_phrases() {
        let names = Names(HashMap::from([("a".into(), "Blur".into()), ("b".into(), "Damon".into())]));
        let credits = [credit("b", "featured", 1, ""), credit("a", "artist", 0, " feat. ")];
        assert_eq!(names.credit_line(&credits), "Blur feat. Damon");
    }

    #[test]
    fn credit_line_skips_other_roles() {
        let names = Names(HashMap::from([("a".into(), "Blur".into()), ("p".into(), "Producer".into())]));
        let credits = [credit("a", "artist", 0, ""), credit("p", "producer", 1, "")];
        assert_eq!(names.credit_line(&credits), "Blur");
    }

    #[test]
    fn year_from_a_full_or_partial_date() {
        assert_eq!(year(&Some("1997-02-10".into())).as_deref(), Some("1997"));
        assert_eq!(year(&Some("1997".into())).as_deref(), Some("1997"));
        assert_eq!(year(&None), None);
    }

    #[test]
    fn cursors_are_encoded() {
        assert_eq!(cursor("/x?a=1".into(), Some("ab/c+=")), "/x?a=1&cursor=ab%2Fc%2B%3D");
        assert_eq!(cursor("/x?a=1".into(), None), "/x?a=1");
    }
}
