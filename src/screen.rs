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
    pub off_after_secs: u64,
    pub controllable: bool,
}

#[derive(Deserialize)]
pub struct Settings {
    pub brightness: Option<u8>,
    pub off_after_secs: Option<u64>,
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
            off_after_secs: self.off_after_secs.load(Ordering::Relaxed),
            controllable: self.path.is_some(),
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
