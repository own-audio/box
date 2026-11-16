use std::{fs, path::PathBuf};

use anyhow::Context;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// The own.audio server this box plays from, until the wizard or Settings choose one.
    /// Empty on a new box: the wizard asks.
    pub server_url: String,
    /// How the box appears in the account's list of signed-in devices.
    pub device_name: String,
    /// mpv's `--audio-device`; `mpv --audio-device=help` lists the choices.
    pub audio_device: String,
    /// Extra mpv options for the output. The defaults suit the Jabra: it takes only 48 kHz
    /// 16-bit, and a one-second buffer rides out the moments the throttled CPU or the USB bus
    /// (shared with Ethernet on a Pi 3) fall behind.
    pub audio_options: Vec<String>,
    /// Where the UI and its API are served. On the LAN, for paired phones; the screen is
    /// the same page on localhost.
    pub listen: String,
    /// Backlight level while the screen is on, 1–255.
    pub screen_brightness: u8,
    /// Seconds without a touch before the backlight goes off; 0 keeps it on.
    pub screen_off_after_secs: u64,
    /// The screen's language until one is chosen in Settings: `cs` or `en`.
    pub language: String,
    /// The device-tree overlay of an I²S audio HAT without an EEPROM (`hifiberry-dacplus`,
    /// `allo-boss-dac-pcm512x-audio`…). The deploy writes it into `config.txt`; empty for USB
    /// sound or a HAT the firmware finds itself.
    pub audio_hat: String,
    /// Where ownbox looks for updates of itself: GitHub's list of releases (betas
    /// included), or one release document such as `…/releases/latest`. Empty: no updates
    /// (a build without the release key has none either).
    pub update_feed: String,
    /// A box without a screen (the radio): the first phone to open it pairs without a code,
    /// and paired phones may set the box up, as the screen would.
    pub headless: bool,
    /// Buttons on a HAT, mapped to media keys in `config.txt` by the deploy: `phat-beat`, or
    /// empty for none.
    pub buttons: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server_url: String::new(),
            device_name: "own.audio box".into(),
            audio_device: "alsa/plughw:CARD=USB".into(),
            audio_options: vec![
                "--audio-buffer=1".into(),
                "--audio-samplerate=48000".into(),
                "--audio-format=s16".into(),
            ],
            listen: "0.0.0.0:8686".into(),
            screen_brightness: 255,
            screen_off_after_secs: 30,
            language: "cs".into(),
            audio_hat: String::new(),
            update_feed: "https://api.github.com/repos/own-audio/box/releases?per_page=20".into(),
            headless: false,
            buttons: String::new(),
        }
    }
}

fn home() -> anyhow::Result<PathBuf> {
    Ok(PathBuf::from(std::env::var_os("HOME").context("HOME is not set")?))
}

/// Where ownbox keeps what it writes itself (the session, settings made on the screen):
/// `$XDG_STATE_HOME/ownbox`, or `~/.local/state/ownbox`.
pub fn state_dir() -> anyhow::Result<PathBuf> {
    let base = match std::env::var_os("XDG_STATE_HOME") {
        Some(dir) => PathBuf::from(dir),
        None => home()?.join(".local/state"),
    };
    Ok(base.join("ownbox"))
}

/// Reads `$OWNBOX_CONFIG`, or `~/.config/ownbox/config.toml`, writing the defaults there
/// the first time so the file is there to edit.
pub fn load() -> anyhow::Result<Config> {
    let path = match std::env::var_os("OWNBOX_CONFIG") {
        Some(p) => PathBuf::from(p),
        None => home()?.join(".config/ownbox/config.toml"),
    };
    if !path.exists() {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        fs::write(&path, toml::to_string_pretty(&Config::default())?)
            .with_context(|| format!("writing {}", path.display()))?;
        tracing::info!(path = %path.display(), "wrote the default config");
    }
    let text = fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}
