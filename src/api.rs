//! The own.audio API calls that need no session: device sign-in and token refresh. The
//! signed-in calls go through `session::Auth`, which adds the token and refreshes it.

use std::time::Duration;

use anyhow::{Context, bail};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const USER_AGENT: &str = concat!("own.audio-box/", env!("CARGO_PKG_VERSION"));
/// The server knows web/ios/android/macos/windows/tvos; everything else is "other".
pub const DEVICE_KIND: &str = "other";

pub fn http_client() -> Client {
    Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(20))
        .build()
        .expect("building the HTTP client")
}

pub fn url(server: &str, path: &str) -> String {
    format!("{}{}", server.trim_end_matches('/'), path)
}

#[derive(Debug, Deserialize)]
pub struct DeviceStart {
    pub device_code: String,
    pub user_code: String,
    /// The web console's `/link` page; empty when the server has no public address set.
    pub verification_url: String,
    pub expires_in: u64,
    pub interval: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub email: String,
    #[serde(default)]
    pub display_name: String,
}

#[derive(Debug, Deserialize)]
pub struct Login {
    pub token: String,
    pub refresh_token: String,
    pub user: User,
}

#[derive(Debug, Deserialize)]
pub struct Tokens {
    pub token: String,
    pub refresh_token: String,
}

#[derive(Debug)]
pub enum Poll {
    Approved(Login),
    Pending,
    SlowDown,
    Denied,
    Expired,
}

pub async fn device_start(http: &Client, server: &str, device_name: &str) -> anyhow::Result<DeviceStart> {
    let r = http
        .post(url(server, "/api/v1/auth/device/start"))
        .json(&json!({ "device_name": device_name, "device_kind": DEVICE_KIND }))
        .send()
        .await
        .context("cannot reach the server")?;
    match r.status() {
        s if s.is_success() => Ok(r.json().await?),
        StatusCode::TOO_MANY_REQUESTS => bail!("too many attempts; try again in a minute"),
        s => bail!("sign-in failed (HTTP {})", s.as_u16()),
    }
}

pub async fn device_poll(http: &Client, server: &str, device_code: &str) -> anyhow::Result<Poll> {
    let r = http
        .post(url(server, "/api/v1/auth/device/poll"))
        .json(&json!({ "device_code": device_code }))
        .send()
        .await
        .context("cannot reach the server")?;
    Ok(match r.status().as_u16() {
        200 => Poll::Approved(r.json().await?),
        202 => {
            let body: Value = r.json().await.unwrap_or_default();
            match body.get("status").and_then(Value::as_str) {
                Some("slow_down") => Poll::SlowDown,
                Some("denied") => Poll::Denied,
                Some("expired") => Poll::Expired,
                _ => Poll::Pending,
            }
        }
        400 => Poll::Expired,
        429 => Poll::SlowDown,
        s => bail!("sign-in failed (HTTP {s})"),
    })
}

/// `Ok(None)` when the server refuses the refresh token: the session is over.
pub async fn refresh(http: &Client, server: &str, refresh_token: &str) -> anyhow::Result<Option<Tokens>> {
    let r = http
        .post(url(server, "/api/v1/auth/refresh"))
        .json(&json!({ "refresh_token": refresh_token }))
        .send()
        .await
        .context("cannot reach the server")?;
    match r.status().as_u16() {
        200..=299 => Ok(Some(r.json().await?)),
        // The server says a refresh token is no good with 401 only. A 400 or 403 can come
        // from something in between (a proxy, a firewall, a hotel's sign-in page): the box
        // tries again later instead of forgetting its sign-in for good.
        401 => Ok(None),
        s => bail!("server error (HTTP {s})"),
    }
}
