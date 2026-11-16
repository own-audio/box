//! What this box is built from, found at start rather than set by hand: the board, the
//! panel, the sound cards. The bench box (Pi 3, the old 7" display, a USB speakerphone) and
//! the real one (Pi 4, Touch Display 2, an I²S DAC) run the same image.

use serde::Serialize;

use crate::{config::Config, prefs::Prefs};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputKind {
    /// A DAC on the GPIO header (I²S): the DAC+, HiFiBerry and the like.
    I2s,
    Usb,
    Hdmi,
    /// The Pi's own 3.5 mm jack.
    Analog,
}

#[derive(Debug, Clone, Serialize)]
pub struct Output {
    /// mpv's `--audio-device`.
    pub device: String,
    pub label: String,
    pub kind: OutputKind,
}

#[derive(Debug, Clone, Serialize)]
pub struct Display {
    pub connector: String,
    pub mode: String,
    pub name: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct Hardware {
    pub board: Option<String>,
    pub display: Option<Display>,
    pub temperature_c: Option<f64>,
    pub cpu_mhz: Option<u64>,
}

pub fn read() -> Hardware {
    let board = std::fs::read_to_string("/proc/device-tree/model")
        .ok()
        .map(|m| m.trim_end_matches('\0').trim().to_owned());
    let temperature_c = std::fs::read_to_string("/sys/class/thermal/thermal_zone0/temp")
        .ok()
        .and_then(|t| t.trim().parse::<f64>().ok())
        .map(|m| (m / 100.0).round() / 10.0);
    let cpu_mhz = std::fs::read_to_string("/sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq")
        .ok()
        .and_then(|f| f.trim().parse::<u64>().ok())
        .map(|khz| khz / 1000);
    Hardware { board, display: display(), temperature_c, cpu_mhz }
}

/// The first connected panel: DSI before HDMI.
fn display() -> Option<Display> {
    let mut found: Vec<(String, String)> = std::fs::read_dir("/sys/class/drm")
        .ok()?
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let connector = name.split_once('-')?.1.to_owned();
            let status = std::fs::read_to_string(e.path().join("status")).ok()?;
            if status.trim() != "connected" {
                return None;
            }
            let mode = std::fs::read_to_string(e.path().join("modes")).ok()?.lines().next()?.to_owned();
            Some((connector, mode))
        })
        .collect();
    found.sort_by_key(|(c, _)| !c.starts_with("DSI"));
    let (connector, mode) = found.into_iter().next()?;
    let name = display_name(&connector, &mode);
    Some(Display { connector, mode, name })
}

/// The three screens the box supports, by connector and mode. The 5" and 7" Touch Display
/// 2 share a mode; they differ only in size.
fn display_name(connector: &str, mode: &str) -> &'static str {
    match (connector.starts_with("DSI"), mode) {
        (true, "800x480") => "Raspberry Pi 7\" display",
        (true, "720x1280" | "1280x720") => "Raspberry Pi Touch Display 2",
        (true, _) => "DSI display",
        (false, _) => "HDMI",
    }
}

/// `/proc/asound/cards`: an index line ` 1 [USB            ]: USB-Audio - Jabra SPEAK 410 USB`
/// followed by a long-name line.
fn parse_cards(text: &str) -> Vec<Output> {
    text.lines()
        .filter_map(|line| {
            let (_, rest) = line.split_once('[')?;
            let (id, rest) = rest.split_once(']')?;
            let (driver, name) = rest.trim_start_matches(':').trim().split_once(" - ")?;
            let id = id.trim();
            let kind = if driver == "USB-Audio" {
                OutputKind::Usb
            } else if driver.starts_with("vc4-hdmi") || id.starts_with("vc4hdmi") {
                OutputKind::Hdmi
            } else if id == "Headphones" || driver.starts_with("bcm2835") {
                OutputKind::Analog
            } else {
                OutputKind::I2s
            };
            let label = match kind {
                OutputKind::Analog => "3.5 mm jack".to_owned(),
                OutputKind::Hdmi => "HDMI".to_owned(),
                _ => name.trim().to_owned(),
            };
            Some(Output { device: format!("alsa/plughw:CARD={id}"), label, kind })
        })
        .collect()
}

