//! The display's backlight. It is the box's largest power draw: on this Pi's supply,
//! under-voltage was reported in 24 of 24 samples with it on and 1 of 12 with it off.

use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};

use crate::prefs::PrefsStore;

/// Below this the panel looks off in daylight, and someone would think the box is dead.
pub const MIN_BRIGHTNESS: u8 = 10;

/// How long past its own timer ownbox waits before it turns a forgotten screen off.
const GRACE: Duration = Duration::from_secs(15);

pub struct Screen {
    /// `/sys/class/backlight/<panel>/brightness`; `None` where there is no backlight to drive.
    path: Option<PathBuf>,
    /// The panel's own top value: 255 on the 7" display, 31 on Touch Display 2, which
    /// refuses anything above it.
    max: u32,
    prefs: Arc<PrefsStore>,
    brightness: AtomicU8,
    off_after_secs: AtomicU64,
    on: AtomicBool,
    /// The last touch the page reported (or the screen being turned on).
    last_touch: Mutex<Instant>,
}

#[derive(Serialize)]
pub struct ScreenState {
    pub on: bool,
    pub brightness: u8,
    pub min_brightness: u8,
    /// How many steps the panel has (31 on Touch Display 2, 255 on the 7" display): the
    /// slider moves in those, not in 255 values the panel cannot show.
    pub levels: u32,
    pub off_after_secs: u64,
    pub controllable: bool,
    /// 0, 90, 180 or 270.
    pub rotation: u16,
}

#[derive(Deserialize)]
pub struct Settings {
    pub brightness: Option<u8>,
    pub off_after_secs: Option<u64>,
    pub rotation: Option<u16>,
}

impl Screen {
    pub fn new(brightness: u8, off_after_secs: u64, prefs: Arc<PrefsStore>) -> Self {
        let saved = prefs.get();
        let panel = std::fs::read_dir("/sys/class/backlight").ok().and_then(|mut dir| dir.next()).and_then(Result::ok).map(|e| e.path());
        let max = panel
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p.join("max_brightness")).ok())
            .and_then(|t| t.trim().parse().ok())
            .filter(|&m: &u32| m > 0)
            .unwrap_or(255);
        let screen = Self {
            path: panel.map(|p| p.join("brightness")),
            max,
            brightness: AtomicU8::new(saved.screen_brightness.unwrap_or(brightness).max(MIN_BRIGHTNESS)),
            off_after_secs: AtomicU64::new(saved.screen_off_after_secs.unwrap_or(off_after_secs)),
            on: AtomicBool::new(true),
            last_touch: Mutex::new(Instant::now()),
            prefs,
        };
        // On at start: a dark screen left by a crash could not be woken by a page that is
        // not there yet.
        screen.set(true);
        screen
    }

    pub fn set(&self, on: bool) {
        let Some(path) = &self.path else { return };
        let value = if on { panel_value(self.brightness.load(Ordering::Relaxed), self.max) } else { 0 };
        if on {
            self.touch();
        }
        match std::fs::write(path, value.to_string()) {
            Ok(()) => self.on.store(on, Ordering::Relaxed),
            Err(err) => tracing::warn!(%err, path = %path.display(), "setting the backlight"),
        }
    }

    /// Applies new settings at once (a lit screen takes the new brightness) and keeps them.
    pub fn configure(&self, s: Settings) {
        if let Some(b) = s.brightness {
            self.brightness.store(b.max(MIN_BRIGHTNESS), Ordering::Relaxed);
            if self.on.load(Ordering::Relaxed) {
                self.set(true);
            }
        }
        if let Some(secs) = s.off_after_secs {
            self.off_after_secs.store(secs, Ordering::Relaxed);
        }
        if let Some(r) = s.rotation.filter(|r| ROTATIONS.contains(r)) {
            self.prefs.update(|p| p.screen_rotation = Some(r));
            tokio::spawn(async move { apply_rotation(r).await });
        }
        let (brightness, off_after) =
            (self.brightness.load(Ordering::Relaxed), self.off_after_secs.load(Ordering::Relaxed));
        self.prefs.update(|p| {
            p.screen_brightness = Some(brightness);
            p.screen_off_after_secs = Some(off_after);
        });
    }

    pub fn touch(&self) {
        *self.last_touch.lock().expect("screen lock") = Instant::now();
    }

    /// The page turns the screen off after its idle time, but a lit screen can outlive
    /// the page's knowledge of it (ownbox restarted and turned it on, a test browser woke it
    /// and went away). The backlight is what browns the Pi out, so ownbox turns it off
    /// itself when no touch has been reported for the idle time and a grace period.
    pub fn turn_off_if_forgotten(&self) {
        let off_after = self.off_after_secs.load(Ordering::Relaxed);
        if self.path.is_none() || off_after == 0 || !self.on.load(Ordering::Relaxed) {
            return;
        }
        if self.last_touch.lock().expect("screen lock").elapsed() > Duration::from_secs(off_after) + GRACE {
            tracing::info!("no touch for a while: screen off");
            self.set(false);
        }
    }

    pub fn state(&self) -> ScreenState {
        ScreenState {
            on: self.on.load(Ordering::Relaxed),
            brightness: self.brightness.load(Ordering::Relaxed),
            min_brightness: MIN_BRIGHTNESS,
            levels: self.max,
            off_after_secs: self.off_after_secs.load(Ordering::Relaxed),
            controllable: self.path.is_some(),
            rotation: self.prefs.get().screen_rotation.unwrap_or(0),
        }
    }
}

