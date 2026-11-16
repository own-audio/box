//! Signing the box in with a code, keeping the session on disk, and making signed-in calls.

use std::{
    io::Write,
    sync::atomic::{AtomicBool, Ordering},
    os::unix::fs::OpenOptionsExt,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::{Context, bail};
use reqwest::{Client, Method, StatusCode};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use tokio::{sync::Mutex, task::JoinHandle};

use crate::api::{self, Poll, User};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Saved {
    server_url: String,
    token: String,
    refresh_token: String,
    user: User,
}

struct Pending {
    user_code: String,
    /// What the QR code opens: the web console's link page with the code filled in.
    link: Option<String>,
    expires_at: Instant,
    task: JoinHandle<()>,
}

#[derive(Default)]
struct State {
    saved: Option<Saved>,
    pending: Option<Pending>,
    /// The last thing that went wrong, shown on the sign-in screen until the next attempt.
    error: Option<String>,
}

/// What the UI is told.
#[derive(Debug, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum View {
    SignedOut { error: Option<String> },
    Pending { user_code: String, link: Option<String>, expires_in: u64 },
    SignedIn { user: User, server_reachable: bool },
}

pub struct Auth {
    http: Client,
    /// Set by the first-run wizard and changeable in Settings, hence behind a lock.
    server_url: std::sync::RwLock<String>,
    device_name: std::sync::RwLock<String>,
    path: PathBuf,
    state: Mutex<State>,
    /// A refresh token works once, so concurrent 401s wait for one refresh instead of each
    /// spending it.
    refreshing: Mutex<()>,
    /// Whether the last call reached the server at all (any answer counts, errors too).
    reachable: AtomicBool,
}

