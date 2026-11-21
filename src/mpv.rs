//! mpv does the playing; ownbox starts it, restarts it when it dies, and talks to it over
//! its JSON IPC socket.

use std::{
    path::PathBuf,
    process::Stdio,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use anyhow::{anyhow, bail};
use serde_json::{Value, json};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::UnixStream,
    process::Command,
    sync::{Mutex, watch},
    task::JoinHandle,
};

use crate::{config::Config, hardware, prefs::PrefsStore};

const IPC_TIMEOUT: Duration = Duration::from_secs(2);

pub struct Mpv {
    socket: PathBuf,
    stop: watch::Sender<bool>,
    task: Mutex<Option<JoinHandle<()>>>,
    next_id: AtomicU64,
    /// Goes up each time mpv is (re)started: a player that loaded something into an earlier
    /// mpv can tell that it is gone rather than finished.
    generation: Arc<AtomicU64>,
}

impl Mpv {
    pub fn start(config: Config, prefs: Arc<PrefsStore>, socket: PathBuf) -> Self {
        let (stop, stop_rx) = watch::channel(false);
        let generation = Arc::new(AtomicU64::new(0));
        let task = tokio::spawn(supervise(
            config,
            prefs,
            socket.clone(),
            Arc::clone(&generation),
            stop_rx,
        ));
        Self { socket, stop, task: Mutex::new(Some(task)), next_id: AtomicU64::new(1), generation }
    }

    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Relaxed)
    }

    /// Ends this mpv; the supervisor starts a new one a second later, with the output and
    /// options read afresh.
    pub async fn restart(&self) {
        let _ = self.command(json!(["quit"])).await;
    }

    pub async fn stop(&self) {
        let _ = self.stop.send(true);
        if let Some(task) = self.task.lock().await.take() {
            let _ = task.await;
        }
    }

    /// The socket appears a moment after mpv starts: right after a restart, wait for it
    /// (up to 2 s) instead of failing the first command.
    async fn connect(&self) -> anyhow::Result<UnixStream> {
        let mut last = None;
        for _ in 0..10 {
            match tokio::time::timeout(IPC_TIMEOUT, UnixStream::connect(&self.socket)).await {
                Ok(Ok(stream)) => return Ok(stream),
                Ok(Err(err)) => last = Some(anyhow::Error::from(err)),
                Err(err) => last = Some(anyhow::Error::from(err)),
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        Err(last.unwrap_or_else(|| anyhow!("mpv is not answering")))
    }

    /// Sends one command and returns its `data`. A connection per command: simple, and the
    /// socket is local.
    pub async fn command(&self, args: Value) -> anyhow::Result<Value> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let stream = self.connect().await?;
        request(stream, id, args).await
    }
}

/// One request on a connected socket, its reply's `data`.
async fn request(stream: UnixStream, id: u64, args: Value) -> anyhow::Result<Value> {
    let (read, mut write) = stream.into_split();
    let request = json!({ "command": args, "request_id": id });
    write.write_all(format!("{request}\n").as_bytes()).await?;

    let reply = async {
        let mut lines = BufReader::new(read).lines();
        // Events arrive on the same socket; skip them until our reply.
        while let Some(line) = lines.next_line().await? {
            let msg: Value = serde_json::from_str(&line)?;
            if msg.get("request_id").and_then(Value::as_u64) != Some(id) {
                continue;
            }
            return match msg.get("error").and_then(Value::as_str) {
                Some("success") => Ok(msg.get("data").cloned().unwrap_or(Value::Null)),
                Some(err) => Err(anyhow!("mpv: {err}")),
                None => Err(anyhow!("mpv: reply without a status")),
            };
        }
        bail!("mpv closed the connection")
    };
    tokio::time::timeout(IPC_TIMEOUT, reply).await?
}

