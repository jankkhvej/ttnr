//! The `bbc` source against responses recorded from Kraków on 2026-11-03
//! (tests/fixtures/bbc; the token is a fake). No network.

// Test code: unwrap/expect allowed (CLAUDE.md conventions).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use ttnr_core::{
    Http, HttpError, HttpResponse, ListQuery, ResolveCache, ResolveCtx, ResolveError, Source,
    SourceRegistry, Station, Strategy,
};
use ttnr_sources::BbcSource;
use ttnr_sources::bbc::{stations_from_networks, streams_from_selection};

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/bbc")
            .join(name),
    )
    .unwrap()
}

const TOKEN_URL: &str = "https://rms.api.bbc.co.uk/v2/sign/token/";
const MS_URL: &str = "https://open.live.bbc.co.uk/mediaselector/6/select/version/3.0/mediaset/pc/cvid/urn:bbc:pips:pid:";
const FAKE_TOKEN: &str = "eyJ0ZXN0IjoidG9rZW4ifQ.fake.token";

/// Routes by URL; media selector answers only with the right bearer token.
#[derive(Default)]
struct FakeBbc {
    selections: HashMap<String, (u16, Vec<u8>)>,
    seen_auth: Mutex<Vec<String>>,
}

#[async_trait]
impl Http for FakeBbc {
    async fn get(
        &self,
        url: &str,
        headers: &[(String, String)],
    ) -> Result<HttpResponse, HttpError> {
        let ok = |status, body: Vec<u8>| HttpResponse {
            status,
            final_url: url.to_owned(),
            content_type: Some("application/json".into()),
            body,
            streaming: false,
        };
        if url.starts_with(TOKEN_URL) {
            return Ok(ok(
                200,
                format!(r#"{{"token":"{FAKE_TOKEN}"}}"#).into_bytes(),
            ));
        }
        if let Some(rest) = url.strip_prefix(MS_URL) {
            let service = rest.trim_end_matches("/format/json");
            let auth = headers
                .iter()
                .find(|(k, _)| k == "Authorization")
                .map(|(_, v)| v.clone())
                .unwrap_or_default();
            self.seen_auth.lock().unwrap().push(auth.clone());
            if auth != format!("Bearer {FAKE_TOKEN}") {
                return Ok(ok(404, fixture("mediaselector_unavailable.json")));
            }
            let (status, body) = self.selections.get(service).cloned().unwrap();
            return Ok(ok(status, body));
        }
        if url.starts_with("https://rms.api.bbc.co.uk/v2/networks") {
            return Ok(ok(200, fixture("networks.json")));
        }
        Err(HttpError(format!("no route {url}")))
    }
    async fn probe(&self, url: &str, _: &[(String, String)]) -> Result<u16, HttpError> {
        Err(HttpError(format!(
            "BBC must never probe guessed URLs: {url}"
        )))
    }
}

fn setup() -> (SourceRegistry, ResolveCtx, Arc<FakeBbc>) {
    let mut http = FakeBbc::default();
    http.selections.insert(
        "bbc_6music".into(),
        (200, fixture("mediaselector_6music.json")),
    );
    http.selections.insert(
        "bbc_radio_five_live_sports_extra".into(),
        (403, fixture("mediaselector_geolocation.json")),
    );
    let http = Arc::new(http);
    let ctx = ResolveCtx::new(http.clone(), Arc::new(ResolveCache::in_memory()));
    let mut reg = SourceRegistry::new();
    reg.register(Arc::new(BbcSource));
    (reg, ctx, http)
}

fn station(id: &str, url: Option<&str>) -> Station {
    Station {
        id: id.into(),
        name: id.into(),
        source: "bbc".into(),
        url: url.map(Into::into),
        params: Default::default(),
    }
}

#[test]
fn station_list_from_networks() {
    let stations = stations_from_networks(&fixture("networks.json")).unwrap();
    let by_id = |id: &str| stations.iter().find(|s| s.id == id).unwrap();
    assert_eq!(by_id("bbc_6music").name, "BBC Radio 6 Music");
    assert_eq!(by_id("bbc_radio_fourfm").name, "BBC Radio 4 FM");
    assert_eq!(by_id("bbc_radio_fourlw").name, "BBC Radio 4 LW");
    assert_eq!(by_id("bbc_radio_scotland_fm").name, "BBC Radio Scotland FM");
    assert_eq!(by_id("bbc_6music").params["coverage"], "national");
    assert!(stations.iter().all(|s| s.source == "bbc"));
    assert!(
        !stations.iter().any(|s| s.id == "bbc_radio_four"),
        "network ids are not streamable"
    );
    assert!(stations.len() > 66, "one station per service");
}

#[test]
fn stream_choice() {
    let urls = streams_from_selection(&fixture("mediaselector_6music.json"), "bbc_6music").unwrap();
    assert_eq!(urls.len(), 2, "one per CDN supplier: {urls:?}");
    assert!(
        urls.iter()
            .all(|u| u.starts_with("https://") && u.ends_with(".m3u8"))
    );
    assert!(urls.iter().all(|u| u.contains("/hls/nonuk/")));
    assert_ne!(urls[0], urls[1]);
}

#[test]
fn refusals_are_clear() {
    let e = streams_from_selection(&fixture("mediaselector_geolocation.json"), "x").unwrap_err();
    assert!(
        e.to_string().contains("not offer it in your location"),
        "{e}"
    );
    let e = streams_from_selection(&fixture("mediaselector_unavailable.json"), "x").unwrap_err();
    assert!(e.to_string().contains("does not offer it here"), "{e}");
}

#[tokio::test]
async fn resolves_like_the_web_player() {
    let (reg, ctx, http) = setup();
    let d = reg
        .resolve(&station("bbc_6music", None), &ctx)
        .await
        .unwrap();
    assert_eq!(d.urls.len(), 2);
    assert_eq!(d.trace.0.last().unwrap().strategy, Strategy::Discovery);
    assert_eq!(
        *http.seen_auth.lock().unwrap(),
        [format!("Bearer {FAKE_TOKEN}")]
    );
    // Cached: no second token round-trip.
    reg.resolve(&station("bbc_6music", None), &ctx)
        .await
        .unwrap();
    assert_eq!(http.seen_auth.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn refusal_is_final_without_override() {
    let (reg, ctx, _) = setup();
    let err = reg
        .resolve(&station("bbc_radio_five_live_sports_extra", None), &ctx)
        .await
        .unwrap_err();
    let ResolveError::Exhausted { trace, .. } = &err else {
        panic!("{err:?}")
    };
    let steps: Vec<_> = trace.0.iter().map(|s| s.strategy).collect();
    assert_eq!(
        steps,
        [Strategy::Cache, Strategy::Discovery, Strategy::Override],
        "no patterns"
    );
    assert!(err.to_string().contains("rights restriction"), "{err}");
}

#[tokio::test]
async fn user_override_is_the_last_resort() {
    let (reg, ctx, _) = setup();
    let st = station(
        "bbc_radio_five_live_sports_extra",
        Some("https://example.org/mine.m3u8"),
    );
    let d = reg.resolve(&st, &ctx).await.unwrap();
    assert_eq!(d.urls, ["https://example.org/mine.m3u8"]);
}

#[tokio::test]
async fn list_with_search() {
    let (_, ctx, _) = setup();
    let q = ListQuery {
        search: Some("6 music".into()),
        limit: None,
    };
    let found = BbcSource.list(&q, &ctx).await.unwrap();
    assert_eq!(
        found.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
        ["bbc_6music"]
    );
}
