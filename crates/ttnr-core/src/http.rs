//! HTTP as seen by sources: a small trait so resolvers and providers can be
//! tested against recorded fixtures (CLAUDE.md: no network in unit tests).
//! The real client lives in `ttnr-sources`.

use async_trait::async_trait;

/// An HTTP response, fully read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    /// Status code.
    pub status: u16,
    /// URL after redirects.
    pub final_url: String,
    /// `Content-Type` header, if any.
    pub content_type: Option<String>,
    /// Body bytes, bounded by the client. Empty when `streaming`.
    pub body: Vec<u8>,
    /// The headers announced a continuous media stream (`audio/*`, `icy-*`
    /// headers, …), so the body was not read.
    pub streaming: bool,
}

impl HttpResponse {
    /// `true` for 2xx.
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }
}

/// Transport-level failure (no HTTP status available).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct HttpError(pub String);

/// Minimal async HTTP client.
#[async_trait]
pub trait Http: Send + Sync {
    /// `GET url` with extra headers. Implementations read at most a small
    /// cap of the body, and none of it for media streams (`streaming`).
    async fn get(&self, url: &str, headers: &[(String, String)])
    -> Result<HttpResponse, HttpError>;

    /// Check that `url` answers with 2xx without downloading a whole stream
    /// (e.g. a ranged or aborted `GET`). Returns the status code.
    async fn probe(&self, url: &str, headers: &[(String, String)]) -> Result<u16, HttpError>;
}

/// The last HTTP request made through an [`ObservedHttp`] (diagnostics).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRecord {
    /// Host of the URL.
    pub host: String,
    /// When it finished.
    pub at: std::time::Instant,
    /// `ok`, `HTTP <status>`, or `error`.
    pub outcome: String,
}

/// Shared view of the last request (cheap to clone).
#[derive(Debug, Clone, Default)]
pub struct HttpLog(std::sync::Arc<std::sync::Mutex<Option<HttpRecord>>>);

impl HttpLog {
    /// The last request, if any.
    pub fn last(&self) -> Option<HttpRecord> {
        self.0.lock().ok().and_then(|g| g.clone())
    }

    fn record(&self, url: &str, outcome: String) {
        let host = url
            .split("://")
            .nth(1)
            .and_then(|rest| rest.split(['/', '?', '#']).next())
            .unwrap_or(url)
            .to_owned();
        if let Ok(mut g) = self.0.lock() {
            *g = Some(HttpRecord {
                host,
                at: std::time::Instant::now(),
                outcome,
            });
        }
    }
}

/// An [`Http`] client that notes the outcome of every request in an
/// [`HttpLog`] (metadata polls, resolution) for the diagnostics line.
pub struct ObservedHttp {
    inner: std::sync::Arc<dyn Http>,
    log: HttpLog,
}

impl ObservedHttp {
    /// Wrap `inner`; returns the client and its log.
    pub fn new(inner: std::sync::Arc<dyn Http>) -> (Self, HttpLog) {
        let log = HttpLog::default();
        (
            Self {
                inner,
                log: log.clone(),
            },
            log,
        )
    }
}

fn outcome(status: Result<u16, &HttpError>) -> String {
    match status {
        Ok(s) if (200..300).contains(&s) => "ok".to_owned(),
        Ok(s) => format!("HTTP {s}"),
        Err(_) => "error".to_owned(),
    }
}

#[async_trait]
impl Http for ObservedHttp {
    async fn get(
        &self,
        url: &str,
        headers: &[(String, String)],
    ) -> Result<HttpResponse, HttpError> {
        let r = self.inner.get(url, headers).await;
        self.log
            .record(url, outcome(r.as_ref().map(|resp| resp.status)));
        r
    }

    async fn probe(&self, url: &str, headers: &[(String, String)]) -> Result<u16, HttpError> {
        let r = self.inner.probe(url, headers).await;
        self.log.record(url, outcome(r.as_ref().copied()));
        r
    }
}

#[cfg(test)]
mod observed_tests {
    use super::*;

    struct Fixed(Result<u16, HttpError>);

    #[async_trait]
    impl Http for Fixed {
        async fn get(&self, url: &str, _: &[(String, String)]) -> Result<HttpResponse, HttpError> {
            self.0.clone().map(|status| HttpResponse {
                status,
                final_url: url.to_owned(),
                content_type: None,
                body: Vec::new(),
                streaming: false,
            })
        }
        async fn probe(&self, _: &str, _: &[(String, String)]) -> Result<u16, HttpError> {
            self.0.clone()
        }
    }

    fn block<F: std::future::Future>(f: F) -> F::Output {
        // Tiny executor: the fakes never pend.
        let waker = std::task::Waker::noop();
        let mut cx = std::task::Context::from_waker(waker);
        let mut f = std::pin::pin!(f);
        loop {
            if let std::task::Poll::Ready(v) = f.as_mut().poll(&mut cx) {
                return v;
            }
        }
    }

    #[test]
    fn records_host_and_outcome() {
        let (http, log) = ObservedHttp::new(std::sync::Arc::new(Fixed(Ok(200))));
        assert_eq!(log.last(), None);
        let _ = block(http.get("https://rms.api.bbc.co.uk/v2/x?y=1", &[]));
        let rec = log.last().unwrap();
        assert_eq!(
            (rec.host.as_str(), rec.outcome.as_str()),
            ("rms.api.bbc.co.uk", "ok")
        );
        let (http, log) = ObservedHttp::new(std::sync::Arc::new(Fixed(Ok(503))));
        let _ = block(http.probe("http://h:8000/live", &[]));
        assert_eq!(log.last().unwrap().outcome, "HTTP 503");
        assert_eq!(log.last().unwrap().host, "h:8000");
        let (http, log) = ObservedHttp::new(std::sync::Arc::new(Fixed(Err(HttpError("x".into())))));
        let _ = block(http.get("https://a.example/", &[]));
        assert_eq!(log.last().unwrap().outcome, "error");
    }
}
