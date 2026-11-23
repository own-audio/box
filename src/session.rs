//! Signing people in with a code, keeping their sessions on disk, and making signed-in calls.
//!
//! A household can sign several people in on one box; one of them is listening at a time
//! (the active account), and the others wait, their sessions kept alive, to be switched to.

use std::{
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::PathBuf,
    sync::Arc,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

use anyhow::{Context, bail};
use reqwest::{Client, Method, StatusCode};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use tokio::{sync::Mutex, task::JoinHandle};

use crate::api::{self, Poll, User};

/// A sign-in that waits this long unused is refreshed, so its refresh token (90 days on the
/// server) never runs out while its owner is away.
const KEEP_ALIVE_AFTER: i64 = 7 * 24 * 3600;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Saved {
    server_url: String,
    token: String,
    refresh_token: String,
    user: User,
    /// When the tokens were last issued, unix seconds (0: before this was kept).
    #[serde(default)]
    renewed_at: i64,
}

/// `session.json`: the active account's session at the top level, as a box with one
/// account has always written it (so an older version, after a rollback, still finds it
/// signed in), and everyone else beside it.
#[derive(Serialize, Deserialize)]
struct OnDisk {
    #[serde(flatten)]
    active: Saved,
    #[serde(default)]
    others: Vec<Saved>,
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
    accounts: Vec<Saved>,
    /// The user id of the account listening now; `None` only with no accounts.
    active: Option<String>,
    pending: Option<Pending>,
    /// The last thing that went wrong, shown on the sign-in screen until the next attempt.
    error: Option<String>,
}

impl State {
    fn active(&self) -> Option<&Saved> {
        let id = self.active.as_deref()?;
        self.accounts.iter().find(|a| a.user.id == id)
    }

    fn code(&self) -> Option<Code> {
        self.pending.as_ref().map(|p| Code {
            user_code: p.user_code.clone(),
            link: p.link.clone(),
            expires_in: p.expires_at.saturating_duration_since(Instant::now()).as_secs(),
        })
    }
}

/// A sign-in code on screen.
#[derive(Debug, Serialize)]
pub struct Code {
    pub user_code: String,
    pub link: Option<String>,
    pub expires_in: u64,
}

/// What the UI is told.
#[derive(Debug, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum View {
    SignedOut {
        error: Option<String>,
    },
    Pending {
        #[serde(flatten)]
        code: Code,
    },
    SignedIn {
        user: User,
        server_reachable: bool,
        /// Everyone signed in on the box, the active one included, in the order added.
        accounts: Vec<User>,
        /// Another person being signed in while this one listens.
        adding: Option<Code>,
        /// Why the last added sign-in did not happen.
        error: Option<String>,
    },
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
            Ok(text) => match serde_json::from_str::<OnDisk>(&text) {
                Ok(disk) => {
                    let before = 1 + disk.others.len();
                    state.active = Some(disk.active.user.id.clone());
                    state.accounts = std::iter::once(disk.active)
                        .chain(disk.others)
                        .filter(|a| a.server_url == server_url)
                        .collect();
                    if state.accounts.len() < before {
                        tracing::warn!("sessions for another server dropped; those people sign in again");
                    }
                    if state.active().is_none() {
                        state.active = state.accounts.first().map(|a| a.user.id.clone());
                    }
                }
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

    /// Points the box at another server. The sessions belong to the old one, so they end
    /// (locally; the old server may be out of reach) along with any code being shown.
    pub async fn set_server(&self, url: String) {
        let mut state = self.state.lock().await;
        if let Some(p) = state.pending.take() {
            p.task.abort();
        }
        state.accounts.clear();
        state.active = None;
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
        if let Some(active) = state.active() {
            return View::SignedIn {
                user: active.user.clone(),
                server_reachable: self.reachable.load(Ordering::Relaxed),
                accounts: state.accounts.iter().map(|a| a.user.clone()).collect(),
                adding: state.code(),
                error: state.error.clone(),
            };
        }
        if let Some(code) = state.code() {
            return View::Pending { code };
        }
        View::SignedOut { error: state.error.clone() }
    }

    /// The QR code's target while a sign-in is waiting.
    pub async fn pending_link(&self) -> Option<String> {
        self.state.lock().await.pending.as_ref().and_then(|p| p.link.clone())
    }

    /// Asks the server for a code and polls in the background until it is approved,
    /// refused or expired. A second call replaces the first code. With someone signed in
    /// already, the approved account joins the others; it does not take over.
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

    /// Leaving the page that shows the code: nobody is waiting for it any more.
    pub async fn cancel_sign_in(&self) {
        let mut state = self.state.lock().await;
        if let Some(p) = state.pending.take() {
            p.task.abort();
        }
        state.error = None;
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
                    renewed_at: chrono::Utc::now().timestamp(),
                };
                tracing::info!(user = %saved.user.email, "signed in");
                // The same person again (a new code for an account already here) replaces
                // their old session in place.
                match state.accounts.iter_mut().find(|a| a.user.id == saved.user.id) {
                    Some(old) => *old = saved.clone(),
                    None => state.accounts.push(saved.clone()),
                }
                if state.active().is_none() {
                    state.active = Some(saved.user.id);
                }
                self.write(&state);
            }
            Err(message) => state.error = Some(message),
        }
    }

    /// Who listens from now on. The player follows on its next look (`Player::follow_account`).
    pub async fn switch(&self, user_id: &str) -> anyhow::Result<()> {
        let mut state = self.state.lock().await;
        anyhow::ensure!(state.accounts.iter().any(|a| a.user.id == user_id), "nobody by that id is signed in here");
        state.active = Some(user_id.to_owned());
        state.error = None;
        self.write(&state);
        tracing::info!(user = user_id, "switched the listener");
        Ok(())
    }

    /// Signs the listening person out; the next one signed in (if any) listens instead.
    pub async fn sign_out(&self) {
        let Some(id) = self.active_id().await else { return };
        self.forget(&id).await;
    }

    /// Signs one person out of the box. Their session ends on the server too, so the box
    /// drops off their account's device list; best effort, the box forgets it either way.
    pub async fn forget(&self, user_id: &str) {
        let _ = self.send_timed(Method::POST, "/api/v1/auth/logout", None, None, Some(user_id)).await;
        let mut state = self.state.lock().await;
        state.accounts.retain(|a| a.user.id != user_id);
        if state.active.as_deref() == Some(user_id) {
            state.active = state.accounts.first().map(|a| a.user.id.clone());
        }
        self.write(&state);
        tracing::info!(user = user_id, "signed out");
    }

    /// Everyone out ("Sign out and start over").
    pub async fn sign_out_all(&self) {
        let ids: Vec<String> = self.state.lock().await.accounts.iter().map(|a| a.user.id.clone()).collect();
        for id in ids {
            self.forget(&id).await;
        }
    }

    /// The listening account's id: its own playlists are the ones it may change, and its
    /// queue and places are the player's.
    pub async fn user_id(&self) -> Option<String> {
        self.active_id().await
    }

    pub async fn active_id(&self) -> Option<String> {
        self.state.lock().await.active().map(|a| a.user.id.clone())
    }

    /// Refreshes the sessions nobody has used for a week, so a person who has been away
    /// for months is still signed in when they come back.
    pub async fn keep_others_alive(self: Arc<Self>) {
        loop {
            tokio::time::sleep(Duration::from_secs(6 * 3600)).await;
            let now = chrono::Utc::now().timestamp();
            let stale: Vec<(String, String)> = {
                let state = self.state.lock().await;
                state
                    .accounts
                    .iter()
                    .filter(|a| Some(&a.user.id) != state.active.as_ref() && now - a.renewed_at > KEEP_ALIVE_AFTER)
                    .map(|a| (a.user.id.clone(), a.token.clone()))
                    .collect()
            };
            for (id, token) in stale {
                if let Err(err) = self.refresh(&id, &token).await {
                    tracing::warn!(%err, user = id, "keeping a waiting sign-in alive");
                }
            }
        }
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
        self.send_json_as(None, method, path, body).await
    }

    /// `send_json` for one person, whoever listens by the time it is sent: a book's place
    /// saved just before a switch belongs to the person who was listening to it.
    pub async fn send_json_as(&self, user_id: Option<&str>, method: Method, path: &str, body: &Value) -> anyhow::Result<()> {
        let r = self.send_timed(method, path, Some(body), None, user_id).await?;
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

    /// One call with the access token; on a 401, one refresh and one retry.
    async fn send(&self, method: Method, path: &str, body: Option<&Value>) -> anyhow::Result<reqwest::Response> {
        self.send_timed(method, path, body, None, None).await
    }

    /// `send` with its own time limit, for the few calls that take minutes (the server
    /// fetching a podcast episode from its feed), as one person (`None`: who listens now).
    async fn send_timed(
        &self,
        method: Method,
        path: &str,
        body: Option<&Value>,
        timeout: Option<Duration>,
        user_id: Option<&str>,
    ) -> anyhow::Result<reqwest::Response> {
        let (id, token) = self.token(user_id).await.context("not signed in")?;
        let r = self.request(method.clone(), path, body, &token, timeout).await?;
        if r.status() != StatusCode::UNAUTHORIZED {
            return Ok(r);
        }
        let token = self.refresh(&id, &token).await?;
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
        let r = self.send_timed(Method::POST, path, None, Some(timeout), None).await?;
        if !r.status().is_success() {
            bail!("server error (HTTP {})", r.status().as_u16());
        }
        Ok(())
    }

    /// The account to call as (`None`: the active one) and its access token.
    async fn token(&self, user_id: Option<&str>) -> Option<(String, String)> {
        let state = self.state.lock().await;
        let account = match user_id {
            Some(id) => state.accounts.iter().find(|a| a.user.id == id),
            None => state.active(),
        }?;
        Some((account.user.id.clone(), account.token.clone()))
    }

    async fn refresh(&self, user_id: &str, stale: &str) -> anyhow::Result<String> {
        let _one_at_a_time = self.refreshing.lock().await;
        let saved = {
            let state = self.state.lock().await;
            state.accounts.iter().find(|a| a.user.id == user_id).cloned().context("not signed in")?
        };
        if saved.token != stale {
            return Ok(saved.token); // someone else refreshed while we waited
        }
        let renewed = api::refresh(&self.http, &self.server(), &saved.refresh_token).await?;
        let mut state = self.state.lock().await;
        match renewed {
            Some(tokens) => {
                let token = tokens.token.clone();
                if let Some(a) = state.accounts.iter_mut().find(|a| a.user.id == user_id) {
                    a.token = tokens.token;
                    a.refresh_token = tokens.refresh_token;
                    a.renewed_at = chrono::Utc::now().timestamp();
                }
                self.write(&state);
                Ok(token)
            }
            None => {
                state.accounts.retain(|a| a.user.id != user_id);
                if state.active.as_deref() == Some(user_id) {
                    state.active = state.accounts.first().map(|a| a.user.id.clone());
                    state.error = Some("The session ended. Sign in again.".to_owned());
                }
                self.write(&state);
                tracing::warn!(user = user_id, "the server ended a session");
                bail!("signed out by the server")
            }
        }
    }

    /// Everyone's sessions, or no file with nobody signed in.
    fn write(&self, state: &State) {
        let Some(active) = state.active().cloned() else {
            if let Err(err) = std::fs::remove_file(&self.path)
                && err.kind() != std::io::ErrorKind::NotFound
            {
                tracing::warn!(%err, "cannot delete the session file");
            }
            return;
        };
        let others = state.accounts.iter().filter(|a| a.user.id != active.user.id).cloned().collect();
        if let Err(err) = self.write_file(&OnDisk { active, others }) {
            tracing::error!(%err, "cannot save the sessions; they last until restart");
        }
    }

    /// Written to a temporary file and renamed, readable by the owner only: it holds tokens.
    fn write_file(&self, disk: &OnDisk) -> anyhow::Result<()> {
        let dir = self.path.parent().context("session path has no folder")?;
        std::fs::create_dir_all(dir)?;
        let tmp = self.path.with_extension("tmp");
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&tmp)?;
        file.write_all(&serde_json::to_vec_pretty(disk)?)?;
        file.sync_all()?;
        std::fs::rename(&tmp, &self.path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn saved(id: &str) -> Saved {
        Saved {
            server_url: "https://x".into(),
            token: format!("t-{id}"),
            refresh_token: format!("r-{id}"),
            user: User { id: id.into(), email: format!("{id}@x"), display_name: String::new() },
            renewed_at: 0,
        }
    }

    #[test]
    fn a_one_account_file_from_before_reads_as_the_active_account() {
        let old = serde_json::to_string(&saved("a")).unwrap();
        let disk: OnDisk = serde_json::from_str(&old).unwrap();
        assert_eq!(disk.active.user.id, "a");
        assert!(disk.others.is_empty());
    }

    #[test]
    fn an_older_version_still_finds_the_active_account() {
        // What an older ownbox reads after a rollback: the top-level session, nothing else.
        #[derive(Deserialize)]
        struct OldSaved {
            token: String,
            user: User,
        }
        let text = serde_json::to_string(&OnDisk { active: saved("a"), others: vec![saved("b")] }).unwrap();
        let old: OldSaved = serde_json::from_str(&text).unwrap();
        assert_eq!(old.user.id, "a");
        assert_eq!(old.token, "t-a");
    }
}
