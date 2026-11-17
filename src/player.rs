//! The play queue. mpv holds at most two entries: the track playing and the one after it, so
//! the next one is already open when the first ends (gapless) while every stream address is
//! fetched only shortly before it is needed (they are signed and expire).
//!
//! An audiobook is a queue too, one entry per file. For books the player also keeps the
//! listening position on the server (every 30 s, on pause, between files, at the end), so
//! the phone continues where the box stopped; and for everything it reports listening
//! sessions, so the account's statistics include the box.

use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use anyhow::Context;
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::Mutex;

use crate::{books, library::Track, mpv::Mpv, podcasts, prefs::PrefsStore, session::Auth};

const TICK: Duration = Duration::from_millis(500);
/// The place in a book or episode goes to the server this often while it plays.
const SAVE_EVERY: Duration = Duration::from_secs(30);
/// …and to the SD card only this often: pause, skip and stop save at once anyway, and a
/// cheap card wears with every write. A power cut loses at most this much of a song.
const PERSIST_EVERY: Duration = Duration::from_secs(120);
/// Shorter listens are not worth a session in the statistics.
const MIN_SESSION_SECS: f64 = 5.0;

/// When to stop for the night.
#[derive(Debug, Clone, Copy)]
enum Sleep {
    At(Instant),
    /// At the end of the file playing (a part of a book, a song).
    EndOfPart,
}

/// One stretch of listening to one entry, reported to the server when it ends.
struct Listening {
    track: Track,
    started_at: String,
    listened_secs: f64,
    id: String,
}

#[derive(Default)]
struct Queue {
    tracks: Vec<Track>,
    index: usize,
    /// Whether mpv is working through this queue (false once it ran out or was stopped).
    active: bool,
    paused: bool,
    last_saved: Option<Instant>,
    last_persisted: Option<Instant>,
    listening: Option<Listening>,
    sleep: Option<Sleep>,
    /// A queue read back from disk after a restart: shown as paused at this second, and
    /// loaded into mpv only when played (so starting needs no network).
    restored: Option<f64>,
    /// The mpv the queue was loaded into, and the last position it reported: if mpv is
    /// restarted (a crash), the queue stops there instead of counting as finished.
    mpv_generation: u64,
    last_position: f64,
    /// When the current pause began (see `release_if_paused_for`).
    paused_since: Option<Instant>,
}

/// What survives a restart of ownbox or the Pi: the queue and where in it.
#[derive(Serialize, Deserialize)]
struct SavedQueue {
    tracks: Vec<Track>,
    index: usize,
    position_secs: f64,
}

pub struct Player {
    mpv: Arc<Mpv>,
    auth: Arc<Auth>,
    prefs: Arc<PrefsStore>,
    queue: Mutex<Queue>,
    sessions: AtomicU64,
    saved_queue: PathBuf,
}

#[derive(Debug, Serialize)]
pub struct NowPlaying {
    pub state: &'static str,
    /// `music`, `book` or `podcast`: spoken word gets ±30 s, speed and the sleep timer.
    pub kind: &'static str,
    pub track: Option<Track>,
    pub index: usize,
    pub queue_length: usize,
    pub position_secs: f64,
    pub duration_secs: f64,
    pub volume: f64,
    pub speed: f64,
    /// Seconds until the sleep timer stops playback, when one is set for a time.
    pub sleep_in_secs: Option<f64>,
    /// The sleep timer waits for the end of the file playing.
    pub sleep_end_of_part: bool,
}

#[derive(Debug, Serialize)]
pub struct QueueView {
    pub index: usize,
    pub active: bool,
    pub tracks: Vec<Track>,
}

#[derive(Deserialize)]
pub struct SleepRequest {
    /// Minutes from now; `None` with `end_of_part` false turns the timer off.
    pub minutes: Option<u64>,
    #[serde(default)]
    pub end_of_part: bool,
}

#[derive(Deserialize)]
struct StreamUrl {
    url: String,
}

