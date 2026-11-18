mod api;
mod books;
mod buttons;
mod config;
mod eq;
mod hardware;
mod hotspot;
mod library;
mod mpv;
mod outside;
mod player;
mod podcasts;
mod power;
mod prefs;
mod remote;
mod screen;
mod session;
mod setup;
mod update;
mod web;
mod wifi;

use std::{path::PathBuf, sync::Arc, time::Instant};

use anyhow::Context;

pub struct AppState {
    pub config: config::Config,
    pub mpv: Arc<mpv::Mpv>,
    pub auth: Arc<session::Auth>,
    pub player: Arc<player::Player>,
    pub screen: screen::Screen,
    pub prefs: Arc<prefs::PrefsStore>,
    pub outside: Arc<outside::Outside>,
    pub remote: remote::Remote,
    pub started: Instant,
    pub updater: Arc<update::Updater>,
    pub http: reqwest::Client,
    pub hotspot: Arc<hotspot::Hotspot>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // `ownbox --version`, and `ownbox verify <file> <signature>` for the root update unit.
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["--version"] => {
            println!("{}", update::VERSION);
            return Ok(());
        }
        ["verify", file, signature] => return update::verify_files(file.as_ref(), signature.as_ref()),
        _ => {}
    }
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ownbox=info".into()),
        )
        .init();

    let config = config::load().context("loading the config")?;
    // systemd's RuntimeDirectory= sets RUNTIME_DIRECTORY; a run by hand falls back to /tmp.
    let runtime_dir = std::env::var_os("RUNTIME_DIRECTORY")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let prefs = Arc::new(prefs::PrefsStore::load(config::state_dir()?.join("prefs.json")));
    let mpv = Arc::new(mpv::Mpv::start(
        config.clone(),
        Arc::clone(&prefs),
        runtime_dir.join("mpv.sock"),
    ));

    let listener = tokio::net::TcpListener::bind(&config.listen)
        .await
        .with_context(|| format!("listening on {}", config.listen))?;
    tracing::info!(listen = %config.listen, "ownbox {}", env!("CARGO_PKG_VERSION"));

    // The wizard's choices win over config.toml's starting values.
    let saved = prefs.get();
    let server_url = saved.server_url.clone().unwrap_or_else(|| config.server_url.clone());
    let device_name = saved.box_name.clone().unwrap_or_else(|| config.device_name.clone());
    let auth = session::Auth::load(
        api::http_client(),
        server_url,
        device_name,
        config::state_dir()?.join("session.json"),
    );
    // A box signed in before the wizard existed has, in effect, finished it.
    if saved.setup_done.is_none() && matches!(auth.view().await, session::View::SignedIn { .. }) {
        prefs.update(|p| p.setup_done = Some(true));
    }
    // Touch the saved session once at start: an expired token is refreshed now, and a revoked
    // one sends the screen back to sign-in instead of failing at the first tap.
    {
        let auth = Arc::clone(&auth);
        tokio::spawn(async move {
            if matches!(auth.view().await, session::View::SignedIn { .. }) {
                match auth.get::<api::User>("/api/v1/auth/me").await {
                    Ok(user) => tracing::info!(user = %user.email, "session is valid"),
                    Err(err) => tracing::warn!(%err, "checking the saved session"),
                }
            }
        });
    }

    let player = player::Player::new(
        Arc::clone(&mpv),
        Arc::clone(&auth),
        Arc::clone(&prefs),
        config::state_dir()?.join("queue.json"),
    );
    let screen = screen::Screen::new(config.screen_brightness, config.screen_off_after_secs, Arc::clone(&prefs));
    // While the server is out of reach, knock every 15 s, so the screen's notice clears
    // when it is back even if nothing else is asked of it.
    {
        let auth = Arc::clone(&auth);
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(std::time::Duration::from_secs(15));
            loop {
                tick.tick().await;
                if !auth.reachable() && matches!(auth.view().await, session::View::SignedIn { .. }) {
                    let _ = auth.get::<api::User>("/api/v1/auth/me").await;
                }
            }
        });
    }

    let outside = outside::Outside::new();
    let http = api::http_client();
    let updater = Arc::new(update::Updater::new(config.update_feed.clone(), config::state_dir()?.join("update")));
    let state = Arc::new(AppState {
        config,
        mpv,
        auth,
        player,
        screen,
        prefs,
        outside,
        remote: remote::Remote::new(),
        started: Instant::now(),
        updater: Arc::clone(&updater),
        http: http.clone(),
        hotspot: hotspot::Hotspot::new(),
    });
    // Only a box without a screen: one with a screen sets its Wi-Fi up on it.
    if state.config.headless && state.config.setup_wifi {
        let named = Arc::clone(&state);
        tokio::spawn(Arc::clone(&state.hotspot).run(move || named.auth.device_name()));
    }
    tokio::spawn(updater.run(http));
    tokio::spawn(night_updates(Arc::clone(&state)));
    tokio::spawn(wifi::roam());
    tokio::spawn(buttons::run(Arc::clone(&state)));

    {
        let state = Arc::clone(&state);
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                state.screen.turn_off_if_forgotten();
                state.player.release_if_paused_for(std::time::Duration::from_secs(15)).await;
            }
        });
    }

    // The equalizer follows the account, as in the apps: once signed in, the server's curve
    // replaces the local copy, unless the server has none yet and this box has one.
    {
        let state = Arc::clone(&state);
        tokio::spawn(async move {
            loop {
                if matches!(state.auth.view().await, session::View::SignedIn { .. }) {
                    match web::sync_eq(&state).await {
                        Ok(()) => break,
                        Err(err) => tracing::warn!(%err, "reading the equalizer from the server"),
                    }
                }
                tokio::time::sleep(std::time::Duration::from_secs(20)).await;
            }
        });
    }
    let plain = if state.config.listen_http.is_empty() {
        None
    } else {
        match tokio::net::TcpListener::bind(&state.config.listen_http).await {
            Ok(l) => Some(l),
            Err(err) => {
                tracing::warn!(%err, listen = %state.config.listen_http, "not serving the plain address");
                None
            }
        }
    };
    let (stopping, stopped) = tokio::sync::oneshot::channel();
    let (stop_all, _) = tokio::sync::broadcast::channel::<()>(1);
    {
        let stop_all = stop_all.clone();
        tokio::spawn(async move {
            shutdown_signal().await;
            let _ = stop_all.send(());
            let _ = stopping.send(());
        });
    }
    let serve_on = |listener: tokio::net::TcpListener| {
        let mut stop = stop_all.subscribe();
        axum::serve(listener, web::router(state.clone()).into_make_service_with_connect_info::<std::net::SocketAddr>())
            .with_graceful_shutdown(async move {
                let _ = stop.recv().await;
            })
            .into_future()
    };
    let main_server = serve_on(listener);
    let serve = async move {
        match plain {
            Some(l) => tokio::try_join!(main_server, serve_on(l)).map(drop),
            None => main_server.await,
        }
    };
    // The screen and phones ask every second over kept-alive connections; waiting for them
    // all to close could last until systemd's SIGKILL. A few seconds is enough.
    tokio::select! {
        served = serve => served?,
        () = async {
            let _ = stopped.await;
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        } => tracing::warn!("connections still open after 3 s; stopping anyway"),
    }

    state.player.shutdown().await;
    state.mpv.stop().await;
    tracing::info!("stopped");
    // Exit here rather than return: the button and AirPlay readers block a thread each in a
    // read that never ends (tokio::fs), and the runtime would wait for them on the way out
    // until systemd killed it 20 s later. Everything worth keeping is saved by now.
    std::process::exit(0)
}

