//! Settings made on the screen (brightness, idle time, volume), kept in `prefs.json` next
//! to the session. `config.toml` only gives the starting values.

use std::{collections::HashMap, path::PathBuf, sync::Mutex};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Prefs {
    pub screen_brightness: Option<u8>,
    pub screen_off_after_secs: Option<u64>,
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
}

pub struct PrefsStore {
    path: PathBuf,
    prefs: Mutex<Prefs>,
}

impl PrefsStore {
    pub fn load(path: PathBuf) -> Self {
        let prefs = std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        Self { path, prefs: Mutex::new(prefs) }
    }

    pub fn get(&self) -> Prefs {
        self.prefs.lock().expect("prefs lock").clone()
    }

    pub fn update(&self, change: impl FnOnce(&mut Prefs)) {
        let mut prefs = self.prefs.lock().expect("prefs lock");
        change(&mut prefs);
        let saved = self
            .path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::write(&self.path, serde_json::to_vec_pretty(&*prefs).unwrap_or_default()));
        if let Err(err) = saved {
            tracing::warn!(%err, path = %self.path.display(), "saving settings");
        }
    }
}
