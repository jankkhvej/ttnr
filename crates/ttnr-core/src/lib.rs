//! Core of ttnr: domain types (`Station`, `StreamDescriptor`, `NowPlaying`,
//! `StreamInfo`), the application state machine, the `SourceRegistry`,
//! metadata merging, sync offset and recent-tracks history.

pub mod stream;

pub use stream::StreamDescriptor;
