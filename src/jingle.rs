//! The start-up sound: the own.audio motif and "own audio. Simple. Private. Yours." (Gemini's
//! voice Achird over a synthesized motif, setup/sounds/). Played once a start: on a box with a
//! screen when the kiosk starts the start-up film (`/api/startup-sound/play`), so it never
//! sounds over a black screen; without a screen as ownbox comes up. Not at night, not over
//! something playing, not with the volume at 0, and not when switched off in Settings → Sound.

use std::sync::Arc;

use crate::AppState;

const SOUND: &[u8] = include_bytes!("../setup/sounds/startup.opus");
/// Never louder than this (mpv's 0–100), whatever the box was last set to.
const CAP: f64 = 60.0;
/// Quiet hours, local time: from–to.
const QUIET: (u32, u32) = (22, 7);

fn quiet_hour(hour: u32) -> bool {
    hour >= QUIET.0 || hour < QUIET.1
}

/// This start's id: a start is played for once, even if ownbox starts again (an update).
fn boot_id() -> String {
    std::fs::read_to_string("/proc/sys/kernel/random/boot_id").unwrap_or_default().trim().to_owned()
}

pub async fn play_once(state: Arc<AppState>) {
    let Ok(dir) = crate::config::state_dir() else { return };
    let mark = dir.join("startup-sound-boot");
    let boot = boot_id();
    if boot.is_empty() || std::fs::read_to_string(&mark).is_ok_and(|b| b.trim() == boot) {
        return;
    }
    // Marked first: a second page load must not play it again, whatever happens below.
    let _ = crate::prefs::write_atomically(&mark, boot.as_bytes());
    let prefs = state.prefs.get();
    if prefs.startup_sound == Some(false) || quiet_hour(chrono::Timelike::hour(&chrono::Local::now())) {
        return;
    }
    let volume = prefs.volume.unwrap_or(50.0).min(CAP);
    if volume <= 0.0 || state.player.now().await.state == "playing" || state.outside.view().is_some() {
        return;
    }
    let file = std::env::temp_dir().join("ownbox-startup.opus");
    if std::fs::write(&file, SOUND).is_err() {
        return;
    }
    let (device, options) = crate::hardware::chosen(&prefs, &state.config);
    // Its own mpv: ownbox's holds the queue restored from before.
    let played = tokio::process::Command::new("mpv")
        .args(["--no-config", "--no-video", "--really-quiet", "--audio-display=no"])
        .arg(format!("--audio-device={device}"))
        .args(&options)
        .arg(format!("--volume={volume:.0}"))
        .arg(&file)
        .kill_on_drop(true)
        .status();
    match tokio::time::timeout(std::time::Duration::from_secs(20), played).await {
        Ok(Ok(st)) if st.success() => tracing::info!(volume, "start-up sound"),
        other => tracing::warn!(?other, "start-up sound not played"),
    }
    let _ = std::fs::remove_file(&file);
}

#[cfg(test)]
mod tests {
    #[test]
    fn quiet_at_night_only() {
        assert!(super::quiet_hour(22));
        assert!(super::quiet_hour(3));
        assert!(super::quiet_hour(6));
        assert!(!super::quiet_hour(7));
        assert!(!super::quiet_hour(21));
    }
}
