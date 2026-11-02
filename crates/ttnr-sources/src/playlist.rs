//! Playlist detection and parsing: M3U/EXTM3U, PLS, XSPF, ASX, and telling
//! HLS (passed to mpv untouched) from plain M3U.
//!
//! Rules follow mpv's `demux_playlist.c`, RFC 8216 and real-world files
//! (docs/RESEARCH.md): detect by *content* first, MIME type/extension only
//! as a hint; ASX and XSPF are scanned leniently because ASX in the wild is
//! often not well-formed XML.

use url::Url;

/// Playlist formats we understand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// HTTP Live Streaming playlist (master or media): mpv plays it itself.
    Hls,
    /// Plain or extended M3U list of stream URLs.
    M3u,
    /// Shoutcast/Winamp PLS.
    Pls,
    /// XML Shareable Playlist Format.
    Xspf,
    /// Windows Media ASX metafile.
    Asx,
}

/// One playlist entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Absolute URL.
    pub url: String,
    /// Title from the playlist, if any.
    pub title: Option<String>,
}

/// Bytes to text: UTF-8 (BOM stripped), else Latin-1 (old PLS/M3U files).
pub fn decode(bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_owned(),
        Err(_) => bytes.iter().map(|&b| char::from(b)).collect(),
    }
}

/// Detect the playlist kind from content, falling back to the MIME type and
/// URL extension. `None` = not a playlist (treat the URL as a stream).
pub fn detect(url: &str, content_type: Option<&str>, body: &str) -> Option<Kind> {
    sniff(body).or_else(|| hint(url, content_type).filter(|_| looks_like_url_list(body)))
}

/// Content-based detection.
fn sniff(body: &str) -> Option<Kind> {
    let head = body.trim_start();
    let lower: String = head
        .chars()
        .take(512)
        .collect::<String>()
        .to_ascii_lowercase();
    if head.starts_with("#EXTM3U") {
        // RFC 8216: any EXT-X- tag means HLS.
        return Some(
            if body.lines().any(|l| l.trim_start().starts_with("#EXT-X-")) {
                Kind::Hls
            } else {
                Kind::M3u
            },
        );
    }
    if lower.starts_with("[playlist]") {
        return Some(Kind::Pls);
    }
    if lower.starts_with("<asx") {
        return Some(Kind::Asx);
    }
    if lower.contains("<playlist") && lower.contains("xspf.org/ns/0") {
        return Some(Kind::Xspf);
    }
    None
}

/// Hint from MIME type or extension (servers often send wrong types).
fn hint(url: &str, content_type: Option<&str>) -> Option<Kind> {
    let mime = content_type
        .and_then(|c| c.split(';').next())
        .map(|m| m.trim().to_ascii_lowercase());
    let by_mime = match mime.as_deref() {
        Some("audio/x-scpls" | "application/pls+xml") => Some(Kind::Pls),
        Some("audio/x-mpegurl" | "audio/mpegurl" | "application/x-mpegurl") => Some(Kind::M3u),
        Some("application/xspf+xml") => Some(Kind::Xspf),
        Some("video/x-ms-asf" | "video/x-ms-asx" | "audio/x-ms-wax" | "video/x-ms-wvx") => {
            Some(Kind::Asx)
        }
        _ => None,
    };
    by_mime.or_else(|| {
        let path = url
            .split(['?', '#'])
            .next()
            .unwrap_or(url)
            .to_ascii_lowercase();
        let ext = path.rsplit_once('.').map(|(_, e)| e)?;
        match ext {
            "m3u" | "m3u8" => Some(Kind::M3u),
            "pls" => Some(Kind::Pls),
            "xspf" => Some(Kind::Xspf),
            "asx" | "wax" | "wvx" => Some(Kind::Asx),
            _ => None,
        }
    })
}

/// A headerless M3U is only believed if it actually lists URLs.
fn looks_like_url_list(body: &str) -> bool {
    body.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .any(|l| l.contains("://"))
}

/// Media types that announce a continuous stream (body must not be read).
pub fn is_stream_mime(content_type: &str) -> bool {
    let m = content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    let playlist = matches!(
        m.as_str(),
        "audio/x-scpls" | "audio/x-mpegurl" | "audio/mpegurl" | "audio/x-ms-wax"
    );
    !playlist
        && (m.starts_with("audio/")
            || matches!(
                m.as_str(),
                "application/ogg" | "video/mp2t" | "application/aacp"
            ))
}

