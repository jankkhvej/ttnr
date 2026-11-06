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
    let lifecycle: Vec<_> = seen
        .iter()
        .filter(|e| {
            !matches!(
                e,
                EngineEvent::StreamInfo(_) | EngineEvent::Devices(_) | EngineEvent::Transfer { .. }
            )
        })
        .collect();
    assert_eq!(
        lifecycle,
        [
            &EngineEvent::Loading,
            &EngineEvent::Playing,
            &EngineEvent::Eof
        ]
    );
    let infos = seen
        .iter()
        .filter(|e| matches!(e, EngineEvent::StreamInfo(_)))
        .count();
    assert!(
        (1..=4).contains(&infos),
        "stream info should be coalesced, got {infos} events"
    );
    let Some(EngineEvent::StreamInfo(last)) = seen
        .iter()
        .rev()
        .find(|e| matches!(e, EngineEvent::StreamInfo(_)))
    else {
        unreachable!()
    };
    assert!(
        last.codec.is_some(),
        "info not cleared at end of file: {last:?}"
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

#[test]
fn stream_info_for_pcm_wav() {
    let _core = single_core();
    let (_dir, path) = fixture();
    let mut p = null_player();
    let mut rx = p.take_events().unwrap();
    p.play(&StreamDescriptor::direct(path.to_str().unwrap()))
        .unwrap();
    let complete = |e: &EngineEvent| {
        matches!(e, EngineEvent::StreamInfo(i)
            if i.codec.is_some() && i.sample_rate.is_some() && i.channels.is_some()
                && i.output_sample_rate.is_some() && i.bitrate.is_some())
    };
    let seen = collect_until(&mut rx, complete);
    let Some(EngineEvent::StreamInfo(info)) = seen.last() else {
        unreachable!()
    };
    assert_eq!(info.codec.as_deref(), Some("pcm_s16le"));
    assert_eq!(info.container.as_deref(), Some("wav"));
    assert_eq!(info.sample_rate, Some(48_000));
    assert_eq!(info.sample_format.as_deref(), Some("s16"));
    assert_eq!(info.bit_depth, Some(16));
    assert_eq!(info.channels.as_deref(), Some("stereo"));
    assert_eq!(info.channel_count, Some(2));
    assert_eq!(info.bitrate, Some(48_000 * 2 * 16));
    assert_eq!(info.output_sample_rate, Some(48_000));
    assert!(
        info.summary()
            .starts_with("PCM_S16LE · 48 kHz · 16-bit · stereo"),
        "{}",
        info.summary()
    );
}

/// One ICY metadata block: length byte (×16) + padded `StreamTitle='…';`.
fn icy_block(title: Option<&str>) -> Vec<u8> {
    let Some(title) = title else {
        return vec![0];
    };
    let mut text = format!("StreamTitle='{title}';").into_bytes();
    let blocks = text.len().div_ceil(16);
    text.resize(blocks * 16, 0);
    let mut out = vec![u8::try_from(blocks).unwrap()];
    out.extend(text);
    out
}

/// An Icecast-style server: MP3 frames with in-band titles every 8000
/// bytes (only if the client asked with `Icy-MetaData: 1`). Title A first,
/// then – after a pause so the player reads it – title B.
fn icy_server() -> String {
    let mp3 = std::fs::read(common::generated("icy_stream.mp3")).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut s) = stream else { continue };
            let mut buf = [0u8; 4096];
            let n = s.read(&mut buf).unwrap_or(0);
            let wants_icy = String::from_utf8_lossy(&buf[..n])
                .to_ascii_lowercase()
                .contains("icy-metadata: 1");
            let metaint = 8000;
            let head = if wants_icy {
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: audio/mpeg\r\nicy-name: Test FM\r\nicy-metaint: {metaint}\r\n\r\n"
                )
            } else {
                "HTTP/1.1 200 OK\r\nContent-Type: audio/mpeg\r\nicy-name: Test FM\r\n\r\n"
                    .to_owned()
            };
            if s.write_all(head.as_bytes()).is_err() {
                continue;
            }
            for (i, chunk) in mp3.chunks(metaint).enumerate() {
                if s.write_all(chunk).is_err() {
                    break;
                }
                if !wants_icy || chunk.len() < metaint {
                    continue;
                }
                let title = match i {
                    0 => Some("Artist One - Song A"),
                    3 => Some("Artist Two - Song B"),
                    _ => None,
                };
                if i == 3 {
                    std::thread::sleep(Duration::from_millis(800));
                }
                if s.write_all(&icy_block(title)).is_err() {
                    break;
                }
            }
        }
    });
    format!("http://{addr}/live")
}

#[test]
fn icy_titles_become_tags() {
    let _core = single_core();
    let url = icy_server();
    let mut p = null_player();
    let mut rx = p.take_events().unwrap();
    p.play(&StreamDescriptor::direct(url)).unwrap();
    let title = |e: &EngineEvent, want: &str| matches!(e, EngineEvent::Tags(t) if t.icy_title.as_deref() == Some(want));
    let seen = collect_until(&mut rx, |e| title(e, "Artist One - Song A"));
    let Some(EngineEvent::Tags(tags)) = seen.last() else {
        unreachable!()
    };
    assert_eq!(tags.icy_name.as_deref(), Some("Test FM"));
    collect_until(&mut rx, |e| title(e, "Artist Two - Song B"));
}

/// mpv 0.42's curl backend stays off (ADR-049): its errors would bypass
/// the FFmpeg log lines the error kinds come from. mpv 0.41 has no such
/// option (nothing to check there).
#[test]
fn the_curl_backend_is_off() {
    let p = null_player();
    let version = p.property_string("mpv-version").unwrap_or_default();
    match p.property_string("curl-enabled") {
        Some(v) => assert_eq!(v, "no", "{version}"),
        None => assert!(version.contains("0.41"), "no curl-enabled in {version}"),
    }
}
