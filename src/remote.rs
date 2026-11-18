//! Phone control: phones on the LAN use the same page as the screen, once paired. The screen
//! shows a one-time code (as a QR code); a phone that opens it gets a long-lived token in a
//! cookie. Only a hash of each token is kept. Requests from the box itself (the kiosk,
//! shairport-sync's hooks) need nothing; box administration stays with the screen.

use std::{
    collections::HashMap,
    net::IpAddr,
    sync::Mutex,
    time::{Duration, Instant},
};

use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::prefs::PrefsStore;

const CODE_TTL: Duration = Duration::from_secs(300);
/// Unambiguous letters and digits, as the server's sign-in codes use.
const CODE_ALPHABET: &[u8] = b"ABCDEFGHJKMNPQRTUVWXYZ2346789";
pub const COOKIE: &str = "ownbox_remote";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Paired {
    pub id: String,
    pub name: String,
    /// SHA-256 of the token, hex.
    pub token_hash: String,
    pub paired_at: String,
}

struct Code {
    code: String,
    expires: Instant,
    /// Wrong guesses so far; at `MAX_TRIES` the code stops working.
    misses: u32,
}

const MAX_TRIES: u32 = 10;

/// A phone that adds the page to its home screen gets an app with its own storage, which
/// does not see the browser's cookie. The manifest's start address carries this code, one
/// per paired phone: the app's first start pairs with it, once, within a day. Long and
/// random, so guessing is no way in.
const INSTALL_TTL: Duration = Duration::from_secs(24 * 3600);
const INSTALL_LEN: usize = 16;

pub struct Remote {
    code: Mutex<Option<Code>>,
    /// Install codes by the paired phone they were made for.
    install: Mutex<HashMap<String, (String, Instant)>>,
}

/// What a request may reach, by who sends it.
#[derive(Debug, PartialEq)]
pub enum Access {
    Allowed,
    /// A phone without a valid token: 401.
    NeedsPairing,
    /// Box administration from the LAN: 403.
    ScreenOnly,
}

