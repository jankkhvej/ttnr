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
