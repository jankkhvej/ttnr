//! Network smoke tests against the real BBC (run with
//! `cargo test -p ttnr-sources --test bbc_live -- --ignored`). They check
//! that the web-player flow still works from where you are.

// Test code: unwrap/expect allowed (CLAUDE.md conventions).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use ttnr_core::{ListQuery, ResolveCache, ResolveCtx, Source, SourceRegistry};
use ttnr_sources::{BbcSource, ReqwestHttp};

fn ctx() -> ResolveCtx {
    ResolveCtx::new(
        Arc::new(ReqwestHttp::new().unwrap()),
        Arc::new(ResolveCache::in_memory()),
    )
}

#[tokio::test]
#[ignore = "network: real BBC"]
async fn live_station_list() {
    let all = BbcSource.list(&ListQuery::default(), &ctx()).await.unwrap();
    assert!(
        all.iter().any(|s| s.id == "bbc_6music"),
        "{} stations",
        all.len()
    );
}

#[tokio::test]
#[ignore = "network: real BBC"]
async fn live_6music_resolves() {
    let ctx = ctx();
    let mut reg = SourceRegistry::new();
    reg.register(Arc::new(BbcSource));
    let all = BbcSource.list(&ListQuery::default(), &ctx).await.unwrap();
    let st = all.into_iter().find(|s| s.id == "bbc_6music").unwrap();
    let d = reg.resolve(&st, &ctx).await.unwrap();
    assert!(d.urls.iter().all(|u| u.ends_with(".m3u8")), "{:?}", d.urls);
    let status = ctx.http.probe(&d.urls[0], &[]).await.unwrap();
    assert_eq!(status, 200);
}
