//! Local control socket (SPEC F7, ADR-042): JSON lines over a Unix socket
//! at `$TMPDIR/ttnr-$UID.sock`, so other tools (a menu-bar widget, scripts,
//! `ttnr now`) can read the status and send commands to a running TUI.
//!
//! Requests, one JSON object per line: `{"cmd": "status"}`,
//! `{"cmd": "subscribe"}` (status now and on every change),
//! `{"cmd": "toggle_pause" | "next" | "prev" | "mute" | "quit"}`,
//! `{"cmd": "volume", "value": 0-100}`, `{"cmd": "play", "station": "<id>"}`.
//! Answers: a status object, `{"ok": true}`, or `{"ok": false, "error": …}`.

use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{mpsc, watch};
use ttnr_core::app::{AppState, Command, Msg, Status};

/// The socket path: `$TMPDIR/ttnr-<uid>.sock` (macOS `$TMPDIR` is per
/// user); the uid is the owner of `$HOME`.
pub fn socket_path() -> PathBuf {
    let dir = std::env::var_os("TMPDIR").map_or_else(|| PathBuf::from("/tmp"), PathBuf::from);
    let uid = std::env::var_os("HOME")
        .and_then(|h| std::fs::metadata(h).ok())
        .map_or(0, |m| m.uid());
    dir.join(format!("ttnr-{uid}.sock"))
}

/// What a client sees.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusJson {
    /// Station, if one is selected.
    pub station: Option<StationJson>,
    /// `idle`, `resolving`, `loading`, `buffering`, `playing`, `paused`,
    /// `reconnecting`, `ended`, `error`.
    pub status: String,
    /// Detail for `reconnecting` / `error`.
    pub message: Option<String>,
    /// Now playing, if known.
    pub now: Option<NowJson>,
    /// Stream format line, e.g. `HE-AAC · 48 kHz · stereo · 96 kbps`.
    pub format: Option<String>,
    /// App volume 0–100.
    pub volume: u8,
    /// Muted.
    pub muted: bool,
    /// Pinned output device (`null` = system default).
    pub device: Option<String>,
}

/// Station part of [`StatusJson`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StationJson {
    /// Id within its source.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Source id.
    pub source: String,
}

/// Now-playing part of [`StatusJson`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NowJson {
    /// Artist.
    pub artist: Option<String>,
    /// Title.
    pub title: Option<String>,
    /// Programme / show.
    pub programme: Option<String>,
    /// Album.
    pub album: Option<String>,
    /// Artwork URL.
    pub artwork_url: Option<String>,
}

impl StatusJson {
    /// The status of `s`.
    pub fn from_state(s: &AppState) -> Self {
        let (status, message) = match &s.status {
            Status::Idle => ("idle", None),
            Status::Resolving => ("resolving", None),
            Status::Loading => ("loading", None),
            Status::Buffering(_) => ("buffering", None),
            Status::Playing => ("playing", None),
            Status::Paused => ("paused", None),
            Status::Reconnecting(m) => ("reconnecting", Some(m.clone())),
            Status::Ended => ("ended", None),
            Status::Error(m) => ("error", Some(m.clone())),
        };
        Self {
            station: s.station.as_ref().map(|st| StationJson {
                id: st.id.clone(),
                name: st.name.clone(),
                source: st.source.clone(),
            }),
            status: status.to_owned(),
            message,
            now: s.now.as_ref().map(|n| NowJson {
                artist: n.artist.clone(),
                title: n.title.clone(),
                programme: n.programme.clone(),
                album: n.album.clone(),
                artwork_url: n.artwork_url.clone(),
            }),
            format: s
                .stream
                .as_ref()
                .map(ttnr_core::StreamInfo::summary)
                .filter(|f| !f.is_empty()),
            volume: s.volume,
            muted: s.muted,
            device: s.device.clone(),
        }
    }

    /// One line for `ttnr now`: "Artist – Title (Station)", falling back to
    /// the programme or the station; the status if nothing plays.
    pub fn line(&self) -> String {
        let station = self.station.as_ref().map(|s| s.name.as_str());
        let now = self.now.as_ref();
        let track = match now.map(|n| (&n.artist, &n.title)) {
            Some((Some(a), Some(t))) => Some(format!("{a} – {t}")),
            Some((None, Some(t))) => Some(t.clone()),
            _ => now.and_then(|n| n.programme.clone()),
        };
        match (track, station) {
            (Some(t), Some(s)) => format!("{t} ({s})"),
            (None, Some(s)) => format!("{s} [{}]", self.status),
            (t, None) => t.unwrap_or_else(|| self.status.clone()),
        }
    }
}

