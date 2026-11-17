use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{StatusCode, header},
    response::IntoResponse,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{AppState, books, eq, hardware, hotspot, update, library, outside, player, podcasts, power, remote, setup, wifi};

// The UI is built into the binary: one file to deploy, nothing to get out of step.
const INDEX_HTML: &str = include_str!("../ui/index.html");
const APP_JS: &str = include_str!("../ui/app.js");
const STYLE_CSS: &str = include_str!("../ui/style.css");
const I18N_JS: &str = include_str!("../ui/i18n.js");

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(|| async { asset("text/html; charset=utf-8", INDEX_HTML) }))
        .route("/app.js", get(|| async { asset("text/javascript; charset=utf-8", APP_JS) }))
        .route("/style.css", get(|| async { asset("text/css; charset=utf-8", STYLE_CSS) }))
        .route("/i18n.js", get(|| async { asset("text/javascript; charset=utf-8", I18N_JS) }))
        .route("/api/status", get(status))
        .route("/api/test-tone", post(test_tone))
        .route("/api/session", get(session))
        .route("/api/sign-in", post(sign_in))
        .route("/api/sign-in/qr.svg", get(sign_in_qr))
        .route("/api/sign-out", post(sign_out))
        .route("/api/library/albums", get(lib_albums))
        .route("/api/library/albums/{id}", get(lib_album))
        .route("/api/library/artists", get(lib_artists))
        .route("/api/library/artists/{id}/albums", get(lib_artist_albums))
        .route("/api/library/playlists", get(lib_playlists).post(lib_playlist_create))
        .route("/api/library/playlists/{id}", get(lib_playlist).delete(lib_playlist_delete))
        .route("/api/library/playlists/{id}/add", post(lib_playlist_add))
        .route("/api/library/smart-playlists/{id}", get(lib_smart_playlist))
        .route("/api/library/smart-presets", get(lib_smart_presets))
        .route("/api/library/recent-albums", get(lib_recent_albums))
        .route("/api/library/smart-resolve", post(lib_smart_resolve))
        .route("/api/library/genres", get(lib_genres))
        .route("/api/library/playlists/{id}/remove", post(lib_playlist_remove))
        .route("/api/library/search", get(lib_search))
        .route("/api/library/favourites", get(lib_favourites))
        .route("/api/player/queue", get(player_queue))
        .route("/api/player/jump", post(player_jump))
        .route("/api/player/enqueue", post(player_enqueue))
        .route("/api/player/volume", post(player_volume))
        .route("/api/player/skip", post(player_skip))
        .route("/api/player/speed", post(player_speed))
        .route("/api/player/sleep", post(player_sleep))
        .route("/api/books", get(books_list))
        .route("/api/books/in-progress", get(books_in_progress))
        .route("/api/books/authors", get(books_authors))
        .route("/api/books/authors/{id}", get(books_by_author))
        .route("/api/books/search", get(books_search))
        .route("/api/books/{id}", get(books_detail))
        .route("/api/podcasts", get(podcasts_shows))
        .route("/api/podcasts/in-progress", get(podcasts_in_progress))
        .route("/api/podcasts/latest", get(podcasts_latest))
        .route("/api/podcasts/mark", post(podcasts_mark))
        .route("/api/podcasts/{id}", get(podcasts_show))
        .route("/api/ui", get(ui_settings).post(ui_set))
        .route("/api/remote/status", get(remote_status))
        .route("/api/remote/pair", post(remote_pair))
        .route("/api/remote/code", post(remote_code))
        .route("/api/remote/qr.svg", get(remote_qr))
        .route("/api/remote/devices", get(remote_devices))
        .route("/api/remote/forget", post(remote_forget))
        .route("/api/eq", get(eq_state).post(eq_set))
        .route("/api/player/release", post(player_release))
        .route("/api/outside", get(outside_state))
        .route("/api/outside/cover", get(outside_cover))
        .route("/api/outside/take-back", post(outside_take_back))
        .route("/api/outside/sources", get(sources_state).post(sources_set))
        .route("/api/update", get(update_state))
        .route("/api/update/check", post(update_check))
        .route("/api/update/install", post(update_install))
        .route("/api/update/rollback", post(update_rollback))
        .route("/api/audio", get(audio_state))
        .route("/api/audio/output", post(audio_set))
        .route("/api/audio/hats", get(hats_state))
        .route("/api/audio/hat", post(hat_set))
        .route("/api/hotspot", get(hotspot_state))
        .route("/api/hotspot/apply", post(hotspot_apply))
        .route("/api/outside/{kind}/{event}", post(outside_event))
        .route("/api/setup", get(setup_state))
        .route("/api/setup/check", post(setup_check))
        .route("/api/setup/server", post(setup_server))
        .route("/api/setup/name", post(setup_name))
        .route("/api/setup/done", post(setup_done))
        .route("/api/wifi", get(wifi_status))
        .route("/api/wifi/radio", post(wifi_radio))
        .route("/api/wifi/scan", get(wifi_scan))
        .route("/api/wifi/connect", post(wifi_connect))
        .route("/api/wifi/forget", post(wifi_forget))
        .route("/api/player", get(player_now))
        .route("/api/player/play", post(player_play))
        .route("/api/player/toggle", post(player_toggle))
        .route("/api/player/pause", post(player_pause))
        .route("/api/player/next", post(player_next))
        .route("/api/player/previous", post(player_previous))
        .route("/api/player/seek", post(player_seek))
        .route("/api/screen", get(screen_state).post(screen_set))
        .route("/api/screen/settings", post(screen_settings))
        .route("/api/screen/touch", post(screen_touch))
        // Before the guard, so it sees unknown paths too: on the setup Wi-Fi a phone's
        // check for a sign-in page (/hotspot-detect.html, /generate_204) is sent to setup.
        .fallback(|| async { StatusCode::NOT_FOUND })
        .layer(axum::middleware::from_fn_with_state(state.clone(), guard))
        .with_state(state)
}

async fn screen_state(State(s): State<Arc<AppState>>) -> Json<crate::screen::ScreenState> {
    Json(s.screen.state())
}

#[derive(Deserialize)]
struct ScreenRequest {
    on: bool,
}

