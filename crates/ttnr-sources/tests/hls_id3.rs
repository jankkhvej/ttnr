//! ID3 reading on a real recorded segment head, and the HLS ID3 poller
//! against a scripted HTTP client with virtual time (no network).

// Test code: unwrap/expect allowed (CLAUDE.md conventions).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use tokio::sync::mpsc;
use ttnr_core::{Http, HttpError, HttpResponse, InfoSource, MetadataProvider};
use ttnr_sources::HlsId3Provider;
use ttnr_sources::id3::read_tags;

fn fixture() -> Vec<u8> {
    std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/id3/streams_packed_aac.id3"),
    )
    .unwrap()
}

/// ID3 head of a live StreamS-encoded HLS segment (recorded 2026-11-02 from
/// a public test stream; only the two ID3 tags, no audio).
#[test]
fn real_segment_head() {
    let info = read_tags(&fixture());
    assert_eq!(info.title.as_deref(), Some("Take A Little Rhythm"));
    assert_eq!(info.artist.as_deref(), Some("Ali Thomson"));
    assert_eq!(info.album.as_deref(), Some("Take A Little Rhythm"));
    assert_eq!(info.station.as_deref(), Some("StreamS LIVE - 5"));
    assert_eq!(
        info.artwork_url.as_deref(),
        Some("https://la2.indexcom.com/img/rdj/5/alithomson.jpg")
    );
}

/// Responses per URL, consumed in order; the last one repeats.
#[derive(Default)]
struct Scripted {
    routes: Mutex<HashMap<String, VecDeque<Vec<u8>>>>,
    ranges: Mutex<Vec<String>>,
}

impl Scripted {
    fn route(&self, url: &str, bodies: Vec<Vec<u8>>) {
        self.routes
            .lock()
            .unwrap()
            .insert(url.to_owned(), bodies.into());
    }
}

#[async_trait]
impl Http for Scripted {
    async fn get(
        &self,
        url: &str,
        headers: &[(String, String)],
    ) -> Result<HttpResponse, HttpError> {
        if let Some((_, v)) = headers.iter().find(|(k, _)| k == "Range") {
            self.ranges.lock().unwrap().push(v.clone());
        }
        let mut routes = self.routes.lock().unwrap();
        let queue = routes
            .get_mut(url)
            .ok_or_else(|| HttpError(format!("no route {url}")))?;
        let body = if queue.len() > 1 {
            queue.pop_front().unwrap()
        } else {
            queue[0].clone()
        };
        Ok(HttpResponse {
            status: 200,
            final_url: url.to_owned(),
            content_type: None,
            body,
            streaming: false,
        })
    }
    async fn probe(&self, _: &str, _: &[(String, String)]) -> Result<u16, HttpError> {
        Ok(200)
    }
}

fn media(segments: &[&str]) -> Vec<u8> {
    let mut m = "#EXTM3U\n#EXT-X-TARGETDURATION:10\n".to_owned();
    for s in segments {
        m.push_str(&format!("#EXTINF:10,\n{s}\n"));
    }
    m.into_bytes()
}

/// A minimal ID3v2.3 tag with TIT2/TPE1 (Latin-1).
fn id3(artist: &str, title: &str) -> Vec<u8> {
    let mut body = Vec::new();
    for (id, text) in [(b"TIT2", title), (b"TPE1", artist)] {
        let data: Vec<u8> = std::iter::once(0).chain(text.bytes()).collect();
        body.extend_from_slice(id);
        body.extend_from_slice(&u32::try_from(data.len()).unwrap().to_be_bytes());
        body.extend_from_slice(&[0, 0]);
        body.extend(data);
    }
    let n = body.len();
    let mut out = vec![b'I', b'D', b'3', 3, 0, 0];
    out.extend(
        [
            (n >> 21) & 0x7F,
            (n >> 14) & 0x7F,
            (n >> 7) & 0x7F,
            n & 0x7F,
        ]
        .map(|b| b as u8),
    );
    out.extend(body);
    out
}

#[tokio::test(start_paused = true)]
async fn polls_newest_segment_and_reports_changes() {
    let http = Arc::new(Scripted::default());
    http.route(
        "https://h/live.m3u8",
        vec![b"#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=64000\nhi/index.m3u8\n".to_vec()],
    );
    http.route(
        "https://h/hi/index.m3u8",
        vec![
            media(&["s1.aac", "s2.aac"]),
            media(&["s2.aac", "s3.aac"]),
            media(&["s3.aac", "s4.aac"]),
        ],
    );
    http.route("https://h/hi/s2.aac", vec![id3("Artist A", "Song A")]);
    http.route("https://h/hi/s3.aac", vec![id3("Artist A", "Song A")]);
    http.route("https://h/hi/s4.aac", vec![id3("Artist B", "Song B")]);

    let (tx, mut rx) = mpsc::channel(8);
    let provider = Box::new(HlsId3Provider::new("https://h/live.m3u8", http.clone()));
    let task = tokio::spawn(provider.run(tx));

    let first = rx.recv().await.unwrap();
    assert_eq!(first.source, InfoSource::Tags);
    assert_eq!(
        (first.artist.as_deref(), first.title.as_deref()),
        (Some("Artist A"), Some("Song A"))
    );
    let second = rx.recv().await.unwrap();
    assert_eq!(
        second.title.as_deref(),
        Some("Song B"),
        "unchanged s3 was not re-sent"
    );
    assert!(
        http.ranges
            .lock()
            .unwrap()
            .iter()
            .all(|r| r == "bytes=0-8191")
    );

    drop(rx);
    tokio::time::advance(std::time::Duration::from_secs(60)).await;
    task.await.unwrap().unwrap();
}

#[tokio::test(start_paused = true)]
async fn stops_when_segments_have_no_id3() {
    let http = Arc::new(Scripted::default());
    http.route(
        "https://h/a.m3u8",
        vec![
            media(&["1.ts"]),
            media(&["2.ts"]),
            media(&["3.ts"]),
            media(&["4.ts"]),
        ],
    );
    for s in ["1", "2", "3", "4"] {
        http.route(
            &format!("https://h/{s}.ts"),
            vec![b"\x47\x40\x00plain TS".to_vec()],
        );
    }
    let (tx, _rx) = mpsc::channel(8);
    let provider = Box::new(HlsId3Provider::new("https://h/a.m3u8", http));
    provider.run(tx).await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn non_hls_url_ends_quietly() {
    let http = Arc::new(Scripted::default());
    http.route("https://h/x.m3u8", vec![b"http://a/stream\n".to_vec()]);
    let (tx, _rx) = mpsc::channel(8);
    Box::new(HlsId3Provider::new("https://h/x.m3u8", http))
        .run(tx)
        .await
        .unwrap();
}
