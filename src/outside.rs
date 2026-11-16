//! Outside sources: AirPlay (shairport-sync) and Music Assistant (squeezelite) playing to the
//! box. They take the sound card while they play: ownbox lets it go when one starts (its
//! hook calls `/api/outside/{kind}/start`), shows what plays, and takes the card back when
//! the box's own player is asked to play (by restarting the source's service, which ends
//! its session).

use std::{
    process::Stdio,
    sync::{Arc, Mutex},
    time::Duration,
};

use base64::Engine;
use serde::Serialize;
use tokio::{io::AsyncReadExt, process::Command};

/// Where shairport-sync writes its metadata (its `metadata.pipe_name`).
pub const AIRPLAY_METADATA_PIPE: &str = "/tmp/shairport-sync-metadata";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Airplay,
    MusicAssistant,
}

impl Kind {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "airplay" => Some(Self::Airplay),
            "music-assistant" | "music_assistant" => Some(Self::MusicAssistant),
            _ => None,
        }
    }

    fn service(self) -> &'static str {
        match self {
            Self::Airplay => "shairport-sync.service",
            Self::MusicAssistant => "squeezelite.service",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Playing {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    /// The phone or computer sending (AirPlay's client name).
    pub sender: Option<String>,
    /// Bumped with each new cover, so the screen knows to fetch it again.
    pub cover_version: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct View {
    pub kind: Kind,
    #[serde(flatten)]
    pub playing: Playing,
}

#[derive(Default)]
struct State {
    active: Option<Kind>,
    playing: Playing,
    cover: Option<(Vec<u8>, &'static str)>,
}

#[derive(Default)]
pub struct Outside {
    state: Mutex<State>,
}

impl Outside {
    pub fn new() -> Arc<Self> {
        let outside = Arc::new(Self::default());
        tokio::spawn(Arc::clone(&outside).read_airplay_metadata());
        tokio::spawn(Arc::clone(&outside).watch_squeezelite());
        outside
    }

    /// Music Assistant's player (squeezelite) has no hook of its own in Debian's build, so
    /// the box watches who holds the sound card: squeezelite opens it when Music Assistant
    /// plays and closes it 2 s after it stops (`-C 2`).
    async fn watch_squeezelite(self: Arc<Self>) {
        loop {
            tokio::time::sleep(Duration::from_secs(2)).await;
            let holds = card_holders().iter().any(|name| name == "squeezelite");
            let active = self.state.lock().expect("outside lock").active;
            match (holds, active) {
                (true, None) => self.started(Kind::MusicAssistant),
                (false, Some(Kind::MusicAssistant)) => self.stopped(Kind::MusicAssistant),
                _ => {}
            }
        }
    }

    pub fn started(&self, kind: Kind) {
        let mut s = self.state.lock().expect("outside lock");
        if s.active != Some(kind) {
            s.playing = Playing::default();
            s.cover = None;
        }
        s.active = Some(kind);
    }

    pub fn stopped(&self, kind: Kind) {
        let mut s = self.state.lock().expect("outside lock");
        if s.active == Some(kind) {
            s.active = None;
        }
    }

    pub fn view(&self) -> Option<View> {
        let s = self.state.lock().expect("outside lock");
        s.active.map(|kind| View { kind, playing: s.playing.clone() })
    }

    pub fn cover(&self) -> Option<(Vec<u8>, &'static str)> {
        self.state.lock().expect("outside lock").cover.clone()
    }

    /// Ends whatever outside source holds the card, so the box's own player can have it.
    /// Restarting the source's service is the one way that works for both (AirPlay 2 gives
    /// the receiver no "stop" of its own); polkit lets ownbox do it for these two units.
    pub async fn take_back(&self) {
        let Some(kind) = self.state.lock().expect("outside lock").active.take() else { return };
        let restarted = Command::new("systemctl")
            .args(["restart", kind.service()])
            .stdin(Stdio::null())
            .status()
            .await;
        match restarted {
            Ok(st) if st.success() => tracing::info!(?kind, "took the sound card back"),
            other => tracing::warn!(?kind, ?other, "could not end the outside source"),
        }
        // Give the card a moment to be closed before mpv opens it.
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    /// shairport-sync's metadata pipe: `<item>`s with a type, a code and base64 data.
    async fn read_airplay_metadata(self: Arc<Self>) {
        loop {
            // Opening a FIFO waits for a writer; when shairport-sync is not there, wait.
            let file = match tokio::fs::File::open(AIRPLAY_METADATA_PIPE).await {
                Ok(f) => f,
                Err(_) => {
                    tokio::time::sleep(Duration::from_secs(10)).await;
                    continue;
                }
            };
            let mut reader = tokio::io::BufReader::new(file);
            let mut buf = String::new();
            let mut chunk = vec![0u8; 16 * 1024];
            loop {
                match reader.read(&mut chunk).await {
                    Ok(0) | Err(_) => break, // the writer went away; reopen
                    Ok(n) => buf.push_str(&String::from_utf8_lossy(&chunk[..n])),
                }
                while let Some(end) = buf.find("</item>") {
                    let item: String = buf.drain(..end + "</item>".len()).collect();
                    if let Some(parsed) = parse_item(&item) {
                        self.apply(parsed);
                    }
                }
                if buf.len() > 4 * 1024 * 1024 {
                    buf.clear(); // a runaway item: start over rather than grow
                }
            }
        }
    }

    fn apply(&self, (code, data): (String, Vec<u8>)) {
        let text = || String::from_utf8_lossy(&data).trim().to_owned();
        let mut s = self.state.lock().expect("outside lock");
        match code.as_str() {
            "minm" => s.playing.title = Some(text()),
            "asar" => s.playing.artist = Some(text()),
            "asal" => s.playing.album = Some(text()),
            "snam" => s.playing.sender = Some(text()),
            "PICT" if !data.is_empty() => {
                let kind = if data.starts_with(&[0x89, b'P', b'N', b'G']) { "image/png" } else { "image/jpeg" };
                s.cover = Some((data, kind));
                s.playing.cover_version += 1;
            }
            _ => {}
        }
    }
}

/// The programs that have a playback device open, from ALSA's own status files.
fn card_holders() -> Vec<String> {
    let mut names = Vec::new();
    let Ok(cards) = std::fs::read_dir("/proc/asound") else { return names };
    for card in cards.flatten() {
        let Ok(pcms) = std::fs::read_dir(card.path()) else { continue };
        for pcm in pcms.flatten().filter(|p| p.file_name().to_string_lossy().ends_with('p')) {
            let Ok(subs) = std::fs::read_dir(pcm.path()) else { continue };
            for sub in subs.flatten() {
                let Ok(status) = std::fs::read_to_string(sub.path().join("status")) else { continue };
                let pid = status.lines().find_map(|l| l.strip_prefix("owner_pid").map(|v| v.trim_start_matches([' ', '\t', ':']).trim().to_owned()));
                if let Some(pid) = pid
                    && let Ok(comm) = std::fs::read_to_string(format!("/proc/{pid}/comm"))
                {
                    names.push(comm.trim().to_owned());
                }
            }
        }
    }
    names
}

/// One metadata item: its four-letter code and decoded data.
fn parse_item(item: &str) -> Option<(String, Vec<u8>)> {
    let between = |open: &str, close: &str| -> Option<&str> {
        let start = item.find(open)? + open.len();
        let end = item[start..].find(close)? + start;
        Some(&item[start..end])
    };
    let code_hex = between("<code>", "</code>")?;
    let code_bytes: Vec<u8> = (0..code_hex.len())
        .step_by(2)
        .filter_map(|i| code_hex.get(i..i + 2).and_then(|h| u8::from_str_radix(h, 16).ok()))
        .collect();
    let code = String::from_utf8(code_bytes).ok()?;
    let data = match item.find("<data") {
        Some(_) => {
            let start = item.find("<data")?;
            let after = item[start..].find('>')? + start + 1;
            let end = item[after..].find("</data>")? + after;
            let b64: String = item[after..end].chars().filter(|c| !c.is_whitespace()).collect();
            base64::engine::general_purpose::STANDARD.decode(b64).ok()?
        }
        None => Vec::new(),
    };
    Some((code, data))
}

#[cfg(test)]
mod tests {
    use super::parse_item;

    #[test]
    fn reads_a_title_item() {
        // "Beetlebum" in base64; code "minm" in hex.
        let item = "<item><type>636f7265</type><code>6d696e6d</code><length>9</length>\n\
                    <data encoding=\"base64\">\nQmVldGxlYnVt</data></item>";
        assert_eq!(parse_item(item), Some(("minm".into(), b"Beetlebum".to_vec())));
    }

    #[test]
    fn an_item_without_data() {
        let item = "<item><type>73736e63</type><code>70626567</code><length>0</length></item>";
        assert_eq!(parse_item(item), Some(("pbeg".into(), Vec::new())));
    }
}
