//! `ReqwestHttp` against throw-away servers on 127.0.0.1 (no external
//! network): playlists, endless streams, redirects, body cap, ICY status.

// Test code: unwrap/expect allowed (CLAUDE.md conventions).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::{Duration, Instant};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use ttnr_core::Http;
use ttnr_sources::ReqwestHttp;
use ttnr_sources::http::BODY_CAP;

/// What the server sends after reading a request.
#[derive(Clone)]
enum Reply {
    /// A complete response.
    Fixed(Vec<u8>),
    /// Headers, then body bytes forever (a live stream).
    Endless(&'static str),
}

/// Serve `reply_for(path)` on a loopback port; returns the base URL.
async fn serve(reply_for: fn(&str) -> Reply) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else {
                return;
            };
            tokio::spawn(async move {
                let mut buf = vec![0u8; 8192];
                let n = sock.read(&mut buf).await.unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]);
                let path = req.split_whitespace().nth(1).unwrap_or("/").to_owned();
                match reply_for(&path) {
                    Reply::Fixed(bytes) => {
                        let _ = sock.write_all(&bytes).await;
                    }
                    Reply::Endless(head) => {
                        if sock.write_all(head.as_bytes()).await.is_err() {
                            return;
                        }
                        let chunk = [0u8; 4096];
                        while sock.write_all(&chunk).await.is_ok() {}
                    }
                }
            });
        }
    });
    format!("http://{addr}")
}

/// A complete reply. The server answers one request per connection, so it
/// always says `Connection: close`; otherwise the client may reuse a
/// connection the server is closing (a rare "connection closed" failure).
fn fixed(status: &str, headers: &str, body: &[u8]) -> Reply {
    let mut r = format!(
        "HTTP/1.1 {status}\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
    r.extend_from_slice(body);
    Reply::Fixed(r)
}

fn ok(content_type: &str, body: &[u8]) -> Reply {
    fixed("200 OK", &format!("Content-Type: {content_type}\r\n"), body)
}

fn routes(path: &str) -> Reply {
    match path {
        "/list.pls" => ok("audio/x-scpls", b"[playlist]\nFile1=http://a/stream\n"),
        "/stream" => Reply::Endless(
            "HTTP/1.1 200 OK\r\nContent-Type: audio/mpeg\r\nicy-name: Test FM\r\n\r\n",
        ),
        "/icy-only" => Reply::Endless("HTTP/1.1 200 OK\r\nicy-metaint: 16000\r\n\r\n"),
        "/shoutcast-v1" => Reply::Endless("ICY 200 OK\r\nicy-name: Old FM\r\n\r\n"),
        "/moved" => fixed("302 Found", "Location: /list.pls\r\n", b""),
        "/big" => ok("text/plain", &vec![b'x'; BODY_CAP * 4]),
        "/segment.aac" => ok("audio/aac", b"ID3\x04\x00\x00\x00\x00\x00\x00audio"),
        _ => fixed("404 Not Found", "", b""),
    }
}

#[tokio::test]
async fn reads_a_playlist() {
    let base = serve(routes).await;
    let r = ReqwestHttp::new()
        .unwrap()
        .get(&format!("{base}/list.pls"), &[])
        .await
        .unwrap();
    assert_eq!(r.status, 200);
    assert_eq!(r.content_type.as_deref(), Some("audio/x-scpls"));
    assert!(!r.streaming);
    assert_eq!(r.body, b"[playlist]\nFile1=http://a/stream\n");
}

#[tokio::test]
async fn endless_streams_are_not_read() {
    let base = serve(routes).await;
    let http = ReqwestHttp::new().unwrap();
    for path in ["/stream", "/icy-only"] {
        let t0 = Instant::now();
        let r = http.get(&format!("{base}{path}"), &[]).await.unwrap();
        assert!(r.streaming, "{path}");
        assert!(r.body.is_empty(), "{path}");
        assert!(
            t0.elapsed() < Duration::from_secs(2),
            "{path} took {:?}",
            t0.elapsed()
        );
    }
}

#[tokio::test]
async fn follows_redirects() {
    let base = serve(routes).await;
    let r = ReqwestHttp::new()
        .unwrap()
        .get(&format!("{base}/moved"), &[])
        .await
        .unwrap();
    assert_eq!(r.status, 200);
    assert_eq!(r.final_url, format!("{base}/list.pls"));
}

#[tokio::test]
async fn caps_the_body() {
    let base = serve(routes).await;
    let r = ReqwestHttp::new()
        .unwrap()
        .get(&format!("{base}/big"), &[])
        .await
        .unwrap();
    assert_eq!(r.body.len(), BODY_CAP);
}

#[tokio::test]
async fn probe_reports_status_without_reading_streams() {
    let base = serve(routes).await;
    let http = ReqwestHttp::new().unwrap();
    assert_eq!(http.probe(&format!("{base}/nope"), &[]).await.unwrap(), 404);
    let t0 = Instant::now();
    assert_eq!(
        http.probe(&format!("{base}/stream"), &[]).await.unwrap(),
        200
    );
    assert!(t0.elapsed() < Duration::from_secs(2));
}

/// Shoutcast v1's "ICY 200 OK" status line is not HTTP: the client errors
/// (and `UrlSource` then hands the URL to mpv, which speaks ICY).
#[tokio::test]
async fn shoutcast_v1_status_line_is_an_error() {
    let base = serve(routes).await;
    let r = ReqwestHttp::new()
        .unwrap()
        .get(&format!("{base}/shoutcast-v1"), &[])
        .await;
    assert!(r.is_err(), "{r:?}");
}

/// A sized media response (e.g. an HLS segment) is not an endless stream.
#[tokio::test]
async fn sized_media_is_read() {
    let base = serve(routes).await;
    let r = ReqwestHttp::new()
        .unwrap()
        .get(&format!("{base}/segment.aac"), &[])
        .await
        .unwrap();
    assert!(!r.streaming);
    assert!(r.body.starts_with(b"ID3"));
}
