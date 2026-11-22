//! Debug access: root over SSH with keys the owner chose (their GitHub account's public
//! keys), and the system journal kept on the card across restarts. Off unless switched on
//! in Settings on the box's own screen. ownbox only keeps the choice in prefs.json;
//! `apply-sources.sh` (root) writes the keys and the journal setting, and checks the keys
//! again, since ownbox is not trusted with root's files.

use anyhow::{bail, Context};

/// A GitHub user name: letters, digits and single hyphens, up to 39 characters.
pub fn valid_user(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 39
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        && !name.starts_with('-')
        && !name.ends_with('-')
        && !name.contains("--")
}

/// A public key line as GitHub gives it: a key type and the key, nothing else (no
/// `command=` or other options, which would come before the type).
pub fn valid_key(line: &str) -> bool {
    const TYPES: [&str; 6] = [
        "ssh-ed25519",
        "ssh-rsa",
        "ecdsa-sha2-nistp256",
        "ecdsa-sha2-nistp384",
        "ecdsa-sha2-nistp521",
        "sk-ssh-ed25519@openssh.com",
    ];
    let mut parts = line.split(' ');
    let (Some(kind), Some(key)) = (parts.next(), parts.next()) else { return false };
    parts.next().is_none()
        && TYPES.contains(&kind)
        && (20..=2000).contains(&key.len())
        && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=')
}

/// The public keys of a GitHub account (`https://github.com/<user>.keys`).
pub async fn github_keys(http: &reqwest::Client, user: &str) -> anyhow::Result<Vec<String>> {
    if !valid_user(user) {
        bail!("not a GitHub user name");
    }
    let text = http
        .get(format!("https://github.com/{user}.keys"))
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .context("cannot reach GitHub")?
        .error_for_status()
        .context("no such GitHub user")?
        .text()
        .await?;
    let keys: Vec<String> = text.lines().map(str::trim).filter(|l| valid_key(l)).map(String::from).take(20).collect();
    if keys.is_empty() {
        bail!("this GitHub user has no SSH keys");
    }
    Ok(keys)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_names() {
        assert!(valid_user("kornelko2"));
        assert!(valid_user("own-audio"));
        assert!(!valid_user(""));
        assert!(!valid_user("-x"));
        assert!(!valid_user("a--b"));
        assert!(!valid_user("a/b"));
        assert!(!valid_user("../x"));
    }

    #[test]
    fn keys() {
        assert!(valid_key("ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIA0Mabcdefghijklmnop"));
        assert!(!valid_key("command=\"x\" ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIA0M"));
        assert!(!valid_key("ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIA0M comment"));
        assert!(!valid_key("ssh-dss AAAAC3NzaC1lZDI1NTE5AAAAIA0Mabcdefghij"));
        assert!(!valid_key("ssh-ed25519 short"));
        assert!(!valid_key("ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIA0M\nssh-rsa AAAA"));
    }
}
