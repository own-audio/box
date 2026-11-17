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

pub const QUICK: Duration = Duration::from_secs(10);
const SCAN: Duration = Duration::from_secs(20);
pub const CONNECT: Duration = Duration::from_secs(45);
pub const IFACE: &str = "wlan0";

#[derive(Debug, Clone, Serialize, PartialEq)]
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

pub async fn nmcli(args: &[&str], timeout: Duration) -> anyhow::Result<String> {
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

/// How often the box looks for a better access point of the network it is on.
const ROAM_EVERY: Duration = Duration::from_secs(120);
/// How much stronger (nmcli's 0–100 signal) another access point must be to move to it: a
/// move drops the network for a second or two, so two equal ones must not ping-pong.
const ROAM_MARGIN: u8 = 20;
/// After a move the box stays put this long: the chip's own roaming or a signal that only
/// looked strong in one scan must not send it back and forth.
const ROAM_REST: Duration = Duration::from_secs(10 * 60);

/// One access point in reach (several may send the same network name).
#[derive(Debug, Clone, PartialEq)]
struct AccessPoint {
    bssid: String,
    ssid: String,
    signal: u8,
    in_use: bool,
}

fn parse_access_points(text: &str) -> Vec<AccessPoint> {
    text.lines()
        .map(fields)
        .filter_map(|f| {
            let [in_use, bssid, ssid, signal] = f.as_slice() else { return None };
            Some(AccessPoint { bssid: bssid.clone(), ssid: ssid.clone(), signal: signal.parse().unwrap_or(0), in_use: in_use == "*" })
        })
        .collect()
}

/// The access point to move to, if one of the same network is clearly stronger than the
/// one in use.
fn better_access_point(aps: &[AccessPoint]) -> Option<&AccessPoint> {
    let current = aps.iter().find(|a| a.in_use)?;
    aps.iter()
        .filter(|a| !a.in_use && a.ssid == current.ssid && a.bssid != current.bssid)
        .max_by_key(|a| a.signal)
        .filter(|a| a.signal >= current.signal.saturating_add(ROAM_MARGIN))
}

/// Keeps the box on the strongest access point of its network. NetworkManager stays with
/// the one it joined until that one is gone — a router replaced by a new one that sends the
/// same name, or a mesh, leaves the box at the far end of the house. The move names the
/// access point for this one activation only; the saved connection is not touched.
pub async fn roam() {
    // One scan can be wrong: a move needs the same stronger access point twice in a row.
    let mut seen_better: Option<String> = None;
    // Access points that refused the box are not tried again until ownbox restarts: a
    // router that will not take it (a Wi-Fi chip it does not like, band steering) would
    // otherwise cost the box its network every few minutes.
    let mut refused: Vec<String> = Vec::new();
    loop {
        tokio::time::sleep(ROAM_EVERY).await;
        let Ok(Status { connected: Some(connection), ethernet: None, .. }) = status().await else { continue };
        if connection == crate::hotspot::CONNECTION {
            continue;
        }
        let Ok(text) = nmcli(&["-t", "-f", "IN-USE,BSSID,SSID,SIGNAL", "device", "wifi", "list", "ifname", IFACE, "--rescan", "auto"], SCAN).await else {
            continue;
        };
        let mut aps = parse_access_points(&text);
        aps.retain(|a| a.in_use || !refused.contains(&a.bssid));
        let Some(better) = better_access_point(&aps) else {
            seen_better = None;
            continue;
        };
        if seen_better.as_deref() != Some(better.bssid.as_str()) {
            seen_better = Some(better.bssid.clone());
            continue;
        }
        seen_better = None;
        let from = aps.iter().find(|a| a.in_use).map_or(0, |a| a.signal);
        tracing::info!(ssid = %better.ssid, from, to = better.signal, bssid = %better.bssid, "moving to a stronger access point");
        let bssid = better.bssid.clone();
        let moved = nmcli(&["connection", "up", "id", &connection, "ifname", IFACE, "ap", &bssid], CONNECT).await;
        let on = nmcli(&["-t", "-f", "IN-USE,BSSID", "device", "wifi", "list", "ifname", IFACE, "--rescan", "no"], QUICK)
            .await
            .unwrap_or_default()
            .lines()
            .map(fields)
            .find(|f| f.first().is_some_and(|c| c == "*"))
            .and_then(|f| f.get(1).cloned());
        if moved.is_err() || on.as_deref() != Some(bssid.as_str()) {
            tracing::warn!(%bssid, err = ?moved.err(), "the stronger access point refused the box; staying with the old one");
            refused.push(bssid);
            // Back where it was, by name: NetworkManager may otherwise keep trying the one
            // that refused, as it is the strongest.
            if let Some(old) = aps.iter().find(|a| a.in_use) {
                let _ = nmcli(&["connection", "up", "id", &connection, "ifname", IFACE, "ap", &old.bssid], CONNECT).await;
            }
        }
        tokio::time::sleep(ROAM_REST).await;
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
    fn moves_only_to_a_clearly_stronger_access_point_of_the_same_network() {
        let text = "*:D4\\:01\\:C3\\:61\\:E7\\:34:Home:27\n :D0\\:EA\\:11\\:A7\\:C2\\:C5:Home:97\n :AA\\:BB\\:CC\\:DD\\:EE\\:FF:Neighbour:100\n";
        let aps = parse_access_points(text);
        assert_eq!(aps[0].bssid, "D4:01:C3:61:E7:34");
        assert_eq!(better_access_point(&aps).map(|a| a.bssid.as_str()), Some("D0:EA:11:A7:C2:C5"));

        let close = parse_access_points("*:A:Home:60\n :B:Home:75\n");
        assert_eq!(better_access_point(&close), None, "not worth a drop");
        let other_network = parse_access_points("*:A:Home:30\n :B:Neighbour:99\n");
        assert_eq!(better_access_point(&other_network), None);
        assert_eq!(better_access_point(&parse_access_points(" :A:Home:30\n :B:Home:99\n")), None, "not connected");
    }

    #[test]
    fn nmcli_errors_in_plain_words() {
        assert_eq!(friendly("Error: Connection activation failed: Secrets were required, but not provided."), "Wrong password.");
        assert_eq!(friendly("Error: No network with SSID 'X' found."), "That network is out of reach.");
    }
}
