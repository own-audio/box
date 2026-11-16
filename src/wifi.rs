//! Wi-Fi from the screen, through NetworkManager's `nmcli` (the system already runs it).
//! ownbox may manage connections because of `setup/50-ownbox-network.rules`; the Wi-Fi
//! country is set once at deploy (`raspi-config nonint do_wifi_country`), without which
//! Raspberry Pi OS keeps the radio blocked.
//!
//! Arguments go to nmcli as separate arguments, never through a shell, and the password is
//! never logged.

use std::{collections::HashMap, time::Duration};

use anyhow::{Context, bail};
use serde::Serialize;
use tokio::process::Command;

const QUICK: Duration = Duration::from_secs(10);
const SCAN: Duration = Duration::from_secs(20);
const CONNECT: Duration = Duration::from_secs(45);
const IFACE: &str = "wlan0";

#[derive(Debug, Serialize, PartialEq)]
pub struct Network {
    pub ssid: String,
    pub signal: u8,
    pub secure: bool,
    pub in_use: bool,
    pub saved: bool,
}

#[derive(Debug, Serialize)]
pub struct Status {
    /// The radio is on (NetworkManager's `radio wifi`).
    pub radio: bool,
    /// There is a Wi-Fi device NetworkManager can use (false while it is blocked).
    pub available: bool,
    pub connected: Option<String>,
    pub address: Option<String>,
    pub ethernet: Option<String>,
    pub saved: Vec<String>,
}