/// A parsed request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    /// Send the status once.
    Status,
    /// Send the status now and on every change.
    Subscribe,
    /// Deliver an app message.
    Do(Msg),
    /// Play the station with this id (from the station list).
    Play(String),
}

#[derive(Deserialize)]
struct RawRequest {
    cmd: String,
    value: Option<i64>,
    station: Option<String>,
}

/// Parse one request line.
pub fn parse(line: &str) -> Result<Request, String> {
    let raw: RawRequest = serde_json::from_str(line).map_err(|e| format!("bad request: {e}"))?;
    let cmd = |c| Ok(Request::Do(Msg::Command(c)));
    match raw.cmd.as_str() {
        "status" => Ok(Request::Status),
        "subscribe" => Ok(Request::Subscribe),
        "toggle_pause" => cmd(Command::TogglePause),
        "next" => cmd(Command::NextStation),
        "prev" => cmd(Command::PrevStation),
        "mute" => cmd(Command::ToggleMute),
        "quit" => cmd(Command::Quit),
        "volume" => match raw.value {
            Some(v) => cmd(Command::SetVolume(
                u8::try_from(v.clamp(0, 100)).unwrap_or(0),
            )),
            None => Err("volume needs \"value\": 0-100".into()),
        },
        "play" => raw
            .station
            .map(Request::Play)
            .ok_or_else(|| "play needs \"station\": <id>".into()),
        other => Err(format!("unknown command {other:?}")),
    }
}

/// Bind the socket unless another ttnr answers on it (a stale file from a
/// crashed run is replaced). `None` = another instance owns it.
pub async fn bind(path: &Path) -> Result<Option<UnixListener>> {
    if path.exists() {
        if UnixStream::connect(path).await.is_ok() {
            return Ok(None);
        }
        std::fs::remove_file(path).with_context(|| format!("cannot remove {}", path.display()))?;
    }
    let listener =
        UnixListener::bind(path).with_context(|| format!("cannot bind {}", path.display()))?;
    Ok(Some(listener))
}

/// Serve clients until the state channel closes; removes the socket then.
pub async fn serve(
    listener: UnixListener,
    path: PathBuf,
    state: watch::Receiver<AppState>,
    msgs: mpsc::UnboundedSender<Msg>,
) {
    let mut closed = state.clone();
    loop {
        tokio::select! {
            accepted = listener.accept() => match accepted {
                Ok((stream, _)) => {
                    tokio::spawn(client(stream, state.clone(), msgs.clone()));
                }
                Err(e) => {
                    tracing::warn!(error = %e, "control socket accept failed");
                    break;
                }
            },
            changed = closed.changed() => {
                if changed.is_err() || closed.borrow().quit {
                    break;
                }
            }
        }
    }
    let _ = std::fs::remove_file(&path);
}

async fn client(
    stream: UnixStream,
    mut state: watch::Receiver<AppState>,
    msgs: mpsc::UnboundedSender<Msg>,
) {
    let (read, mut write) = stream.into_split();
    let mut lines = BufReader::new(read).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        if line.trim().is_empty() {
            continue;
        }
        let answer = match parse(&line) {
            Ok(Request::Status) => status_line(&state.borrow()),
            Ok(Request::Subscribe) => {
                // Status now and on every change, until the client leaves.
                loop {
                    let line = status_line(&state.borrow_and_update());
                    if write.write_all(line.as_bytes()).await.is_err() {
                        return;
                    }
                    if state.changed().await.is_err() {
                        return;
                    }
                }
            }
            Ok(Request::Do(msg)) => ok(msgs.send(msg).is_ok()),
            Ok(Request::Play(id)) => {
                let st = state.borrow().stations.iter().find(|s| s.id == id).cloned();
                match st {
                    Some(st) => ok(msgs.send(Msg::Play(st)).is_ok()),
                    None => error(&format!("unknown station {id:?}")),
                }
            }
            Err(e) => error(&e),
        };
        if write.write_all(answer.as_bytes()).await.is_err() {
            return;
        }
    }
}

fn status_line(s: &AppState) -> String {
    let mut line = serde_json::to_string(&StatusJson::from_state(s)).unwrap_or_default();
    line.push('\n');
    line
}

fn ok(sent: bool) -> String {
    if sent {
        "{\"ok\":true}\n".to_owned()
    } else {
        error("ttnr is shutting down")
    }
}

fn error(e: &str) -> String {
    format!("{}\n", serde_json::json!({ "ok": false, "error": e }))
}