pub fn outputs() -> Vec<Output> {
    std::fs::read_to_string("/proc/asound/cards").map(|t| parse_cards(&t)).unwrap_or_default()
}

/// The output to play through and mpv's options for it: the one chosen in Settings, else
/// `config.toml`'s, else the best found (an I²S DAC, then USB, then HDMI, then the jack).
/// A choice whose card is not plugged in falls through to the next.
pub fn chosen(prefs: &Prefs, config: &Config) -> (String, Vec<String>) {
    pick(prefs.audio_device.as_deref(), config, &outputs())
}

fn pick(preferred: Option<&str>, config: &Config, found: &[Output]) -> (String, Vec<String>) {
    let present = |device: &str| found.is_empty() || found.iter().any(|o| o.device == device);
    let device = [preferred, Some(config.audio_device.as_str())]
        .into_iter()
        .flatten()
        .find(|d| !d.is_empty() && present(d))
        .map(str::to_owned)
        .or_else(|| {
            [OutputKind::I2s, OutputKind::Usb, OutputKind::Hdmi, OutputKind::Analog]
                .iter()
                .find_map(|k| found.iter().find(|o| o.kind == *k))
                .map(|o| o.device.clone())
        })
        .unwrap_or_else(|| "auto".to_owned());
    // config.toml's options were tuned for its own device (the Jabra plays 48 kHz / 16 bit
    // only); any other output plays at the file's own rate.
    let options = if device == config.audio_device { config.audio_options.clone() } else { vec!["--audio-buffer=1".to_owned()] };
    (device, options)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PI3: &str = " 0 [Headphones     ]: bcm2835_headpho - bcm2835 Headphones
                      bcm2835 Headphones
 1 [USB            ]: USB-Audio - Jabra SPEAK 410 USB
                      Jabra SPEAK 410 USB at usb-3f980000.usb-1.3, full speed
 2 [vc4hdmi        ]: vc4-hdmi - vc4-hdmi
                      vc4-hdmi
";

    fn dac() -> Output {
        Output { device: "alsa/plughw:CARD=DACplus".into(), label: "RPi DAC+".into(), kind: OutputKind::I2s }
    }

    #[test]
    fn reads_the_bench_boxs_cards() {
        let cards = parse_cards(PI3);
        let kinds: Vec<_> = cards.iter().map(|c| c.kind).collect();
        assert_eq!(kinds, [OutputKind::Analog, OutputKind::Usb, OutputKind::Hdmi]);
        assert_eq!(cards[1].device, "alsa/plughw:CARD=USB");
        assert_eq!(cards[1].label, "Jabra SPEAK 410 USB");
    }

    #[test]
    fn config_device_keeps_its_own_options() {
        let config = Config::default();
        let (device, options) = pick(None, &config, &parse_cards(PI3));
        assert_eq!(device, "alsa/plughw:CARD=USB");
        assert_eq!(options, config.audio_options);
    }

    #[test]
    fn a_dac_wins_when_the_configured_card_is_missing() {
        let config = Config::default();
        let mut found = parse_cards(PI3);
        found.retain(|o| o.kind != OutputKind::Usb);
        found.push(dac());
        let (device, options) = pick(None, &config, &found);
        assert_eq!(device, "alsa/plughw:CARD=DACplus");
        assert_eq!(options, ["--audio-buffer=1"]);
    }

    #[test]
    fn the_settings_choice_wins_while_plugged_in() {
        let config = Config::default();
        let mut found = parse_cards(PI3);
        found.push(dac());
        assert_eq!(pick(Some("alsa/plughw:CARD=DACplus"), &config, &found).0, "alsa/plughw:CARD=DACplus");
        assert_eq!(pick(Some("alsa/plughw:CARD=Gone"), &config, &found).0, "alsa/plughw:CARD=USB");
    }

    #[test]
    fn names_the_supported_screens() {
        assert_eq!(display_name("DSI-1", "800x480"), "Raspberry Pi 7\" display");
        assert_eq!(display_name("DSI-1", "720x1280"), "Raspberry Pi Touch Display 2");
        assert_eq!(display_name("HDMI-A-1", "1920x1080"), "HDMI");
    }
}