async fn nmcli(args: &[&str], timeout: Duration) -> anyhow::Result<String> {
    let out = tokio::time::timeout(timeout, Command::new("nmcli").args(args).output())
        .await
        .context("NetworkManager took too long")?
        .context("cannot run nmcli")?;
    if !out.status.success() {
        bail!("{}", friendly(String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// nmcli's messages are for administrators; the two that people hit get plain words.
fn friendly(message: &str) -> String {
    let m = message.trim_start_matches("Error: ");
    if m.contains("Secrets were required") || m.contains("802-11-wireless-security.psk") {
        "Wrong password.".to_owned()
    } else if m.contains("No network with SSID") {
        "That network is out of reach.".to_owned()
    } else {
        m.to_owned()
    }
}

/// One line of nmcli's terse output: fields split on ':' except where escaped as '\:'.
fn fields(line: &str) -> Vec<String> {
    let mut out = vec![String::new()];
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                if let Some(next) = chars.next() {
                    out.last_mut().expect("one field").push(next);
                }
            }
            ':' => out.push(String::new()),
            _ => out.last_mut().expect("one field").push(c),
        }
    }
    out
}

async fn ipv4_of(device: &str) -> Option<String> {
    let text = nmcli(&["-t", "-f", "IP4.ADDRESS", "device", "show", device], QUICK).await.ok()?;
    text.lines()
        .filter_map(|l| fields(l).get(1).cloned())
        .find(|a| !a.is_empty())
        .map(|a| a.split('/').next().unwrap_or(&a).to_owned())
}

async fn saved() -> Vec<String> {
    let text = nmcli(&["-t", "-f", "NAME,TYPE", "connection", "show"], QUICK).await.unwrap_or_default();
    text.lines()
        .map(fields)
        .filter(|f| f.get(1).is_some_and(|t| t == "802-11-wireless"))
        .filter_map(|f| f.into_iter().next())
        .collect()
}

pub async fn status() -> anyhow::Result<Status> {
    let radio = nmcli(&["radio", "wifi"], QUICK).await?.trim() == "enabled";
    let devices = nmcli(&["-t", "-f", "DEVICE,TYPE,STATE,CONNECTION", "device"], QUICK).await?;
    let mut available = false;
    let mut connected = None;
    let mut ethernet_up = false;
    for f in devices.lines().map(fields) {
        match (f.first().map(String::as_str), f.get(1).map(String::as_str), f.get(2).map(String::as_str)) {
            (Some(IFACE), _, Some(state)) => {
                available = state != "unavailable";
                if state == "connected" {
                    connected = f.get(3).cloned().filter(|c| !c.is_empty());
                }
            }
            (_, Some("ethernet"), Some("connected")) => ethernet_up = true,
            _ => {}
        }
    }
    let address = if connected.is_some() { ipv4_of(IFACE).await } else { None };
    let ethernet = if ethernet_up { ipv4_of("eth0").await } else { None };
    Ok(Status { radio, available, connected, address, ethernet, saved: saved().await })
}

pub async fn set_radio(on: bool) -> anyhow::Result<()> {
    nmcli(&["radio", "wifi", if on { "on" } else { "off" }], QUICK).await.map(drop)
}

/// Networks in reach, one entry per name (the strongest), the one in use first.
pub async fn scan() -> anyhow::Result<Vec<Network>> {
    let text = nmcli(&["-t", "-f", "IN-USE,SSID,SIGNAL,SECURITY", "device", "wifi", "list", "--rescan", "yes"], SCAN).await?;
    let saved = saved().await;
    Ok(parse_scan(&text, &saved))
}

fn parse_scan(text: &str, saved: &[String]) -> Vec<Network> {
    let mut best: HashMap<String, Network> = HashMap::new();
    for f in text.lines().map(fields) {
        let [in_use, ssid, signal, security] = f.as_slice() else { continue };
        if ssid.is_empty() {
            continue; // hidden network
        }
        let n = Network {
            ssid: ssid.clone(),
            signal: signal.parse().unwrap_or(0),
            secure: !security.is_empty() && security != "--",
            in_use: in_use == "*",
            saved: saved.contains(ssid),
        };
        match best.get(ssid) {
            Some(old) if old.signal >= n.signal && !n.in_use => {}
            _ => {
                let in_use = n.in_use || best.get(ssid).is_some_and(|o| o.in_use);
                best.insert(ssid.clone(), Network { in_use, ..n });
            }
        }
    }
    let mut list: Vec<Network> = best.into_values().collect();
    list.sort_by(|a, b| b.in_use.cmp(&a.in_use).then(b.signal.cmp(&a.signal)).then(a.ssid.cmp(&b.ssid)));
    list
}

/// Joins a network: a saved one by its stored settings unless a new password is given.
pub async fn connect(ssid: &str, password: Option<&str>) -> anyhow::Result<()> {
    let is_saved = saved().await.iter().any(|s| s == ssid);
    match password.filter(|p| !p.is_empty()) {
        None if is_saved => nmcli(&["connection", "up", "id", ssid], CONNECT).await.map(drop),
        None => nmcli(&["device", "wifi", "connect", ssid, "ifname", IFACE], CONNECT).await.map(drop),
        Some(pw) => {
            if is_saved {
                // A new password for a known network: start clean rather than half-edit it.
                let _ = nmcli(&["connection", "delete", "id", ssid], QUICK).await;
            }
            nmcli(&["device", "wifi", "connect", ssid, "password", pw, "ifname", IFACE], CONNECT).await.map(drop)
        }
    }
}

pub async fn forget(ssid: &str) -> anyhow::Result<()> {
    nmcli(&["connection", "delete", "id", ssid], QUICK).await.map(drop)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terse_fields_keep_escaped_colons() {
        assert_eq!(fields(r"*:Home\:5G:72:WPA2"), ["*", "Home:5G", "72", "WPA2"]);
        assert_eq!(fields(r" :Back\\slash:40:"), [" ", r"Back\slash", "40", ""]);
    }

    #[test]
    fn scan_keeps_the_strongest_of_each_name_and_the_one_in_use_first() {
        let text = " :Neighbour:80:WPA2\n*:Home:55:WPA2\n :Home:70:WPA2\n :Cafe:60:\n ::90:WPA2\n";
        let list = parse_scan(text, &["Home".to_owned()]);
        let names: Vec<&str> = list.iter().map(|n| n.ssid.as_str()).collect();
        assert_eq!(names, ["Home", "Neighbour", "Cafe"]);
        assert!(list[0].in_use && list[0].saved && list[0].signal == 70);
        assert!(!list[2].secure);
    }

    #[test]
    fn nmcli_errors_in_plain_words() {
        assert_eq!(friendly("Error: Connection activation failed: Secrets were required, but not provided."), "Wrong password.");
        assert_eq!(friendly("Error: No network with SSID 'X' found."), "That network is out of reach.");
    }
}
