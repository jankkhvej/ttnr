//! Core of ttnr: domain types (`Station`, `StreamDescriptor`, `NowPlaying`,
//! `StreamInfo`), the source plugin traits, the `SourceRegistry` and the
//! resolver chain; later the app state machine, metadata merging, sync
//! offset and recent-tracks history.

pub mod cache;
pub mod failover;
pub mod http;
pub mod merge;
pub mod nowplaying;
pub mod registry;
pub mod resolve;
pub mod source;
pub mod station;
pub mod stream;

pub use cache::ResolveCache;
pub use failover::{Action, Failover};
pub use http::{Http, HttpError, HttpResponse};
pub use merge::Merger;
pub use nowplaying::{InfoSource, NowPlaying, StreamTags, split_icy_title};
pub use registry::SourceRegistry;
pub use resolve::{Mode, ResolveCtx, ResolveError, resolve_chain};
pub use source::{ListQuery, MetadataProvider, Source, SourceCaps, SourceError};
pub use station::Station;
pub use stream::{ResolverTrace, Strategy, StreamDescriptor, StreamInfo, TraceStep};
