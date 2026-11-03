//! Source plugins for ttnr. Each source (`url`, `stations`, `bbc`,
//! `radio-browser`, `exec`) lives in its own module and implements the
//! `Source` / `MetadataProvider` traits from `ttnr-core`.

pub mod bbc;
pub mod exec;
pub mod hls_id3;
pub mod http;
pub mod id3;
pub mod playlist;
pub mod radio_browser;
pub mod stations;
pub mod url_source;

pub use bbc::BbcSource;
pub use exec::ExecSource;
pub use hls_id3::HlsId3Provider;
pub use http::ReqwestHttp;
pub use radio_browser::RadioBrowserSource;
pub use stations::StationsSource;
pub use url_source::UrlSource;
