//! End-to-end tests of the `ttnr` binary. Audio goes to `ao=null`; HTTP
//! errors come from a throw-away server on 127.0.0.1 (no external network).
//! `XDG_STATE_HOME` points into a temp dir so logs never touch `$HOME`.

// Test code: unwrap/expect allowed (CLAUDE.md conventions).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::process::{Command, Output};

/// Write a 1-second 48 kHz stereo 16-bit silent WAV.
fn write_wav(path: &Path) {
    let (rate, channels, secs) = (48_000u32, 2u16, 1u32);
    let data_len = rate * u32::from(channels) * 2 * secs;
    let mut b = Vec::new();
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data_len).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&channels.to_le_bytes());
    b.extend_from_slice(&rate.to_le_bytes());
    b.extend_from_slice(&(rate * u32::from(channels) * 2).to_le_bytes());
    b.extend_from_slice(&(channels * 2).to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data_len.to_le_bytes());
    b.resize(b.len() + data_len as usize, 0);
    std::fs::write(path, b).unwrap();
}

fn ttnr(state: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ttnr"))
        .args(args)
        .env("XDG_STATE_HOME", state)
        .env("XDG_CACHE_HOME", state)
        .env("XDG_CONFIG_HOME", state)
        .env_remove("TTNR_LOG")
        .output()
        .unwrap()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn version_and_help() {
    let state = tempfile::tempdir().unwrap();
    let out = ttnr(state.path(), &["--version"]);
    assert!(out.status.success());
    assert_eq!(
        text(&out.stdout).trim(),
        format!("ttnr {}", env!("CARGO_PKG_VERSION"))
    );
    let out = ttnr(state.path(), &["play", "--help"]);
    assert!(out.status.success());
    assert!(text(&out.stdout).contains("--volume"));
    assert!(
        !text(&out.stdout).contains("--audio-output"),
        "test flag is hidden"
    );
}

#[test]
fn play_local_file_to_end() {
    let dir = tempfile::tempdir().unwrap();
    let wav = dir.path().join("silence.wav");
    write_wav(&wav);
    let out = ttnr(
        dir.path(),
        &[
            "play",
            wav.to_str().unwrap(),
            "--audio-output",
            "null",
            "--volume",
            "0",
        ],
    );
    let stdout = text(&out.stdout);
    assert!(
        out.status.success(),
        "stdout: {stdout}\nstderr: {}",
        text(&out.stderr)
    );
    let lines: Vec<&str> = stdout.lines().collect();
    assert!(lines[0].starts_with("ttnr: "), "{stdout}");
    for expected in ["[loading]", "[playing]", "[end of stream]"] {
        assert!(lines.contains(&expected), "missing {expected}: {stdout}");
    }
    assert!(
        lines
            .iter()
            .any(|l| l.starts_with("format: PCM_S16LE · 48 kHz · 16-bit · stereo")),
        "{stdout}"
    );

    // The log file went to $XDG_STATE_HOME/ttnr, not $HOME.
    let logs: Vec<_> = std::fs::read_dir(dir.path().join("ttnr"))
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    assert!(
        logs.iter()
            .any(|n| n.starts_with("ttnr.") && n.ends_with(".log")),
        "{logs:?}"
    );
}

#[test]
fn http_404_fails_with_message() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut s) = stream else { continue };
            let mut buf = [0u8; 4096];
            let _ = s.read(&mut buf);
            let _ = s.write_all(
                b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            );
        }
    });
    let state = tempfile::tempdir().unwrap();
    let url = format!("http://{addr}/gone.mp3");
    let out = ttnr(state.path(), &["play", &url, "--audio-output", "null"]);
    assert!(!out.status.success());
    let stderr = text(&out.stderr);
    assert!(stderr.contains("HTTP 404 Not Found"), "{stderr}");
}

#[test]
fn volume_out_of_range_is_rejected() {
    let state = tempfile::tempdir().unwrap();
    let out = ttnr(state.path(), &["play", "x", "--volume", "150"]);
    assert!(!out.status.success());
    assert!(text(&out.stderr).contains("between 0 and 100"));
}

/// A PLS whose first stream is gone (404) and whose second plays: ttnr
/// expands the playlist and falls over to the second stream.
#[test]
fn playlist_falls_over_to_the_next_stream() {
    let dir = tempfile::tempdir().unwrap();
    let addr = playlist_server(dir.path());
    let url = format!("http://{addr}/list.pls");
    let out = ttnr(dir.path(), &["play", &url, "--audio-output", "null"]);
    let stdout = text(&out.stdout);
    assert!(
        out.status.success(),
        "stdout: {stdout}\nstderr: {}",
        text(&out.stderr)
    );
    for expected in [
        "playlist: 2 streams",
        "[HTTP 404 Not Found; trying the next stream]",
        "[end of stream]",
    ] {
        assert!(
            stdout.contains(expected),
            "missing {expected:?} in:\n{stdout}"
        );
    }
    assert!(
        stdout.contains(&format!("stream: http://{addr}/ok.wav")),
        "{stdout}"
    );
    // The expansion was cached under $XDG_CACHE_HOME/ttnr.
    assert!(dir.path().join("ttnr/resolve.json").is_file());
}