impl Player {
    pub fn new(mpv: Arc<Mpv>, auth: Arc<Auth>, prefs: Arc<PrefsStore>, saved_queue: PathBuf) -> Arc<Self> {
        if let Some(dir) = saved_queue.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let mut queue = Queue::default();
        match std::fs::read_to_string(&saved_queue).map(|t| serde_json::from_str::<SavedQueue>(&t)) {
            Ok(Ok(saved)) if saved.index < saved.tracks.len() => {
                tracing::info!(tracks = saved.tracks.len(), index = saved.index, "restored the queue");
                queue.tracks = saved.tracks;
                queue.index = saved.index;
                queue.restored = Some(saved.position_secs);
            }
            Ok(_) => tracing::warn!(path = %saved_queue.display(), "unreadable saved queue; starting empty"),
            Err(_) => {}
        }
        let player = Arc::new(Self { mpv, auth, prefs, queue: Mutex::new(queue), sessions: AtomicU64::new(0), saved_queue });
        tokio::spawn(Arc::clone(&player).follow());
        player
    }

    /// Writes the queue and the place in it, for the next start. Small and local, so done
    /// in line; a temporary file and a rename keep a half-written file from ever being read.
    fn persist(&self, q: &Queue, position: f64) {
        let saved = SavedQueue { tracks: q.tracks.clone(), index: q.index, position_secs: position.max(0.0) };
        let tmp = self.saved_queue.with_extension("tmp");
        let written = serde_json::to_vec(&saved)
            .map_err(std::io::Error::other)
            .and_then(|bytes| std::fs::write(&tmp, bytes))
            .and_then(|()| std::fs::rename(&tmp, &self.saved_queue));
        if let Err(err) = written {
            tracing::warn!(%err, "saving the queue");
        }
    }

    fn forget_persisted(&self) {
        let _ = std::fs::remove_file(&self.saved_queue);
    }

    /// Lets the sound card go for an outside source (AirPlay, Music Assistant): the place is
    /// saved and kept, as after a restart, and mpv stops, which closes the card.
    pub async fn release(&self) {
        let mut q = self.queue.lock().await;
        if q.active {
            let position = self.property_f64("time-pos").await;
            if let Some(p) = position {
                self.save_position(&q, p, false);
                self.persist(&q, p);
            }
            self.end_listening(&mut q, "stopped").await;
            let at = position.unwrap_or(q.last_position);
            q.active = false;
            q.paused = true;
            q.restored = Some(at);
        }
        let _ = self.mpv.command(json!(["stop"])).await;
    }

    /// A pause that has lasted lets the sound card go: the card is one opener at a time,
    /// and a paused mpv keeps it, so Music Assistant's squeezelite (which has no hook to
    /// ask for it) could not play. Play picks up at the same second, as after a restart.
    pub async fn release_if_paused_for(&self, after: Duration) {
        let since = {
            let q = self.queue.lock().await;
            if !q.active || !q.paused {
                return;
            }
            q.paused_since
        };
        if since.is_some_and(|t| t.elapsed() >= after) {
            tracing::info!("paused a while: letting the sound card go");
            self.release().await;
        }
    }

    /// Empties the queue and stops mpv: the box is moving to another server, whose library
    /// the old queue does not belong to.
    pub async fn clear(&self) {
        let mut q = self.queue.lock().await;
        let _ = self.mpv.command(json!(["stop"])).await;
        *q = Queue::default();
        self.forget_persisted();
    }

    /// At shutdown (the power button, a deploy): the place goes to the server and to disk,
    /// waited for this time, since the process is about to end.
    pub async fn shutdown(&self) {
        let mut q = self.queue.lock().await;
        if !q.active {
            return;
        }
        // No answer from mpv means no position: better keep the last saved one than write 0.
        let Some(position) = self.property_f64("time-pos").await else {
            tracing::warn!("mpv gave no position at shutdown; keeping the last saved one");
            return;
        };
        self.persist(&q, position);
        if let Some(track) = q.tracks.get(q.index).cloned()
            && let Some((path, body)) = Self::progress_request(&track, position, false, false)
            && let Err(err) = self.auth.send_json(Method::PUT, &path, &body).await
        {
            tracing::warn!(%err, "saving the listening position at shutdown");
        }
        if let Some(body) = self.listening_report(&mut q, "stopped").await
            && let Err(err) = self.auth.send_json(Method::POST, "/api/v1/playback/sessions", &body).await
        {
            tracing::warn!(%err, "reporting the listening session at shutdown");
        }
        tracing::info!(position, "saved the place for the next start");
    }