/// Client side: send one request line, return the answer line.
pub async fn request(line: &str) -> Result<String> {
    let path = socket_path();
    let stream = UnixStream::connect(&path).await.map_err(|_| {
        anyhow::anyhow!(
            "ttnr is not running (no control socket at {})",
            path.display()
        )
    })?;
    let (read, mut write) = stream.into_split();
    write.write_all(format!("{line}\n").as_bytes()).await?;
    let mut lines = BufReader::new(read).lines();
    match lines.next_line().await? {
        Some(answer) => Ok(answer),
        None => bail!("ttnr closed the control socket"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;
    use ttnr_core::{InfoSource, NowPlaying, Station};

    fn playing() -> AppState {
        AppState {
            station: Some(Station {
                name: "BBC Radio 6 Music".into(),
                ..Station::from_url("http://x")
            }),
            status: Status::Playing,
            now: Some(NowPlaying {
                artist: Some("The Strokes".into()),
                title: Some("Someday".into()),
                ..NowPlaying::empty(InfoSource::External, SystemTime::UNIX_EPOCH)
            }),
            volume: 80,
            ..AppState::default()
        }
    }

    #[test]
    fn status_json_and_line() {
        let s = StatusJson::from_state(&playing());
        assert_eq!(s.status, "playing");
        assert_eq!(s.volume, 80);
        assert_eq!(s.line(), "The Strokes – Someday (BBC Radio 6 Music)");
        let json = serde_json::to_value(&s).unwrap();
        assert_eq!(json["now"]["title"], "Someday");
        assert_eq!(json["station"]["name"], "BBC Radio 6 Music");
        let idle = StatusJson::from_state(&AppState::default());
        assert_eq!(idle.line(), "idle");
        let mut err = playing();
        err.now = None;
        err.status = Status::Error("HTTP 404".into());
        let e = StatusJson::from_state(&err);
        assert_eq!(
            (e.status.as_str(), e.message.as_deref()),
            ("error", Some("HTTP 404"))
        );
        assert_eq!(e.line(), "BBC Radio 6 Music [error]");
    }

    #[test]
    fn requests() {
        assert_eq!(parse(r#"{"cmd":"status"}"#), Ok(Request::Status));
        assert_eq!(
            parse(r#"{"cmd":"next"}"#),
            Ok(Request::Do(Msg::Command(Command::NextStation)))
        );
        assert_eq!(
            parse(r#"{"cmd":"volume","value":250}"#),
            Ok(Request::Do(Msg::Command(Command::SetVolume(100))))
        );
        assert_eq!(
            parse(r#"{"cmd":"play","station":"fip"}"#),
            Ok(Request::Play("fip".into()))
        );
        assert!(parse(r#"{"cmd":"volume"}"#).is_err());
        assert!(
            parse(r#"{"cmd":"dance"}"#)
                .unwrap_err()
                .contains("unknown command")
        );
        assert!(parse("not json").is_err());
    }

    #[tokio::test]
    async fn serves_status_commands_and_subscriptions() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.sock");
        let listener = bind(&path).await.unwrap().unwrap();
        assert!(
            bind(&path).await.unwrap().is_none(),
            "second instance backs off"
        );
        let (state_tx, state_rx) = watch::channel(playing());
        let (msg_tx, mut msg_rx) = mpsc::unbounded_channel();
        let server = tokio::spawn(serve(listener, path.clone(), state_rx, msg_tx));

        let stream = UnixStream::connect(&path).await.unwrap();
        let (read, mut write) = stream.into_split();
        let mut lines = BufReader::new(read).lines();
        write.write_all(b"{\"cmd\":\"status\"}\n{\"cmd\":\"next\"}\n{\"cmd\":\"play\",\"station\":\"nope\"}\n").await.unwrap();
        let status: StatusJson =
            serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
        assert_eq!(status.status, "playing");
        assert_eq!(lines.next_line().await.unwrap().unwrap(), r#"{"ok":true}"#);
        assert!(
            lines
                .next_line()
                .await
                .unwrap()
                .unwrap()
                .contains("unknown station")
        );
        assert_eq!(
            msg_rx.recv().await,
            Some(Msg::Command(Command::NextStation))
        );

        write.write_all(b"{\"cmd\":\"subscribe\"}\n").await.unwrap();
        let first: StatusJson =
            serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
        assert_eq!(first.volume, 80);
        state_tx.send_modify(|s| s.volume = 55);
        let second: StatusJson =
            serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
        assert_eq!(second.volume, 55);

        state_tx.send_modify(|s| s.quit = true);
        server.await.unwrap();
        assert!(!path.exists(), "socket removed on quit");
    }
}
