//! The `exec` source against shell scripts speaking the protocol
//! (tests/fixtures/exec). No network.

// Test code: unwrap/expect allowed (CLAUDE.md conventions).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use ttnr_core::{
    Http, HttpError, HttpResponse, ListQuery, ResolveCache, ResolveCtx, Source, Station,
};
use ttnr_sources::ExecSource;

fn script(name: &str) -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/exec")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

struct NoHttp;

#[async_trait]
impl Http for NoHttp {
    async fn get(&self, url: &str, _: &[(String, String)]) -> Result<HttpResponse, HttpError> {
        Err(HttpError(format!("exec tests use no HTTP: {url}")))
    }
    async fn probe(&self, url: &str, _: &[(String, String)]) -> Result<u16, HttpError> {
        Err(HttpError(format!("exec tests use no HTTP: {url}")))
    }
}

fn ctx() -> ResolveCtx {
    ResolveCtx::new(Arc::new(NoHttp), Arc::new(ResolveCache::in_memory()))
}

fn source(name: &str) -> ExecSource {
    ExecSource::new("fake", vec![script(name)]).unwrap()
}

fn st(id: &str) -> Station {
    Station {
        id: id.into(),
        name: id.into(),
        source: "fake".into(),
        url: None,
        params: Default::default(),
    }
}

#[tokio::test]
async fn list_and_search() {
    let s = source("fake_source.sh");
    let all = s.list(&ListQuery::default(), &ctx()).await.unwrap();
    let ids: Vec<_> = all.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["jazz", "news"], "empty ids skipped");
    assert!(all.iter().all(|s| s.source == "fake"));
    assert_eq!(all[1].url.as_deref(), Some("http://news.example/live"));
    let found = s
        .list(
            &ListQuery {
                search: Some("jazz".into()),
                limit: None,
            },
            &ctx(),
        )
        .await
        .unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].params["genre"], "jazz");
}

#[tokio::test]
async fn resolve_urls_headers_ttl_and_fallbacks() {
    let s = source("fake_source.sh");
    let d = s.discover(&st("jazz"), &ctx()).await.unwrap().unwrap();
    assert_eq!(d.urls, ["http://a.example/jazz", "http://b.example/jazz"]);
    assert_eq!(d.headers, [("X-Key".to_owned(), "k".to_owned())]);
    assert_eq!(d.ttl, Some(std::time::Duration::from_secs(60)));
    assert_eq!(
        s.discover(&st("other"), &ctx()).await.unwrap(),
        None,
        "unsupported → chain"
    );
    let err = s.discover(&st("empty"), &ctx()).await.unwrap_err();
    assert!(err.to_string().contains("no stream URL"), "{err}");
}

#[tokio::test]
async fn errors_carry_stderr_and_unsupported_is_quiet() {
    let s = source("broken_source.sh");
    let err = s.list(&ListQuery::default(), &ctx()).await.unwrap_err();
    assert!(
        err.to_string()
            .contains("cannot reach the station directory"),
        "{err}"
    );
    assert_eq!(s.discover(&st("x"), &ctx()).await.unwrap(), None, "exit 64");
    let missing = ExecSource::new("gone", vec!["/nonexistent/ttnr-source".into()]).unwrap();
    let err = missing
        .list(&ListQuery::default(), &ctx())
        .await
        .unwrap_err();
    assert!(err.to_string().contains("cannot run"), "{err}");
    assert!(ExecSource::new("empty", vec![]).is_err());
}

#[tokio::test]
async fn now_playing_lines_until_the_station_changes() {
    let s = source("fake_source.sh");
    let provider = s.metadata(&st("jazz"), &ctx()).unwrap();
    let (tx, mut rx) = tokio::sync::mpsc::channel(8);
    let task = tokio::spawn(provider.run(tx));
    let first = rx.recv().await.unwrap();
    assert_eq!(first.artist.as_deref(), Some("Miles Davis"));
    assert_eq!(first.title.as_deref(), Some("So What"));
    assert_eq!(first.programme.as_deref(), Some("Late Jazz"));
    assert_eq!(first.station.as_deref(), Some("jazz"));
    let second = rx.recv().await.unwrap();
    assert_eq!(
        second.title.as_deref(),
        Some("Blue in Green"),
        "non-JSON line skipped"
    );
    // Dropping the receiver stops the provider (and kills the script).
    drop(rx);
    tokio::time::timeout(std::time::Duration::from_secs(5), task)
        .await
        .expect("provider stops when the receiver is gone")
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn a_chatty_now_script_is_not_blocked_by_its_stderr() {
    let s = source("chatty_now.sh");
    let provider = s.metadata(&st("x"), &ctx()).unwrap();
    let (tx, mut rx) = tokio::sync::mpsc::channel(8);
    let task = tokio::spawn(provider.run(tx));
    let np = tokio::time::timeout(std::time::Duration::from_secs(10), rx.recv())
        .await
        .expect("title arrives despite ~120 KiB on stderr")
        .unwrap();
    assert_eq!(np.title.as_deref(), Some("After the noise"));
    drop(rx);
    let _ = tokio::time::timeout(std::time::Duration::from_secs(5), task).await;
}