async fn screen_set(State(s): State<Arc<AppState>>, Json(req): Json<ScreenRequest>) -> Json<crate::screen::ScreenState> {
    s.screen.set(req.on);
    Json(s.screen.state())
}

/// The page says it is being used (or must stay lit, as for a sign-in code).
async fn screen_touch(State(s): State<Arc<AppState>>) -> Json<crate::screen::ScreenState> {
    s.screen.touch();
    Json(s.screen.state())
}

async fn screen_settings(
    State(s): State<Arc<AppState>>,
    Json(req): Json<crate::screen::Settings>,
) -> Json<crate::screen::ScreenState> {
    s.screen.configure(req);
    Json(s.screen.state())
}

/// The server's answer, or its failure as `{error}`: 401 when the box is not signed in, so
/// the screen can go back to sign-in; 502 for everything else (server, network).
fn answer<T: Serialize>(r: anyhow::Result<T>) -> axum::response::Response {
    match r {
        Ok(v) => Json(v).into_response(),
        Err(e) => {
            let status = if e.to_string().contains("not signed in") || e.to_string().contains("signed out") {
                StatusCode::UNAUTHORIZED
            } else {
                StatusCode::BAD_GATEWAY
            };
            (status, Json(json!({ "error": format!("{e:#}") }))).into_response()
        }
    }
}

#[derive(Deserialize)]
struct PageQuery {
    sort: Option<String>,
    cursor: Option<String>,
}

#[derive(Deserialize)]
struct SearchQuery {
    q: String,
}

async fn lib_albums(State(s): State<Arc<AppState>>, Query(q): Query<PageQuery>) -> axum::response::Response {
    answer(library::albums(&s.auth, q.sort.as_deref().unwrap_or("added"), q.cursor.as_deref()).await)
}

async fn lib_album(State(s): State<Arc<AppState>>, Path(id): Path<String>) -> axum::response::Response {
    answer(library::album_detail(&s.auth, &id).await)
}

async fn lib_artists(State(s): State<Arc<AppState>>, Query(q): Query<PageQuery>) -> axum::response::Response {
    answer(library::artists(&s.auth, q.cursor.as_deref()).await)
}

async fn lib_artist_albums(State(s): State<Arc<AppState>>, Path(id): Path<String>) -> axum::response::Response {
    answer(library::artist_albums(&s.auth, &id).await)
}

async fn lib_playlists(State(s): State<Arc<AppState>>) -> axum::response::Response {
    let played = s.prefs.get().playlists_played;
    answer(library::playlists(&s.auth).await.map(|mut lists| {
        for p in &mut lists {
            p.played_at = played.get(&p.id).copied();
        }
        lists
    }))
}

async fn lib_recent_albums(State(s): State<Arc<AppState>>) -> axum::response::Response {
    answer(library::recent_albums(&s.auth).await)
}

async fn lib_playlist(State(s): State<Arc<AppState>>, Path(id): Path<String>) -> axum::response::Response {
    answer(library::playlist_tracks(&s.auth, &id).await)
}

async fn lib_smart_presets(State(s): State<Arc<AppState>>) -> axum::response::Response {
    answer(library::smart_presets(&s.auth).await)
}

#[derive(Deserialize)]
struct ResolveRequest {
    rule: serde_json::Value,
}

async fn lib_smart_resolve(State(s): State<Arc<AppState>>, Json(req): Json<ResolveRequest>) -> axum::response::Response {
    answer(library::resolve_rule(&s.auth, &req.rule).await)
}

async fn lib_genres(State(s): State<Arc<AppState>>) -> axum::response::Response {
    answer(library::genres(&s.auth).await)
}

async fn lib_smart_playlist(State(s): State<Arc<AppState>>, Path(id): Path<String>) -> axum::response::Response {
    answer(library::smart_tracks(&s.auth, &id).await)
}

#[derive(Deserialize)]
struct NewPlaylist {
    name: String,
    #[serde(default)]
    track_ids: Vec<String>,
}

async fn lib_playlist_create(State(s): State<Arc<AppState>>, Json(req): Json<NewPlaylist>) -> axum::response::Response {
    answer(library::create_playlist(&s.auth, &req.name, &req.track_ids).await)
}

#[derive(Deserialize)]
struct AddTracks {
    track_ids: Vec<String>,
}

async fn lib_playlist_add(State(s): State<Arc<AppState>>, Path(id): Path<String>, Json(req): Json<AddTracks>) -> axum::response::Response {
    answer(library::add_to_playlist(&s.auth, &id, &req.track_ids).await)
}

#[derive(Deserialize)]
struct RemoveTrack {
    index: usize,
    track_id: String,
}

async fn lib_playlist_remove(State(s): State<Arc<AppState>>, Path(id): Path<String>, Json(req): Json<RemoveTrack>) -> axum::response::Response {
    answer(library::remove_from_playlist(&s.auth, &id, req.index, &req.track_id).await.map(|()| json!({ "ok": true })))
}

async fn lib_playlist_delete(State(s): State<Arc<AppState>>, Path(id): Path<String>) -> axum::response::Response {
    answer(library::delete_playlist(&s.auth, &id).await.map(|()| json!({ "ok": true })))
}

async fn lib_search(State(s): State<Arc<AppState>>, Query(q): Query<SearchQuery>) -> axum::response::Response {
    answer(library::search(&s.auth, q.q.trim()).await)
}

async fn lib_favourites(State(s): State<Arc<AppState>>) -> axum::response::Response {
    answer(library::favourites(&s.auth).await)
}

async fn player_queue(State(s): State<Arc<AppState>>) -> Json<player::QueueView> {
    Json(s.player.queue().await)
}

#[derive(Deserialize)]
struct JumpRequest {
    index: usize,
    /// Where in that entry: a chapter starts mid-file.
    #[serde(default)]
    secs: f64,
}

async fn player_jump(State(s): State<Arc<AppState>>, Json(req): Json<JumpRequest>) -> axum::response::Response {
    own_turn(&s).await;
    answer(s.player.jump(req.index, req.secs).await.map(|()| json!({ "ok": true })))
}