    async fn stream_url(&self, track: &Track) -> anyhow::Result<String> {
        if let Some(book_id) = &track.book_id {
            return books::stream_url(&self.auth, book_id, &track.id).await;
        }
        if let Some(show_id) = &track.podcast_id {
            return podcasts::stream_url(&self.auth, show_id, &track.id).await;
        }
        let s: StreamUrl = self.auth.get(&format!("/api/v1/music/tracks/{}/stream", crate::library::encode(&track.id))).await?;
        Ok(s.url)
    }

    /// Replaces the queue and starts at `start`, `start_secs` into it.
    pub async fn play(&self, tracks: Vec<Track>, start: usize, start_secs: f64) -> anyhow::Result<()> {
        anyhow::ensure!(start < tracks.len(), "nothing to play");
        let mut q = self.queue.lock().await;
        self.leave(&mut q, "replaced").await;
        q.tracks = tracks;
        q.index = start;
        q.sleep = None;
        self.load_from(&mut q, start_secs).await
    }

    /// Loads the current entry and the one after it into mpv, replacing what was there.
    async fn load_from(&self, q: &mut Queue, start_secs: f64) -> anyhow::Result<()> {
        let track = q.tracks[q.index].clone();
        let url = self.stream_url(&track).await?;
        if start_secs > 0.0 {
            // loadfile <url> <flags> <index> <options>: start inside the file, not at 0.
            self.mpv.command(json!(["loadfile", url, "replace", -1, format!("start={start_secs:.1}")])).await?;
        } else {
            self.mpv.command(json!(["loadfile", url, "replace"])).await?;
        }
        self.mpv.command(json!(["set_property", "speed", self.speed_for(&track)])).await?;
        self.mpv.command(json!(["set_property", "pause", false])).await?;
        if let Some(next) = q.tracks.get(q.index + 1) {
            // The first track already plays; a failure here only costs the gapless join.
            match self.stream_url(next).await {
                Ok(url) => {
                    let _ = self.mpv.command(json!(["loadfile", url, "append"])).await;
                }
                Err(err) => tracing::warn!(%err, "preparing the next track"),
            }
        }
        q.active = true;
        q.paused = false;
        q.restored = None;
        q.mpv_generation = self.mpv.generation();
        q.last_position = start_secs;
        self.start_listening(q);
        self.save_position(q, start_secs, false);
        self.persist(q, start_secs);
        Ok(())
    }

    /// Before playing something else: keep the place in what was playing, close its session.
    async fn leave(&self, q: &mut Queue, reason: &str) {
        if q.active
            && let Some(position) = self.property_f64("time-pos").await
        {
            self.save_position(q, position, false);
        }
        self.end_listening(q, reason).await;
    }

    /// Spoken word keeps a speed per book or per show; music always plays at 1×.
    fn speed_key(track: &Track) -> Option<String> {
        track.book_id.clone().or_else(|| track.podcast_id.clone())
    }

    fn speed_for(&self, track: &Track) -> f64 {
        Self::speed_key(track).and_then(|k| self.prefs.get().book_speeds.get(&k).copied()).unwrap_or(1.0)
    }

    /// Pause or play, as asked: unlike `toggle`, sending it twice changes nothing.
    pub async fn set_paused(&self, paused: bool) -> anyhow::Result<()> {
        let mut q = self.queue.lock().await;
        if let Some(secs) = q.restored {
            return if paused { Ok(()) } else { self.load_from(&mut q, secs).await };
        }
        if !q.active {
            return Ok(());
        }
        self.mpv.command(json!(["set_property", "pause", paused])).await?;
        self.paused_changed(&mut q, paused).await;
        Ok(())
    }

    pub async fn toggle(&self) -> anyhow::Result<()> {
        let mut q = self.queue.lock().await;
        if let Some(secs) = q.restored {
            return self.load_from(&mut q, secs).await;
        }
        self.mpv.command(json!(["cycle", "pause"])).await?;
        let paused = self.property_bool("pause").await.unwrap_or(false);
        self.paused_changed(&mut q, paused).await;
        Ok(())
    }

