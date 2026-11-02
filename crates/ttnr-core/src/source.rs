//! The source plugin traits (SPEC F1.2): one implementation per source,
//! registered once in the [`crate::SourceRegistry`].

use async_trait::async_trait;
use tokio::sync::mpsc::Sender;

use crate::resolve::ResolveCtx;
use crate::{HttpError, NowPlaying, Station, StreamDescriptor};

/// What a source can do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SourceCaps {
    /// Can list stations without a query.
    pub browse: bool,
    /// Can search stations by text.
    pub search: bool,
    /// Provides external now-playing metadata for some stations.
    pub metadata: bool,
}

/// Parameters for [`Source::list`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ListQuery {
    /// Free-text search, if any.
    pub search: Option<String>,
    /// Maximum number of results, if any.
    pub limit: Option<usize>,
}

/// Errors a source reports.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SourceError {
    /// Transport failure.
    #[error("network: {0}")]
    Http(#[from] HttpError),
    /// The server answered with a non-success status.
    #[error("HTTP {status} for {url}")]
    Status {
        /// Status code.
        status: u16,
        /// Requested URL.
        url: String,
    },
    /// A response or file could not be understood.
    #[error("cannot parse {what}: {detail}")]
    Parse {
        /// What was being parsed (e.g. `PLS playlist`).
        what: String,
        /// Why it failed.
        detail: String,
    },
    /// Anything else.
    #[error("{0}")]
    Other(String),
}

/// A place stations come from (generic URL, BBC, a directory, a script…).
///
/// Sources do not resolve stream URLs end to end: they provide the hooks
/// [`Source::discover`] and [`Source::patterns`], and the shared resolver
/// chain in [`crate::resolve`] adds caching, probing, the user override and
/// the trace (ADR-017).
#[async_trait]
pub trait Source: Send + Sync {
    /// Stable id, e.g. `url`, `stations`, `bbc`.
    fn id(&self) -> &'static str;

    /// What this source supports.
    fn capabilities(&self) -> SourceCaps;

    /// Stations matching `q` (empty for sources that cannot browse).
    async fn list(&self, _q: &ListQuery, _ctx: &ResolveCtx) -> Result<Vec<Station>, SourceError> {
        Ok(Vec::new())
    }

    /// Authoritative lookup of a playable descriptor right now; `Ok(None)`
    /// if this source has no discovery for the station.
    async fn discover(
        &self,
        _st: &Station,
        _ctx: &ResolveCtx,
    ) -> Result<Option<StreamDescriptor>, SourceError> {
        Ok(None)
    }

    /// Known URL templates for the station, best first; the chain probes
    /// each and takes the first that answers 2xx.
    fn patterns(&self, _st: &Station) -> Vec<StreamDescriptor> {
        Vec::new()
    }

    /// Optional external now-playing provider for this station.
    fn metadata(&self, _st: &Station) -> Option<Box<dyn MetadataProvider>> {
        None
    }
}

/// Produces now-playing updates for one station; owns its polling cadence
/// and backoff, and stops when the receiver is dropped.
#[async_trait]
pub trait MetadataProvider: Send {
    /// Run until the receiver goes away or an unrecoverable error occurs.
    async fn run(self: Box<Self>, tx: Sender<NowPlaying>) -> Result<(), SourceError>;
}
