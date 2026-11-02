//! Resolver chain and registry against a fake HTTP client and a scripted
//! source (no network).

// Test code: unwrap/expect allowed (CLAUDE.md conventions).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, UNIX_EPOCH};

use async_trait::async_trait;
use ttnr_core::{
    Http, HttpError, HttpResponse, ResolveCache, ResolveCtx, ResolveError, Source, SourceCaps,
    SourceError, SourceRegistry, Station, Strategy, StreamDescriptor,
};

/// Probe results by URL; unknown URLs are a transport error.
#[derive(Default)]
struct FakeHttp {
    probes: HashMap<String, u16>,
    probed: Mutex<Vec<String>>,
}

#[async_trait]
impl Http for FakeHttp {
    async fn get(&self, url: &str, _: &[(String, String)]) -> Result<HttpResponse, HttpError> {
        Err(HttpError(format!("unexpected GET {url}")))
    }
    async fn probe(&self, url: &str, _: &[(String, String)]) -> Result<u16, HttpError> {
        self.probed.lock().unwrap().push(url.to_owned());
        self.probes
            .get(url)
            .copied()
            .ok_or_else(|| HttpError(format!("connection refused {url}")))
    }
}

/// A source whose discovery answer can be changed between calls.
struct Scripted {
    discovery: Mutex<Result<Option<StreamDescriptor>, SourceError>>,
    patterns: Vec<StreamDescriptor>,
    discover_calls: AtomicUsize,
}

impl Scripted {
    fn new(discovery: Result<Option<StreamDescriptor>, SourceError>) -> Self {
        Self {
            discovery: Mutex::new(discovery),
            patterns: Vec::new(),
            discover_calls: AtomicUsize::new(0),
        }
    }
    fn set_discovery(&self, d: Result<Option<StreamDescriptor>, SourceError>) {
        *self.discovery.lock().unwrap() = d;
    }
}

#[async_trait]
impl Source for Scripted {
    fn id(&self) -> &'static str {
        "test"
    }
    fn capabilities(&self) -> SourceCaps {
        SourceCaps::default()
    }
    async fn discover(
        &self,
        _: &Station,
        _: &ResolveCtx,
    ) -> Result<Option<StreamDescriptor>, SourceError> {
        self.discover_calls.fetch_add(1, Ordering::SeqCst);
        self.discovery.lock().unwrap().clone()
    }
    fn patterns(&self, _: &Station) -> Vec<StreamDescriptor> {
        self.patterns.clone()
    }
}

fn station(url: Option<&str>) -> Station {
    Station {
        id: "s1".into(),
        name: "Station One".into(),
        source: "test".into(),
        url: url.map(Into::into),
        params: BTreeMap::new(),
    }
}

/// Context with a fake clock (seconds since epoch, settable).
fn ctx(http: FakeHttp) -> (ResolveCtx, Arc<AtomicU64>, Arc<FakeHttp>) {
    let secs = Arc::new(AtomicU64::new(1_000_000));
    let clock_secs = Arc::clone(&secs);
    let http = Arc::new(http);
    let ctx = ResolveCtx::with_clock(
        http.clone(),
        Arc::new(ResolveCache::in_memory()),
        Arc::new(move || UNIX_EPOCH + Duration::from_secs(clock_secs.load(Ordering::SeqCst))),
    );
    (ctx, secs, http)
}

fn registry(source: Arc<Scripted>) -> SourceRegistry {
    let mut r = SourceRegistry::new();
    r.register(source);
    r
}

fn strategies(d: &StreamDescriptor) -> Vec<(Strategy, String)> {
    d.trace
        .0
        .iter()
        .map(|s| (s.strategy, s.outcome.clone()))
        .collect()
}