    /// Pausing saves the place and closes the listening session; resuming opens a new one.
    async fn paused_changed(&self, q: &mut Queue, paused: bool) {
        if paused == q.paused || !q.active {
            return;
        }
        q.paused = paused;
        q.paused_since = paused.then(Instant::now);
        if paused {
            if let Some(position) = self.property_f64("time-pos").await {
                self.save_position(q, position, false);
                self.persist(q, position);
            }
            self.end_listening(q, "stopped").await;
        } else {
            self.start_listening(q);
        }
    }

    pub async fn next(&self) -> anyhow::Result<()> {
        let mut q = self.queue.lock().await;
        anyhow::ensure!(q.index + 1 < q.tracks.len(), "this is the last track");
        self.leave(&mut q, "skipped").await;
        q.index += 1;
        self.load_from(&mut q, 0.0).await
    }

    /// Back to the start of the track, or to the previous one when near the start already.
    pub async fn previous(&self) -> anyhow::Result<()> {
        let position = self.property_f64("time-pos").await.unwrap_or(0.0);
        let mut q = self.queue.lock().await;
        if position > 3.0 || q.index == 0 {
            return self.mpv.command(json!(["seek", 0, "absolute"])).await.map(drop);
        }
        self.leave(&mut q, "skipped").await;
        q.index -= 1;
        self.load_from(&mut q, 0.0).await
    }

    pub async fn seek(&self, secs: f64) -> anyhow::Result<()> {
        {
            let mut q = self.queue.lock().await;
            if q.restored.is_some() {
                q.restored = Some(secs.max(0.0));
                return Ok(());
            }
        }
        self.mpv.command(json!(["seek", secs.max(0.0), "absolute"])).await.map(drop)
    }

    /// Back or forward by `delta` seconds within the file (books: −30 / +30).
    pub async fn skip(&self, delta: f64) -> anyhow::Result<()> {
        {
            let mut q = self.queue.lock().await;
            if let Some(secs) = q.restored {
                q.restored = Some((secs + delta).max(0.0));
                return Ok(());
            }
        }
        self.mpv.command(json!(["seek", delta, "relative"])).await.map(drop)
    }

    /// Plays the queue's entry at `index`, `secs` into it.
    pub async fn jump(&self, index: usize, secs: f64) -> anyhow::Result<()> {
        let mut q = self.queue.lock().await;
        anyhow::ensure!(index < q.tracks.len(), "no such track in the queue");
        if index == q.index && q.active {
            return self.seek(secs).await;
        }
        self.leave(&mut q, "skipped").await;
        q.index = index;
        self.load_from(&mut q, secs).await
    }

    /// Whether there is a queue to add to (playing, paused, or brought back after a restart).
    pub async fn has_queue(&self) -> bool {
        let q = self.queue.lock().await;
        q.active || q.restored.is_some()
    }

    /// Adds tracks after the current one (`next`) or at the end. mpv holds the current
    /// entry and the one after it; when that one changes, its prepared stream is replaced.
    pub async fn enqueue(&self, tracks: Vec<Track>, next: bool) -> anyhow::Result<()> {
        anyhow::ensure!(!tracks.is_empty(), "nothing to add");
        let mut q = self.queue.lock().await;
        let spoken = |t: &Track| t.book_id.is_some() || t.podcast_id.is_some();
        anyhow::ensure!(!q.tracks.iter().any(spoken), "a book or an episode is playing");
        let following = q.tracks.get(q.index + 1).map(|t| t.id.clone());
        let at = if next { (q.index + 1).min(q.tracks.len()) } else { q.tracks.len() };
        q.tracks.splice(at..at, tracks);
        if q.active && q.tracks.get(q.index + 1).map(|t| t.id.clone()) != following {
            if following.is_some() {
                let _ = self.mpv.command(json!(["playlist-remove", 1])).await;
            }
            if let Some(track) = q.tracks.get(q.index + 1).cloned() {
                match self.stream_url(&track).await {
                    Ok(url) => {
                        let _ = self.mpv.command(json!(["loadfile", url, "append"])).await;
                    }
                    Err(err) => tracing::warn!(%err, "preparing the next track"),
                }
            }
        }
        let position = if q.active { self.property_f64("time-pos").await.unwrap_or(q.last_position) } else { q.restored.unwrap_or(0.0) };
        self.persist(&q, position);
        Ok(())
    }

    pub async fn queue(&self) -> QueueView {
        let q = self.queue.lock().await;
        QueueView { index: q.index, active: q.active || q.restored.is_some(), tracks: q.tracks.clone() }
    }

