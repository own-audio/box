//! The setup Wi-Fi: a box without a screen and without a network opens a Wi-Fi of its own,
//! named after the box. A phone or laptop that joins gets the setup page by itself — the
//! network's DNS answers every name with the box (`setup/dnsmasq-ownbox-setup.conf`) and
//! ownbox on port 80 sends every other address to its page, which is how phones find a
//! hotel's sign-in page. There the home Wi-Fi, the box's name, its language and its sound
//! card are chosen; the box then leaves its own network and joins the home one.
//!
//! The Zero 2 W has one radio: networks are listed before the box's own Wi-Fi goes up, and
//! while it is up the box cannot look for the home network. So a box that knows a network
//! closes its own every few minutes (unless someone is on the page) and tries again.

use std::{
    net::{IpAddr, Ipv4Addr},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use serde::Serialize;

use crate::wifi::{self, Network};

/// The box on its own network (NetworkManager's shared mode).
pub const ADDRESS: Ipv4Addr = Ipv4Addr::new(10, 42, 0, 1);
const CONNECTION: &str = "ownbox-setup";
/// Without a network this long, the box opens its own.
const OFFLINE_FOR: Duration = Duration::from_secs(60);
/// With a network it knows, the box closes its own after this long to look for it again…
const OPEN_FOR: Duration = Duration::from_secs(5 * 60);
/// …unless the page was used this recently.
const IN_USE_FOR: Duration = Duration::from_secs(3 * 60);
const CHECK_EVERY: Duration = Duration::from_secs(10);

#[derive(Default)]
struct State {
    on: bool,
    since: Option<Instant>,
    networks: Vec<Network>,
    /// Why the last try to join failed, for the page when the phone is back.
    error: Option<String>,
    used: Option<Instant>,
    /// The network being joined right now: the box's own Wi-Fi is down meanwhile.
    joining: Option<String>,
}

#[derive(Serialize)]
pub struct Snapshot {
    pub active: bool,
    pub networks: Vec<Network>,
    pub error: Option<String>,
}

#[derive(Default)]
pub struct Hotspot {
    state: Mutex<State>,
}

impl Hotspot {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn active(&self) -> bool {
        self.lock().on
    }

    /// Whether a request comes from someone on the box's own network.
    pub fn serves(&self, peer: IpAddr) -> bool {
        let IpAddr::V4(v4) = peer.to_canonical() else { return false };
        self.active() && v4.octets()[..3] == ADDRESS.octets()[..3] && v4 != ADDRESS
    }

    /// Someone is on the page: the box keeps its own Wi-Fi up for them.
    pub fn touch(&self) {
        self.lock().used = Some(Instant::now());
    }

    pub fn snapshot(&self) -> Snapshot {
        let s = self.lock();
        Snapshot { active: s.on, networks: s.networks.clone(), error: s.error.clone() }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().expect("hotspot lock")
    }

    /// Watches the network for as long as ownbox runs. `ssid` names the box's own Wi-Fi.
    pub async fn run(self: Arc<Self>, ssid: impl Fn() -> String + Send + 'static) {
        // A setup network left over from a crash or a restart is not ours to keep.
        let _ = wifi::nmcli(&["connection", "down", "id", CONNECTION], wifi::QUICK).await;
        let mut offline_since: Option<Instant> = None;
        loop {
            tokio::time::sleep(CHECK_EVERY).await;
            if self.lock().joining.is_some() {
                continue;
            }
            let online = online().await;
            let on = self.active();
            if online {
                offline_since = None;
                if on {
                    tracing::info!("a network is back; closing the setup Wi-Fi");
                    self.stop().await;
                }
                continue;
            }
            if !on {
                let since = *offline_since.get_or_insert_with(Instant::now);
                if since.elapsed() >= OFFLINE_FOR
                    && let Err(err) = self.start(&ssid()).await
                {
                    tracing::warn!(%err, "opening the setup Wi-Fi");
                }
                continue;
            }
            // Open: now and then give a known network another chance.
            let (since, used) = {
                let s = self.lock();
                (s.since, s.used)
            };
            let open_long = since.is_some_and(|t| t.elapsed() >= OPEN_FOR);
            let in_use = used.is_some_and(|t| t.elapsed() < IN_USE_FOR);
            if open_long && !in_use && knows_a_network().await {
                tracing::info!("trying the known networks again");
                self.stop().await;
                offline_since = Some(Instant::now());
            }
        }
    }

    async fn start(&self, ssid: &str) -> anyhow::Result<()> {
        // One radio: what is around is listed before it starts sending.
        let networks = wifi::scan().await.unwrap_or_default();
        let _ = wifi::nmcli(&["connection", "delete", "id", CONNECTION], wifi::QUICK).await;
        let address = format!("{ADDRESS}/24");
        wifi::nmcli(
            &[
                "connection", "add", "type", "wifi", "ifname", wifi::IFACE, "con-name", CONNECTION,
                "autoconnect", "no", "ssid", ssid,
                "802-11-wireless.mode", "ap", "802-11-wireless.band", "bg",
                "ipv4.method", "shared", "ipv4.addresses", &address, "ipv6.method", "disabled",
            ],
            wifi::QUICK,
        )
        .await?;
        wifi::nmcli(&["connection", "up", "id", CONNECTION], wifi::CONNECT).await?;
        tracing::info!(%ssid, networks = networks.len(), "setup Wi-Fi open");
        let mut s = self.lock();
        s.on = true;
        s.since = Some(Instant::now());
        s.networks = networks;
        Ok(())
    }

    async fn stop(&self) {
        let _ = wifi::nmcli(&["connection", "down", "id", CONNECTION], wifi::QUICK).await;
        let mut s = self.lock();
        s.on = false;
        s.since = None;
    }

    /// Leaves the box's own Wi-Fi and joins `ssid`; back to its own (with the reason) if
    /// that fails. `then` runs once the home network is joined.
    pub async fn join(self: Arc<Self>, ssid: String, password: Option<String>, own_ssid: String, then: impl FnOnce() + Send) {
        {
            let mut s = self.lock();
            s.joining = Some(ssid.clone());
            s.error = None;
        }
        // Let the page's answer reach the phone before the network goes.
        tokio::time::sleep(Duration::from_secs(3)).await;
        self.stop().await;
        // Fresh from sending, the radio knows no networks: NetworkManager would answer "out
        // of reach" at once. Look first, and give it a few tries.
        let mut joined = Err(anyhow::anyhow!("not tried"));
        for attempt in 1..=3 {
            let _ = wifi::scan().await;
            joined = wifi::connect(&ssid, password.as_deref()).await;
            match &joined {
                Ok(()) => break,
                Err(err) => tracing::info!(%ssid, attempt, %err, "joining from the setup Wi-Fi"),
            }
            tokio::time::sleep(Duration::from_secs(3)).await;
        }
        self.lock().joining = None;
        match joined {
            Ok(()) => {
                tracing::info!(%ssid, "joined from the setup Wi-Fi");
                then();
            }
            Err(err) => {
                tracing::warn!(%ssid, %err, "joining from the setup Wi-Fi");
                self.lock().error = Some(err.to_string());
                if let Err(err) = self.start(&own_ssid).await {
                    tracing::warn!(%err, "opening the setup Wi-Fi again");
                }
            }
        }
    }
}

/// A network other than the box's own: Wi-Fi or a cable.
async fn online() -> bool {
    match wifi::status().await {
        Ok(s) => s.ethernet.is_some() || s.connected.is_some_and(|c| c != CONNECTION),
        // Without NetworkManager there is nothing to set up from here.
        Err(_) => true,
    }
}

async fn knows_a_network() -> bool {
    wifi::status().await.is_ok_and(|s| s.saved.iter().any(|n| n != CONNECTION))
}

/// The box's address name as `apply-sources.sh` sets it from the name given on the box:
/// "own.audio pirate radio" → `pirate-radio`. Accents go, the rest becomes dashes.
pub fn host_name(name: &str) -> String {
    let folded: String = name
        .to_lowercase()
        .replace("own.audio", "")
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ä' | 'ã' | 'å' => 'a',
            'č' | 'ç' | 'ć' => 'c',
            'ď' => 'd',
            'é' | 'è' | 'ê' | 'ë' | 'ě' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ľ' | 'ĺ' | 'ł' => 'l',
            'ň' | 'ñ' | 'ń' => 'n',
            'ó' | 'ò' | 'ô' | 'ö' | 'õ' | 'ø' => 'o',
            'ř' | 'ŕ' => 'r',
            'š' | 'ś' => 's',
            'ť' => 't',
            'ú' | 'ù' | 'û' | 'ü' | 'ů' => 'u',
            'ý' | 'ÿ' => 'y',
            'ž' | 'ź' | 'ż' => 'z',
            c if c.is_ascii_alphanumeric() => c,
            _ => '-',
        })
        .collect();
    let mut out = String::new();
    for part in folded.split('-').filter(|p| !p.is_empty()) {
        if !out.is_empty() {
            out.push('-');
        }
        out.push_str(part);
    }
    let out: String = out.chars().take(40).collect::<String>().trim_matches('-').to_owned();
    if out.is_empty() || out == "box" { "ownbox".to_owned() } else { out }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_become_addresses_as_the_script_makes_them() {
        assert_eq!(host_name("own.audio pirate radio box"), "pirate-radio-box");
        assert_eq!(host_name("own.audio box"), "ownbox");
        assert_eq!(host_name("Kuchyň"), "kuchyn");
        assert_eq!(host_name("Obývák 2"), "obyvak-2");
        assert_eq!(host_name("own.audio rádio"), "radio");
        assert_eq!(host_name("  ***  "), "ownbox");
    }

    #[test]
    fn only_its_own_network_is_served() {
        let h = Hotspot::default();
        let phone: IpAddr = "10.42.0.23".parse().unwrap();
        assert!(!h.serves(phone), "closed");
        h.lock().on = true;
        assert!(h.serves(phone));
        assert!(h.serves("::ffff:10.42.0.23".parse().unwrap()));
        assert!(!h.serves("192.168.1.20".parse().unwrap()));
        assert!(!h.serves(IpAddr::V4(ADDRESS)), "the box itself");
    }
}
