//! Phone control: phones on the LAN use the same page as the screen, once paired. The screen
//! shows a one-time code (as a QR code); a phone that opens it gets a long-lived token in a
//! cookie. Only a hash of each token is kept. Requests from the box itself (the kiosk,
//! shairport-sync's hooks) need nothing; box administration stays with the screen.

use std::{
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

pub struct Remote {
    code: Mutex<Option<Code>>,
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
        Self { code: Mutex::new(None) }
    }

    /// A new one-time code; the previous one stops working.
    pub fn new_code(&self) -> (String, u64) {
        let mut bytes = [0u8; 6];
        rand::thread_rng().fill_bytes(&mut bytes);
        let code: String = bytes.iter().map(|b| CODE_ALPHABET[*b as usize % CODE_ALPHABET.len()] as char).collect();
        *self.code.lock().expect("code lock") = Some(Code { code: code.clone(), expires: Instant::now() + CODE_TTL, misses: 0 });
        (code, CODE_TTL.as_secs())
    }

    /// Spends the code (once) and pairs the phone: the token to put in its cookie.
    pub fn pair(&self, code: &str, name: &str, prefs: &PrefsStore) -> Option<String> {
        let mut current = self.code.lock().expect("code lock");
        let Some(c) = current.as_mut() else { return None };
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
    const PREFIXES: [&str; 14] = [
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
        "/api/update",
    ];
    PREFIXES.iter().any(|p| path.starts_with(p)) || path == "/api/player/release"
}

/// Paths an unpaired phone may load: the page itself and the pairing call.
fn public(path: &str) -> bool {
    matches!(path, "/" | "/app.js" | "/style.css" | "/i18n.js" | "/api/remote/pair" | "/api/remote/status")
}

/// `headless`: the box has no screen, so a paired phone stands in for it and may run the box.
pub fn access(peer: IpAddr, path: &str, token: Option<&str>, paired: &[Paired], headless: bool) -> Access {
    if peer.is_loopback() {
        return Access::Allowed;
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
    fn the_box_itself_needs_nothing() {
        assert_eq!(access("127.0.0.1".parse().unwrap(), "/api/wifi", None, &[], false), Access::Allowed);
    }

    #[test]
    fn a_phone_without_a_token_gets_the_page_and_nothing_else() {
        assert_eq!(access(lan(), "/", None, &[], false), Access::Allowed);
        assert_eq!(access(lan(), "/api/remote/pair", None, &[], false), Access::Allowed);
        assert_eq!(access(lan(), "/api/player", None, &[], false), Access::NeedsPairing);
        assert_eq!(access(lan(), "/api/player", Some("wrong"), &one("right"), false), Access::NeedsPairing);
    }

    #[test]
    fn a_paired_phone_controls_playback_but_not_the_box() {
        let paired = one("secret");
        assert_eq!(access(lan(), "/api/player/play", Some("secret"), &paired, false), Access::Allowed);
        assert_eq!(access(lan(), "/api/eq", Some("secret"), &paired, false), Access::Allowed);
        assert_eq!(access(lan(), "/api/sign-out", Some("secret"), &paired, false), Access::ScreenOnly);
        assert_eq!(access(lan(), "/api/setup/server", Some("secret"), &paired, false), Access::ScreenOnly);
        assert_eq!(access(lan(), "/api/outside/airplay/start", Some("secret"), &paired, false), Access::ScreenOnly);
        assert_eq!(access(lan(), "/api/player/release", Some("secret"), &paired, false), Access::ScreenOnly);
    }

    #[test]
    fn without_a_screen_a_paired_phone_runs_the_box() {
        let paired = one("secret");
        assert_eq!(access(lan(), "/api/setup/server", Some("secret"), &paired, true), Access::Allowed);
        assert_eq!(access(lan(), "/api/remote/code", Some("secret"), &paired, true), Access::Allowed);
        assert_eq!(access(lan(), "/api/setup/server", Some("wrong"), &paired, true), Access::ScreenOnly);
        assert_eq!(access(lan(), "/api/player", None, &paired, true), Access::NeedsPairing);
        assert_eq!(access(lan(), "/api/outside/airplay/start", Some("secret"), &paired, true), Access::ScreenOnly);
        assert_eq!(access(lan(), "/api/player/release", Some("secret"), &paired, true), Access::ScreenOnly);
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