    pub async fn set_volume(&self, volume: f64) -> anyhow::Result<()> {
        let volume = volume.clamp(0.0, 100.0);
        self.mpv.command(json!(["set_property", "volume", volume])).await?;
        self.prefs.update(|p| p.volume = Some(volume));
        Ok(())
    }

    /// Speed for the book playing, kept per book (one is listened to at 1.3×, another at 1×).
    pub async fn set_speed(&self, speed: f64) -> anyhow::Result<()> {
        let speed = speed.clamp(0.5, 3.0);
        let q = self.queue.lock().await;
        self.mpv.command(json!(["set_property", "speed", speed])).await?;
        if let Some(key) = q.tracks.get(q.index).and_then(Self::speed_key) {
            self.prefs.update(|p| {
                p.book_speeds.insert(key, speed);
            });
        }
        Ok(())
    }

    pub async fn set_sleep(&self, req: SleepRequest) {
        let mut q = self.queue.lock().await;
        q.sleep = if req.end_of_part {
            Some(Sleep::EndOfPart)
        } else {
            req.minutes.map(|m| Sleep::At(Instant::now() + Duration::from_secs(m * 60)))
        };
    }

    pub async fn now(&self) -> NowPlaying {
        let q = self.queue.lock().await;
        let restored = q.restored.filter(|_| !q.active);
        let track = (q.active || restored.is_some()).then(|| q.tracks.get(q.index).cloned()).flatten();
        let paused = restored.is_some() || self.property_bool("pause").await.unwrap_or(false);
        let position = match restored {
            Some(secs) => secs,
            None => self.property_f64("time-pos").await.unwrap_or(0.0),
        };
        let duration = if restored.is_some() { None } else { self.property_f64("duration").await }
            .or_else(|| track.as_ref().map(|t| t.duration_secs))
            .unwrap_or(0.0);
        NowPlaying {
            state: match (&track, paused) {
                (None, _) => "stopped",
                (Some(_), true) => "paused",
                (Some(_), false) => "playing",
            },
            kind: match &track {
                Some(t) if t.book_id.is_some() => "book",
                Some(t) if t.podcast_id.is_some() => "podcast",
                _ => "music",
            },
            index: q.index,
            queue_length: q.tracks.len(),
            track,
            position_secs: position,
            duration_secs: duration,
            volume: self.property_f64("volume").await.or(self.prefs.get().volume).unwrap_or(100.0),
            speed: self.property_f64("speed").await.unwrap_or(1.0),
            sleep_in_secs: match q.sleep {
                Some(Sleep::At(at)) => Some(at.saturating_duration_since(Instant::now()).as_secs_f64()),
                _ => None,
            },
            sleep_end_of_part: matches!(q.sleep, Some(Sleep::EndOfPart)),
        }
    }

    async fn property(&self, name: &str) -> Option<Value> {
        self.mpv.command(json!(["get_property", name])).await.ok()
    }

    async fn property_f64(&self, name: &str) -> Option<f64> {
        self.property(name).await.and_then(|v| v.as_f64())
    }

    async fn property_bool(&self, name: &str) -> Option<bool> {
        self.property(name).await.and_then(|v| v.as_bool())
    }

    // --- following mpv ----------------------------------------------------------------

    /// Watches mpv move from one entry to the next, keeps the two-entry window filled, and
    /// does the timed work: saving positions, counting listening time, the sleep timer.
    async fn follow(self: Arc<Self>) {
        let mut tick = tokio::time::interval(TICK);
        loop {
            tick.tick().await;
            if let Err(err) = self.step().await {
                tracing::warn!(%err, "following the queue");
            }
        }
    }