/// How often the supervisor asks mpv whether it still answers, and after how many missed
/// answers in a row it is killed: a process that lives but no longer answers (stuck on a
/// dead stream, a sound card that went away) would leave the box silent until a reboot.
const PING_EVERY: Duration = Duration::from_secs(10);
const PINGS_MISSED: u32 = 3;
/// Far above the ids `command` hands out, and inside mpv's 64-bit signed range.
const PING_ID: u64 = 1 << 40;

async fn answers(socket: &std::path::Path) -> bool {
    let Ok(Ok(stream)) = tokio::time::timeout(IPC_TIMEOUT, UnixStream::connect(socket)).await else { return false };
    request(stream, PING_ID, json!(["get_property", "mpv-version"])).await.is_ok()
}

async fn supervise(
    config: Config,
    prefs: Arc<PrefsStore>,
    socket: PathBuf,
    generation: Arc<AtomicU64>,
    mut stop: watch::Receiver<bool>,
) {
    let mut backoff = Duration::from_secs(1);
    loop {
        let _ = std::fs::remove_file(&socket);
        // Read at every start: a new output chosen in Settings takes effect by restarting mpv.
        let (audio_device, audio_options) = hardware::chosen(&prefs.get(), &config);
        let spawned = Command::new("mpv")
            // --prefetch-playlist opens the next entry before the current one ends.
            .args(["--idle=yes", "--no-video", "--no-terminal", "--gapless-audio=yes", "--prefetch-playlist=yes"])
            // Ride out short network drops: ffmpeg reconnects to the stream and resumes at the
            // byte it had reached, for up to 30 s, before mpv gives the file up.
            .arg("--stream-lavf-o=reconnect=1,reconnect_streamed=1,reconnect_on_network_error=1,reconnect_delay_max=30")
            .arg("--network-timeout=30")
            // mpv reads up to 150 MiB ahead by default; a Zero 2 W has 512 MiB for everything.
            // A few minutes of audio is plenty to ride out a drop.
            .args(["--demuxer-max-bytes=24MiB", "--demuxer-max-back-bytes=8MiB"])
            // With ffmpeg's advanced edit lists mpv drops the last AAC frame of every M4A
            // (23–46 ms), a jump at each join of an album that runs on; measured, the plain
            // edit list keeps M4A whole and leaves MP3, FLAC and Opus as they were.
            .arg("--demuxer-lavf-o=advanced_editlist=0")
            .arg(format!("--audio-device={audio_device}"))
            .args(&audio_options)
            // Read on every start, so an mpv restarted after a crash keeps the chosen volume
            // and equalizer.
            .arg(format!("--volume={}", prefs.get().volume.unwrap_or(100.0)))
            .arg(format!("--af={}", prefs.get().eq.unwrap_or_default().filter()))
            .arg(format!("--input-ipc-server={}", socket.display()))
            .stdin(Stdio::null())
            .kill_on_drop(true)
            .spawn();

        match spawned {
            Ok(mut child) => {
                generation.fetch_add(1, Ordering::Relaxed);
                let started = Instant::now();
                let mut ping = tokio::time::interval_at(tokio::time::Instant::now() + PING_EVERY, PING_EVERY);
                let mut missed = 0;
                loop {
                    tokio::select! {
                        status = child.wait() => {
                            tracing::warn!(?status, "mpv exited");
                            break;
                        }
                        _ = ping.tick() => {
                            missed = if answers(&socket).await { 0 } else { missed + 1 };
                            if missed >= PINGS_MISSED {
                                tracing::warn!("mpv stopped answering; killing it");
                                let _ = child.kill().await;
                                break;
                            }
                        }
                        _ = stop.changed() => {
                            let _ = child.kill().await;
                            return;
                        }
                    }
                }
                // A crash after a good run starts over quickly; a crash loop backs off.
                backoff = if started.elapsed() > Duration::from_secs(30) {
                    Duration::from_secs(1)
                } else {
                    (backoff * 2).min(Duration::from_secs(30))
                };
            }
            Err(err) => tracing::error!(%err, "cannot start mpv"),
        }

        tokio::select! {
            _ = tokio::time::sleep(backoff) => {}
            _ = stop.changed() => return,
        }
    }
}
