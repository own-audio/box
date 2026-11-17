//! Updates of ownbox itself from GitHub Releases: a release carries the `aarch64` binary
//! and its minisign signature. ownbox checks once a day, downloads on a tap and verifies
//! the signature against the public key built into it; a root unit
//! (`setup/install-update.sh`) then verifies again with the installed binary, swaps the
//! file, and puts the previous one back if the new one does not answer within a minute.
//! The system (Debian, Chromium, mpv) is left to Debian.

use std::{
    path::{Path, PathBuf},
    sync::Mutex,
    time::Duration,
};

use anyhow::{Context, anyhow, bail};
use serde::Serialize;
use serde_json::Value;

/// The release key's public half, given at build time (`OWNBOX_UPDATE_PUBKEY`, the base64
/// line of a minisign `.pub` file). A build without it does not update itself.
pub const PUBLIC_KEY: Option<&str> = option_env!("OWNBOX_UPDATE_PUBKEY");
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
const BINARY_ASSET: &str = "ownbox-aarch64";
/// Where the root unit reports (ownbox cannot write there, so it cannot fake a result).
const RESULT_FILE: &str = "/var/lib/ownbox/update-result";
const PREVIOUS_VERSION_FILE: &str = "/var/lib/ownbox/previous-version";

#[derive(Debug, Clone, Serialize)]
pub struct Release {
    pub version: String,
    pub notes: String,
    #[serde(skip)]
    binary_url: String,
    #[serde(skip)]
    signature_url: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct View {
    pub current: &'static str,
    /// Updates are off: no feed configured, or a build without the release key.
    pub enabled: bool,
    pub available: Option<Release>,
    /// "checking", "downloading", "installing", or an error from the last attempt.
    pub state: Option<String>,
    /// What the root unit reported last ("installed 0.2.0", "rolled back …").
    pub last_result: Option<String>,
    /// The version kept for going back by hand.
    pub previous: Option<String>,
}

pub struct Updater {
    feed: String,
    dir: PathBuf,
    available: Mutex<Option<Release>>,
    state: Mutex<Option<String>>,
}

/// A version as semver orders it: `v1.2.3`, `1.0.0-beta.2`; build metadata (`+…`) is
/// ignored.
#[derive(Debug, PartialEq, Eq)]
struct Version {
    core: (u64, u64, u64),
    /// Empty for a release, which comes after all of its pre-releases.
    pre: Vec<String>,
}

fn parse_version(v: &str) -> Option<Version> {
    let v = v.trim().trim_start_matches('v').split('+').next()?;
    let (core, pre) = match v.split_once('-') {
        Some((c, p)) => (c, p.split('.').map(str::to_owned).collect()),
        None => (v, Vec::new()),
    };
    let mut parts = core.split('.').map(|p| p.parse::<u64>().ok());
    let core = (parts.next()??, parts.next().flatten().unwrap_or(0), parts.next().flatten().unwrap_or(0));
    Some(Version { core, pre })
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering::*;
        self.core.cmp(&other.core).then_with(|| match (self.pre.is_empty(), other.pre.is_empty()) {
            (true, true) => Equal,
            (true, false) => Greater,
            (false, true) => Less,
            // Numeric identifiers by number and before words, as semver says.
            (false, false) => {
                for (a, b) in self.pre.iter().zip(&other.pre) {
                    let o = match (a.parse::<u64>(), b.parse::<u64>()) {
                        (Ok(x), Ok(y)) => x.cmp(&y),
                        (Ok(_), Err(_)) => Less,
                        (Err(_), Ok(_)) => Greater,
                        (Err(_), Err(_)) => a.cmp(b),
                    };
                    if o != Equal {
                        return o;
                    }
                }
                self.pre.len().cmp(&other.pre.len())
            }
        })
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

pub fn newer(candidate: &str, current: &str) -> bool {
    matches!((parse_version(candidate), parse_version(current)), (Some(a), Some(b)) if a > b)
}

/// The newest signed release in a feed: GitHub's list of releases (which, unlike
/// `releases/latest`, includes the betas) or a single release document. Drafts are left
/// out, and so is a release without the signed binary.
fn newest_release(doc: &Value) -> Option<Release> {
    match doc.as_array() {
        Some(list) => list
            .iter()
            .filter(|r| !r.get("draft").and_then(Value::as_bool).unwrap_or(false))
            .filter_map(parse_release)
            .filter(|r| parse_version(&r.version).is_some())
            .max_by(|a, b| parse_version(&a.version).cmp(&parse_version(&b.version))),
        None => parse_release(doc),
    }
}

/// A GitHub "latest release" document: the tag, the notes and the two assets.
fn parse_release(v: &Value) -> Option<Release> {
    let asset = |name: &str| {
        v.get("assets")?
            .as_array()?
            .iter()
            .find(|a| a.get("name").and_then(Value::as_str) == Some(name))?
            .get("browser_download_url")?
            .as_str()
            .map(str::to_owned)
    };
    Some(Release {
        version: v.get("tag_name")?.as_str()?.trim_start_matches('v').to_owned(),
        notes: v.get("body").and_then(Value::as_str).unwrap_or_default().to_owned(),
        binary_url: asset(BINARY_ASSET)?,
        signature_url: asset(&format!("{BINARY_ASSET}.minisig"))?,
    })
}

/// Whether `data` carries a good signature by the release key. `ownbox verify` (run by the
/// root unit) and the download both use it.
pub fn verify(data: &[u8], signature: &str) -> anyhow::Result<()> {
    let key = PUBLIC_KEY.filter(|k| !k.is_empty()).ok_or_else(|| anyhow!("this build has no release key"))?;
    verify_with(key, data, signature)
}

fn verify_with(key: &str, data: &[u8], signature: &str) -> anyhow::Result<()> {
    let key = minisign_verify::PublicKey::from_base64(key.trim()).map_err(|e| anyhow!("release key: {e}"))?;
    let sig = minisign_verify::Signature::decode(signature).map_err(|e| anyhow!("signature: {e}"))?;
    key.verify(data, &sig, false).map_err(|e| anyhow!("signature does not match: {e}"))
}

/// `ownbox verify <file> <signature>`: exit status 0 only for a good signature.
pub fn verify_files(file: &Path, signature: &Path) -> anyhow::Result<()> {
    let data = std::fs::read(file).with_context(|| format!("reading {}", file.display()))?;
    let sig = std::fs::read_to_string(signature).with_context(|| format!("reading {}", signature.display()))?;
    verify(&data, &sig)
}

fn read_trimmed(path: &str) -> Option<String> {
    std::fs::read_to_string(path).ok().map(|s| s.trim().to_owned()).filter(|s| !s.is_empty())
}

impl Updater {
    pub fn new(feed: String, dir: PathBuf) -> Self {
        Self { feed, dir, available: Mutex::new(None), state: Mutex::new(None) }
    }

    pub fn enabled(&self) -> bool {
        !self.feed.is_empty() && PUBLIC_KEY.is_some_and(|k| !k.is_empty())
    }

    fn set_state(&self, state: Option<&str>) {
        *self.state.lock().expect("update lock") = state.map(str::to_owned);
    }

    pub fn view(&self) -> View {
        View {
            current: VERSION,
            enabled: self.enabled(),
            available: self.available.lock().expect("update lock").clone(),
            state: self.state.lock().expect("update lock").clone(),
            last_result: read_trimmed(RESULT_FILE),
            previous: Path::new("/opt/ownbox/ownbox.previous")
                .exists()
                .then(|| read_trimmed(PREVIOUS_VERSION_FILE).unwrap_or_else(|| "?".into())),
        }
    }

    pub async fn check(&self, http: &reqwest::Client) -> anyhow::Result<Option<Release>> {
        if !self.enabled() {
            return Ok(None);
        }
        let doc: Value = http
            .get(&self.feed)
            .header(reqwest::header::USER_AGENT, format!("ownbox/{VERSION}"))
            .header(reqwest::header::ACCEPT, "application/vnd.github+json")
            .timeout(Duration::from_secs(20))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let release = newest_release(&doc).ok_or_else(|| anyhow!("no release with {BINARY_ASSET} and a signature"))?;
        let found = newer(&release.version, VERSION).then_some(release);
        *self.available.lock().expect("update lock") = found.clone();
        Ok(found)
    }

    /// Once a day; the first check a minute after start, when the network is up.
    pub async fn run(self: std::sync::Arc<Self>, http: reqwest::Client) {
        if !self.enabled() {
            return;
        }
        tokio::time::sleep(Duration::from_secs(60)).await;
        loop {
            // Daily; after a failed check (no network yet, GitHub busy) again within the hour
            // rather than a day later.
            let wait = match self.check(&http).await {
                Ok(Some(r)) => {
                    tracing::info!(version = %r.version, "update available");
                    24 * 3600
                }
                Ok(None) => 24 * 3600,
                Err(err) => {
                    tracing::warn!(%err, "update check; trying again in an hour");
                    3600
                }
            };
            tokio::time::sleep(Duration::from_secs(wait)).await;
        }
    }

    /// Downloads the available release, verifies it, and asks the root unit to install it.
    pub async fn install(&self, http: &reqwest::Client) -> anyhow::Result<()> {
        let release = self.available.lock().expect("update lock").clone().ok_or_else(|| anyhow!("no update available"))?;
        self.set_state(Some("downloading"));
        let result = self.fetch_and_request(http, &release).await;
        self.set_state(match &result {
            Ok(()) => Some("installing"),
            Err(_) => None,
        });
        result
    }

    async fn fetch_and_request(&self, http: &reqwest::Client, release: &Release) -> anyhow::Result<()> {
        let get = |url: &str| {
            http.get(url)
                .header(reqwest::header::USER_AGENT, format!("ownbox/{VERSION}"))
                .timeout(Duration::from_secs(300))
                .send()
        };
        let binary = get(&release.binary_url).await?.error_for_status()?.bytes().await?;
        let signature = get(&release.signature_url).await?.error_for_status()?.text().await?;
        if binary.len() < 100_000 || !binary.starts_with(b"\x7fELF") {
            bail!("the download is not a program");
        }
        verify(&binary, &signature)?;
        tokio::fs::create_dir_all(&self.dir).await?;
        tokio::fs::write(self.dir.join("ownbox.new"), &binary).await?;
        tokio::fs::write(self.dir.join("ownbox.new.minisig"), signature.as_bytes()).await?;
        // The root unit's path unit watches for this file.
        tokio::fs::write(self.dir.join("request"), format!("install {}\n", release.version)).await?;
        Ok(())
    }

    /// Asks the root unit to put the previous binary back.
    pub async fn rollback(&self) -> anyhow::Result<()> {
        tokio::fs::create_dir_all(&self.dir).await?;
        tokio::fs::write(self.dir.join("request"), "rollback\n").await?;
        self.set_state(Some("installing"));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // A throwaway key made for this test only; releases are signed with another.
    const TEST_KEY: &str = "RWR3l11h/7sRiQJWIWnWz493JsAcTEg4kgj9pC8O9V9s/4CImfqZYsKO";
    const TEST_SIG: &str = "untrusted comment: signature from minisign secret key
RUR3l11h/7sRicuFIWZpZhxJ/3mHjjocJphedVjB2XTCRjo+E1/hlA5mdm8VYz3QcmSFVljnfRBOPfK8ejsnpX5IJC3U6otN6gI=
trusted comment: timestamp:1794701022\tfile:data.txt\thashed
5mc/HnOpDo5gj+kAIcBXoa71ISXfXNC90ZjZ38lV5Q/BREvSPmj+tNJeI4SiYhBm0ZlEeF1SHgSwI8OEneHuBg==
";

    #[test]
    fn a_good_signature_passes_and_anything_else_does_not() {
        assert!(verify_with(TEST_KEY, b"own.audio box test data\n", TEST_SIG).is_ok());
        assert!(verify_with(TEST_KEY, b"own.audio box test data, changed\n", TEST_SIG).is_err());
        assert!(verify_with(TEST_KEY, b"own.audio box test data\n", "not a signature").is_err());
        let other = "RWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO3";
        assert!(verify_with(other, b"own.audio box test data\n", TEST_SIG).is_err());
    }

    #[test]
    fn compares_versions() {
        assert!(newer("v0.2.0", "0.1.0"));
        assert!(newer("0.1.10", "0.1.9"));
        assert!(!newer("0.1.0", "0.1.0"));
        assert!(!newer("v0.0.9", "0.1.0"));
        assert!(!newer("nightly", "0.1.0"));
        assert!(newer("1.0.0-rc.1", "0.9.0"));
        assert!(newer("1.0.0-beta.2", "1.0.0-beta.1"));
        assert!(newer("1.0.0-beta.10", "1.0.0-beta.9"));
        assert!(newer("1.0.0", "1.0.0-rc.1"));
        assert!(newer("1.0.0-rc.1", "1.0.0-beta.3"));
        assert!(!newer("1.0.0-beta.1", "1.0.0"));
        assert!(!newer("1.0.0-beta.1", "1.0.0-beta.1+build.5"));
        assert!(newer("1.0.0-beta.1", "0.1.0"));
    }

    #[test]
    fn picks_the_newest_from_a_list() {
        let rel = |tag: &str, draft: bool| json!({
            "tag_name": tag, "draft": draft,
            "assets": [
                { "name": "ownbox-aarch64", "browser_download_url": format!("https://example/{tag}") },
                { "name": "ownbox-aarch64.minisig", "browser_download_url": "https://example/sig" },
            ],
        });
        let list = json!([rel("v1.0.0-beta.2", false), rel("v1.0.0-beta.3", true), rel("v1.0.0-beta.10", false), rel("v1.0.0-beta.1", false)]);
        assert_eq!(newest_release(&list).expect("release").version, "1.0.0-beta.10");
        assert!(newest_release(&json!([])).is_none());
    }

    #[test]
    fn reads_a_github_release() {
        let doc = json!({
            "tag_name": "v0.2.0",
            "body": "Notes",
            "assets": [
                { "name": "ownbox-aarch64", "browser_download_url": "https://example/bin" },
                { "name": "ownbox-aarch64.minisig", "browser_download_url": "https://example/sig" },
            ],
        });
        let r = parse_release(&doc).expect("release");
        assert_eq!((r.version.as_str(), r.binary_url.as_str(), r.signature_url.as_str()), ("0.2.0", "https://example/bin", "https://example/sig"));
        let unsigned = json!({ "tag_name": "v0.2.0", "assets": [{ "name": "ownbox-aarch64", "browser_download_url": "x" }] });
        assert!(parse_release(&unsigned).is_none());
    }
}