/// Brightness as the box keeps it (0–255) on the panel's own scale; a lit screen never at 0.
fn panel_value(brightness: u8, max: u32) -> u32 {
    ((u32::from(brightness) * max + 127) / 255).max(1)
}

#[cfg(test)]
mod tests {
    use super::panel_value;

    #[test]
    fn brightness_fits_the_panels_scale() {
        assert_eq!(panel_value(255, 255), 255);
        assert_eq!(panel_value(255, 31), 31);
        assert_eq!(panel_value(128, 31), 16);
        assert_eq!(panel_value(10, 31), 1);
    }
}

pub const ROTATIONS: [u16; 4] = [0, 90, 180, 270];

/// wlr-randr's name for a turn.
fn transform(rotation: u16) -> &'static str {
    match rotation {
        90 => "90",
        180 => "180",
        270 => "270",
        _ => "normal",
    }
}

/// The outputs cage shows and how each is turned, from `wlr-randr`'s listing: an output's
/// line starts at the margin, its `Transform:` line is indented under it.
fn outputs(listing: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut name: Option<String> = None;
    for line in listing.lines() {
        if !line.starts_with(' ') && !line.trim().is_empty() {
            name = line.split_whitespace().next().map(str::to_owned);
        } else if let (Some(n), Some(t)) = (&name, line.trim().strip_prefix("Transform:")) {
            out.push((n.clone(), t.trim().to_owned()));
        }
    }
    out
}

async fn wlr_randr(args: &[&str]) -> Option<String> {
    use std::os::unix::fs::MetadataExt;
    let uid = std::fs::metadata("/proc/self").ok()?.uid();
    let out = tokio::time::timeout(
        Duration::from_secs(5),
        tokio::process::Command::new("wlr-randr")
            .args(args)
            .env("WAYLAND_DISPLAY", "wayland-0")
            .env("XDG_RUNTIME_DIR", format!("/run/user/{uid}"))
            .kill_on_drop(true)
            .output(),
    )
    .await
    .ok()?
    .ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Turns every output cage shows to `rotation`, when it is not already. Without cage (no
/// screen) or wlr-randr (an older image), nothing.
pub async fn apply_rotation(rotation: u16) {
    let want = transform(rotation);
    let Some(listing) = wlr_randr(&[]).await else { return };
    for (name, now) in outputs(&listing) {
        if now != want && wlr_randr(&["--output", &name, "--transform", want]).await.is_some() {
            tracing::info!(output = %name, transform = want, "screen turned");
        }
    }
}

/// Keeps the screen turned as set: the kiosk starting again (after a restart, a change of
/// touch calibration) starts unturned.
pub async fn keep_rotation(prefs: Arc<PrefsStore>) {
    loop {
        if let Some(r) = prefs.get().screen_rotation.filter(|r| *r != 0) {
            apply_rotation(r).await;
        }
        tokio::time::sleep(Duration::from_secs(10)).await;
    }
}

#[cfg(test)]
mod rotation_tests {
    #[test]
    fn outputs_and_their_turn_come_from_the_listing() {
        let listing = "DSI-1 \"(null) (null) (DSI-1)\"\n  Enabled: yes\n  Modes:\n    720x1280 px\n  Transform: normal\n  Scale: 1.000000\nHDMI-A-1 \"x\"\n  Transform: 90\n";
        assert_eq!(
            super::outputs(listing),
            vec![("DSI-1".to_owned(), "normal".to_owned()), ("HDMI-A-1".to_owned(), "90".to_owned())]
        );
        assert_eq!(super::transform(270), "270");
        assert_eq!(super::transform(0), "normal");
    }
}