#[derive(Deserialize)]
struct SkipRequest {
    secs: f64,
}

async fn player_skip(State(s): State<Arc<AppState>>, Json(req): Json<SkipRequest>) -> axum::response::Response {
    answer(s.player.skip(req.secs).await.map(|()| json!({ "ok": true })))
}

#[derive(Deserialize)]
struct SpeedRequest {
    speed: f64,
}

async fn player_speed(State(s): State<Arc<AppState>>, Json(req): Json<SpeedRequest>) -> axum::response::Response {
    answer(s.player.set_speed(req.speed).await.map(|()| json!({ "ok": true })))
}

async fn player_sleep(State(s): State<Arc<AppState>>, Json(req): Json<player::SleepRequest>) -> Json<serde_json::Value> {
    s.player.set_sleep(req).await;
    Json(json!({ "ok": true }))
}

async fn books_list(State(s): State<Arc<AppState>>, Query(q): Query<PageQuery>) -> axum::response::Response {
    answer(books::books(&s.auth, q.sort.as_deref().unwrap_or("added"), q.cursor.as_deref()).await)
}

async fn books_in_progress(State(s): State<Arc<AppState>>) -> axum::response::Response {
    answer(books::in_progress(&s.auth).await)
}

async fn books_authors(State(s): State<Arc<AppState>>, Query(q): Query<PageQuery>) -> axum::response::Response {
    answer(books::authors(&s.auth, q.cursor.as_deref()).await)
}

async fn books_by_author(State(s): State<Arc<AppState>>, Path(id): Path<String>) -> axum::response::Response {
    answer(books::author_books(&s.auth, &id).await)
}

async fn books_search(State(s): State<Arc<AppState>>, Query(q): Query<SearchQuery>) -> axum::response::Response {
    answer(books::search(&s.auth, q.q.trim()).await)
}

// --- phone control ----------------------------------------------------------------------

/// Who may reach what: the box itself everything, a paired phone the player and library,
/// anyone else the page and the pairing call (see `remote::access`).
async fn guard(
    State(s): State<Arc<AppState>>,
    axum::extract::ConnectInfo(peer): axum::extract::ConnectInfo<std::net::SocketAddr>,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let token = req.headers().get(header::COOKIE).and_then(|v| v.to_str().ok()).and_then(remote::token_from_cookies);
    let paired = s.prefs.get().remote_devices;
    let setup = s.hotspot.serves(peer.ip());
    if setup {
        s.hotspot.touch();
        // On the setup Wi-Fi every name leads to the box; a phone checking for a sign-in page
        // (captive.apple.com, connectivitycheck…) is sent to the setup page, so it opens it.
        let host = req.headers().get(header::HOST).and_then(|v| v.to_str().ok()).unwrap_or("");
        let host = host.rsplit_once(':').map_or(host, |(h, _)| h);
        if host != hotspot::ADDRESS.to_string() {
            return axum::response::Redirect::to(&format!("http://{}/", hotspot::ADDRESS)).into_response();
        }
    }
    if !setup {
        let host = req.headers().get(header::HOST).and_then(|v| v.to_str().ok()).unwrap_or("");
        if !remote::host_allowed(host) {
            return (StatusCode::MISDIRECTED_REQUEST, Json(json!({ "error": "not this box's name" }))).into_response();
        }
    }
    match remote::access(peer.ip(), req.uri().path(), token, &paired, s.config.headless, setup) {
        remote::Access::Allowed => next.run(req).await,
        remote::Access::NeedsPairing => (StatusCode::UNAUTHORIZED, Json(json!({ "error": "pair this phone on the box first" }))).into_response(),
        remote::Access::ScreenOnly => (StatusCode::FORBIDDEN, Json(json!({ "error": "only on the box's own screen" }))).into_response(),
    }
}

/// Whether this browser is the box's screen or a phone, and whether a phone is paired.
async fn remote_status(
    State(s): State<Arc<AppState>>,
    axum::extract::ConnectInfo(peer): axum::extract::ConnectInfo<std::net::SocketAddr>,
    headers: axum::http::HeaderMap,
) -> Json<serde_json::Value> {
    let token = headers.get(header::COOKIE).and_then(|v| v.to_str().ok()).and_then(remote::token_from_cookies);
    let devices = s.prefs.get().remote_devices;
    let paired = token.is_some_and(|t| devices.iter().any(|p| p.token_hash == remote::hash(t)));
    let headless = s.config.headless;
    Json(json!({
        // Without a screen, a paired phone is the screen.
        "screen": remote::is_box(peer.ip()) || (headless && paired),
        "paired": paired,
        "headless": headless,
        // The first phone pairs without a code (see `Remote::pair_first`).
        "first": headless && devices.is_empty(),
        // On the box's own setup Wi-Fi: the setup page, nothing else.
        "setup_wifi": s.hotspot.serves(peer.ip()),
    }))
}

#[derive(Deserialize)]
struct PairRequest {
    #[serde(default)]
    code: String,
    #[serde(default)]
    first: bool,
    #[serde(default)]
    name: String,
}

async fn remote_pair(State(s): State<Arc<AppState>>, Json(req): Json<PairRequest>) -> axum::response::Response {
    let name = if req.name.trim().is_empty() { "Phone".to_owned() } else { req.name };
    let token = if req.first && s.config.headless { s.remote.pair_first(&name, &s.prefs) } else { s.remote.pair(&req.code, &name, &s.prefs) };
    match token {
        Some(token) => {
            // HttpOnly: page scripts never see it; SameSite=Strict: other sites cannot use it.
            let cookie = format!("{}={token}; Path=/; Max-Age=31536000; HttpOnly; SameSite=Strict", remote::COOKIE);
            ([(header::SET_COOKIE, cookie)], Json(json!({ "ok": true }))).into_response()
        }
        None => (StatusCode::BAD_REQUEST, Json(json!({ "error": "that code is not valid any more" }))).into_response(),
    }
}

