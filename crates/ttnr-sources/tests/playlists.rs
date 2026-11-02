//! Playlist detection and parsing against real playlists recorded from
//! popular stations (tests/fixtures/playlists/real, recorded 2026-11-02;
//! URLs and Content-Types as served then).

// Test code: unwrap/expect allowed (CLAUDE.md conventions).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;

use ttnr_sources::playlist::{Kind, decode, detect, parse};
use url::Url;

struct Case {
    file: &'static str,
    url: &'static str,
    content_type: &'static str,
    kind: Kind,
    count: usize,
    first: &'static str,
    first_title: Option<&'static str>,
}

const CASES: &[Case] = &[
    Case {
        file: "somafm_groovesalad.pls",
        url: "https://somafm.com/groovesalad.pls",
        content_type: "audio/x-scpls",
        kind: Kind::Pls,
        count: 3,
        first: "https://ice6.somafm.com/groovesalad-128-mp3",
        first_title: Some(
            "SomaFM: Groove Salad (#1): A nicely chilled plate of ambient/downtempo beats and grooves.",
        ),
    },
    Case {
        file: "streamtheworld_977_country.pls",
        url: "http://playerservices.streamtheworld.com/pls/977_COUNTRY.pls",
        content_type: "audio/x-scpls; charset=UTF-8",
        kind: Kind::Pls,
        count: 6,
        first: "http://15113.live.streamtheworld.com/977_COUNTRY_SC",
        first_title: Some("977_COUNTRY_SC"),
    },
    Case {
        file: "101smoothjazz.m3u",
        url: "http://www.101smoothjazz.com/101-smoothjazz.m3u",
        content_type: "audio/x-mpegurl",
        kind: Kind::M3u,
        count: 1,
        first: "http://jking.cdnstream1.com/b22139_128mp3",
        first_title: Some("101 Smooth Jazz"),
    },
    Case {
        file: "bassdrive.m3u",
        url: "http://www.bassdrive.com/bassdrive.m3u",
        content_type: "audio/x-mpegurl",
        kind: Kind::M3u,
        count: 5,
        first: "http://ice.bassdrive.net/stream",
        first_title: None,
    },
    Case {
        file: "trancebase.asx",
        url: "http://listen.trancebase.fm/dsl.asx",
        content_type: "video/x-ms-asx",
        kind: Kind::Asx,
        count: 1,
        first: "http://listen.trancebase.fm/tunein-mp3-asx",
        first_title: Some("TranceBase.FM - MP3 192k - High Quality - (DSL/Cable)"),
    },
    Case {
        file: "darkfire.asx",
        url: "https://www.radio-darkfire.com/listen/radiodar.asx",
        content_type: "video/x-ms-asf",
        kind: Kind::Asx,
        count: 1,
        first: "http://37.59.82.8:8344/",
        first_title: None,
    },
    Case {
        file: "goldfm.xspf",
        url: "https://stream.goldfm.lt/goldfm.aac.xspf",
        content_type: "application/xspf+xml; charset=UTF-8",
        kind: Kind::Xspf,
        count: 1,
        first: "http://stream.goldfm.lt/goldfm.aac",
        first_title: Some("London Boys - Harlem Desire"),
    },
    Case {
        file: "wdvx.xspf",
        url: "https://wdvx.streamguys1.com/live.xspf",
        content_type: "application/xspf+xml",
        kind: Kind::Xspf,
        count: 1,
        first: "http://wdvx.streamguys1.com/live",
        first_title: Some(
            "Colter Wall - Big Iron - Western Swing & Waltzes and Other Punchy Songs",
        ),
    },
    Case {
        file: "rtp_antena1_master.m3u8",
        url: "http://streaming-live-app.rtp.pt/liveradio/antena180a/playlist.m3u8",
        content_type: "application/vnd.apple.mpegurl",
        kind: Kind::Hls,
        count: 0,
        first: "",
        first_title: None,
    },
    Case {
        file: "aljazeera_media.m3u8",
        url: "https://live-hls-audio-web-aja.getaj.net/VOICE-AJA/01.m3u8",
        content_type: "application/vnd.apple.mpegurl",
        kind: Kind::Hls,
        count: 0,
        first: "",
        first_title: None,
    },
];

fn read(file: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/playlists/real")
        .join(file);
    decode(&std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display())))
}

#[test]
fn real_playlists() {
    for c in CASES {
        let body = read(c.file);
        let kind = detect(c.url, Some(c.content_type), &body);
        assert_eq!(kind, Some(c.kind), "{}: kind", c.file);
        // Content alone must be enough (servers send wrong types) – except
        // for a headerless M3U, which is only believed by MIME/extension.
        let headerless_m3u = c.kind == Kind::M3u && !body.trim_start().starts_with("#EXTM3U");
        let want = if headerless_m3u { None } else { Some(c.kind) };
        assert_eq!(
            detect("http://x/listen", None, &body),
            want,
            "{}: sniff",
            c.file
        );
        let entries = parse(c.kind, &body, &Url::parse(c.url).unwrap());
        assert_eq!(entries.len(), c.count, "{}: {entries:?}", c.file);
        if let Some(first) = entries.first() {
            assert_eq!(first.url, c.first, "{}", c.file);
            assert_eq!(first.title.as_deref(), c.first_title, "{}", c.file);
        }
    }
}
