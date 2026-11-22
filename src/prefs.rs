//! Settings made on the screen (brightness, idle time, volume), kept in `prefs.json` next
//! to the session. `config.toml` only gives the starting values.

use std::{collections::HashMap, path::PathBuf, sync::Mutex};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Prefs {
    pub screen_brightness: Option<u8>,
    pub screen_off_after_secs: Option<u64>,
    /// The picture turned by 0, 90, 180 or 270 degrees (missing: 0); touch turns with it
    /// (`boot-config.sh`).
    pub screen_rotation: Option<u16>,
    /// mpv's volume, 0–100.
    pub volume: Option<f64>,
    /// Playback speed chosen for each audiobook, by book id.
    #[serde(default)]
    pub book_speeds: HashMap<String, f64>,
    /// When each playlist (or smart playlist) was last played on this box, unix seconds:
    /// the server keeps no such history, and the home page shows the latest first.
    #[serde(default)]
    pub playlists_played: HashMap<String, i64>,
    /// The screen's language, `cs` or `en`.
    pub language: Option<String>,
    /// `dark` (missing), `light`, or `auto` (each device follows its own system setting).
    pub theme: Option<String>,
    /// The sound card on the GPIO header, by its overlay ("" for none), chosen on the box;
    /// `boot-config.sh` puts it into config.txt. Missing: config.toml's `audio_hat`.
    pub audio_hat: Option<String>,
    /// The HAT's own buttons (`phat-beat`), set with the card.
    pub hat_buttons: Option<String>,
    /// How the action buttons (play, queue, playlist…) look: `text` (missing) or `icons`.
    pub buttons: Option<String>,
    /// The own.audio server, as chosen in the wizard (otherwise `config.toml`'s).
    pub server_url: Option<String>,
    /// How the box is named in the account's devices (and later to AirPlay).
    pub box_name: Option<String>,
    /// The first-run wizard has been finished.
    pub setup_done: Option<bool>,
    /// The equalizer, as last set here or read from the server.
    pub eq: Option<crate::eq::Eq>,
    /// Phones paired for control (hashes of their tokens).
    #[serde(default)]
    pub remote_devices: Vec<crate::remote::Paired>,
    /// Outside sources switched on (missing = on); `apply-sources.sh` reads these.
    pub airplay_enabled: Option<bool>,
    pub music_assistant_enabled: Option<bool>,
    /// The sound output chosen in Settings (mpv's `--audio-device`).
    pub audio_device: Option<String>,
    /// Updates install themselves at night (missing = yes).
    pub auto_update: Option<bool>,
    /// Favourite radio stations, in the order they were added.
    #[serde(default)]
    pub radios: Vec<crate::radio::Station>,
    /// Stations played lately, the latest first.
    #[serde(default)]
    pub radios_recent: Vec<crate::radio::Station>,
    /// Debug access (`debug.rs`): the read-only user `debug` over SSH with `debug_keys`,
    /// the journal kept on the card. `apply-sources.sh` applies it.
    pub debug_access: Option<bool>,
    /// The GitHub account the keys came from, to show and to load again.
    pub debug_github: Option<String>,
    #[serde(default)]
    pub debug_keys: Vec<String>,
}

/// Writes `path` so that a power cut leaves the old file or the new one, never half of it:
/// a temporary file beside it, on the card, then renamed over it. The path unit watching
/// prefs.json sees the rename as a change.
pub fn write_atomically(path: &std::path::Path, data: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let dir = path.parent().unwrap_or(std::path::Path::new("."));
    std::fs::create_dir_all(dir)?;
    let tmp = path.with_extension("tmp");
    let mut file = std::fs::File::create(&tmp)?;
    file.write_all(data)?;
    file.sync_all()?;
    drop(file);
    std::fs::rename(&tmp, path)?;
    std::fs::File::open(dir)?.sync_all()
}

pub struct PrefsStore {
    path: PathBuf,
    prefs: Mutex<Prefs>,
}

impl PrefsStore {
    pub fn load(path: PathBuf) -> Self {
        let prefs = match std::fs::read_to_string(&path) {
            Err(_) => Prefs::default(),
            Ok(text) => serde_json::from_str(&text).unwrap_or_else(|err| {
                // Kept aside rather than overwritten by the first change: the sign-in, the
                // sound card and the paired phones may still be read from it by hand.
                let aside = path.with_extension("json.bad");
                tracing::warn!(%err, aside = %aside.display(), "settings unreadable; starting from defaults");
                let _ = std::fs::rename(&path, &aside);
                Prefs::default()
            }),
        };
        Self { path, prefs: Mutex::new(prefs) }
    }

    pub fn get(&self) -> Prefs {
        self.prefs.lock().expect("prefs lock").clone()
    }

    pub fn update(&self, change: impl FnOnce(&mut Prefs)) {
        let mut prefs = self.prefs.lock().expect("prefs lock");
        change(&mut prefs);
        if let Err(err) = write_atomically(&self.path, &serde_json::to_vec_pretty(&*prefs).unwrap_or_default()) {
            tracing::warn!(%err, path = %self.path.display(), "saving settings");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_broken_file_is_kept_aside_and_a_change_writes_a_whole_one() {
        let dir = std::env::temp_dir().join(format!("ownbox-prefs-{}", std::process::id()));
        let path = dir.join("prefs.json");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&path, b"{\"volume\": 4").unwrap();
        let store = PrefsStore::load(path.clone());
        assert!(dir.join("prefs.json.bad").exists());
        store.update(|p| p.volume = Some(33.0));
        let back: Prefs = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(back.volume, Some(33.0));
        assert!(!dir.join("prefs.tmp").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