    async fn step(&self) -> anyhow::Result<()> {
        let mut q = self.queue.lock().await;
        if !q.active {
            return Ok(());
        }
        if self.mpv.generation() != q.mpv_generation {
            // mpv was restarted under us: what it played is gone, not finished. Stop where it
            // was last heard of; a tap on play loads it again.
            tracing::warn!(position = q.last_position, "mpv restarted; the queue waits at its last position");
            self.end_listening(&mut q, "stopped").await;
            let position = q.last_position;
            q.active = false;
            q.paused = true;
            q.restored = Some(position);
            self.persist(&q, position);
            return Ok(());
        }
        let pos = self.property("playlist-pos").await.and_then(|v| v.as_i64()).context("no playlist-pos")?;
        let idle = self.property_bool("idle-active").await.unwrap_or(false);
        // Where the entry that was playing got to, before this tick reads the next one's.
        let reached = q.last_position;
        if let Some(position) = self.property_f64("time-pos").await {
            q.last_position = position;
        }
        let spoken = q.tracks.get(q.index).is_some_and(|t| t.book_id.is_some() || t.podcast_id.is_some());
        let cut_short = q.tracks.get(q.index).is_some_and(|t| ended_early(t.duration_secs, reached));
        if (pos >= 1 || (idle && pos < 0)) && spoken && cut_short {
            // mpv also "ends" a file when the network drops. A book or an episode that stops
            // well before its length was interrupted, not finished: wait at the last second
            // heard instead of marking it played or moving on.
            tracing::warn!(position = reached, "the stream ended early; waiting there");
            self.mpv.command(json!(["stop"])).await?;
            self.end_listening(&mut q, "stopped").await;
            self.save_position(&q, reached, false);
            q.active = false;
            q.paused = true;
            q.restored = Some(reached);
            q.last_position = reached;
            self.persist(&q, reached);
            return Ok(());
        }
        let paused = self.property_bool("pause").await.unwrap_or(false);
        // A pause from elsewhere (mpv itself, a future hardware button) counts as one too.
        self.paused_changed(&mut q, paused).await;
        if !paused && let Some(l) = q.listening.as_mut() {
            l.listened_secs += TICK.as_secs_f64();
        }

        if pos >= 1 {
            // mpv went on to the prepared entry: that is the queue's next track. An episode
            // that ran to its end is played (a book's file is not the end of the book).
            if let Some(done) = q.tracks.get(q.index).filter(|t| t.podcast_id.is_some()) {
                self.save_track(done, done.duration_secs, true, true);
            }
            self.end_listening(&mut q, "completed").await;
            q.index += 1;
            self.mpv.command(json!(["playlist-remove", 0])).await?;
            if let Some(next) = q.tracks.get(q.index + 1).cloned() {
                let url = self.stream_url(&next).await?;
                self.mpv.command(json!(["loadfile", url, "append"])).await?;
            }
            if matches!(q.sleep, Some(Sleep::EndOfPart)) {
                q.sleep = None;
                self.mpv.command(json!(["set_property", "pause", true])).await?;
                q.paused = true;
            } else {
                self.start_listening(&mut q);
            }
            // The new file from its start, so another device resumes here and not in the old one.
            self.save_position(&q, 0.0, false);
            self.persist(&q, 0.0);
        } else if idle && pos < 0 {
            // Ran out: the last file ended, so a book is finished.
            if let Some(last) = q.tracks.get(q.index).cloned() {
                self.save_position(&q, last.duration_secs, true);
            }
            self.end_listening(&mut q, "completed").await;
            q.active = false;
            q.sleep = None;
            self.forget_persisted();
        } else {
            if let Some(Sleep::At(at)) = q.sleep
                && Instant::now() >= at
            {
                q.sleep = None;
                self.mpv.command(json!(["set_property", "pause", true])).await?;
                self.paused_changed(&mut q, true).await;
            }
            let due = q.last_saved.is_none_or(|t| t.elapsed() >= SAVE_EVERY);
            if !paused
                && due
                && let Some(position) = self.property_f64("time-pos").await
            {
                self.save_position(&q, position, false);
                q.last_saved = Some(Instant::now());
                if q.last_persisted.is_none_or(|t| t.elapsed() >= PERSIST_EVERY) {
                    self.persist(&q, position);
                    q.last_persisted = Some(Instant::now());
                }
            }
        }
        Ok(())
    }

    // --- the server's record ------------------------------------------------------------


    /// Saves the place in the book playing; does nothing for music. Sent in the background
    /// so a slow server never holds up playback.
    fn save_position(&self, q: &Queue, position: f64, completed: bool) {
        let Some(track) = q.tracks.get(q.index) else { return };
        self.save_track(track, position, completed, q.index + 1 == q.tracks.len());
    }

