//! Hardware buttons: media keys from any input device. A HAT's buttons arrive this way once
//! `config.txt` maps their pins with the `gpio-key` overlay (the pHAT BEAT on the radio, see
//! `setup/boot-config.sh`); a USB keyboard's or speakerphone's media keys work the same.

use std::{collections::HashSet, path::PathBuf, sync::Arc, time::Duration};

use tokio::io::AsyncReadExt;

use crate::AppState;

const EV_KEY: u16 = 1;
const KEY_VOLUMEDOWN: u16 = 114;
const KEY_VOLUMEUP: u16 = 115;
const KEY_NEXTSONG: u16 = 163;
const KEY_PLAYPAUSE: u16 = 164;
const KEY_PREVIOUSSONG: u16 = 165;
/// `struct input_event` on a 64-bit kernel: a 16-byte timeval, type, code, value.
const EVENT_SIZE: usize = 24;
const VOLUME_STEP: f64 = 5.0;

#[derive(Debug, PartialEq)]
enum Action {
    Toggle,
    Next,
    Previous,
    Volume(f64),
}

/// A key going down (1) or held (2, the kernel's repeat); volume repeats, the rest do not.
fn action(kind: u16, code: u16, value: i32) -> Option<Action> {
    if kind != EV_KEY || !(value == 1 || value == 2) {
        return None;
    }
    match (code, value) {
        (KEY_VOLUMEUP, _) => Some(Action::Volume(VOLUME_STEP)),
        (KEY_VOLUMEDOWN, _) => Some(Action::Volume(-VOLUME_STEP)),
        (KEY_PLAYPAUSE, 1) => Some(Action::Toggle),
        (KEY_NEXTSONG, 1) => Some(Action::Next),
        (KEY_PREVIOUSSONG, 1) => Some(Action::Previous),
        _ => None,
    }
}

fn parse(buf: &[u8; EVENT_SIZE]) -> (u16, u16, i32) {
    (
        u16::from_ne_bytes([buf[16], buf[17]]),
        u16::from_ne_bytes([buf[18], buf[19]]),
        i32::from_ne_bytes([buf[20], buf[21], buf[22], buf[23]]),
    )
}

async fn perform(state: &AppState, action: Action) {
    if !matches!(action, Action::Volume(_)) {
        crate::web::own_turn(state).await;
    }
    let result = match action {
        Action::Toggle => state.player.toggle().await,
        Action::Next => state.player.next().await,
        Action::Previous => state.player.previous().await,
        Action::Volume(delta) => {
            let now = state.player.now().await.volume;
            state.player.set_volume((now + delta).clamp(0.0, 100.0)).await
        }
    };
    if let Err(err) = result {
        tracing::warn!(%err, "button");
    }
}

async fn watch(state: Arc<AppState>, path: PathBuf) {
    let Ok(mut file) = tokio::fs::File::open(&path).await else { return };
    tracing::info!(device = %path.display(), "reading buttons");
    let mut buf = [0u8; EVENT_SIZE];
    while file.read_exact(&mut buf).await.is_ok() {
        let (kind, code, value) = parse(&buf);
        if let Some(a) = action(kind, code, value) {
            perform(&state, a).await;
        }
    }
}

/// Every input device, and new ones as they appear (a USB device plugged in later).
pub async fn run(state: Arc<AppState>) {
    let mut seen = HashSet::new();
    loop {
        if let Ok(dir) = std::fs::read_dir("/dev/input") {
            for entry in dir.flatten() {
                let path = entry.path();
                let is_event = path.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("event"));
                if is_event && seen.insert(path.clone()) {
                    tokio::spawn(watch(Arc::clone(&state), path));
                }
            }
        }
        tokio::time::sleep(Duration::from_secs(15)).await;
        // A device that went away ended its watcher; its name may come back as another one.
        seen.retain(|p| p.exists());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_media_keys() {
        assert_eq!(action(EV_KEY, KEY_PLAYPAUSE, 1), Some(Action::Toggle));
        assert_eq!(action(EV_KEY, KEY_PLAYPAUSE, 0), None);
        assert_eq!(action(EV_KEY, KEY_PLAYPAUSE, 2), None);
        assert_eq!(action(EV_KEY, KEY_VOLUMEUP, 2), Some(Action::Volume(VOLUME_STEP)));
        assert_eq!(action(EV_KEY, KEY_NEXTSONG, 1), Some(Action::Next));
        assert_eq!(action(0, KEY_NEXTSONG, 1), None);
    }

    #[test]
    fn reads_an_event() {
        let mut buf = [0u8; EVENT_SIZE];
        buf[16..18].copy_from_slice(&EV_KEY.to_ne_bytes());
        buf[18..20].copy_from_slice(&KEY_PREVIOUSSONG.to_ne_bytes());
        buf[20..24].copy_from_slice(&1i32.to_ne_bytes());
        assert_eq!(parse(&buf), (EV_KEY, KEY_PREVIOUSSONG, 1));
    }
}
