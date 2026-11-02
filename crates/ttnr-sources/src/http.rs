//! [`ReqwestHttp`]: the real [`Http`] client (reqwest + macOS native TLS).

use std::time::Duration;

use async_trait::async_trait;
use ttnr_core::{Http, HttpError, HttpResponse};

use crate::playlist::is_stream_mime;

/// Largest body read by [`Http::get`]; playlists and API answers are far
/// smaller, anything bigger is not something we want to parse.
pub const BODY_CAP: usize = 256 * 1024;

/// reqwest-based client: redirects (≤ 10), connect/read timeouts, body cap,
/// and no body read at all for media streams.
#[derive(Debug, Clone)]
pub struct ReqwestHttp {
    client: reqwest::Client,
}

impl ReqwestHttp {
    /// Build the client.
    pub fn new() -> Result<Self, HttpError> {
        let client = reqwest::Client::builder()
            .user_agent(concat!("ttnr/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(10))
            .read_timeout(Duration::from_secs(15))
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()
            .map_err(err)?;
        Ok(Self { client })
    }

    async fn send(
        &self,
        url: &str,
        headers: &[(String, String)],
    ) -> Result<reqwest::Response, HttpError> {
        let mut req = self.client.get(url);
        for (name, value) in headers {
            req = req.header(name, value);
        }
        req.send().await.map_err(err)
    }
}

/// reqwest's `Display` omits the cause chain ("error sending request");
/// include it so logs say *why*.
fn err(e: reqwest::Error) -> HttpError {
    let mut text = e.to_string();
    let mut source = std::error::Error::source(&e);
    while let Some(s) = source {
        text.push_str(": ");
        text.push_str(&s.to_string());
        source = s.source();
    }
    HttpError(text)
}

#[async_trait]
impl Http for ReqwestHttp {
    async fn get(
        &self,
        url: &str,
        headers: &[(String, String)],
    ) -> Result<HttpResponse, HttpError> {
        let mut resp = self.send(url, headers).await?;
        let status = resp.status().as_u16();
        let final_url = resp.url().to_string();
        let content_type = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let icy = resp
            .headers()
            .keys()
            .any(|k| k.as_str().starts_with("icy-"));
        let streaming = icy || content_type.as_deref().is_some_and(is_stream_mime);
        let mut body = Vec::new();
        if !streaming {
            while body.len() < BODY_CAP {
                match resp.chunk().await.map_err(err)? {
                    Some(chunk) => body.extend_from_slice(&chunk),
                    None => break,
                }
            }
            body.truncate(BODY_CAP);
        }
        // Dropping `resp` aborts any unread body (e.g. a stream).
        Ok(HttpResponse {
            status,
            final_url,
            content_type,
            body,
            streaming,
        })
    }

    async fn probe(&self, url: &str, headers: &[(String, String)]) -> Result<u16, HttpError> {
        // GET + drop after the headers: HEAD is unreliable on Icecast mounts.
        Ok(self.send(url, headers).await?.status().as_u16())
    }
}