#[tokio::test]
async fn discovery_result_is_cached() {
    let src = Arc::new(Scripted::new(Ok(Some(StreamDescriptor::direct(
        "http://a",
    )))));
    let r = registry(src.clone());
    let (ctx, _, _) = ctx(FakeHttp::default());
    let st = station(None);

    let d = r.resolve(&st, &ctx).await.unwrap();
    assert_eq!(d.urls, ["http://a"]);
    assert_eq!(
        strategies(&d),
        [
            (Strategy::Cache, "miss".into()),
            (Strategy::Discovery, "ok".into())
        ]
    );
    assert!(d.resolved_at.is_some());

    let again = r.resolve(&st, &ctx).await.unwrap();
    assert_eq!(again.urls, ["http://a"]);
    assert_eq!(strategies(&again), [(Strategy::Cache, "hit".into())]);
    assert_eq!(src.discover_calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn expired_cache_rediscovers() {
    let src = Arc::new(Scripted::new(Ok(Some(StreamDescriptor {
        ttl: Some(Duration::from_secs(60)),
        ..StreamDescriptor::direct("http://a")
    }))));
    let r = registry(src.clone());
    let (ctx, secs, _) = ctx(FakeHttp::default());
    let st = station(None);
    r.resolve(&st, &ctx).await.unwrap();
    secs.fetch_add(61, Ordering::SeqCst);
    let d = r.resolve(&st, &ctx).await.unwrap();
    assert_eq!(d.trace.0[0].outcome, "miss");
    assert_eq!(src.discover_calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn patterns_are_probed_in_order_after_failed_discovery() {
    let mut src = Scripted::new(Err(SourceError::Other("endpoint down".into())));
    src.patterns = vec![
        StreamDescriptor::direct("http://p1"),
        StreamDescriptor::direct("http://p2"),
        StreamDescriptor::direct("http://p3"),
    ];
    let src = Arc::new(src);
    let r = registry(src.clone());
    let mut http = FakeHttp::default();
    http.probes.insert("http://p1".into(), 404);
    http.probes.insert("http://p2".into(), 200);
    http.probes.insert("http://p3".into(), 200);
    let (ctx, _, http) = ctx(http);

    let d = r.resolve(&station(None), &ctx).await.unwrap();
    assert_eq!(d.urls, ["http://p2"]);
    assert_eq!(
        strategies(&d),
        [
            (Strategy::Cache, "miss".into()),
            (Strategy::Discovery, "failed: endpoint down".into()),
            (Strategy::Patterns, "HTTP 404 http://p1".into()),
            (Strategy::Patterns, "ok http://p2".into()),
        ]
    );
    assert_eq!(*http.probed.lock().unwrap(), ["http://p1", "http://p2"]);
    // The pattern hit is cached.
    let again = r.resolve(&station(None), &ctx).await.unwrap();
    assert_eq!(again.urls, ["http://p2"]);
}

#[tokio::test]
async fn override_is_last_resort_and_not_cached() {
    let src = Arc::new(Scripted::new(Ok(None)));
    let r = registry(src.clone());
    let (ctx, _, _) = ctx(FakeHttp::default());
    let st = station(Some("http://pinned"));

    let d = r.resolve(&st, &ctx).await.unwrap();
    assert_eq!(d.urls, ["http://pinned"]);
    assert_eq!(d.trace.0.last().unwrap().strategy, Strategy::Override);

    // Discovery starts working: it wins, because the override was not cached.
    src.set_discovery(Ok(Some(StreamDescriptor::direct("http://found"))));
    let d = r.resolve(&st, &ctx).await.unwrap();
    assert_eq!(d.urls, ["http://found"]);
}

#[tokio::test]
async fn exhausted_chain_reports_the_trace() {
    let src = Arc::new(Scripted::new(Ok(Some(StreamDescriptor::default()))));
    let r = registry(src);
    let (ctx, _, _) = ctx(FakeHttp::default());
    let err = r.resolve(&station(None), &ctx).await.unwrap_err();
    let ResolveError::Exhausted { station, trace } = &err else {
        panic!("{err:?}")
    };
    assert_eq!(station, "test:s1");
    let outcomes: Vec<_> = trace.0.iter().map(|s| s.outcome.as_str()).collect();
    assert_eq!(outcomes, ["miss", "no URLs", "none set"]);
    assert!(err.to_string().contains("Discovery: no URLs"), "{err}");
}

#[tokio::test]
async fn refresh_replaces_a_stale_cached_url() {
    let src = Arc::new(Scripted::new(Ok(Some(StreamDescriptor::direct(
        "http://old",
    )))));
    let r = registry(src.clone());
    let (ctx, _, _) = ctx(FakeHttp::default());
    let st = station(None);
    assert_eq!(r.resolve(&st, &ctx).await.unwrap().urls, ["http://old"]);

    // The stream moved (engine reported 403/404): refresh re-discovers…
    src.set_discovery(Ok(Some(StreamDescriptor::direct("http://new"))));
    assert_eq!(r.resolve(&st, &ctx).await.unwrap().urls, ["http://old"]);
    let d = r.refresh(&st, &ctx).await.unwrap();
    assert_eq!(d.urls, ["http://new"]);
    assert_eq!(d.trace.0[0].outcome, "bypassed (refresh)");
    // …and the cache now holds the new URL.
    assert_eq!(r.resolve(&st, &ctx).await.unwrap().urls, ["http://new"]);
}

#[tokio::test]
async fn failed_refresh_does_not_resurrect_the_stale_url() {
    let src = Arc::new(Scripted::new(Ok(Some(StreamDescriptor::direct(
        "http://old",
    )))));
    let r = registry(src.clone());
    let (ctx, _, _) = ctx(FakeHttp::default());
    let st = station(None);
    r.resolve(&st, &ctx).await.unwrap();
    src.set_discovery(Err(SourceError::Other("down".into())));
    assert!(r.refresh(&st, &ctx).await.is_err());
    assert!(
        r.resolve(&st, &ctx).await.is_err(),
        "known-bad URL must not come back from the cache"
    );
}

#[tokio::test]
async fn unknown_source_is_an_error() {
    let r = SourceRegistry::new();
    let (ctx, _, _) = ctx(FakeHttp::default());
    let err = r.resolve(&station(None), &ctx).await.unwrap_err();
    assert_eq!(err, ResolveError::UnknownSource("test".into()));
}

#[test]
fn registry_lists_ids() {
    let r = registry(Arc::new(Scripted::new(Ok(None))));
    assert_eq!(r.ids().collect::<Vec<_>>(), ["test"]);
    assert!(r.get("test").is_some());
}
