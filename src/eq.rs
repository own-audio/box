//! The equalizer, the same one as the own.audio Music apps: six peaking bands at fixed
//! frequencies (Q 1.0), ±12 dB each, a preamp and the apps' seven presets. Kept on the
//! server like the apps keep theirs (`/api/v1/playback/settings/eq`, per account and
//! device kind) with a copy in `prefs.json`; played through ffmpeg filters in mpv.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::api::DEVICE_KIND;

/// `EqBand.standardFrequenciesHz` in the apps.
pub const FREQUENCIES_HZ: [f64; 6] = [60.0, 150.0, 400.0, 1_000.0, 2_400.0, 15_000.0];
pub const LIMIT_DB: f64 = 12.0;

/// `EqPreset.all` in the apps: names (identity, also what the server stores) and curves.
pub const PRESETS: [(&str, [f64; 6]); 7] = [
    ("Flat", [0.0, 0.0, 0.0, 0.0, 0.0, 0.0]),
    ("Bass Boost", [6.0, 4.0, 2.0, 0.0, 0.0, 0.0]),
    ("Treble Boost", [0.0, 0.0, 0.0, 2.0, 4.0, 6.0]),
    ("Spoken Word", [-4.0, -2.0, 3.0, 4.0, 3.0, -2.0]),
    ("Loudness", [5.0, 2.0, 0.0, 0.0, 2.0, 5.0]),
    ("High Speed 1", [0.0, 0.0, 1.0, 2.0, 3.0, 2.0]),
    ("High Speed 2", [0.0, -1.0, 1.0, 3.0, 4.0, 3.0]),
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Eq {
    pub enabled: bool,
    pub preamp_db: f64,
    pub bands_db: [f64; 6],
    pub preset_name: Option<String>,
}

impl Default for Eq {
    fn default() -> Self {
        Self { enabled: false, preamp_db: 0.0, bands_db: [0.0; 6], preset_name: Some("Flat".into()) }
    }
}

impl Eq {
    /// Gains held to ±12 dB, as the apps' sliders and the server do.
    pub fn clamped(mut self) -> Self {
        self.preamp_db = self.preamp_db.clamp(-LIMIT_DB, LIMIT_DB);
        for g in &mut self.bands_db {
            *g = g.clamp(-LIMIT_DB, LIMIT_DB);
        }
        self
    }

    /// mpv's `af` value: the preamp as `volume`, then one `equalizer` (a peaking biquad,
    /// width as Q, like `AVAudioUnitEQ`'s `.parametric`) per band. Off is the empty chain.
    pub fn filter(&self) -> String {
        if !self.enabled {
            return String::new();
        }
        let mut parts = vec![format!("volume={:.1}dB", self.preamp_db)];
        for (f, g) in FREQUENCIES_HZ.iter().zip(self.bands_db) {
            parts.push(format!("equalizer=f={f}:t=q:w=1:g={g:.1}"));
        }
        format!("lavfi=[{}]", parts.join(","))
    }

    pub fn from_server(v: &Value) -> Option<Self> {
        let band = |n: usize| v.get(format!("band{n}_db")).and_then(Value::as_f64);
        Some(
            Self {
                enabled: v.get("enabled")?.as_bool()?,
                preamp_db: v.get("preamp_db")?.as_f64()?,
                bands_db: [band(1)?, band(2)?, band(3)?, band(4)?, band(5)?, band(6)?],
                preset_name: v.get("preset_name").and_then(Value::as_str).map(str::to_owned),
            }
            .clamped(),
        )
    }

    pub fn to_server(&self) -> Value {
        json!({
            "device_kind": DEVICE_KIND,
            "enabled": self.enabled,
            "preamp_db": self.preamp_db,
            "band1_db": self.bands_db[0],
            "band2_db": self.bands_db[1],
            "band3_db": self.bands_db[2],
            "band4_db": self.bands_db[3],
            "band5_db": self.bands_db[4],
            "band6_db": self.bands_db[5],
            "preset_name": self.preset_name,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn off_is_no_filter_at_all() {
        assert_eq!(Eq::default().filter(), "");
    }

    #[test]
    fn on_is_a_preamp_and_six_peaking_bands() {
        let eq = Eq { enabled: true, preamp_db: -3.0, bands_db: PRESETS[1].1, preset_name: Some("Bass Boost".into()) };
        assert_eq!(
            eq.filter(),
            "lavfi=[volume=-3.0dB,equalizer=f=60:t=q:w=1:g=6.0,equalizer=f=150:t=q:w=1:g=4.0,\
             equalizer=f=400:t=q:w=1:g=2.0,equalizer=f=1000:t=q:w=1:g=0.0,\
             equalizer=f=2400:t=q:w=1:g=0.0,equalizer=f=15000:t=q:w=1:g=0.0]"
        );
    }

    #[test]
    fn gains_are_held_to_twelve_db() {
        let eq = Eq { enabled: true, preamp_db: 20.0, bands_db: [-30.0, 0.0, 0.0, 0.0, 0.0, 13.0], preset_name: None }.clamped();
        assert_eq!((eq.preamp_db, eq.bands_db[0], eq.bands_db[5]), (12.0, -12.0, 12.0));
    }

    #[test]
    fn the_servers_record_round_trips() {
        let eq = Eq { enabled: true, preamp_db: -2.0, bands_db: PRESETS[3].1, preset_name: Some("Spoken Word".into()) };
        assert_eq!(Eq::from_server(&eq.to_server()), Some(eq));
    }
}