    /// Book: the place in the file (only the book's last file can finish it). Episode: the
    /// place in the episode, or played. Music: nothing to keep.
    fn save_track(&self, track: &Track, position: f64, completed: bool, last_in_queue: bool) {
        let Some((path, body)) = Self::progress_request(track, position, completed, last_in_queue) else { return };
        let auth = Arc::clone(&self.auth);
        tokio::spawn(async move {
            if let Err(err) = auth.send_json(Method::PUT, &path, &body).await {
                tracing::warn!(%err, path, "saving the listening position");
            }
        });
    }

    fn progress_request(track: &Track, position: f64, completed: bool, last_in_queue: bool) -> Option<(String, Value)> {
        Some(if let Some(book_id) = &track.book_id {
            (
                format!("/api/v1/playback/books/{}/progress", crate::library::encode(book_id)),
                json!({
                    "file_id": track.id,
                    "position_secs": position.max(0.0),
                    "completed": completed && last_in_queue,
                    "device_kind": crate::api::DEVICE_KIND,
                }),
            )
        } else if track.podcast_id.is_some() {
            (
                format!("/api/v1/playback/episodes/{}/progress", crate::library::encode(&track.id)),
                json!({
                    // A played episode starts over next time.
                    "position_secs": if completed { 0.0 } else { position.max(0.0) },
                    "completed": completed,
                    "device_kind": crate::api::DEVICE_KIND,
                }),
            )
        } else {
            return None;
        })
    }

    fn start_listening(&self, q: &mut Queue) {
        let Some(track) = q.tracks.get(q.index).cloned() else { return };
        let n = self.sessions.fetch_add(1, Ordering::Relaxed);
        q.listening = Some(Listening {
            track,
            started_at: chrono::Utc::now().to_rfc3339(),
            listened_secs: 0.0,
            id: format!("ownbox-{:x}-{n}", chrono::Utc::now().timestamp_millis()),
        });
    }

    /// Reports the listening that just ended (`completed`, `skipped`, `stopped`, `replaced`).
    async fn end_listening(&self, q: &mut Queue, reason: &str) {
        let Some(body) = self.listening_report(q, reason).await else { return };
        let auth = Arc::clone(&self.auth);
        tokio::spawn(async move {
            if let Err(err) = auth.send_json(Method::POST, "/api/v1/playback/sessions", &body).await {
                tracing::warn!(%err, "reporting the listening session");
            }
        });
    }

    async fn listening_report(&self, q: &mut Queue, reason: &str) -> Option<Value> {
        let l = q.listening.take()?;
        if l.listened_secs < MIN_SESSION_SECS {
            return None;
        }
        let speed = self.property_f64("speed").await.unwrap_or(1.0);
        let (kind, item, part) = match (&l.track.book_id, &l.track.podcast_id) {
            (Some(book), _) => ("audiobook", book.clone(), Some(l.track.id.clone())),
            (None, Some(_)) => ("podcast", l.track.id.clone(), None),
            (None, None) => ("music", l.track.id.clone(), None),
        };
        let body = json!({ "sessions": [{
            "media_kind": kind,
            "item_id": item,
            "part_id": part,
            "started_at": l.started_at,
            "ended_at": chrono::Utc::now().to_rfc3339(),
            "seconds_listened": l.listened_secs.round() as i64,
            "playback_speed": speed,
            "device_kind": crate::api::DEVICE_KIND,
            "client_session_id": l.id,
            "ended_reason": reason,
        }]});
        Some(body)
    }
}

/// Whether a file stopped well before its length: the network dropped, not the end. The
/// position is read twice a second, so a file played to its end is seen within a second
/// of it; ten seconds leaves room for lengths the tags round.
fn ended_early(duration_secs: f64, reached_secs: f64) -> bool {
    duration_secs > 30.0 && reached_secs < duration_secs - 10.0
}

#[cfg(test)]
mod tests {
    use super::ended_early;

    #[test]
    fn a_file_played_to_its_end_is_finished() {
        assert!(!ended_early(1800.0, 1799.6));
        assert!(!ended_early(1800.0, 1792.0));
    }

    #[test]
    fn a_file_cut_off_midway_is_interrupted() {
        assert!(ended_early(1800.0, 412.0));
        assert!(ended_early(1800.0, 0.0));
    }

    #[test]
    fn very_short_files_are_never_called_interrupted() {
        assert!(!ended_early(20.0, 3.0));
    }
}
