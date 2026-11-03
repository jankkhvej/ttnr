//! The `radio-browser` source against answers recorded on 2026-11-03
//! (tests/fixtures/radio-browser). No network.

// Test code: unwrap/expect allowed (CLAUDE.md conventions).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use ttnr_core::{
    Http, HttpError, HttpResponse, ListQuery, ResolveCache, ResolveCtx, Source, Station,
};
use ttnr_sources::RadioBrowserSource;
use ttnr_sources::radio_browser::{API, stations_from_search};

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/radio-browser")
            .join(name),
    )
    .unwrap()
}

/// Routes by URL and records every request.
struct FakeRb {
    click_ok: bool,
    seen: Mutex<Vec<String>>,
}

impl FakeRb {
    fn new(click_ok: bool) -> Arc<Self> {
        Arc::new(Self {
            click_ok,
            seen: Mutex::new(Vec::new()),
        })
    }
}

#[async_trait]
impl Http for FakeRb {
    async fn get(&self, url: &str, _: &[(String, String)]) -> Result<HttpResponse, HttpError> {
        self.seen.lock().unwrap().push(url.to_owned());
        let json = |status, body: Vec<u8>| HttpResponse {
            status,
            final_url: url.to_owned(),
            content_type: Some("application/json".into()),
            body,
            streaming: false,
        };
        if url.starts_with(&format!("{API}/json/stations/search")) {
            let body = if url.contains("name=groove") {
                fixture("search_groove_salad.json")
            } else {
                fixture("search_empty.json")
            };
            return Ok(json(200, body));
        }
        if url.starts_with(&format!("{API}/json/url/")) {
            return if self.click_ok {
                Ok(json(200, fixture("url_click.json")))
            } else {
                Err(HttpError("connection refused".into()))
            };
        }
        if url.starts_with("https://ice") {
            // A live MP3 stream: headers only.
            return Ok(HttpResponse {
                status: 200,
                final_url: url.to_owned(),
                content_type: Some("audio/mpeg".into()),
                body: Vec::new(),
                streaming: true,
            });
        }
        Err(HttpError(format!("no route {url}")))
    }

    async fn probe(&self, _: &str, _: &[(String, String)]) -> Result<u16, HttpError> {
        Ok(200)
    }
}

fn ctx(http: Arc<FakeRb>) -> ResolveCtx {
    ResolveCtx::new(http, Arc::new(ResolveCache::in_memory()))
}

#[tokio::test]
async fn search_lists_stations_with_details() {
    let http = FakeRb::new(true);
    let list = RadioBrowserSource
        .list(
            &ListQuery {
                search: Some("groove salad".into()),
                limit: Some(5),
            },
            &ctx(http.clone()),
        )
        .await
        .unwrap();
    assert_eq!(list.len(), 5);
    let first = &list[0];
    assert_eq!(first.source, "radio-browser");
    assert_eq!(first.id, "960cf833-0601-11e8-ae97-52543be04c81");
    assert_eq!(first.name, "SomaFM Groove Salad (128k MP3)");
    assert_eq!(first.params["detail"], "MP3 128k US");
    assert_eq!(
        first.params["url_resolved"],
        "https://ice2.somafm.com/groovesalad-128-mp3"
    );
    assert!(
        list[2].params["detail"].contains("320k HLS"),
        "bit/s entry normalised"
    );
    let empty = RadioBrowserSource
        .list(
            &ListQuery {
                search: Some("zzzzqqqxx".into()),
                limit: None,
            },
            &ctx(http),
        )
        .await
        .unwrap();
    assert!(empty.is_empty());
}

#[tokio::test]
async fn playing_counts_a_click_and_plays_the_resolved_url() {
    let http = FakeRb::new(true);
    let st = stations_from_search(&fixture("search_groove_salad.json")).unwrap()[0].clone();
    let d = RadioBrowserSource
        .discover(&st, &ctx(http.clone()))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(d.urls, ["https://ice2.somafm.com/groovesalad-128-mp3"]);
    let seen = http.seen.lock().unwrap().clone();
    assert_eq!(
        seen[0],
        format!("{API}/json/url/960cf833-0601-11e8-ae97-52543be04c81"),
        "click counted first"
    );
}

#[tokio::test]
async fn a_failed_click_does_not_stop_playback() {
    let http = FakeRb::new(false);
    let st = stations_from_search(&fixture("search_groove_salad.json")).unwrap()[0].clone();
    let d = RadioBrowserSource
        .discover(&st, &ctx(http))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(d.urls, ["https://ice2.somafm.com/groovesalad-128-mp3"]);
}

#[tokio::test]
async fn a_station_known_by_uuid_only_uses_the_click_url() {
    let st = RadioBrowserSource::station("960cf833-0601-11e8-ae97-52543be04c81", Some("GS"));
    let d = RadioBrowserSource
        .discover(&st, &ctx(FakeRb::new(true)))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(d.urls, ["https://ice2.somafm.com/groovesalad-128-mp3"]);
    let err = RadioBrowserSource
        .discover(
            &Station {
                ..RadioBrowserSource::station("dead", None)
            },
            &ctx(FakeRb::new(false)),
        )
        .await
        .unwrap_err();
    assert!(err.to_string().contains("no stream URL"), "{err}");
}

#[tokio::test]
async fn the_fresh_click_url_wins_over_a_stale_search_result() {
    let mut st = stations_from_search(&fixture("search_groove_salad.json")).unwrap()[0].clone();
    st.params.insert(
        "url_resolved".into(),
        "https://ice1.somafm.com/moved-away".into(),
    );
    let d = RadioBrowserSource
        .discover(&st, &ctx(FakeRb::new(true)))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        d.urls,
        ["https://ice2.somafm.com/groovesalad-128-mp3"],
        "from /json/url"
    );
}
