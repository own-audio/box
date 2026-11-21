//! How long the box took to start, from systemd's own record (`systemd-analyze`): shown in
//! Settings → About the box, so a slow start can be read without a console.

use std::{sync::OnceLock, time::Duration};

use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Startup {
    /// "2.9s (kernel) + 12.3s (userspace) = 15.2s", as systemd says it.
    pub total: String,
    /// The slowest units, slowest first: ("NetworkManager-wait-online.service", "6.1s").
    pub slowest: Vec<(String, String)>,
}

static STARTUP: OnceLock<Startup> = OnceLock::new();

async fn analyze(args: &[&str]) -> Option<String> {
    let out = tokio::time::timeout(
        Duration::from_secs(10),
        tokio::process::Command::new("systemd-analyze").args(args).kill_on_drop(true).output(),
    )
    .await
    .ok()?
    .ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

fn total(text: &str) -> Option<String> {
    let line = text.lines().find(|l| l.starts_with("Startup finished in "))?;
    Some(line.trim_start_matches("Startup finished in ").trim().to_owned())
}

fn slowest(text: &str, n: usize) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|l| {
            let l = l.trim();
            let (time, unit) = l.rsplit_once(' ')?;
            Some((unit.to_owned(), time.trim().to_owned()))
        })
        .take(n)
        .collect()
}

/// The start's times, once the start has finished (before that systemd has none).
pub async fn read() -> Option<Startup> {
    if let Some(s) = STARTUP.get() {
        return Some(s.clone());
    }
    let total = total(&analyze(&["time"]).await?)?;
    let slowest = analyze(&["blame", "--no-pager"]).await.map(|b| slowest(&b, 5)).unwrap_or_default();
    Some(STARTUP.get_or_init(|| Startup { total, slowest }).clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn systemds_words_become_the_figures() {
        let time = "Startup finished in 2.912s (kernel) + 12.301s (userspace) = 15.213s \ngraphical.target reached after 12.2s in userspace.\n";
        assert_eq!(total(time).as_deref(), Some("2.912s (kernel) + 12.301s (userspace) = 15.213s"));
        let blame = "  6.120s NetworkManager-wait-online.service\n1min 2.004s cloud-init-local.service\n   820ms ownbox-boot.service\n";
        assert_eq!(
            slowest(blame, 2),
            vec![
                ("NetworkManager-wait-online.service".to_owned(), "6.120s".to_owned()),
                ("cloud-init-local.service".to_owned(), "1min 2.004s".to_owned())
            ]
        );
    }
}