/// Write `$XDG_CONFIG_HOME/ttnr/stations.toml` in `dir`.
fn write_stations(dir: &Path, toml: &str) {
    std::fs::create_dir_all(dir.join("ttnr")).unwrap();
    std::fs::write(dir.join("ttnr/stations.toml"), toml).unwrap();
}

#[test]
fn stations_list_and_search() {
    let dir = tempfile::tempdir().unwrap();
    let out = ttnr(dir.path(), &["stations"]);
    assert!(out.status.success());
    assert!(
        text(&out.stdout).starts_with("no stations"),
        "{}",
        text(&out.stdout)
    );

    write_stations(
        dir.path(),
        "[[station]]\nid='gs'\nname='Groove Salad'\nsource='url'\nurl='https://somafm.com/groovesalad.pls'\n\
         [[station]]\nid='bbc_6music'\nname='BBC Radio 6 Music'\nsource='bbc'\n",
    );
    let out = ttnr(dir.path(), &["stations"]);
    let stdout = text(&out.stdout);
    assert_eq!(
        stdout.lines().collect::<Vec<_>>(),
        [
            "gs          url  Groove Salad",
            "bbc_6music  bbc  BBC Radio 6 Music"
        ],
        "{stdout}"
    );
    let out = ttnr(dir.path(), &["stations", "--search", "radio 6"]);
    assert_eq!(text(&out.stdout).lines().count(), 1);
}

#[test]
fn play_by_station_id() {
    let dir = tempfile::tempdir().unwrap();
    let wav = dir.path().join("silence.wav");
    write_wav(&wav);
    write_stations(
        dir.path(),
        &format!(
            "[[station]]\nid='local'\nname='Local Silence'\nsource='url'\nurl='{}'\n\
             [[station]]\nid='bbc_6music'\nname='BBC Radio 6 Music'\nsource='bbc'\n",
            wav.display()
        ),
    );
    let out = ttnr(dir.path(), &["play", "local", "--audio-output", "null"]);
    let stdout = text(&out.stdout);
    assert!(out.status.success(), "{stdout}{}", text(&out.stderr));
    assert!(stdout.starts_with("ttnr: Local Silence\n"), "{stdout}");
    assert!(stdout.contains("[end of stream]"), "{stdout}");

    let out = ttnr(
        dir.path(),
        &["play", "bbc_6music", "--audio-output", "null"],
    );
    assert!(!out.status.success());
    assert!(
        text(&out.stderr).contains("needs the `bbc` source"),
        "{}",
        text(&out.stderr)
    );

    let out = ttnr(dir.path(), &["play", "nope", "--audio-output", "null"]);
    assert!(!out.status.success());
    assert!(
        text(&out.stderr).contains("unknown station \"nope\"; known: local, bbc_6music"),
        "{}",
        text(&out.stderr)
    );
}

#[test]
fn broken_stations_file_is_reported() {
    let dir = tempfile::tempdir().unwrap();
    write_stations(dir.path(), "[[station]]\nid='a'\nname='A'\nsource='url'\n");
    let out = ttnr(dir.path(), &["stations"]);
    assert!(!out.status.success());
    let stderr = text(&out.stderr);
    assert!(
        stderr.contains("stations.toml") && stderr.contains("needs a url"),
        "{stderr}"
    );
}

/// Loopback server: `/list.pls` lists `/gone` (404) then `/ok.wav` (a
/// 1-second WAV); everything else is 404. Returns the address.
fn playlist_server(dir: &Path) -> std::net::SocketAddr {
    let wav_path = dir.join("served.wav");
    write_wav(&wav_path);
    let wav = std::fs::read(&wav_path).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut s) = stream else { continue };
            let mut buf = [0u8; 4096];
            let n = s.read(&mut buf).unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]);
            let path = req.split_whitespace().nth(1).unwrap_or("/").to_owned();
            let (head, body): (String, Vec<u8>) = match path.as_str() {
                "/list.pls" => {
                    let pls = format!(
                        "[playlist]\nFile1=http://{addr}/gone\nFile2=http://{addr}/ok.wav\n"
                    );
                    (
                        format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: audio/x-scpls\r\nContent-Length: {}\r\n",
                            pls.len()
                        ),
                        pls.into_bytes(),
                    )
                }
                "/ok.wav" => (
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: audio/x-wav\r\nContent-Length: {}\r\n",
                        wav.len()
                    ),
                    wav.clone(),
                ),
                _ => (
                    "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n".to_owned(),
                    Vec::new(),
                ),
            };
            let _ = s.write_all(format!("{head}Connection: close\r\n\r\n").as_bytes());
            let _ = s.write_all(&body);
        }
    });
    addr
}