/// Parse a playlist into absolute entries, resolving relative URLs against
/// `base` (the playlist's final URL). Entries with non-network schemes are
/// dropped: a remote playlist must not point at local files.
pub fn parse(kind: Kind, body: &str, base: &Url) -> Vec<Entry> {
    let raw = match kind {
        Kind::Hls => Vec::new(),
        Kind::M3u => parse_m3u(body),
        Kind::Pls => parse_pls(body),
        Kind::Xspf => parse_xspf(body),
        Kind::Asx => parse_asx(body),
    };
    let mut out: Vec<Entry> = Vec::new();
    for (href, title) in raw {
        let Ok(url) = base.join(href.trim()) else {
            continue;
        };
        if !matches!(
            url.scheme(),
            "http" | "https" | "mms" | "mmsh" | "rtsp" | "rtmp"
        ) {
            continue;
        }
        let url = url.to_string();
        if !out.iter().any(|e| e.url == url) {
            out.push(Entry { url, title });
        }
    }
    out
}

fn parse_m3u(body: &str) -> Vec<(String, Option<String>)> {
    let mut out = Vec::new();
    let mut title = None;
    for line in body.lines().map(str::trim) {
        if let Some(info) = line.strip_prefix("#EXTINF:") {
            title = info
                .split_once(',')
                .map(|(_, t)| t.trim().to_owned())
                .filter(|t| !t.is_empty());
        } else if !line.is_empty() && !line.starts_with('#') {
            out.push((line.to_owned(), title.take()));
        }
    }
    out
}

fn parse_pls(body: &str) -> Vec<(String, Option<String>)> {
    let mut files: Vec<(u32, String)> = Vec::new();
    let mut titles: Vec<(u32, String)> = Vec::new();
    for line in body.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim().to_ascii_lowercase();
        let value = value.trim().to_owned();
        let numbered = |prefix: &str| key.strip_prefix(prefix).and_then(|n| n.parse::<u32>().ok());
        if let Some(n) = numbered("file") {
            files.push((n, value));
        } else if let Some(n) = numbered("title") {
            titles.push((n, value));
        }
    }
    files.sort_by_key(|(n, _)| *n);
    files
        .into_iter()
        .map(|(n, url)| {
            let title = titles
                .iter()
                .find(|(t, _)| *t == n)
                .map(|(_, v)| v.clone())
                .filter(|t| !t.is_empty());
            (url, title)
        })
        .collect()
}

fn parse_xspf(body: &str) -> Vec<(String, Option<String>)> {
    blocks(body, "track")
        .filter_map(|track| {
            let location = element_text(track, "location")?;
            Some((
                xml_unescape(&location),
                element_text(track, "title").map(|t| xml_unescape(&t)),
            ))
        })
        .collect()
}

fn parse_asx(body: &str) -> Vec<(String, Option<String>)> {
    let mut out = Vec::new();
    for entry in blocks(body, "entry") {
        let title = element_text(entry, "title").map(|t| xml_unescape(&t));
        for href in attr_values(entry, "ref", "href") {
            out.push((xml_unescape(&href), title.clone()));
        }
    }
    out
}

/// Inner text of each `<tag …>…</tag>` block (case-insensitive tag names).
fn blocks<'a>(body: &'a str, tag: &str) -> impl Iterator<Item = &'a str> {
    let lower = body.to_ascii_lowercase();
    let open = format!("<{tag}");
    let close = format!("</{tag}");
    let mut spans = Vec::new();
    let mut pos = 0;
    while let Some(start) = lower[pos..].find(&open).map(|i| i + pos) {
        let after = start + open.len();
        // `<track>` but not `<tracklist>`.
        if lower[after..].starts_with(|c: char| c.is_ascii_alphanumeric()) {
            pos = after;
            continue;
        }
        let Some(gt) = lower[after..].find('>').map(|i| i + after + 1) else {
            break;
        };
        let end = lower[gt..].find(&close).map_or(lower.len(), |i| i + gt);
        spans.push((gt, end));
        pos = end;
    }
    spans.into_iter().map(move |(a, b)| &body[a..b])
}

/// Text of the first `<tag>…</tag>` in `block`.
fn element_text(block: &str, tag: &str) -> Option<String> {
    blocks(block, tag)
        .next()
        .map(|t| t.trim().to_owned())
        .filter(|t| !t.is_empty())
}

/// Values of `attr` on every `<tag …>` in `block`; tolerates spaces around
/// `=`, single/double/no quotes and any letter case.
fn attr_values(block: &str, tag: &str, attr: &str) -> Vec<String> {
    let lower = block.to_ascii_lowercase();
    let open = format!("<{tag}");
    let mut out = Vec::new();
    let mut pos = 0;
    while let Some(start) = lower[pos..].find(&open).map(|i| i + pos) {
        let end = lower[start..].find('>').map_or(lower.len(), |i| i + start);
        let tag_lower = &lower[start..end];
        let tag_orig = &block[start..end];
        let mut search = 0;
        while let Some(i) = tag_lower[search..].find(attr).map(|i| i + search) {
            let rest = tag_lower[i + attr.len()..].trim_start();
            if let Some(after_eq) = rest.strip_prefix('=') {
                let value_start = end - start - after_eq.trim_start().len();
                let raw = &tag_orig[value_start..];
                let value = match raw.chars().next() {
                    Some(q @ ('"' | '\'')) => raw[1..].split(q).next().unwrap_or(""),
                    _ => raw
                        .split(|c: char| c.is_whitespace() || c == '/')
                        .next()
                        .unwrap_or(""),
                };
                if !value.is_empty() {
                    out.push(value.to_owned());
                }
                break;
            }
            search = i + attr.len();
        }
        pos = end;
    }
    out
}