async fn shutdown_signal() {
    let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .expect("SIGTERM handler");
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = term.recv() => {}
    }
    tracing::info!("stopping");
}

/// Installs an available update at night, when the settings allow it and nothing plays:
/// not the box's own queue, not AirPlay or Music Assistant. Checks first if the day's check
/// found nothing, so a release from the evening is installed the same night.
async fn night_updates(state: Arc<AppState>) {
    if !state.updater.enabled() {
        return;
    }
    let mut checked_night = None;
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(10 * 60)).await;
        let now = chrono::Local::now();
        if !update::at_night(chrono::Timelike::hour(&now)) || state.prefs.get().auto_update == Some(false) {
            continue;
        }
        if state.player.now().await.state == "playing" || state.outside.view().is_some() {
            continue;
        }
        let view = state.updater.view();
        if view.state.is_some() {
            continue;
        }
        if view.available.is_none() {
            let night = now.date_naive();
            if checked_night == Some(night) {
                continue;
            }
            checked_night = Some(night);
            if !matches!(state.updater.check(&state.http).await, Ok(Some(_))) {
                continue;
            }
        }
        tracing::info!("installing the update at night");
        state.player.release().await;
        if let Err(err) = state.updater.install(&state.http).await {
            tracing::warn!(%err, "night update");
        }
    }
}
