//! Audiobooks. Browsing goes through the catalog API (`/api/v2/books`: signed cover
//! addresses, authors and narrators, files and chapters in one answer); streams and
//! listening positions through `/api/v1`, as the other own.audio apps do.

use std::{collections::HashMap, sync::Arc};

use serde::{Deserialize, Serialize};
use tokio::{sync::Semaphore, task::JoinSet};

use crate::{
    library::{Artist, Included, Names, Page, Track, V2Contributor, V2Credit, V2Page, cursor, encode},
    session::Auth,
};

#[derive(Debug, Clone, Serialize)]
pub struct Book {
    pub id: String,
    pub title: String,
    pub author: String,
    pub narrator: String,
    pub image: Option<String>,
    pub duration_secs: f64,
    pub progress: Option<Progress>,
}

/// Where the listener is: `position_secs` across the whole book (for "2 h left"),
/// `file_id` + `file_position_secs` for where playing resumes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Progress {
    pub position_secs: f64,
    pub file_id: String,
    pub file_position_secs: f64,
    pub completed: bool,
}

#[derive(Debug, Serialize)]
pub struct Part {
    pub id: String,
    pub position: i32,
    pub title: String,
    pub duration_secs: f64,
}

#[derive(Debug, Serialize)]
pub struct Chapter {
    pub title: String,
    pub start_secs: f64,
    pub file_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Series {
    pub name: String,
    pub position: f64,
}

#[derive(Debug, Serialize)]
pub struct BookDetail {
    pub book: Book,
    pub description: Option<String>,
    pub year: Option<i32>,
    pub series: Vec<Series>,
    pub parts: Vec<Part>,
    pub chapters: Vec<Chapter>,
}

// --- the server's shapes ------------------------------------------------------------

#[derive(Deserialize)]
struct V2Book {
    id: String,
    title: String,
    #[serde(default)]
    credits: Vec<V2Credit>,
    description: Option<String>,
    duration_ms: Option<i64>,
    published_year: Option<i32>,
    image: Option<String>,
    #[serde(default)]
    files: Option<Vec<V2BookFile>>,
    #[serde(default)]
    chapters: Option<Vec<V2Chapter>>,
    #[serde(default)]
    series: Option<Vec<V2Series>>,
}

#[derive(Deserialize)]
struct V2BookAnswer {
    #[serde(flatten)]
    book: V2Book,
    #[serde(default)]
    included: Option<Included>,
}

#[derive(Deserialize)]
struct V2BookFile {
    id: String,
    position: i32,
    title: Option<String>,
    duration_ms: Option<i64>,
}

#[derive(Deserialize)]
struct V2Chapter {
    title: String,
    start_ms: i64,
    file_id: Option<String>,
}

#[derive(Deserialize)]
struct V2Series {
    name: String,
    position: f64,
}

#[derive(Deserialize)]
struct V1Continue {
    kind: String,
    book_id: Option<String>,
}

#[derive(Deserialize)]
struct V1Progress {
    book_id: String,
    #[serde(flatten)]
    progress: Progress,
}

#[derive(Deserialize)]
struct StreamUrl {
    url: String,
}

fn book(b: &V2Book, names: &Names, progress: &HashMap<String, Progress>) -> Book {
    Book {
        id: b.id.clone(),
        title: b.title.clone(),
        author: names.role_line(&b.credits, "author"),
        narrator: names.role_line(&b.credits, "narrator"),
        image: b.image.clone(),
        duration_secs: b.duration_ms.unwrap_or(0) as f64 / 1000.0,
        progress: progress.get(&b.id).cloned(),
    }
}

/// Every position the account has, one call: lists show a progress bar on each book.
async fn progress_map(auth: &Auth) -> HashMap<String, Progress> {
    match auth.get::<Vec<V1Progress>>("/api/v1/playback/books/progress").await {
        Ok(list) => list.into_iter().map(|p| (p.book_id, p.progress)).collect(),
        Err(err) => {
            tracing::warn!(%err, "reading book positions");
            HashMap::new()
        }
    }
}

async fn page(auth: &Auth, path: String) -> anyhow::Result<Page<Book>> {
    let (page, progress) = tokio::join!(auth.get::<V2Page<V2Book>>(&path), progress_map(auth));
    let page = page?;
    let names = Names::from(&page.included);
    Ok(Page { items: page.items.iter().map(|b| book(b, &names, &progress)).collect(), next: page.next })
}

// --- what the screen asks for -------------------------------------------------------

pub async fn books(auth: &Auth, sort: &str, after: Option<&str>) -> anyhow::Result<Page<Book>> {
    let sort = if sort == "added" { "added" } else { "title" };
    page(auth, cursor(format!("/api/v2/books?sort={sort}&limit=40&include=contributors"), after)).await
}

pub async fn author_books(auth: &Auth, author_id: &str) -> anyhow::Result<Vec<Book>> {
    let path = format!("/api/v2/books?contributor_id={}&limit=200&include=contributors", encode(author_id));
    Ok(page(auth, path).await?.items)
}

pub async fn search(auth: &Auth, query: &str) -> anyhow::Result<Vec<Book>> {
    let path = format!("/api/v2/books?q={}&limit=20&include=contributors", encode(query));
    Ok(page(auth, path).await?.items)
}

pub async fn authors(auth: &Auth, after: Option<&str>) -> anyhow::Result<Page<Artist>> {
    let page: V2Page<V2Contributor> =
        auth.get(&cursor("/api/v2/contributors?role=author&limit=60".to_owned(), after)).await?;
    Ok(Page {
        items: page.items.into_iter().map(|c| Artist { id: c.id, name: c.name, image: c.image }).collect(),
        next: page.next,
    })
}

pub async fn detail(auth: &Auth, book_id: &str) -> anyhow::Result<BookDetail> {
    let path = format!("/api/v2/books/{}", encode(book_id));
    let (answer, progress) = tokio::join!(auth.get::<V2BookAnswer>(&path), progress_map(auth));
    let answer = answer?;
    let names = Names::from(&answer.included);
    let b = answer.book;
    let summary = book(&b, &names, &progress);
    let mut parts: Vec<Part> = b
        .files
        .unwrap_or_default()
        .into_iter()
        .map(|f| Part {
            title: f.title.unwrap_or_else(|| format!("Part {}", f.position)),
            id: f.id,
            position: f.position,
            duration_secs: f.duration_ms.unwrap_or(0) as f64 / 1000.0,
        })
        .collect();
    parts.sort_by_key(|p| p.position);
    // Files are often all named after the book ("Leonardo da Vinci" 36 times): numbered parts
    // say more then.
    if parts.len() > 1 && parts.iter().all(|p| p.title == parts[0].title || p.title == b.title) {
        for p in &mut parts {
            p.title = format!("Part {}", p.position);
        }
    }
    Ok(BookDetail {
        book: summary,
        description: b.description,
        year: b.published_year,
        series: b.series.unwrap_or_default().into_iter().map(|s| Series { name: s.name, position: s.position }).collect(),
        chapters: b
            .chapters
            .unwrap_or_default()
            .into_iter()
            .map(|c| Chapter { title: c.title, start_secs: c.start_ms as f64 / 1000.0, file_id: c.file_id })
            .collect(),
        parts,
    })
}

/// Books started and not finished, most recent first, as the server orders them.
pub async fn in_progress(auth: &Arc<Auth>) -> anyhow::Result<Vec<Book>> {
    let items: Vec<V1Continue> = auth.get("/api/v1/library/continue").await?;
    let ids: Vec<String> = items
        .into_iter()
        .filter(|i| i.kind.eq_ignore_ascii_case("book"))
        .filter_map(|i| i.book_id)
        .take(20)
        .collect();
    let progress = Arc::new(progress_map(auth).await);
    let limit = Arc::new(Semaphore::new(6));
    let mut lookups = JoinSet::new();
    for (n, id) in ids.into_iter().enumerate() {
        let (auth, limit, progress) = (Arc::clone(auth), Arc::clone(&limit), Arc::clone(&progress));
        lookups.spawn(async move {
            let _slot = limit.acquire_owned().await;
            let answer: anyhow::Result<V2BookAnswer> = auth.get(&format!("/api/v2/books/{}", encode(&id))).await;
            (n, answer.ok().map(|a| book(&a.book, &Names::from(&a.included), &progress)))
        });
    }
    let mut found = Vec::new();
    while let Some(done) = lookups.join_next().await {
        if let Ok((n, Some(b))) = done {
            found.push((n, b));
        }
    }
    found.sort_by_key(|(n, _)| *n);
    Ok(found.into_iter().map(|(_, b)| b).collect())
}

/// The book as a play queue (one entry per file) and where to start in it: the saved
/// position, or the beginning when asked or when the book was finished.
pub fn queue(detail: &BookDetail, from_start: bool) -> (Vec<Track>, usize, f64) {
    let b = &detail.book;
    let tracks: Vec<Track> = detail
        .parts
        .iter()
        .map(|p| Track {
            id: p.id.clone(),
            title: if detail.parts.len() > 1 { p.title.clone() } else { b.title.clone() },
            artist: b.author.clone(),
            album: Some(b.title.clone()),
            album_id: None,
            image: b.image.clone(),
            duration_secs: p.duration_secs,
            disc: None,
            number: Some(p.position),
            book_id: Some(b.id.clone()),
            podcast_id: None,
        })
        .collect();
    let resume = (!from_start)
        .then_some(b.progress.as_ref())
        .flatten()
        .filter(|p| !p.completed)
        .and_then(|p| tracks.iter().position(|t| t.id == p.file_id).map(|i| (i, p.file_position_secs)));
    let (index, secs) = resume.unwrap_or((0, 0.0));
    (tracks, index, secs)
}

pub async fn stream_url(auth: &Auth, book_id: &str, file_id: &str) -> anyhow::Result<String> {
    let s: StreamUrl =
        auth.get(&format!("/api/v1/audiobooks/{}/files/{}/stream", encode(book_id), encode(file_id))).await?;
    Ok(s.url)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detail(progress: Option<Progress>) -> BookDetail {
        let part = |n: i32, id: &str| Part { id: id.into(), position: n, title: format!("Part {n}"), duration_secs: 600.0 };
        BookDetail {
            book: Book {
                id: "b".into(),
                title: "Book".into(),
                author: "A".into(),
                narrator: String::new(),
                image: None,
                duration_secs: 1800.0,
                progress,
            },
            description: None,
            year: None,
            series: vec![],
            parts: vec![part(1, "f1"), part(2, "f2"), part(3, "f3")],
            chapters: vec![],
        }
    }

    fn at(file: &str, secs: f64, completed: bool) -> Option<Progress> {
        Some(Progress { position_secs: 0.0, file_id: file.into(), file_position_secs: secs, completed })
    }

    #[test]
    fn resumes_in_the_saved_file_at_the_saved_second() {
        let (tracks, index, secs) = queue(&detail(at("f2", 123.5, false)), false);
        assert_eq!((tracks.len(), index, secs), (3, 1, 123.5));
        assert_eq!(tracks[1].book_id.as_deref(), Some("b"));
    }

    #[test]
    fn starts_over_when_asked_finished_or_never_started() {
        assert_eq!(queue(&detail(at("f2", 123.5, false)), true).1, 0);
        assert_eq!(queue(&detail(at("f3", 599.0, true)), false).1, 0);
        assert_eq!(queue(&detail(None), false).1, 0);
    }

    #[test]
    fn an_unknown_file_starts_over() {
        let (_, index, secs) = queue(&detail(at("gone", 10.0, false)), false);
        assert_eq!((index, secs), (0, 0.0));
    }
}
