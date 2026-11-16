//! The first-run wizard's server step: is this address an own.audio server the box can
//! sign in to? Asked of the server's public discovery document, `/.well-known/own-audio`.

use std::time::Duration;

use reqwest::Client;
use serde::{Deserialize, Serialize};

/// The hosted service, offered as the first choice.
pub const HOSTED_URL: &str = "https://api.own.audio";

#[derive(Debug, Serialize, PartialEq)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum Check {
    Ok { url: String, name: String, edition: Option<String>, version: Option<String> },
    /// Nothing answered (wrong address, the server is off, no network).
    Unreachable { url: String },
    /// Something answered, but not an own.audio server.
    NotOwnAudio { url: String },
    /// An own.audio server too old to sign a device in with a code.
    NoDeviceSignIn { url: String, version: Option<String> },
}

#[derive(Deserialize)]
struct WellKnown {
    name: Option<String>,
    edition: Option<String>,
    version: Option<String>,
    #[serde(default)]
    auth: Option<WellKnownAuth>,
}

#[derive(Deserialize)]
struct WellKnownAuth {
    #[serde(default)]
    device_grant: bool,
}

/// What people type: "nas:8099", "192.168.1.5:8099/", "https://api.own.audio". A missing
/// scheme means plain HTTP (a server at home seldom has a certificate); the trailing slash
/// goes, so paths can be appended.
pub fn normalize(input: &str) -> Option<String> {
    let input = input.trim();
    let (scheme, rest) = match input.split_once("://") {
        Some((scheme @ ("http" | "https"), rest)) => (scheme, rest),
        Some(_) => return None,
        None => ("http", input),
    };
    let rest = rest.trim_end_matches('/');
    if rest.is_empty() || rest.chars().any(char::is_whitespace) {
        return None;
    }
    Some(format!("{scheme}://{rest}"))
}

pub async fn check(http: &Client, url: &str) -> Check {
    let url = url.to_owned();
    let answer = http
        .get(format!("{url}/.well-known/own-audio"))
        .timeout(Duration::from_secs(8))
        .send()
        .await;
    let response = match answer {
        Ok(r) => r,
        Err(_) => return Check::Unreachable { url },
    };
    if !response.status().is_success() {
        return Check::NotOwnAudio { url };
    }
    let Ok(doc) = response.json::<WellKnown>().await else {
        return Check::NotOwnAudio { url };
    };
    if doc.name.as_deref() != Some("own.audio") {
        return Check::NotOwnAudio { url };
    }
    if !doc.auth.is_some_and(|a| a.device_grant) {
        return Check::NoDeviceSignIn { url, version: doc.version };
    }
    Check::Ok { url, name: "own.audio".into(), edition: doc.edition, version: doc.version }
}

#[cfg(test)]
mod tests {
    use super::normalize;

    #[test]
    fn addresses_people_type() {
        assert_eq!(normalize("nas:8099").as_deref(), Some("http://nas:8099"));
        assert_eq!(normalize(" 192.168.1.5:8099/ ").as_deref(), Some("http://192.168.1.5:8099"));
        assert_eq!(normalize("https://api.own.audio/").as_deref(), Some("https://api.own.audio"));
    }

    #[test]
    fn nothing_usable() {
        assert_eq!(normalize(""), None);
        assert_eq!(normalize("http://"), None);
        assert_eq!(normalize("my nas"), None);
        assert_eq!(normalize("ftp://nas"), None);
    }
}
