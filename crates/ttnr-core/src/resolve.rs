//! The resolver chain shared by all sources (ARCHITECTURE.md, "Stream
//! resolution and refresh"): cache → discovery → patterns → override.

use std::sync::Arc;
use std::time::SystemTime;

use crate::{Http, ResolveCache, ResolverTrace, Source, Station, Strategy, StreamDescriptor};

/// Everything a source needs while resolving: HTTP, the cache, a clock.
#[derive(Clone)]
pub struct ResolveCtx {
    /// HTTP client.
    pub http: Arc<dyn Http>,
    /// Cache of last good URLs.
    pub cache: Arc<ResolveCache>,
    clock: Arc<dyn Fn() -> SystemTime + Send + Sync>,
}

impl ResolveCtx {
    /// A context using the system clock.
    pub fn new(http: Arc<dyn Http>, cache: Arc<ResolveCache>) -> Self {
        Self::with_clock(http, cache, Arc::new(SystemTime::now))
    }

    /// A context with an injected clock (tests).
    pub fn with_clock(
        http: Arc<dyn Http>,
        cache: Arc<ResolveCache>,
        clock: Arc<dyn Fn() -> SystemTime + Send + Sync>,
    ) -> Self {
        Self { http, cache, clock }
    }

    /// Current time according to the context's clock.
    pub fn now(&self) -> SystemTime {
        (self.clock)()
    }
}

impl std::fmt::Debug for ResolveCtx {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResolveCtx").finish_non_exhaustive()
    }
}

/// Use the cache, or bypass it (after a 403/404 or `ttnr refresh`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Cache first.
    Normal,
    /// Drop the cached entry and re-discover.
    Refresh,
}

/// Resolution failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ResolveError {
    /// No source with this id is registered.
    #[error("unknown source `{0}`")]
    UnknownSource(String),
    /// Every strategy failed.
    #[error("cannot resolve {station}: {trace}")]
    Exhausted {
        /// Station key.
        station: String,
        /// What was tried.
        trace: ResolverTrace,
    },
}

/// Cache key of a station: its key plus its own URL, if any, so editing a
/// station's `url` in `stations.toml` never replays the old result.
pub fn cache_key(st: &Station) -> String {
    match &st.url {
        Some(url) => format!("{} {url}", st.key()),
        None => st.key(),
    }
}

/// Resolve `st` through the chain. Successful discovery/pattern results are
/// cached; the user override is the last resort and never cached.
pub async fn resolve_chain(
    source: &dyn Source,
    st: &Station,
    ctx: &ResolveCtx,
    mode: Mode,
) -> Result<StreamDescriptor, ResolveError> {
    let key = cache_key(st);
    let mut trace = ResolverTrace::default();
    let done = |mut d: StreamDescriptor, trace: ResolverTrace| {
        d.trace = trace;
        tracing::debug!(station = %key, trace = %d.trace, "resolved");
        Ok(d)
    };

    match mode {
        Mode::Normal => match ctx.cache.get_fresh(&key, ctx.now()) {
            Some(d) => {
                trace.push(Strategy::Cache, "hit");
                return done(d, trace);
            }
            None => trace.push(Strategy::Cache, "miss"),
        },
        Mode::Refresh => {
            ctx.cache.remove(&key);
            trace.push(Strategy::Cache, "bypassed (refresh)");
        }
    }

    match source.discover(st, ctx).await {
        Ok(Some(mut d)) if !d.urls.is_empty() => {
            d.resolved_at = Some(ctx.now());
            ctx.cache.put(&key, &d, Strategy::Discovery, ctx.now());
            trace.push(Strategy::Discovery, "ok");
            return done(d, trace);
        }
        Ok(Some(_)) => trace.push(Strategy::Discovery, "no URLs"),
        Ok(None) => trace.push(Strategy::Discovery, "not available"),
        Err(e) => trace.push(Strategy::Discovery, format!("failed: {e}")),
    }

    for mut d in source.patterns(st) {
        let Some(url) = d.urls.first().cloned() else {
            continue;
        };
        match ctx.http.probe(&url, &d.headers).await {
            Ok(status) if (200..300).contains(&status) => {
                d.resolved_at = Some(ctx.now());
                ctx.cache.put(&key, &d, Strategy::Patterns, ctx.now());
                trace.push(Strategy::Patterns, format!("ok {url}"));
                return done(d, trace);
            }
            Ok(status) => trace.push(Strategy::Patterns, format!("HTTP {status} {url}")),
            Err(e) => trace.push(Strategy::Patterns, format!("{e} {url}")),
        }
    }

    if let Some(url) = &st.url {
        trace.push(Strategy::Override, format!("using {url}"));
        let d = StreamDescriptor {
            resolved_at: Some(ctx.now()),
            ..StreamDescriptor::direct(url.clone())
        };
        return done(d, trace);
    }
    trace.push(Strategy::Override, "none set");

    Err(ResolveError::Exhausted {
        station: key,
        trace,
    })
}