/// The address phones use: this box's LAN address (the source address a packet out would
/// take; nothing is sent) and the port it listens on.
fn box_url(s: &AppState) -> String {
    let port = s.config.listen.rsplit(':').next().unwrap_or("8686");
    let ip = std::net::UdpSocket::bind("0.0.0.0:0")
        .and_then(|sock| sock.connect("192.0.2.1:9").map(|()| sock))
        .and_then(|sock| sock.local_addr())
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|_| "ownbox.local".into());
    format!("http://{ip}:{port}")
}

async fn remote_code(State(s): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let (code, expires_in) = s.remote.new_code();
    let url = format!("{}/?pair={code}", box_url(&s));
    Json(json!({ "code": code, "url": url, "expires_in": expires_in }))
}

#[derive(Deserialize)]
struct QrQuery {
    url: String,
}

async fn remote_qr(Query(q): Query<QrQuery>) -> axum::response::Response {
    match qrcode::QrCode::new(q.url.as_bytes()) {
        Ok(code) => {
            let svg = code.render::<qrcode::render::svg::Color>().min_dimensions(220, 220).quiet_zone(true).build();
            ([(header::CONTENT_TYPE, "image/svg+xml"), (header::CACHE_CONTROL, "no-store")], svg).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn remote_devices(State(s): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let list: Vec<_> = s
        .prefs
        .get()
        .remote_devices
        .into_iter()
        .map(|p| json!({ "id": p.id, "name": p.name, "paired_at": p.paired_at }))
        .collect();
    Json(json!(list))
}

#[derive(Deserialize)]
struct ForgetDeviceRequest {
    id: String,
}

async fn remote_forget(State(s): State<Arc<AppState>>, Json(req): Json<ForgetDeviceRequest>) -> Json<serde_json::Value> {
    s.prefs.update(|p| p.remote_devices.retain(|d| d.id != req.id));
    Json(json!({ "ok": true }))
}

// --- outside sources --------------------------------------------------------------------

/// The box's own player is about to play: an outside source holding the card ends first.
pub(crate) async fn own_turn(s: &AppState) {
    if s.outside.view().is_some() {
        s.outside.take_back().await;
    }
}

async fn player_release(State(s): State<Arc<AppState>>) -> Json<serde_json::Value> {
    s.player.release().await;
    Json(json!({ "ok": true }))
}

/// From the sources' hooks: `start` (it is about to play: the card is let go) or `stop`.
async fn outside_event(State(s): State<Arc<AppState>>, Path((kind, event)): Path<(String, String)>) -> axum::response::Response {
    let Some(kind) = outside::Kind::parse(&kind) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match event.as_str() {
        "start" => {
            s.player.release().await;
            s.outside.started(kind);
        }
        "stop" => s.outside.stopped(kind),
        _ => return StatusCode::NOT_FOUND.into_response(),
    }
    Json(json!({ "ok": true })).into_response()
}

async fn outside_state(State(s): State<Arc<AppState>>) -> Json<Option<outside::View>> {
    Json(s.outside.view())
}

async fn outside_cover(State(s): State<Arc<AppState>>) -> axum::response::Response {
    match s.outside.cover() {
        Some((bytes, kind)) => ([(header::CONTENT_TYPE, kind), (header::CACHE_CONTROL, "no-store")], bytes).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn outside_take_back(State(s): State<Arc<AppState>>) -> Json<serde_json::Value> {
    s.outside.take_back().await;
    Json(json!({ "ok": true }))
}

/// Whether a source's service is installed (shairport-sync is built by hand, so a box
/// may not have it).
fn unit_installed(unit: &str) -> bool {
    [
        "/etc/systemd/system",
        "/lib/systemd/system",
        "/usr/lib/systemd/system",
        "/usr/local/lib/systemd/system",
        // Debian's squeezelite is an init.d script; systemd makes its unit at boot.
        "/run/systemd/generator.late",
    ]
        .iter()
        .any(|dir| std::path::Path::new(dir).join(unit).exists())
}

fn sources_json(s: &AppState) -> serde_json::Value {
    let p = s.prefs.get();
    json!({
        "airplay": { "installed": unit_installed("shairport-sync.service"), "enabled": p.airplay_enabled.unwrap_or(true) },
        "music_assistant": { "installed": unit_installed("squeezelite.service"), "enabled": p.music_assistant_enabled.unwrap_or(true) },
    })
}

async fn sources_state(State(s): State<Arc<AppState>>) -> Json<serde_json::Value> {
    Json(sources_json(&s))
}

#[derive(Deserialize)]
struct SourcesRequest {
    airplay: Option<bool>,
    music_assistant: Option<bool>,
}

/// Saved in `prefs.json`; `ownbox-sources.path` sees the change and switches the services.
async fn sources_set(State(s): State<Arc<AppState>>, Json(req): Json<SourcesRequest>) -> Json<serde_json::Value> {
    s.prefs.update(|p| {
        if let Some(on) = req.airplay {
            p.airplay_enabled = Some(on);
        }
        if let Some(on) = req.music_assistant {
            p.music_assistant_enabled = Some(on);
        }
    });
    Json(sources_json(&s))
}

// --- updates ---------------------------------------------------------------------------

async fn update_state(State(s): State<Arc<AppState>>) -> Json<update::View> {
    Json(s.updater.view())
}

fn update_reply(s: &AppState, result: anyhow::Result<()>) -> axum::response::Response {
    match result {
        Ok(()) => Json(s.updater.view()).into_response(),
        Err(e) => (StatusCode::BAD_GATEWAY, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

async fn update_check(State(s): State<Arc<AppState>>) -> axum::response::Response {
    let r = s.updater.check(&s.http).await.map(|_| ());
    update_reply(&s, r)
}

async fn update_install(State(s): State<Arc<AppState>>) -> axum::response::Response {
    // The root unit restarts ownbox: save the place first.
    s.player.release().await;
    let r = s.updater.install(&s.http).await;
    update_reply(&s, r)
}

async fn update_rollback(State(s): State<Arc<AppState>>) -> axum::response::Response {
    s.player.release().await;
    let r = s.updater.rollback().await;
    update_reply(&s, r)
}

// --- sound output ---------------------------------------------------------------------

async fn audio_state(State(s): State<Arc<AppState>>) -> Json<serde_json::Value> {
    Json(json!({ "outputs": hardware::outputs(), "chosen": hardware::chosen(&s.prefs.get(), &s.config).0 }))
}

#[derive(Deserialize)]
struct OutputRequest {
    device: String,
}

/// A new output: the place is saved, mpv lets the old card go and starts again on the new
/// one; the queue stays, paused, as after a restart.
async fn audio_set(State(s): State<Arc<AppState>>, Json(req): Json<OutputRequest>) -> axum::response::Response {
    if !hardware::outputs().iter().any(|o| o.device == req.device) {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": "no such sound output" }))).into_response();
    }
    if hardware::chosen(&s.prefs.get(), &s.config).0 != req.device {
        s.prefs.update(|p| p.audio_device = Some(req.device.clone()));
        s.player.release().await;
        s.mpv.restart().await;
    }
    audio_state(State(s)).await.into_response()
}

// --- equalizer ----------------------------------------------------------------------

const EQ_PATH: &str = "/api/v1/playback/settings/eq";

fn current_eq(s: &AppState) -> eq::Eq {
    s.prefs.get().eq.unwrap_or_default()
}

/// Plays a curve at once (mpv's `af`, live) and keeps it here.
async fn apply_eq(s: &AppState, curve: &eq::Eq) -> anyhow::Result<()> {
    s.mpv.command(json!(["set_property", "af", curve.filter()])).await?;
    let saved = curve.clone();
    s.prefs.update(|p| p.eq = Some(saved));
    Ok(())
}

pub async fn sync_eq(s: &AppState) -> anyhow::Result<()> {
    let path = format!("{EQ_PATH}?device_kind={}", crate::api::DEVICE_KIND);
    let theirs: serde_json::Value = s.auth.get(&path).await?;
    let theirs = eq::Eq::from_server(&theirs).ok_or_else(|| anyhow::anyhow!("an equalizer record without its fields"))?;
    let ours = s.prefs.get().eq;
    match ours {
        // The server has never been told (it answers off and flat); this box has a curve.
        Some(ours) if theirs == eq::Eq::default() && ours != theirs => {
            s.auth.send_json(reqwest::Method::PUT, EQ_PATH, &ours.to_server()).await
        }
        _ => apply_eq(s, &theirs).await,
    }
}

async fn eq_state(State(s): State<Arc<AppState>>) -> Json<serde_json::Value> {
    Json(json!({
        "eq": current_eq(&s),
        "frequencies_hz": eq::FREQUENCIES_HZ,
        "limit_db": eq::LIMIT_DB,
        "presets": eq::PRESETS.iter().map(|(name, bands)| json!({ "name": name, "bands_db": bands })).collect::<Vec<_>>(),
    }))
}

async fn eq_set(State(s): State<Arc<AppState>>, Json(curve): Json<eq::Eq>) -> axum::response::Response {
    let curve = curve.clamped();
    if let Err(e) = apply_eq(&s, &curve).await {
        return answer::<()>(Err(e));
    }
    // To the server in the background: a slider dragged across sends many, the last wins.
    let auth = Arc::clone(&s.auth);
    let body = curve.to_server();
    tokio::spawn(async move {
        if let Err(err) = auth.send_json(reqwest::Method::PUT, EQ_PATH, &body).await {
            tracing::warn!(%err, "saving the equalizer on the server");
        }
    });
    Json(json!({ "eq": curve })).into_response()
}

// --- first-run wizard ---------------------------------------------------------------

async fn setup_state(State(s): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let prefs = s.prefs.get();
    Json(json!({
        "done": prefs.setup_done.unwrap_or(false),
        "server_url": s.auth.server(),
        "hosted_url": setup::HOSTED_URL,
        "box_name": s.auth.device_name(),
        "signed_in": matches!(s.auth.view().await, crate::session::View::SignedIn { .. }),
        // Chosen already (on the setup Wi-Fi, or before a reload): the wizard skips it.
        "language_set": prefs.language.is_some(),
        "address": box_address(&s),
    }))
}

/// Where phones find the box: `<host>.local`. A box from the SD card image takes its name
/// for its address (`apply-sources.sh`), so a name just given shows at once.
fn box_address(s: &AppState) -> String {
    let host = if std::path::Path::new("/etc/ownbox-image").exists() && s.prefs.get().box_name.is_some() {
        hotspot::host_name(&s.auth.device_name())
    } else {
        std::fs::read_to_string("/proc/sys/kernel/hostname").map(|h| h.trim().to_owned()).unwrap_or_else(|_| "ownbox".into())
    };
    format!("{host}.local")
}

// --- sound cards on the GPIO header -----------------------------------------------------

/// The cards offered by name: their overlay (as `boot-config.sh` writes it into
/// config.txt) and, for the pHAT BEAT, its buttons. One without an EEPROM is not found by
/// the firmware on its own, hence the list.
const HATS: [(&str, &str, &str); 7] = [
    ("", "", "None (USB, HDMI or the headphone jack)"),
    ("hifiberry-dac", "phat-beat", "Pimoroni pHAT BEAT (Pirate Radio)"),
    ("hifiberry-dac", "", "Pimoroni pHAT DAC, HiFiBerry DAC or MiniAmp"),
    ("iqaudio-dacplus", "", "Raspberry Pi DAC+ or IQaudIO DAC+"),
    ("hifiberry-dacplus", "", "HiFiBerry DAC+ or Amp2"),
    ("justboom-dac", "", "JustBoom DAC or Amp"),
    ("allo-boss-dac-pcm512x-audio", "", "Allo Boss DAC"),
];

fn hats_json(s: &AppState) -> serde_json::Value {
    let p = s.prefs.get();
    let hat = p.audio_hat.clone().unwrap_or_else(|| s.config.audio_hat.clone());
    let buttons = p.hat_buttons.clone().unwrap_or_else(|| s.config.buttons.clone());
    json!({
        "choices": HATS.iter().enumerate().map(|(i, (overlay, buttons, label))| json!({ "id": i, "overlay": overlay, "buttons": buttons, "label": label })).collect::<Vec<_>>(),
        "current": HATS.iter().position(|(o, b, _)| *o == hat && *b == buttons),
    })
}

async fn hats_state(State(s): State<Arc<AppState>>) -> Json<serde_json::Value> {
    Json(hats_json(&s))
}

#[derive(Deserialize)]
struct HatRequest {
    id: usize,
}

/// A card chosen on the box: saved, and the root unit writes config.txt and restarts the
/// box when it changed (`apply-sources.sh`, `boot-config.sh`).
async fn hat_set(State(s): State<Arc<AppState>>, Json(req): Json<HatRequest>) -> axum::response::Response {
    let Some((overlay, buttons, _)) = HATS.get(req.id) else {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": "no such sound card" }))).into_response();
    };
    s.prefs.update(|p| {
        p.audio_hat = Some((*overlay).to_owned());
        p.hat_buttons = Some((*buttons).to_owned());
    });
    Json(hats_json(&s)).into_response()
}

// --- the setup Wi-Fi --------------------------------------------------------------------

async fn hotspot_state(State(s): State<Arc<AppState>>) -> Json<serde_json::Value> {
    s.hotspot.refresh_networks().await;
    let snap = s.hotspot.snapshot();
    Json(json!({
        "active": snap.active,
        "networks": snap.networks,
        "error": snap.error,
        "box_name": s.auth.device_name(),
        "language": language(&s),
        "hats": hats_json(&s),
    }))
}

#[derive(Deserialize)]
struct HotspotApply {
    ssid: String,
    #[serde(default)]
    password: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    hat: Option<usize>,
}

/// The setup page's one form: language and name now, the home Wi-Fi next (the box's own
/// network goes down for it), the sound card last, since a new one restarts the box.
async fn hotspot_apply(State(s): State<Arc<AppState>>, Json(req): Json<HotspotApply>) -> axum::response::Response {
    let ssid = req.ssid.trim().to_owned();
    if ssid.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": "choose a network" }))).into_response();
    }
    if req.hat.is_some_and(|i| i >= HATS.len()) {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": "no such sound card" }))).into_response();
    }
    if let Some(lang) = req.language.filter(|l| matches!(l.as_str(), "cs" | "en")) {
        s.prefs.update(|p| p.language = Some(lang));
    }
    if let Some(name) = req.name.map(|n| clean_name(&n)).filter(|n| !n.is_empty()) {
        s.auth.set_device_name(name.clone());
        s.prefs.update(|p| p.box_name = Some(name));
    }
    let own = s.auth.device_name();
    let address = box_address(&s);
    let state = Arc::clone(&s);
    let hat = req.hat;
    tokio::spawn(Arc::clone(&s.hotspot).join(ssid.clone(), req.password, own, move || {
        if let Some((overlay, buttons, _)) = hat.and_then(|i| HATS.get(i)) {
            state.prefs.update(|p| {
                p.audio_hat = Some((*overlay).to_owned());
                p.hat_buttons = Some((*buttons).to_owned());
            });
        }
    }));
    Json(json!({ "ssid": ssid, "address": address })).into_response()
}

#[derive(Deserialize)]
struct UrlRequest {
    url: String,
}

async fn setup_check(Json(req): Json<UrlRequest>) -> axum::response::Response {

    match setup::normalize(&req.url) {
        Some(url) => Json(setup::check(&crate::api::http_client(), &url).await).into_response(),
        None => (StatusCode::BAD_REQUEST, Json(json!({ "error": "not an address" }))).into_response(),
    }
}

/// Uses the server, after checking it again: a different one signs out and empties the
/// queue, since both belong to the old server.
async fn setup_server(State(s): State<Arc<AppState>>, Json(req): Json<UrlRequest>) -> axum::response::Response {
    let Some(url) = setup::normalize(&req.url) else {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": "not an address" }))).into_response();
    };
    let check = setup::check(&crate::api::http_client(), &url).await;
    if !matches!(check, setup::Check::Ok { .. }) {
        return (StatusCode::BAD_REQUEST, Json(check)).into_response();
    }
    if url != s.auth.server() {
        s.player.clear().await;
        s.auth.set_server(url.clone()).await;
    }
    s.prefs.update(|p| p.server_url = Some(url.clone()));
    Json(check).into_response()
}

#[derive(Deserialize)]
struct NameRequest {
    name: String,
}

/// A name as people type it, without what would break the files it ends up in (AirPlay's and
/// Squeezelite's settings): no control characters (a new line), quotes or backslashes.
fn clean_name(raw: &str) -> String {
    let name: String = raw
        .chars()
        .map(|c| match c {
            c if c.is_control() => ' ',
            '"' | '`' => '\'',
            '\\' => '/',
            '$' => ' ',
            c => c,
        })
        .collect();
    name.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(60).collect()
}

async fn setup_name(State(s): State<Arc<AppState>>, Json(req): Json<NameRequest>) -> axum::response::Response {
    let name = clean_name(&req.name);
    if name.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": "a name is needed" }))).into_response();
    }
    s.auth.set_device_name(name.clone());
    s.prefs.update(|p| p.box_name = Some(name.clone()));
    Json(json!({ "box_name": name })).into_response()
}

#[derive(Deserialize)]
struct DoneRequest {
    done: bool,
}

async fn setup_done(State(s): State<Arc<AppState>>, Json(req): Json<DoneRequest>) -> Json<serde_json::Value> {
    s.prefs.update(|p| p.setup_done = Some(req.done));
    Json(json!({ "done": req.done }))
}

fn language(s: &AppState) -> String {
    s.prefs.get().language.unwrap_or_else(|| s.config.language.clone())
}

fn theme(s: &AppState) -> String {
    s.prefs.get().theme.unwrap_or_else(|| "dark".into())
}

fn buttons(s: &AppState) -> String {
    s.prefs.get().buttons.unwrap_or_else(|| "text".into())
}

async fn ui_settings(State(s): State<Arc<AppState>>) -> Json<serde_json::Value> {
    Json(json!({ "language": language(&s), "theme": theme(&s), "buttons": buttons(&s) }))
}

/// Either or both; what is left out stays.
#[derive(Deserialize)]
struct UiRequest {
    language: Option<String>,
    theme: Option<String>,
    buttons: Option<String>,
}

async fn ui_set(State(s): State<Arc<AppState>>, Json(req): Json<UiRequest>) -> axum::response::Response {
    if req.language.as_deref().is_some_and(|l| !matches!(l, "cs" | "en")) {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": "cs or en" }))).into_response();
    }
    if req.theme.as_deref().is_some_and(|t| !matches!(t, "dark" | "light" | "auto")) {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": "dark, light or auto" }))).into_response();
    }
    if req.buttons.as_deref().is_some_and(|b| !matches!(b, "text" | "icons")) {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": "text or icons" }))).into_response();
    }
    s.prefs.update(|p| {
        if req.language.is_some() {
            p.language = req.language.clone();
        }
        if req.theme.is_some() {
            p.theme = req.theme.clone();
        }
        if req.buttons.is_some() {
            p.buttons = req.buttons.clone();
        }
    });
    Json(json!({ "language": language(&s), "theme": theme(&s), "buttons": buttons(&s) })).into_response()
}

async fn wifi_status() -> axum::response::Response {
    answer(wifi::status().await)
}

#[derive(Deserialize)]
struct RadioRequest {
    on: bool,
}

async fn wifi_radio(Json(req): Json<RadioRequest>) -> axum::response::Response {
    answer(wifi::set_radio(req.on).await.map(|()| json!({ "ok": true })))
}

async fn wifi_scan() -> axum::response::Response {
    answer(wifi::scan().await)
}

#[derive(Deserialize)]
struct ConnectRequest {
    ssid: String,
    password: Option<String>,
}

async fn wifi_connect(Json(req): Json<ConnectRequest>) -> axum::response::Response {
    answer(wifi::connect(&req.ssid, req.password.as_deref()).await.map(|()| json!({ "ok": true })))
}

#[derive(Deserialize)]
struct ForgetRequest {
    ssid: String,
}

async fn wifi_forget(Json(req): Json<ForgetRequest>) -> axum::response::Response {
    answer(wifi::forget(&req.ssid).await.map(|()| json!({ "ok": true })))
}

async fn podcasts_shows(State(s): State<Arc<AppState>>) -> axum::response::Response {
    answer(podcasts::shows(&s.auth).await)
}

async fn podcasts_in_progress(State(s): State<Arc<AppState>>) -> axum::response::Response {
    answer(podcasts::in_progress(&s.auth).await)
}

async fn podcasts_latest(State(s): State<Arc<AppState>>) -> axum::response::Response {
    answer(podcasts::latest(&s.auth).await)
}

#[derive(Deserialize)]
struct OffsetQuery {
    #[serde(default)]
    offset: usize,
}

async fn podcasts_show(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(q): Query<OffsetQuery>,
) -> axum::response::Response {
    answer(podcasts::episodes(&s.auth, &id, q.offset).await.map(|(show, episodes)| json!({ "show": show, "episodes": episodes })))
}

#[derive(Deserialize)]
struct MarkRequest {
    ids: Vec<String>,
    completed: bool,
}

async fn podcasts_mark(State(s): State<Arc<AppState>>, Json(req): Json<MarkRequest>) -> axum::response::Response {
    answer(podcasts::mark(&s.auth, &req.ids, req.completed).await.map(|()| json!({ "ok": true })))
}

async fn books_detail(State(s): State<Arc<AppState>>, Path(id): Path<String>) -> axum::response::Response {
    answer(books::detail(&s.auth, &id).await)
}

#[derive(Deserialize)]
struct VolumeRequest {
    volume: f64,
}

async fn player_volume(State(s): State<Arc<AppState>>, Json(req): Json<VolumeRequest>) -> axum::response::Response {
    answer(s.player.set_volume(req.volume).await.map(|()| json!({ "ok": true })))
}

/// What to play: an album, a playlist, or tracks the screen already has (a search result).
#[derive(Deserialize)]
struct PlayRequest {
    album_id: Option<String>,
    /// Everything by an artist, album by album.
    artist_id: Option<String>,
    /// The playlist (or smart playlist) the tracks come from, remembered for the home page.
    from_playlist: Option<String>,
    playlist_id: Option<String>,
    tracks: Option<Vec<library::Track>>,
    /// An audiobook: from the saved position, or from the start with `from_start`.
    book_id: Option<String>,
    /// Podcast episodes as the screen lists them; `start` picks the one to play.
    episodes: Option<Vec<podcasts::Episode>>,
    #[serde(default)]
    from_start: bool,
    /// For a book: start at this part (file), `secs` into it, instead of the saved place.
    part: Option<usize>,
    #[serde(default)]
    secs: f64,
    #[serde(default)]
    start: usize,
}

async fn player_play(State(s): State<Arc<AppState>>, Json(req): Json<PlayRequest>) -> axum::response::Response {
    own_turn(&s).await;
    let what = async {
        if let Some(id) = &req.book_id {
            let detail = books::detail(&s.auth, id).await?;
            let (tracks, index, secs) = books::queue(&detail, req.from_start);
            return Ok(match req.part {
                Some(part) => (tracks, part, req.secs),
                None => (tracks, index, secs),
            });
        }
        if let Some(list) = &req.episodes {
            anyhow::ensure!(req.start < list.len(), "no such episode");
            let (tracks, secs) = podcasts::queue(list, req.start);
            return Ok((tracks, 0, secs));
        }
        let tracks = if let Some(id) = &req.album_id {
            library::album_detail(&s.auth, id).await?.tracks
        } else if let Some(id) = &req.playlist_id {
            library::playlist_tracks(&s.auth, id).await?
        } else if let Some(id) = &req.artist_id {
            library::artist_tracks(&s.auth, id).await?
        } else {
            req.tracks.clone().ok_or_else(|| anyhow::anyhow!("nothing to play"))?
        };
        Ok::<_, anyhow::Error>((tracks, req.start, 0.0))
    };
    let r = async {
        let (tracks, start, secs) = what.await?;
        s.player.play(tracks, start, secs).await
    }
    .await;
    if r.is_ok()
        && let Some(id) = req.playlist_id.clone().or_else(|| req.from_playlist.clone())
    {
        let now = chrono::Utc::now().timestamp();
        s.prefs.update(|p| {
            p.playlists_played.insert(id, now);
            // Only the latest matter; the oldest go, so the file stays small.
            if p.playlists_played.len() > 50 {
                let mut by_age: Vec<(String, i64)> = p.playlists_played.drain().collect();
                by_age.sort_by_key(|(_, at)| -*at);
                by_age.truncate(30);
                p.playlists_played = by_age.into_iter().collect();
            }
        });
    }
    answer(r.map(|()| json!({ "ok": true })))
}

async fn player_now(State(s): State<Arc<AppState>>) -> Json<player::NowPlaying> {
    Json(s.player.now().await)
}

#[derive(Deserialize)]
struct PauseRequest {
    paused: bool,
}

async fn player_pause(State(s): State<Arc<AppState>>, Json(req): Json<PauseRequest>) -> axum::response::Response {
    if !req.paused {
        own_turn(&s).await;
    }
    answer(s.player.set_paused(req.paused).await.map(|()| json!({ "ok": true })))
}

/// Tracks for the queue: after what plays (`next`) or at its end. With nothing to add to,
/// they simply play.
#[derive(Deserialize)]
struct EnqueueRequest {
    album_id: Option<String>,
    tracks: Option<Vec<library::Track>>,
    #[serde(default)]
    next: bool,
}

async fn player_enqueue(State(s): State<Arc<AppState>>, Json(req): Json<EnqueueRequest>) -> axum::response::Response {
    let r = async {
        let tracks = match (&req.album_id, req.tracks) {
            (Some(id), _) => library::album_detail(&s.auth, id).await?.tracks,
            (None, Some(t)) => t,
            (None, None) => anyhow::bail!("nothing to add"),
        };
        if s.player.has_queue().await {
            s.player.enqueue(tracks, req.next).await?;
            return Ok(json!({ "ok": true, "started": false }));
        }
        own_turn(&s).await;
        s.player.play(tracks, 0, 0.0).await?;
        Ok(json!({ "ok": true, "started": true }))
    };
    answer(r.await)
}

async fn player_toggle(State(s): State<Arc<AppState>>) -> axum::response::Response {
    own_turn(&s).await;
    answer(s.player.toggle().await.map(|()| json!({ "ok": true })))
}

async fn player_next(State(s): State<Arc<AppState>>) -> axum::response::Response {
    own_turn(&s).await;
    answer(s.player.next().await.map(|()| json!({ "ok": true })))
}

async fn player_previous(State(s): State<Arc<AppState>>) -> axum::response::Response {
    own_turn(&s).await;
    answer(s.player.previous().await.map(|()| json!({ "ok": true })))
}

#[derive(Deserialize)]
struct SeekRequest {
    secs: f64,
}

async fn player_seek(State(s): State<Arc<AppState>>, Json(req): Json<SeekRequest>) -> axum::response::Response {
    answer(s.player.seek(req.secs).await.map(|()| json!({ "ok": true })))
}

async fn session(State(s): State<Arc<AppState>>) -> Json<crate::session::View> {
    Json(s.auth.view().await)
}

async fn sign_in(State(s): State<Arc<AppState>>) -> impl IntoResponse {
    match s.auth.start_sign_in().await {
        Ok(()) => (StatusCode::OK, Json(json!({ "ok": true }))),
        Err(e) => (StatusCode::BAD_GATEWAY, Json(json!({ "ok": false, "error": e.to_string() }))),
    }
}

async fn sign_in_qr(State(s): State<Arc<AppState>>) -> axum::response::Response {
    let Some(link) = s.auth.pending_link().await else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match qrcode::QrCode::new(link.as_bytes()) {
        Ok(code) => {
            let svg = code
                .render::<qrcode::render::svg::Color>()
                .min_dimensions(240, 240)
                .quiet_zone(true)
                .build();
            ([(header::CONTENT_TYPE, "image/svg+xml"), (header::CACHE_CONTROL, "no-store")], svg)
                .into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn sign_out(State(s): State<Arc<AppState>>) -> Json<serde_json::Value> {
    s.auth.sign_out().await;
    Json(json!({ "ok": true }))
}

fn asset(content_type: &'static str, body: &'static str) -> impl IntoResponse {
    ([(header::CONTENT_TYPE, content_type), (header::CACHE_CONTROL, "no-cache")], body)
}

#[derive(Serialize)]
struct Status {
    version: &'static str,
    uptime_secs: u64,
    server_url: String,
    audio_device: String,
    mpv: MpvStatus,
    power: Option<power::Power>,
    hardware: hardware::Hardware,
}

#[derive(Serialize)]
struct MpvStatus {
    running: bool,
    version: Option<String>,
    error: Option<String>,
}

async fn status(State(s): State<Arc<AppState>>) -> Json<Status> {
    let mpv = match s.mpv.command(json!(["get_property", "mpv-version"])).await {
        Ok(v) => MpvStatus { running: true, version: v.as_str().map(str::to_owned), error: None },
        Err(e) => MpvStatus { running: false, version: None, error: Some(e.to_string()) },
    };
    Json(Status {
        version: env!("CARGO_PKG_VERSION"),
        uptime_secs: s.started.elapsed().as_secs(),
        server_url: s.auth.server(),
        audio_device: hardware::chosen(&s.prefs.get(), &s.config).0,
        mpv,
        power: power::read().await,
        hardware: hardware::read(),
    })
}

/// One second of 440 Hz through mpv: checks touch, mpv and the audio device in one go.
async fn test_tone(State(s): State<Arc<AppState>>) -> impl IntoResponse {
    let tone = "av://lavfi:sine=frequency=440:duration=1";
    match s.mpv.command(json!(["loadfile", tone, "replace"])).await {
        Ok(_) => (StatusCode::OK, Json(json!({ "ok": true }))),
        Err(e) => (StatusCode::BAD_GATEWAY, Json(json!({ "ok": false, "error": e.to_string() }))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_lose_what_would_break_the_settings_files() {
        assert_eq!(clean_name("  Kuchyň  "), "Kuchyň");
        assert_eq!(clean_name("a\"b\nc\\d$e`f"), "a'b c/d e'f");
        assert_eq!(clean_name(&"x".repeat(80)).len(), 60);
    }
}
