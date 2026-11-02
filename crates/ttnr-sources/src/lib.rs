//! Source plugins for ttnr. Each source (`url`, `stations`, `bbc`,
//! `radio-browser`, `exec`) lives in its own module and implements the
//! `Source` / `MetadataProvider` traits from `ttnr-core`.

pub mod http;
pub mod playlist;
pub mod url_source;

pub use http::ReqwestHttp;
pub use url_source::UrlSource;
