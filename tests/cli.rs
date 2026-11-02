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