fn random_hex(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    rand::thread_rng().fill_bytes(&mut buf);
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn hash(token: &str) -> String {
    Sha256::digest(token.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

impl Remote {
    pub fn new() -> Self {
        Self { code: Mutex::new(None), install: Mutex::new(HashMap::new()) }
    }

    /// A new one-time code; the previous one stops working.
    pub fn new_code(&self) -> (String, u64) {
        let mut bytes = [0u8; 6];
        rand::thread_rng().fill_bytes(&mut bytes);
        let code: String = bytes.iter().map(|b| CODE_ALPHABET[*b as usize % CODE_ALPHABET.len()] as char).collect();
        *self.code.lock().expect("code lock") = Some(Code { code: code.clone(), expires: Instant::now() + CODE_TTL, misses: 0 });
        (code, CODE_TTL.as_secs())
    }

    /// The install code for the phone paired as `device`: the one it has while unspent and
    /// fresh, else a new one.
    pub fn install_code(&self, device: &str) -> String {
        let mut codes = self.install.lock().expect("install lock");
        codes.retain(|_, (_, made)| made.elapsed() < INSTALL_TTL);
        codes
            .entry(device.to_owned())
            .or_insert_with(|| {
                let mut bytes = [0u8; INSTALL_LEN];
                rand::thread_rng().fill_bytes(&mut bytes);
                (bytes.iter().map(|b| CODE_ALPHABET[*b as usize % CODE_ALPHABET.len()] as char).collect(), Instant::now())
            })
            .0
            .clone()
    }

    /// Spends the code (once) and pairs the phone: the token to put in its cookie. An
    /// install code (see `install_code`) works as well.
    pub fn pair(&self, code: &str, name: &str, prefs: &PrefsStore) -> Option<String> {
        {
            let mut codes = self.install.lock().expect("install lock");
            codes.retain(|_, (_, made)| made.elapsed() < INSTALL_TTL);
            let given = code.trim();
            if given.len() == INSTALL_LEN
                && let Some(device) = codes.iter().find(|(_, (c, _))| c.eq_ignore_ascii_case(given)).map(|(d, _)| d.clone())
            {
                codes.remove(&device);
                return Some(Self::add(name, prefs));
            }
        }
        let mut current = self.code.lock().expect("code lock");
        let c = current.as_mut()?;
        if c.expires <= Instant::now() || c.misses >= MAX_TRIES {
            *current = None;
            return None;
        }
        if !c.code.eq_ignore_ascii_case(code.trim()) {
            c.misses += 1;
            return None;
        }
        *current = None;
        Some(Self::add(name, prefs))
    }

    /// A box without a screen has nowhere to show a code: the first phone pairs without one
    /// (trust on first use, as a new router does). Once one is paired, this is closed.
    pub fn pair_first(&self, name: &str, prefs: &PrefsStore) -> Option<String> {
        let _held = self.code.lock().expect("code lock");
        if !prefs.get().remote_devices.is_empty() {
            return None;
        }
        Some(Self::add(name, prefs))
    }

    fn add(name: &str, prefs: &PrefsStore) -> String {
        let token = random_hex(32);
        let paired = Paired {
            id: random_hex(8),
            name: name.chars().take(60).collect::<String>(),
            token_hash: hash(&token),
            paired_at: chrono::Utc::now().to_rfc3339(),
        };
        prefs.update(|p| p.remote_devices.push(paired));
        token
    }
}

/// Hooks for programs on the box itself (shairport-sync, squeezelite): never from the LAN.
fn box_only(path: &str) -> bool {
    path.starts_with("/api/outside/airplay") || path.starts_with("/api/outside/music-assistant") || path == "/api/player/release"
}

/// Paths a phone may never use: they run the box itself (sign-in, server, network, screen,
/// pairing) or are hooks for programs on the box.
fn screen_only(path: &str) -> bool {
    const PREFIXES: [&str; 17] = [
        "/api/setup",
        "/api/sign-in",
        "/api/sign-out",
        "/api/wifi",
        "/api/screen",
        "/api/remote/code",
        "/api/remote/qr.svg",
        "/api/remote/devices",
        "/api/remote/forget",
        "/api/outside/airplay",
        "/api/outside/music-assistant",
        "/api/outside/sources",
        "/api/audio/output",
        "/api/audio/hat",
        "/api/hotspot",
        // Its actions; reading what waits (`/api/update`) is fine from a phone.
        "/api/update/",
        "/api/time/",
    ];
    PREFIXES.iter().any(|p| path.starts_with(p)) || path == "/api/player/release"
}

/// Paths an unpaired phone may load: the page itself and the pairing call.
fn public(path: &str) -> bool {
    matches!(
        path,
        "/" | "/app.js" | "/style.css" | "/i18n.js" | "/api/remote/pair" | "/api/remote/status" | "/manifest.webmanifest"
    ) || (path.starts_with("/icons/") && path.ends_with(".png"))
}

/// The box itself: loopback, also as an IPv4 address mapped into IPv6 (`::ffff:127.0.0.1`),
/// which is how IPv4 arrives on the dual-stack `[::]` socket.
pub fn is_box(peer: IpAddr) -> bool {
    peer.to_canonical().is_loopback()
}

/// `headless`: the box has no screen, so a paired phone stands in for it and may run the box.
/// `setup`: the request comes over the box's own setup Wi-Fi, where whoever joined may set
/// the box up (the network exists only while the box has no other) and nothing else. The
/// network has no password, so a phone never pairs over it: anyone near the box could, and a
/// paired phone runs a box without a screen.
pub fn access(peer: IpAddr, path: &str, token: Option<&str>, paired: &[Paired], headless: bool, setup: bool) -> Access {
    if is_box(peer) {
        return Access::Allowed;
    }
    if setup {
        if path == "/api/remote/pair" {
            return Access::ScreenOnly;
        }
        return if public(path) || path.starts_with("/api/hotspot") { Access::Allowed } else { Access::NeedsPairing };
    }
    let is_paired = token.is_some_and(|t| paired.iter().any(|p| p.token_hash == hash(t)));
    if box_only(path) || (screen_only(path) && !(headless && is_paired)) {
        return Access::ScreenOnly;
    }
    if public(path) || is_paired {
        Access::Allowed
    } else {
        Access::NeedsPairing
    }
}

/// Whether a request's `Host:` names the box: an address, `localhost`, a `.local` name or a
/// bare host name. Anything else is a page elsewhere that pointed its own name at the box
/// (DNS rebinding) — its cookies are not the box's, but on a new box the first phone pairs
/// without a code, so such a page could pair itself.
pub fn host_allowed(host: &str) -> bool {
    let name = match host.strip_prefix('[') {
        Some(v6) => return v6.split_once(']').is_some_and(|(a, _)| a.parse::<std::net::Ipv6Addr>().is_ok()),
        None => host.rsplit_once(':').map_or(host, |(h, _)| h),
    };
    let name = name.trim_end_matches('.').to_ascii_lowercase();
    name.is_empty()
        || name.parse::<std::net::Ipv4Addr>().is_ok()
        || name == "localhost"
        || name.ends_with(".local")
        || !name.contains('.')
}

/// The token from a `Cookie:` header.
pub fn token_from_cookies(header: &str) -> Option<&str> {
    header.split(';').map(str::trim).find_map(|c| c.strip_prefix(COOKIE)?.strip_prefix('='))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lan() -> IpAddr {
        "192.168.1.20".parse().unwrap()
    }

    fn one(token: &str) -> Vec<Paired> {
        vec![Paired { id: "a".into(), name: "phone".into(), token_hash: hash(token), paired_at: String::new() }]
    }

    #[test]
    fn only_the_box_s_own_names_are_served() {
        for ok in ["", "192.168.88.64:8686", "10.42.0.1", "[fe80::1]:8686", "[::1]", "localhost:8686", "pirate-radio-box.local:8686", "ownbox.local.", "ownbox"] {
            assert!(host_allowed(ok), "{ok}");
        }
        for bad in ["evil.example.com", "evil.example.com:8686", "192.168.88.64.nip.io", "[not-an-ip]", "rebind.local.evil.com"] {
            assert!(!host_allowed(bad), "{bad}");
        }
    }

    #[test]
    fn an_install_code_pairs_once() {
        let dir = std::env::temp_dir().join(format!("ownbox-install-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let prefs = PrefsStore::load(dir.join("prefs.json"));
        let remote = Remote::new();
        let code = remote.install_code("phone-a");
        assert_eq!(code.len(), INSTALL_LEN);
        assert_eq!(remote.install_code("phone-a"), code, "the same one while unspent");
        assert!(remote.pair(&code.to_lowercase(), "Home screen", &prefs).is_some());
        assert!(remote.pair(&code, "Again", &prefs).is_none(), "spent");
        assert_ne!(remote.install_code("phone-a"), code, "a new one after");
        assert_eq!(prefs.get().remote_devices.len(), 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_box_itself_needs_nothing() {
        assert_eq!(access("127.0.0.1".parse().unwrap(), "/api/wifi", None, &[], false, false), Access::Allowed);
        assert_eq!(access("::1".parse().unwrap(), "/api/wifi", None, &[], false, false), Access::Allowed);
        assert_eq!(access("::ffff:127.0.0.1".parse().unwrap(), "/api/wifi", None, &[], false, false), Access::Allowed);
        // A phone over IPv4 on the dual-stack socket is still a phone.
        assert_eq!(access("::ffff:192.168.1.20".parse().unwrap(), "/api/wifi", None, &[], false, false), Access::ScreenOnly);
    }

    #[test]
    fn the_setup_wifi_sets_up_and_nothing_else() {
        let phone: IpAddr = "10.42.0.23".parse().unwrap();
        assert_eq!(access(phone, "/", None, &[], true, true), Access::Allowed);
        assert_eq!(access(phone, "/api/hotspot/apply", None, &[], true, true), Access::Allowed);
        assert_eq!(access(phone, "/api/player", None, &[], true, true), Access::NeedsPairing);
        assert_eq!(access(phone, "/api/wifi", None, &[], true, true), Access::NeedsPairing);
        assert_eq!(access(phone, "/api/remote/pair", None, &[], true, true), Access::ScreenOnly, "no pairing over an open network");
        assert_eq!(access(phone, "/api/remote/status", None, &[], true, true), Access::Allowed);
        // Not on the setup Wi-Fi: its page is the box's alone.
        assert_eq!(access(lan(), "/api/hotspot", None, &[], true, false), Access::ScreenOnly);
    }

    #[test]
    fn a_phone_without_a_token_gets_the_page_and_nothing_else() {
        assert_eq!(access(lan(), "/", None, &[], false, false), Access::Allowed);
        assert_eq!(access(lan(), "/api/remote/pair", None, &[], false, false), Access::Allowed);
        assert_eq!(access(lan(), "/api/player", None, &[], false, false), Access::NeedsPairing);
        assert_eq!(access(lan(), "/api/player", Some("wrong"), &one("right"), false, false), Access::NeedsPairing);
    }

    #[test]
    fn a_paired_phone_controls_playback_but_not_the_box() {
        let paired = one("secret");
        assert_eq!(access(lan(), "/api/player/play", Some("secret"), &paired, false, false), Access::Allowed);
        assert_eq!(access(lan(), "/api/eq", Some("secret"), &paired, false, false), Access::Allowed);
        assert_eq!(access(lan(), "/api/sign-out", Some("secret"), &paired, false, false), Access::ScreenOnly);
        assert_eq!(access(lan(), "/api/setup/server", Some("secret"), &paired, false, false), Access::ScreenOnly);
        assert_eq!(access(lan(), "/api/outside/airplay/start", Some("secret"), &paired, false, false), Access::ScreenOnly);
        assert_eq!(access(lan(), "/api/player/release", Some("secret"), &paired, false, false), Access::ScreenOnly);
    }

    #[test]
    fn without_a_screen_a_paired_phone_runs_the_box() {
        let paired = one("secret");
        assert_eq!(access(lan(), "/api/setup/server", Some("secret"), &paired, true, false), Access::Allowed);
        assert_eq!(access(lan(), "/api/setup/reset", Some("secret"), &paired, true, false), Access::Allowed);
        assert_eq!(access(lan(), "/api/update", Some("secret"), &paired, false, false), Access::Allowed, "a phone sees that a version waits");
        assert_eq!(access(lan(), "/api/update/install", Some("secret"), &paired, false, false), Access::ScreenOnly, "and only the screen installs it");
        assert_eq!(access(lan(), "/api/setup/reset", Some("wrong"), &paired, true, false), Access::ScreenOnly);
        assert_eq!(access(lan(), "/api/setup/reset", Some("secret"), &paired, false, false), Access::ScreenOnly, "a box with a screen resets from it");
        assert_eq!(access(lan(), "/api/remote/code", Some("secret"), &paired, true, false), Access::Allowed);
        assert_eq!(access(lan(), "/api/setup/server", Some("wrong"), &paired, true, false), Access::ScreenOnly);
        assert_eq!(access(lan(), "/api/player", None, &paired, true, false), Access::NeedsPairing);
        assert_eq!(access(lan(), "/api/outside/airplay/start", Some("secret"), &paired, true, false), Access::ScreenOnly);
        assert_eq!(access(lan(), "/api/player/release", Some("secret"), &paired, true, false), Access::ScreenOnly);
    }

    #[test]
    fn only_the_first_phone_pairs_without_a_code() {
        let prefs = crate::prefs::PrefsStore::load(std::env::temp_dir().join(format!("ownbox-test-{}.json", random_hex(4))));
        let remote = Remote::new();
        assert!(remote.pair_first("phone", &prefs).is_some());
        assert!(remote.pair_first("another", &prefs).is_none());
        assert_eq!(prefs.get().remote_devices.len(), 1);
    }

    #[test]
    fn the_cookie_is_found_among_others() {
        assert_eq!(token_from_cookies("a=1; ownbox_remote=abc; b=2"), Some("abc"));
        assert_eq!(token_from_cookies("a=1"), None);
    }

    #[test]
    fn a_code_works_once() {
        let prefs = crate::prefs::PrefsStore::load(std::env::temp_dir().join(format!("ownbox-test-{}.json", random_hex(4))));
        let remote = Remote::new();
        let (code, _) = remote.new_code();
        assert!(remote.pair("WRONG1", "phone", &prefs).is_none());
        let token = remote.pair(&code.to_lowercase(), "phone", &prefs).expect("paired");
        assert!(remote.pair(&code, "phone", &prefs).is_none());
        assert_eq!(prefs.get().remote_devices[0].token_hash, hash(&token));
    }

    #[test]
    fn too_many_wrong_guesses_end_the_code() {
        let prefs = crate::prefs::PrefsStore::load(std::env::temp_dir().join(format!("ownbox-test-{}.json", random_hex(4))));
        let remote = Remote::new();
        let (code, _) = remote.new_code();
        for _ in 0..MAX_TRIES {
            assert!(remote.pair("WRONG1", "phone", &prefs).is_none());
        }
        assert!(remote.pair(&code, "phone", &prefs).is_none());
    }
}
