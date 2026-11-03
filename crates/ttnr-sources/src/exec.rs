//! The `exec` source: stations from an external command speaking a small
//! JSON protocol (docs/SOURCES.md, ADR-041), so sources can be scripted in
//! any language without recompiling ttnr.
//!
//! ttnr runs `command… <verb>` with a JSON request on stdin and reads one
//! JSON document from stdout (`list`, `resolve`), or JSON lines until it is
//! stopped (`now`). Exit code 64 or `{"unsupported": true}` = verb not
//! implemented; any other non-zero exit is an error (first stderr line).

use std::collections::BTreeMap;
use std::process::Stdio;
use std::time::{Duration, SystemTime};

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc::Sender;
use ttnr_core::{
    InfoSource, ListQuery, MetadataProvider, NowPlaying, ResolveCtx, Source, SourceCaps,
    SourceError, Station, StreamDescriptor,
};

/// Exit code for "verb not implemented" (EX_USAGE).
pub const UNSUPPORTED_EXIT: i32 = 64;

/// How long `list` / `resolve` may take.
const TIMEOUT: Duration = Duration::from_secs(15);

/// A source backed by an external command.
#[derive(Debug, Clone)]
pub struct ExecSource {
    id: &'static str,
    command: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct StationOut {
    id: String,
    name: String,
    url: Option<String>,
    params: BTreeMap<String, String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ListOut {
    stations: Vec<StationOut>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ResolveOut {
    urls: Vec<String>,
    headers: BTreeMap<String, String>,
    ttl: Option<u64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct NowOut {
    artist: Option<String>,
    title: Option<String>,
    programme: Option<String>,
    album: Option<String>,
    artwork_url: Option<String>,
}

impl ExecSource {
    /// A source `id` that runs `command` (program and arguments).
    pub fn new(id: &str, command: Vec<String>) -> Result<Self, SourceError> {
        if command.first().is_none_or(String::is_empty) {
            return Err(SourceError::Other(format!("source {id}: empty command")));
        }
        Ok(Self {
            // Source ids are `&'static str`; a handful of configured
            // sources live for the whole process.
            id: String::leak(id.to_owned()),
            command,
        })
    }

    fn spawn(&self, verb: &str) -> Result<tokio::process::Child, SourceError> {
        Command::new(&self.command[0])
            .args(&self.command[1..])
            .arg(verb)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| {
                SourceError::Other(format!(
                    "source {}: cannot run {:?}: {e}",
                    self.id, self.command[0]
                ))
            })
    }

    /// Run `verb` with `request`; `Ok(None)` if the command does not
    /// implement it.
    async fn call(&self, verb: &str, request: &Value) -> Result<Option<Value>, SourceError> {
        let mut child = self.spawn(verb)?;
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(request.to_string().as_bytes()).await;
        }
        let out = tokio::time::timeout(TIMEOUT, child.wait_with_output())
            .await
            .map_err(|_| SourceError::Other(format!("source {}: `{verb}` timed out", self.id)))?
            .map_err(|e| SourceError::Other(format!("source {}: {e}", self.id)))?;
        if out.status.code() == Some(UNSUPPORTED_EXIT) {
            return Ok(None);
        }
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            let first = stderr.lines().next().unwrap_or("").trim();
            return Err(SourceError::Other(format!(
                "source {}: `{verb}` failed ({}){}",
                self.id,
                out.status,
                if first.is_empty() {
                    String::new()
                } else {
                    format!(": {first}")
                }
            )));
        }
        let value: Value = serde_json::from_slice(&out.stdout).map_err(|e| SourceError::Parse {
            what: format!("source {} `{verb}` output", self.id),
            detail: e.to_string(),
        })?;
        if value.get("unsupported").and_then(Value::as_bool) == Some(true) {
            return Ok(None);
        }
        if let Some(e) = value.get("error").and_then(Value::as_str) {
            return Err(SourceError::Other(format!("source {}: {e}", self.id)));
        }
        Ok(Some(value))
    }

    fn parse<T: for<'de> Deserialize<'de>>(&self, verb: &str, v: Value) -> Result<T, SourceError> {
        serde_json::from_value(v).map_err(|e| SourceError::Parse {
            what: format!("source {} `{verb}` output", self.id),
            detail: e.to_string(),
        })
    }
}

fn station_json(st: &Station) -> Value {
    json!({ "id": st.id, "name": st.name, "url": st.url, "params": st.params })
}

#[async_trait]
impl Source for ExecSource {
    fn id(&self) -> &'static str {
        self.id
    }

