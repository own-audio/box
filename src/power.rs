//! The firmware's throttle flags. The box runs on a weak supply, so they are on the status
//! screen rather than only in a terminal.

use serde::Serialize;
use tokio::process::Command;

#[derive(Debug, Serialize)]
pub struct Power {
    pub raw: String,
    pub under_voltage_now: bool,
    pub throttled_now: bool,
    pub under_voltage_since_boot: bool,
    pub throttled_since_boot: bool,
}

/// `None` where there is no `vcgencmd` (not a Pi) or it fails.
pub async fn read() -> Option<Power> {
    let out = Command::new("vcgencmd").arg("get_throttled").output().await.ok()?;
    if !out.status.success() {
        return None;
    }
    parse(&String::from_utf8_lossy(&out.stdout))
}

fn parse(text: &str) -> Option<Power> {
    let hex = text.trim().strip_prefix("throttled=0x")?;
    let v = u32::from_str_radix(hex, 16).ok()?;
    Some(Power {
        raw: format!("0x{v:x}"),
        under_voltage_now: v & 0x1 != 0,
        throttled_now: v & 0x4 != 0,
        under_voltage_since_boot: v & 0x1_0000 != 0,
        throttled_since_boot: v & 0x4_0000 != 0,
    })
}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn reads_the_flags_this_pi_reports() {
        let p = parse("throttled=0x50005\n").unwrap();
        assert!(p.under_voltage_now && p.throttled_now);
        assert!(p.under_voltage_since_boot && p.throttled_since_boot);
        assert_eq!(p.raw, "0x50005");
    }

    #[test]
    fn healthy_supply() {
        let p = parse("throttled=0x0").unwrap();
        assert!(!p.under_voltage_now && !p.under_voltage_since_boot);
    }

    #[test]
    fn rejects_other_output() {
        assert!(parse("error=1 error_msg=\"Command not registered\"").is_none());
    }
}
