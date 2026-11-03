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

// --- now playing -------------------------------------------------------

use ttnr_core::{InfoSource, MetadataProvider};
use ttnr_sources::bbc::{BbcNowPlaying, programme_from_broadcasts, track_from_segments};

/// The recorded 6 Music segment, with `now_playing` switched on.
fn playing_segment() -> Vec<u8> {
    String::from_utf8(fixture("segments_6music.json"))
        .unwrap()
        .replace("\"now_playing\": false", "\"now_playing\": true")
        .into_bytes()
}

#[test]
fn track_parsing() {
    // Recorded just after a track ended: no current track.
    assert_eq!(
        track_from_segments(&fixture("segments_6music.json")).unwrap(),
        None
    );
    let (artist, title, art) = track_from_segments(&playing_segment()).unwrap().unwrap();
    assert_eq!(
        (artist.as_str(), title.as_str()),
        ("twisted teens", "When We First Met")
    );
    assert_eq!(
        art.as_deref(),
        Some("https://ichef.bbci.co.uk/images/ic/320x320/p0pk0t3q.jpg")
    );
    // Speech programme (recorded on Radio 4): empty data.
    assert_eq!(
        track_from_segments(&fixture("segments_radio4.json")).unwrap(),
        None
    );
}

#[test]
fn programme_parsing() {
    assert_eq!(
        programme_from_broadcasts(&fixture("broadcast_6music.json"))
            .unwrap()
            .as_deref(),
        Some("Chris Hawkins – The biggest tunes to start the day")
    );
    assert!(
        programme_from_broadcasts(&fixture("broadcast_radio4.json"))
            .unwrap()
            .unwrap()
            .starts_with("Farming Today – 03/11/26")
    );
    assert_eq!(programme_from_broadcasts(br#"{"data":[]}"#).unwrap(), None);
}

/// Now-playing endpoints; segment answers are consumed in order (the last
/// repeats); `fail` makes the next N requests fail.
struct FakeNow {
    segments: Mutex<Vec<Vec<u8>>>,
    fail: Mutex<u32>,
    calls: Mutex<u32>,
}

#[async_trait]
impl Http for FakeNow {
    async fn get(&self, url: &str, _: &[(String, String)]) -> Result<HttpResponse, HttpError> {
        *self.calls.lock().unwrap() += 1;
        {
            let mut fail = self.fail.lock().unwrap();
            if *fail > 0 {
                *fail -= 1;
                return Err(HttpError("timed out".into()));
            }
        }
        let body = if url.contains("/broadcasts/latest") {
            fixture("broadcast_6music.json")
        } else {
            let mut s = self.segments.lock().unwrap();
            if s.len() > 1 {
                s.remove(0)
            } else {
                s[0].clone()
            }
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

#[tokio::test(start_paused = true)]
async fn provider_reports_changes_and_speech() {
    let http = Arc::new(FakeNow {
        segments: Mutex::new(vec![
            playing_segment(),
            playing_segment(),
            fixture("segments_radio4.json"),
        ]),
        fail: Mutex::new(0),
        calls: Mutex::new(0),
    });
    let mut st = station("bbc_6music", None);
    st.name = "BBC Radio 6 Music".into();
    let (tx, mut rx) = tokio::sync::mpsc::channel(8);
    let task = tokio::spawn(Box::new(BbcNowPlaying::new(&st, http.clone())).run(tx));

    let first = rx.recv().await.unwrap();
    assert_eq!(first.source, InfoSource::External);
    assert_eq!(first.station.as_deref(), Some("BBC Radio 6 Music"));
    assert_eq!(
        first.programme.as_deref(),
        Some("Chris Hawkins – The biggest tunes to start the day")
    );
    assert_eq!(first.title.as_deref(), Some("When We First Met"));
    // The second poll repeats the track (not re-sent); the third is speech.
    let second = rx.recv().await.unwrap();
    assert_eq!((second.artist, second.title), (None, None));
    assert!(second.programme.is_some(), "programme stays");
    drop(rx);
    tokio::time::advance(std::time::Duration::from_secs(120)).await;
    task.await.unwrap().unwrap();
}

#[tokio::test(start_paused = true)]
async fn provider_backs_off_and_recovers() {
    let http = Arc::new(FakeNow {
        segments: Mutex::new(vec![playing_segment()]),
        fail: Mutex::new(3),
        calls: Mutex::new(0),
    });
    let (tx, mut rx) = tokio::sync::mpsc::channel(8);
    let start = tokio::time::Instant::now();
    let task =
        tokio::spawn(Box::new(BbcNowPlaying::new(&station("bbc_6music", None), http)).run(tx));
    let np = rx.recv().await.unwrap();
    assert_eq!(np.title.as_deref(), Some("When We First Met"));
    // 3 failures (poll interval 10 s, doubled): 20 + 40 + 80 s before the
    // successful poll.
    assert!(
        start.elapsed() >= std::time::Duration::from_secs(140),
        "{:?}",
        start.elapsed()
    );
    drop(rx);
    tokio::time::advance(std::time::Duration::from_secs(600)).await;
    task.await.unwrap().unwrap();
}

#[test]
fn rfc3339_parsing() {
    use std::time::{Duration, UNIX_EPOCH};
    use ttnr_sources::bbc::parse_rfc3339;
    let t = |s| parse_rfc3339(s).map(|t| t.duration_since(UNIX_EPOCH).unwrap());
    // 2026-11-03T05:00:00Z = 1793682000 (date -u -j -f … +%s)
    assert_eq!(
        t("2026-11-03T05:00:00Z"),
        Some(Duration::from_secs(1_793_682_000))
    );
    assert_eq!(
        t("2026-11-03T06:00:00+01:00"),
        Some(Duration::from_secs(1_793_682_000))
    );
    assert_eq!(
        t("2026-11-03T04:30:00-00:30"),
        Some(Duration::from_secs(1_793_682_000))
    );
    assert_eq!(
        t("2026-11-03T05:00:06.400Z"),
        Some(Duration::from_millis(1_793_682_006_400))
    );
    assert_eq!(
        t("2024-02-29T00:00:00Z"),
        Some(Duration::from_secs(1_709_164_800))
    );
    assert_eq!(t("2026-13-03T05:00:00Z"), None);
    assert_eq!(t("yesterday"), None);
}

#[tokio::test(start_paused = true)]
async fn provider_times_updates_by_track_start() {
    use std::time::{Duration, UNIX_EPOCH};
    let http = Arc::new(FakeNow {
        segments: Mutex::new(vec![playing_segment(), fixture("segments_6music.json")]),
        fail: Mutex::new(0),
        calls: Mutex::new(0),
    });
    let (tx, mut rx) = tokio::sync::mpsc::channel(8);
    let task =
        tokio::spawn(Box::new(BbcNowPlaying::new(&station("bbc_6music", None), http)).run(tx));
    // Broadcast started 05:00:00Z; the track at offset 3286 s → 05:54:46Z.
    let playing = rx.recv().await.unwrap();
    assert_eq!(
        playing.at.duration_since(UNIX_EPOCH).unwrap(),
        Duration::from_secs(1_793_682_000 + 3286)
    );
    // The same segment, ended (now_playing false): valid from its end, 3399 s.
    let ended = rx.recv().await.unwrap();
    assert_eq!(ended.title, None);
    assert_eq!(
        ended.at.duration_since(UNIX_EPOCH).unwrap(),
        Duration::from_secs(1_793_682_000 + 3399)
    );
    drop(rx);
    tokio::time::advance(Duration::from_secs(60)).await;
    task.await.unwrap().unwrap();
}
