//! The box's time zone: the night updates and the clock follow it. Images start on
//! Raspberry Pi OS's default (Europe/London) or UTC;
//! the box sets it through systemd-timedated (`timedatectl`), which polkit allows the
//! box's user (`setup/50-ownbox-network.rules`).

use std::time::Duration;

use tokio::process::Command;

/// Zones nobody chose: UTC, and Raspberry Pi OS's own default, Europe/London.
const UNCHOSEN: [&str; 4] = ["UTC", "Etc/UTC", "Universal", "Europe/London"];

async fn timedatectl(args: &[&str]) -> anyhow::Result<String> {
    let out = tokio::time::timeout(Duration::from_secs(10), Command::new("timedatectl").args(args).kill_on_drop(true).output())
        .await??;
    if !out.status.success() {
        anyhow::bail!("{}", String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

pub async fn current() -> String {
    timedatectl(&["show", "-p", "Timezone", "--value"]).await.map(|z| z.trim().to_owned()).unwrap_or_else(|_| "UTC".into())
}

pub async fn zones() -> Vec<String> {
    timedatectl(&["list-timezones"]).await.map(|t| t.lines().map(str::to_owned).collect()).unwrap_or_default()
}

/// Only a zone the system knows: the name goes to timedatectl as it is.
pub async fn set(zone: &str) -> anyhow::Result<()> {
    if !zones().await.iter().any(|z| z == zone) {
        anyhow::bail!("unknown time zone");
    }
    timedatectl(&["set-timezone", zone]).await.map(drop)
}

/// The zone a language suggests, for a box still on the default: Czech is spoken in one.
pub fn suggested(language: &str) -> Option<&'static str> {
    match language {
        "cs" => Some("Europe/Prague"),
        _ => None,
    }
}

/// When a language is chosen: a box still on the default takes the zone it suggests.
pub async fn follow_language(language: &str) {
    let Some(zone) = suggested(language) else { return };
    if UNCHOSEN.contains(&current().await.as_str())
        && let Err(err) = set(zone).await
    {
        tracing::warn!(%err, zone, "setting the time zone");
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn czech_suggests_prague_and_english_nothing() {
        assert_eq!(super::suggested("cs"), Some("Europe/Prague"));
        assert_eq!(super::suggested("en"), None);
    }
}
