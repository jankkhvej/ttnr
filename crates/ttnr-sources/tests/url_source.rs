//! `UrlSource` through the registry's resolver chain with a scripted HTTP
//! client (no network).

// Test code: unwrap/expect allowed (CLAUDE.md conventions).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use ttnr_core::{
    Http, HttpError, HttpResponse, ResolveCache, ResolveCtx, SourceRegistry, Strategy,
};
use ttnr_sources::UrlSource;

#[derive(Default)]
struct FakeHttp {
    responses: HashMap<&'static str, Result<HttpResponse, HttpError>>,
}

#[async_trait]
impl Http for FakeHttp {
    async fn get(&self, url: &str, _: &[(String, String)]) -> Result<HttpResponse, HttpError> {
        self.responses
            .get(url)
            .cloned()
            .unwrap_or_else(|| Err(HttpError(format!("no route {url}"))))
    }
    async fn probe(&self, _: &str, _: &[(String, String)]) -> Result<u16, HttpError> {
        Ok(200)
    }
}

fn response(status: u16, ct: &str, body: &str, streaming: bool) -> HttpResponse {
    HttpResponse {
        status,
        final_url: String::new(),
        content_type: Some(ct.into()),
        body: body.as_bytes().to_vec(),
        streaming,
    }
}

async fn resolve(
    url: &'static str,
    resp: Result<HttpResponse, HttpError>,
) -> ttnr_core::StreamDescriptor {
    let mut http = FakeHttp::default();
    let resp = resp.map(|mut r| {
        if r.final_url.is_empty() {
            r.final_url = url.into();
        }
        r
    });
    http.responses.insert(url, resp);
    let ctx = ResolveCtx::new(Arc::new(http), Arc::new(ResolveCache::in_memory()));
    let mut reg = SourceRegistry::new();
    reg.register(Arc::new(UrlSource));
    reg.resolve(&UrlSource::station(url), &ctx).await.unwrap()
}

#[tokio::test]
async fn pls_expands_to_mirror_candidates() {
    let body = "[playlist]\nFile1=https://ice6.example/gs\nFile2=https://ice2.example/gs\n";
    let d = resolve(
        "https://example.org/gs.pls",
        Ok(response(200, "audio/x-scpls", body, false)),
    )
    .await;
    assert_eq!(
        d.urls,
        ["https://ice6.example/gs", "https://ice2.example/gs"]
    );
    assert_eq!(d.trace.0.last().unwrap().strategy, Strategy::Discovery);
}

#[tokio::test]
async fn hls_is_passed_through() {
    let body = "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=96000\nlow/index.m3u8\n";
    let d = resolve(
        "https://example.org/live.m3u8",
        Ok(response(200, "application/vnd.apple.mpegurl", body, false)),
    )
    .await;
    assert_eq!(d.urls, ["https://example.org/live.m3u8"]);
}

#[tokio::test]
async fn media_stream_is_played_directly() {
    let d = resolve(
        "http://example.org:8000/live",
        Ok(response(200, "audio/mpeg", "", true)),
    )
    .await;
    assert_eq!(d.urls, ["http://example.org:8000/live"]);
}

#[tokio::test]
async fn uninspectable_url_goes_to_the_player() {
    let d = resolve(
        "http://old.example/;",
        Err(HttpError("invalid HTTP version parsed".into())),
    )
    .await;
    assert_eq!(d.urls, ["http://old.example/;"]);
}

#[tokio::test]
async fn http_error_falls_back_to_the_url_itself() {
    let d = resolve(
        "https://example.org/gone.pls",
        Ok(response(404, "text/html", "<h1>404</h1>", false)),
    )
    .await;
    assert_eq!(d.urls, ["https://example.org/gone.pls"]);
    let outcomes: Vec<_> = d
        .trace
        .0
        .iter()
        .map(|s| (s.strategy, s.outcome.clone()))
        .collect();
    assert_eq!(
        outcomes[1],
        (
            Strategy::Discovery,
            "failed: HTTP 404 for https://example.org/gone.pls".into()
        )
    );
    assert_eq!(outcomes.last().unwrap().0, Strategy::Override);
}

#[tokio::test]
async fn local_files_are_not_fetched() {
    let d = resolve(
        "/Users/me/Music/a.flac",
        Err(HttpError("must not be called".into())),
    )
    .await;
    assert_eq!(d.urls, ["/Users/me/Music/a.flac"]);
}
