//! Engine events from a real mpv core: local WAV files over `ao=null` and
//! HTTP errors from a throw-away server on 127.0.0.1 (no external network).

// Test code: unwrap/expect allowed (CLAUDE.md conventions).
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::io::{Read, Write};
use std::net::TcpListener;
use std::time::{Duration, Instant};

use common::{fixture, fixture_secs, null_player, single_core};
use ttnr_core::StreamDescriptor;
use ttnr_engine::{EngineEvent, EngineEvents, ErrorKind, Player};

/// Collect events until `stop` matches one (inclusive) or 10 s pass.
fn collect_until(rx: &mut EngineEvents, stop: impl Fn(&EngineEvent) -> bool) -> Vec<EngineEvent> {
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut seen = Vec::new();
    loop {
        match rx.try_recv() {
            Ok(ev) => {
                let done = stop(&ev);
                seen.push(ev);
                if done {
                    return seen;
                }
            }
            Err(_) => {
                assert!(Instant::now() < deadline, "timed out; got {seen:?}");
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }
}

/// Serve every request on a loopback port with `status_line` (e.g.
/// `"403 Forbidden"`) and an empty body. Returns the base URL.
fn http_status_server(status_line: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut s) = stream else { continue };
            let mut buf = [0u8; 4096];
            let _ = s.read(&mut buf);
            let _ = write!(
                s,
                "HTTP/1.1 {status_line}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            );
        }
    });
    format!("http://{addr}")
}

fn error_kind(events: &[EngineEvent]) -> Option<ErrorKind> {
    events.iter().find_map(|e| match e {
        EngineEvent::Error { kind, .. } => Some(*kind),
        _ => None,
    })
}

#[test]
fn local_file_loading_playing_eof() {
    let _core = single_core();
    let (_dir, path) = fixture_secs(1);
    let mut p = null_player();
    let mut rx = p.take_events().unwrap();
    assert!(p.take_events().is_none(), "receiver is handed out once");
    p.play(&StreamDescriptor::direct(path.to_str().unwrap()))
        .unwrap();
    let seen = collect_until(&mut rx, |e| *e == EngineEvent::Eof);
    assert_eq!(
        seen,
        [EngineEvent::Loading, EngineEvent::Playing, EngineEvent::Eof]
    );
}

#[test]
fn pause_and_stop_events() {
    let _core = single_core();
    let (_dir, path) = fixture();
    let mut p = null_player();
    let mut rx = p.take_events().unwrap();
    p.play(&StreamDescriptor::direct(path.to_str().unwrap()))
        .unwrap();
    collect_until(&mut rx, |e| *e == EngineEvent::Playing);
    p.pause(true).unwrap();
    collect_until(&mut rx, |e| *e == EngineEvent::Paused(true));
    p.pause(false).unwrap();
    collect_until(&mut rx, |e| *e == EngineEvent::Paused(false));
    p.stop().unwrap();
    collect_until(&mut rx, |e| *e == EngineEvent::Stopped);
}

#[test]
fn http_403_and_404_are_classified() {
    let _core = single_core();
    for (status, kind) in [
        ("403 Forbidden", ErrorKind::Http403),
        ("404 Not Found", ErrorKind::Http404),
        ("503 Service Unavailable", ErrorKind::Http(503)),
    ] {
        let base = http_status_server(status);
        let mut p = null_player();
        let mut rx = p.take_events().unwrap();
        p.play(&StreamDescriptor::direct(format!("{base}/live.mp3")))
            .unwrap();
        let seen = collect_until(&mut rx, |e| matches!(e, EngineEvent::Error { .. }));
        assert_eq!(error_kind(&seen), Some(kind), "{status}: {seen:?}");
    }
}

#[test]
fn hls_playlist_403_is_classified() {
    let _core = single_core();
    let base = http_status_server("403 Forbidden");
    let mut p = null_player();
    let mut rx = p.take_events().unwrap();
    p.play(&StreamDescriptor::direct(format!("{base}/live.m3u8")))
        .unwrap();
    let seen = collect_until(&mut rx, |e| matches!(e, EngineEvent::Error { .. }));
    assert_eq!(error_kind(&seen), Some(ErrorKind::Http403), "{seen:?}");
}

#[test]
fn connection_refused_is_unreachable() {
    let _core = single_core();
    // Bind then drop to get a port that is (almost certainly) closed.
    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let mut p = null_player();
    let mut rx = p.take_events().unwrap();
    p.play(&StreamDescriptor::direct(format!(
        "http://127.0.0.1:{port}/x.mp3"
    )))
    .unwrap();
    let seen = collect_until(&mut rx, |e| matches!(e, EngineEvent::Error { .. }));
    assert_eq!(error_kind(&seen), Some(ErrorKind::Unreachable), "{seen:?}");
}

#[test]
fn drop_shuts_down_and_closes_channel() {
    let _core = single_core();
    let mut p = null_player();
    let mut rx = p.take_events().unwrap();
    drop(p);
    let seen = collect_until(&mut rx, |e| *e == EngineEvent::Shutdown);
    assert_eq!(seen.last(), Some(&EngineEvent::Shutdown));
    assert!(
        rx.blocking_recv().is_none(),
        "sender dropped after shutdown"
    );
}