/// Decode the five XML entities and numeric references; a lone `&` (common
/// in ASX URLs) is kept as is.
fn xml_unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let decoded = rest.find(';').filter(|&j| j <= 10).and_then(|j| {
            let ent = &rest[1..j];
            let c = match ent {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                _ => ent
                    .strip_prefix("#x")
                    .or_else(|| ent.strip_prefix("#X"))
                    .and_then(|h| u32::from_str_radix(h, 16).ok())
                    .or_else(|| ent.strip_prefix('#').and_then(|d| d.parse().ok()))
                    .and_then(char::from_u32),
            };
            c.map(|c| (c, j))
        });
        match decoded {
            Some((c, j)) => {
                out.push(c);
                rest = &rest[j + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Url {
        Url::parse("https://example.org/radio/list.pls").unwrap()
    }

    #[test]
    fn decode_strips_bom_and_falls_back_to_latin1() {
        assert_eq!(decode(b"\xEF\xBB\xBF[playlist]"), "[playlist]");
        assert_eq!(decode(b"Title1=Caf\xE9"), "Title1=Café");
    }

    #[test]
    fn hls_versus_plain_m3u() {
        assert_eq!(sniff("#EXTM3U\n#EXT-X-VERSION:3\nseg.ts"), Some(Kind::Hls));
        assert_eq!(
            sniff("#EXTM3U\r\n#EXTINF:0, X\r\nhttp://a"),
            Some(Kind::M3u)
        );
        // .m3u8 extension alone does not make it HLS.
        assert_eq!(
            detect("http://x/list.m3u8", None, "http://a/stream\n"),
            Some(Kind::M3u)
        );
    }

    #[test]
    fn headerless_m3u_needs_urls() {
        assert_eq!(
            detect("http://x/a.m3u", None, "http://a\nhttp://b"),
            Some(Kind::M3u)
        );
        assert_eq!(detect("http://x/a.m3u", None, "<html>404</html>"), None);
        assert_eq!(
            detect("http://x/stream", Some("text/plain"), "http://a"),
            None
        );
    }

    #[test]
    fn mime_hint_beats_extension() {
        assert_eq!(
            detect(
                "http://x/listen",
                Some("audio/x-scpls; charset=UTF-8"),
                "File1=http://a"
            ),
            Some(Kind::Pls)
        );
    }

    #[test]
    fn stream_mime_types() {
        assert!(is_stream_mime("audio/mpeg"));
        assert!(is_stream_mime("audio/aacp"));
        assert!(is_stream_mime("application/ogg"));
        assert!(!is_stream_mime("audio/x-scpls"));
        assert!(!is_stream_mime("audio/x-mpegurl; charset=utf-8"));
        assert!(!is_stream_mime("text/html"));
    }

    #[test]
    fn pls_orders_by_number_and_pairs_titles() {
        let body =
            "[Playlist]\nnumberofentries=2\nFile2=http://b\nTitle2=B\nfile1 = http://a\nTitle1=A\n";
        let got = parse(Kind::Pls, body, &base());
        assert_eq!(
            got,
            [
                Entry {
                    url: "http://a/".into(),
                    title: Some("A".into())
                },
                Entry {
                    url: "http://b/".into(),
                    title: Some("B".into())
                },
            ]
        );
    }

    #[test]
    fn relative_urls_and_unsafe_schemes() {
        let body = "stream.mp3\n../other.aac\nfile:///etc/passwd\njavascript:alert(1)\nhttp://a/x\nhttp://a/x\n";
        let urls: Vec<_> = parse(Kind::M3u, body, &base())
            .into_iter()
            .map(|e| e.url)
            .collect();
        assert_eq!(
            urls,
            [
                "https://example.org/radio/stream.mp3",
                "https://example.org/other.aac",
                "http://a/x"
            ]
        );
    }

    #[test]
    fn asx_lenient() {
        let body = r#"<ASX Version="3.0"><Entry><Title>Main &amp; Co</Title>
            <REF HREF='http://a/x?b=1&c=2'/><ref href = "http://b/y" ></ref></Entry></ASX>"#;
        let got = parse(Kind::Asx, body, &base());
        assert_eq!(got[0].url, "http://a/x?b=1&c=2");
        assert_eq!(got[0].title.as_deref(), Some("Main & Co"));
        assert_eq!(got[1].url, "http://b/y");
    }

    #[test]
    fn xml_entities() {
        assert_eq!(
            xml_unescape("a &amp; b &#38; &#x26; &lt;x&gt; & c"),
            "a & b & & <x> & c"
        );
    }
}