impl Auth {
    pub fn load(http: Client, server_url: String, device_name: String, path: PathBuf) -> Arc<Self> {
        let mut state = State::default();
        match std::fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str::<Saved>(&text) {
                Ok(saved) if saved.server_url == server_url => state.saved = Some(saved),
                Ok(saved) => tracing::warn!(old = %saved.server_url, "the session is for another server; signing in again"),
                Err(err) => tracing::warn!(%err, "unreadable session file; signing in again"),
            },
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => tracing::warn!(%err, path = %path.display(), "cannot read the session"),
        }
        Arc::new(Self {
            http,
            server_url: std::sync::RwLock::new(server_url),
            device_name: std::sync::RwLock::new(device_name),
            path,
            state: Mutex::new(state),
            refreshing: Mutex::new(()),
            reachable: AtomicBool::new(true),
        })
    }

    pub fn server(&self) -> String {
        self.server_url.read().expect("server lock").clone()
    }

    pub fn device_name(&self) -> String {
        self.device_name.read().expect("name lock").clone()
    }

    /// The name the next sign-in gives the box in the account's devices.
    pub fn set_device_name(&self, name: String) {
        *self.device_name.write().expect("name lock") = name;
    }

    /// Points the box at another server. The session belongs to the old one, so it ends
    /// (locally; the old server may be out of reach) along with any code being shown.
    pub async fn set_server(&self, url: String) {
        let mut state = self.state.lock().await;
        if let Some(p) = state.pending.take() {
            p.task.abort();
        }
        state.saved = None;
        state.error = None;
        let _ = std::fs::remove_file(&self.path);
        *self.server_url.write().expect("server lock") = url;
        self.reachable.store(true, Ordering::Relaxed);
    }

    pub fn reachable(&self) -> bool {
        self.reachable.load(Ordering::Relaxed)
    }

    pub async fn view(&self) -> View {
        let state = self.state.lock().await;
        if let Some(saved) = &state.saved {
            return View::SignedIn { user: saved.user.clone(), server_reachable: self.reachable.load(Ordering::Relaxed) };
        }
        if let Some(p) = &state.pending {
            return View::Pending {
                user_code: p.user_code.clone(),
                link: p.link.clone(),
                expires_in: p.expires_at.saturating_duration_since(Instant::now()).as_secs(),
            };
        }
        View::SignedOut { error: state.error.clone() }
    }

    /// The QR code's target while a sign-in is waiting.
    pub async fn pending_link(&self) -> Option<String> {
        self.state.lock().await.pending.as_ref().and_then(|p| p.link.clone())
    }

    /// Asks the server for a code and polls in the background until it is approved,
    /// refused or expired. A second call replaces the first code.
    pub async fn start_sign_in(self: &Arc<Self>) -> anyhow::Result<()> {
        let start = match api::device_start(&self.http, &self.server(), &self.device_name()).await {
            Ok(s) => s,
            Err(err) => {
                self.state.lock().await.error = Some(err.to_string());
                return Err(err);
            }
        };
        let link = (!start.verification_url.is_empty())
            .then(|| format!("{}?code={}", start.verification_url, start.user_code));
        let expires_at = Instant::now() + Duration::from_secs(start.expires_in);
        let task = tokio::spawn(Arc::clone(self).poll(start.device_code, start.interval, expires_at));

        let mut state = self.state.lock().await;
        if let Some(old) = state.pending.take() {
            old.task.abort();
        }
        state.error = None;
        state.pending = Some(Pending { user_code: start.user_code, link, expires_at, task });
        Ok(())
    }

    async fn poll(self: Arc<Self>, device_code: String, interval: u64, expires_at: Instant) {
        let mut wait = Duration::from_secs(interval.max(1));
        let outcome = loop {
            tokio::time::sleep(wait).await;
            if Instant::now() >= expires_at {
                break Err("The code expired. Get a new one.".to_owned());
            }
            match api::device_poll(&self.http, &self.server(), &device_code).await {
                Ok(Poll::Approved(login)) => break Ok(login),
                Ok(Poll::Pending) => {}
                Ok(Poll::SlowDown) => wait += Duration::from_secs(5),
                Ok(Poll::Denied) => break Err("The sign-in was refused.".to_owned()),
                Ok(Poll::Expired) => break Err("The code expired. Get a new one.".to_owned()),
                // A dropped connection is not the end of the code; keep trying until it expires.
                Err(err) => tracing::warn!(%err, "polling the sign-in"),
            }
        };

        let mut state = self.state.lock().await;
        state.pending = None;
        match outcome {
            Ok(login) => {
                let saved = Saved {
                    server_url: self.server(),
                    token: login.token,
                    refresh_token: login.refresh_token,
                    user: login.user,
                };
                if let Err(err) = self.write(&saved) {
                    tracing::error!(%err, "cannot save the session; it lasts until restart");
                }
                tracing::info!(user = %saved.user.email, "signed in");
                state.saved = Some(saved);
            }
            Err(message) => state.error = Some(message),
        }
    }

    pub async fn sign_out(&self) {
        // Ending the session on the server too, so it drops off the account's device list.
        // Best effort: the box signs out locally whatever the server says.
        let _ = self.send(Method::POST, "/api/v1/auth/logout", None).await;
        let mut state = self.state.lock().await;
        state.saved = None;
        if let Err(err) = std::fs::remove_file(&self.path)
            && err.kind() != std::io::ErrorKind::NotFound
        {
            tracing::warn!(%err, "cannot delete the session file");
        }
        tracing::info!("signed out");
    }

    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> anyhow::Result<T> {
        let r = self.send(Method::GET, path, None).await?;
        if !r.status().is_success() {
            bail!("server error (HTTP {})", r.status().as_u16());
        }
        Ok(r.json().await?)
    }

    /// A write (`PUT`, `POST`) with a JSON body; only success or failure matters.
    pub async fn send_json(&self, method: Method, path: &str, body: &Value) -> anyhow::Result<()> {
        let r = self.send(method, path, Some(body)).await?;
        if !r.status().is_success() {
            bail!("server error (HTTP {})", r.status().as_u16());
        }
        Ok(())
    }

    /// A write whose answer matters (the playlist just made, with its id).
    pub async fn send_json_for<T: DeserializeOwned>(&self, method: Method, path: &str, body: &Value) -> anyhow::Result<T> {
        let r = self.send(method, path, Some(body)).await?;
        if !r.status().is_success() {
            bail!("server error (HTTP {})", r.status().as_u16());
        }
        Ok(r.json().await?)
    }

    pub async fn delete(&self, path: &str) -> anyhow::Result<()> {
        let r = self.send(Method::DELETE, path, None).await?;
        if !r.status().is_success() {
            bail!("server error (HTTP {})", r.status().as_u16());
        }
        Ok(())
    }

    /// The signed-in account's id: its own playlists are the ones it may change.
    pub async fn user_id(&self) -> Option<String> {
        self.state.lock().await.saved.as_ref().map(|s| s.user.id.clone())
    }

    /// One call with the access token; on a 401, one refresh and one retry.
    async fn send(&self, method: Method, path: &str, body: Option<&Value>) -> anyhow::Result<reqwest::Response> {
        self.send_timed(method, path, body, None).await
    }

    /// `send` with its own time limit, for the few calls that take minutes (the server
    /// fetching a podcast episode from its feed).
    async fn send_timed(
        &self,
        method: Method,
        path: &str,
        body: Option<&Value>,
        timeout: Option<Duration>,
    ) -> anyhow::Result<reqwest::Response> {
        let token = self.token().await.context("not signed in")?;
        let r = self.request(method.clone(), path, body, &token, timeout).await?;
        if r.status() != StatusCode::UNAUTHORIZED {
            return Ok(r);
        }
        let token = self.refresh(&token).await?;
        self.request(method, path, body, &token, timeout).await
    }

    async fn request(
        &self,
        method: Method,
        path: &str,
        body: Option<&Value>,
        token: &str,
        timeout: Option<Duration>,
    ) -> anyhow::Result<reqwest::Response> {
        let mut req = self.http.request(method, api::url(&self.server(), path)).bearer_auth(token);
        if let Some(body) = body {
            req = req.json(body);
        }
        if let Some(t) = timeout {
            req = req.timeout(t);
        }
        let sent = req.send().await;
        self.reachable.store(sent.is_ok(), Ordering::Relaxed);
        sent.context("cannot reach the server")
    }

    /// A GET whose status matters beyond success, with the body as JSON (or null).
    pub async fn get_with_status(&self, path: &str) -> anyhow::Result<(StatusCode, Value)> {
        let r = self.send(Method::GET, path, None).await?;
        let status = r.status();
        Ok((status, r.json().await.unwrap_or(Value::Null)))
    }

    /// A POST without a body that may take up to `timeout`.
    pub async fn post_slow(&self, path: &str, timeout: Duration) -> anyhow::Result<()> {
        let r = self.send_timed(Method::POST, path, None, Some(timeout)).await?;
        if !r.status().is_success() {
            bail!("server error (HTTP {})", r.status().as_u16());
        }
        Ok(())
    }

    async fn token(&self) -> Option<String> {
        self.state.lock().await.saved.as_ref().map(|s| s.token.clone())
    }

    async fn refresh(&self, stale: &str) -> anyhow::Result<String> {
        let _one_at_a_time = self.refreshing.lock().await;
        let saved = self.state.lock().await.saved.clone().context("not signed in")?;
        if saved.token != stale {
            return Ok(saved.token); // someone else refreshed while we waited
        }
        match api::refresh(&self.http, &self.server(), &saved.refresh_token).await? {
            Some(tokens) => {
                let renewed = Saved { token: tokens.token, refresh_token: tokens.refresh_token, ..saved };
                if let Err(err) = self.write(&renewed) {
                    tracing::error!(%err, "cannot save the renewed session");
                }
                let token = renewed.token.clone();
                self.state.lock().await.saved = Some(renewed);
                Ok(token)
            }
            None => {
                let mut state = self.state.lock().await;
                state.saved = None;
                state.error = Some("The session ended. Sign in again.".to_owned());
                let _ = std::fs::remove_file(&self.path);
                bail!("signed out by the server")
            }
        }
    }

    /// Written to a temporary file and renamed, readable by the owner only: it holds tokens.
    fn write(&self, saved: &Saved) -> anyhow::Result<()> {
        let dir = self.path.parent().context("session path has no folder")?;
        std::fs::create_dir_all(dir)?;
        let tmp = self.path.with_extension("tmp");
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&tmp)?;
        file.write_all(&serde_json::to_vec_pretty(saved)?)?;
        file.sync_all()?;
        std::fs::rename(&tmp, &self.path)?;
        Ok(())
    }
}