/// A stale cached URL (the stream "moved"): playback hits 404, re-resolves
/// the station once, plays the fresh URL, and the cache is healed.
#[test]
fn stale_cache_self_heals() {
    let dir = tempfile::tempdir().unwrap();
    let addr = playlist_server(dir.path());
    write_stations(
        dir.path(),
        &format!(
            "[[station]]\nid='loop'\nname='Loop FM'\nsource='url'\nurl='http://{addr}/list.pls'\n"
        ),
    );
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    std::fs::write(
        dir.path().join("ttnr/resolve.json"),
        format!(
            r#"{{"url:loop": {{"urls": ["http://{addr}/moved-away"], "resolved_at": {now}, "ttl_secs": 3600, "strategy": "discovery"}}}}"#
        ),
    )
    .unwrap();

    let out = ttnr(dir.path(), &["play", "loop", "--audio-output", "null"]);
    let stdout = text(&out.stdout);
    assert!(
        out.status.success(),
        "stdout: {stdout}\nstderr: {}",
        text(&out.stderr)
    );
    let wanted = [
        "stream: http://".to_owned() + &addr.to_string() + "/moved-away",
        "[HTTP 404 Not Found; re-resolving the station]".to_owned(),
        "[HTTP 404 Not Found; trying the next stream]".to_owned(),
        format!("stream: http://{addr}/ok.wav"),
        "[end of stream]".to_owned(),
    ];
    let mut rest = stdout.as_str();
    for w in &wanted {
        let i = rest
            .find(w.as_str())
            .unwrap_or_else(|| panic!("missing/out of order {w:?} in:\n{stdout}"));
        rest = &rest[i + w.len()..];
    }
    let cache = std::fs::read_to_string(dir.path().join("ttnr/resolve.json")).unwrap();
    assert!(
        !cache.contains("moved-away") && cache.contains("/ok.wav"),
        "{cache}"
    );
}

#[test]
fn refresh_command() {
    let dir = tempfile::tempdir().unwrap();
    let addr = playlist_server(dir.path());
    write_stations(
        dir.path(),
        &format!(
            "[[station]]\nid='loop'\nname='Loop FM'\nsource='url'\nurl='http://{addr}/list.pls'\n\
             [[station]]\nid='bbc_6music'\nname='BBC Radio 6 Music'\nsource='bbc'\n"
        ),
    );
    let out = ttnr(dir.path(), &["refresh", "loop"]);
    let stdout = text(&out.stdout);
    assert!(out.status.success(), "{stdout}{}", text(&out.stderr));
    assert!(
        stdout.starts_with(&format!(
            "loop: http://{addr}/gone | http://{addr}/ok.wav\n"
        )),
        "{stdout}"
    );
    assert!(
        stdout.contains("Cache: bypassed (refresh) → Discovery: ok"),
        "{stdout}"
    );

    let out = ttnr(dir.path(), &["refresh"]);
    assert!(!out.status.success());
    let stdout = text(&out.stdout);
    assert!(
        stdout.contains("loop: http://")
            && stdout.contains("bbc_6music: error: unknown source `bbc`"),
        "{stdout}"
    );
    assert!(
        text(&out.stderr).contains("1 of 2 station(s)"),
        "{}",
        text(&out.stderr)
    );
}

/// M2 acceptance, offline: a `.pls` URL whose stream carries ICY titles
/// shows them as `now:` lines.
#[test]
fn pls_with_icy_titles() {
    let mp3 = std::fs::read(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/generated/icy_stream.mp3"),
    )
    .expect("run scripts/gen-fixtures.zsh");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut s) = stream else { continue };
            let mut buf = [0u8; 4096];
            let n = s.read(&mut buf).unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]).to_ascii_lowercase();
            if req.starts_with("get /radio.pls") {
                let pls = format!("[playlist]\nFile1=http://{addr}/live\n");
                let _ = write!(
                    s,
                    "HTTP/1.1 200 OK\r\nContent-Type: audio/x-scpls\r\nContent-Length: {}\r\n\r\n{pls}",
                    pls.len()
                );
                continue;
            }
            let icy = req.contains("icy-metadata: 1");
            let _ = write!(
                s,
                "HTTP/1.1 200 OK\r\nContent-Type: audio/mpeg\r\nicy-name: Loop FM\r\n{}\r\n",
                if icy { "icy-metaint: 8000\r\n" } else { "" }
            );
            // ~3 s of audio, then close (end of stream).
            for (i, chunk) in mp3.chunks(8000).take(3).enumerate() {
                let _ = s.write_all(chunk);
                if icy {
                    let block = if i == 0 {
                        let mut t = b"StreamTitle='Artist One - Song A';".to_vec();
                        let n = t.len().div_ceil(16);
                        t.resize(n * 16, 0);
                        std::iter::once(u8::try_from(n).unwrap()).chain(t).collect()
                    } else {
                        vec![0]
                    };
                    let _ = s.write_all(&block);
                }
            }
        }
    });
    let state = tempfile::tempdir().unwrap();
    let url = format!("http://{addr}/radio.pls");
    let out = ttnr(state.path(), &["play", &url, "--audio-output", "null"]);
    let stdout = text(&out.stdout);
    assert!(out.status.success(), "{stdout}{}", text(&out.stderr));
    assert!(stdout.contains("now: Artist One – Song A"), "{stdout}");
}