    fn capabilities(&self) -> SourceCaps {
        SourceCaps {
            browse: true,
            search: true,
            metadata: true,
        }
    }

    async fn list(&self, q: &ListQuery, _ctx: &ResolveCtx) -> Result<Vec<Station>, SourceError> {
        let request = json!({ "search": q.search, "limit": q.limit });
        let Some(v) = self.call("list", &request).await? else {
            return Ok(Vec::new());
        };
        let out: ListOut = self.parse("list", v)?;
        Ok(out
            .stations
            .into_iter()
            .filter(|s| !s.id.is_empty())
            .map(|s| Station {
                name: if s.name.is_empty() {
                    s.id.clone()
                } else {
                    s.name
                },
                id: s.id,
                source: self.id.to_owned(),
                url: s.url,
                params: s.params,
            })
            .collect())
    }

    /// `resolve`; if the command does not implement it, the station's own
    /// `url` (if any) is played by the resolver chain.
    async fn discover(
        &self,
        st: &Station,
        _ctx: &ResolveCtx,
    ) -> Result<Option<StreamDescriptor>, SourceError> {
        let request = json!({ "station": station_json(st) });
        let Some(v) = self.call("resolve", &request).await? else {
            return Ok(None);
        };
        let out: ResolveOut = self.parse("resolve", v)?;
        if out.urls.is_empty() {
            return Err(SourceError::Other(format!(
                "source {}: no stream URL for {}",
                self.id, st.id
            )));
        }
        Ok(Some(StreamDescriptor {
            urls: out.urls,
            headers: out.headers.into_iter().collect(),
            ttl: out.ttl.map(Duration::from_secs),
            ..StreamDescriptor::default()
        }))
    }

    fn metadata(&self, st: &Station, _ctx: &ResolveCtx) -> Option<Box<dyn MetadataProvider>> {
        Some(Box::new(ExecNowPlaying {
            source: self.clone(),
            station: st.clone(),
        }))
    }
}

/// Runs `command now` and turns its JSON lines into now-playing updates;
/// the process is killed when the station changes (receiver dropped).
struct ExecNowPlaying {
    source: ExecSource,
    station: Station,
}

#[async_trait]
impl MetadataProvider for ExecNowPlaying {
    async fn run(self: Box<Self>, tx: Sender<NowPlaying>) -> Result<(), SourceError> {
        let mut child = self.source.spawn("now")?;
        if let Some(mut stdin) = child.stdin.take() {
            let request = json!({ "station": station_json(&self.station) });
            let _ = stdin.write_all(request.to_string().as_bytes()).await;
        }
        let Some(stdout) = child.stdout.take() else {
            return Ok(());
        };
        // Drain stderr into the log: an unread pipe fills up (~64 KiB) and
        // blocks a chatty script, silently stopping its updates.
        if let Some(stderr) = child.stderr.take() {
            let id = self.source.id;
            tokio::spawn(async move {
                let mut lines = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    tracing::debug!(source = id, line, "exec now stderr");
                }
            });
        }
        let mut lines = BufReader::new(stdout).lines();
        loop {
            tokio::select! {
                () = tx.closed() => return Ok(()), // child killed on drop
                line = lines.next_line() => {
                    let Ok(Some(line)) = line else {
                        return Ok(()); // ended (or "unsupported")
                    };
                    let Ok(v) = serde_json::from_str::<Value>(&line) else {
                        tracing::debug!(line, "exec now: not JSON");
                        continue;
                    };
                    if v.get("unsupported").and_then(Value::as_bool) == Some(true) {
                        return Ok(());
                    }
                    let Ok(n) = serde_json::from_value::<NowOut>(v) else {
                        continue;
                    };
                    let np = NowPlaying {
                        station: Some(self.station.name.clone()),
                        artist: n.artist,
                        title: n.title,
                        programme: n.programme,
                        album: n.album,
                        artwork_url: n.artwork_url,
                        ..NowPlaying::empty(InfoSource::External, SystemTime::now())
                    };
                    if tx.send(np).await.is_err() {
                        return Ok(());
                    }
                }
            }
        }
    }
}
