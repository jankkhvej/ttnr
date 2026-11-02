//! What the engine needs to start playing: candidate URLs and HTTP headers.

/// Something playable *now*, produced by a source at play time.
///
/// Minimal M1 shape; resolution metadata (`resolved_at`, `ttl`, resolver
/// trace) is added with the resolver chain in M2 (see ARCHITECTURE.md).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StreamDescriptor {
    /// Ordered candidate URLs (quality / region fallbacks), best first.
    pub urls: Vec<String>,
    /// Extra HTTP request headers as `(name, value)` pairs.
    pub headers: Vec<(String, String)>,
}

impl StreamDescriptor {
    /// A descriptor for a single URL with no extra headers.
    pub fn direct(url: impl Into<String>) -> Self {
        Self {
            urls: vec![url.into()],
            headers: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_has_one_url_and_no_headers() {
        let d = StreamDescriptor::direct("https://example.org/a.m3u8");
        assert_eq!(d.urls, ["https://example.org/a.m3u8"]);
        assert!(d.headers.is_empty());
    }
}
