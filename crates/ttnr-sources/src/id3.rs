//! Minimal ID3v2.3/2.4 reader for now-playing frames at the start of HLS
//! packed-audio segments (several tags may follow each other, e.g. a PRIV
//! timestamp tag, then the text tag). Spec: https://id3.org/id3v2.4.0-structure
//! and id3v2.3.0 (docs/RESEARCH.md).

/// Now-playing fields found in ID3 tags.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Id3Info {
    /// TIT2.
    pub title: Option<String>,
    /// TPE1.
    pub artist: Option<String>,
    /// TALB.
    pub album: Option<String>,
    /// TRSN (internet radio station name).
    pub station: Option<String>,
    /// WXXX whose description starts with `artworkURL` (StreamS encoders).
    pub artwork_url: Option<String>,
}

impl Id3Info {
    /// `true` if no field is set.
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// Read all consecutive ID3v2 tags at the start of `data`. Unknown,
/// compressed, encrypted or unsynchronised content is skipped, never fatal.
pub fn read_tags(data: &[u8]) -> Id3Info {
    let mut info = Id3Info::default();
    let mut pos = 0;
    while let Some(tag) = data.get(pos..).filter(|d| d.starts_with(b"ID3")) {
        let Some(header) = tag.get(..10) else { break };
        let (major, flags) = (header[3], header[5]);
        let Some(size) = syncsafe(&header[6..10]) else {
            break;
        };
        let footer = if flags & 0x10 != 0 { 10 } else { 0 };
        let end = (10 + size).min(tag.len());
        // Tag-wide unsynchronisation (0x80) is rare in streams: skip.
        if matches!(major, 3 | 4) && flags & 0x80 == 0 {
            read_frames(&tag[10..end], major, flags & 0x40 != 0, &mut info);
        }
        pos += 10 + size + footer;
    }
    info
}

fn read_frames(mut body: &[u8], major: u8, extended: bool, info: &mut Id3Info) {
    if extended {
        let ext = match major {
            4 => body.get(..4).and_then(syncsafe),
            _ => body.get(..4).map(|b| be32(b) + 4),
        };
        match ext {
            Some(n) if n <= body.len() => body = &body[n..],
            _ => return,
        }
    }
    while body.len() >= 10 && body[0] != 0 {
        let id = &body[..4];
        let size = match major {
            4 => syncsafe(&body[4..8]),
            _ => Some(be32(&body[4..8])),
        };
        let Some(size) = size.filter(|s| 10 + s <= body.len()) else {
            return;
        };
        let format_flags = body[9];
        let data = &body[10..10 + size];
        body = &body[10 + size..];
        // v2.4 format flags: compression 0x08, encryption 0x04, unsync 0x02;
        // v2.3: compression 0x80, encryption 0x40.
        let unsupported = match major {
            4 => format_flags & 0x0E != 0,
            _ => format_flags & 0xC0 != 0,
        };
        if unsupported {
            continue;
        }
        match id {
            b"TIT2" => info.title = text(data),
            b"TPE1" => info.artist = text(data),
            b"TALB" => info.album = text(data),
            b"TRSN" => info.station = text(data),
            b"WXXX" => {
                if let Some((desc, url)) = wxxx(data)
                    && desc.starts_with("artworkURL")
                    && !url.is_empty()
                {
                    info.artwork_url = Some(url);
                }
            }
            _ => {}
        }
    }
}

fn be32(b: &[u8]) -> usize {
    b.iter().fold(0, |acc, &x| (acc << 8) | usize::from(x))
}

fn syncsafe(b: &[u8]) -> Option<usize> {
    if b.len() != 4 || b.iter().any(|&x| x & 0x80 != 0) {
        return None;
    }
    Some(b.iter().fold(0, |acc, &x| (acc << 7) | usize::from(x)))
}

/// Text frame: encoding byte + (possibly NUL-separated) strings; first one.
fn text(data: &[u8]) -> Option<String> {
    let (&enc, rest) = data.split_first()?;
    let s = decode(enc, rest);
    let first = s.split('\0').next().unwrap_or("").trim().to_owned();
    (!first.is_empty()).then_some(first)
}

/// WXXX: encoding, description (encoded, NUL-terminated), URL (Latin-1).
fn wxxx(data: &[u8]) -> Option<(String, String)> {
    let (&enc, rest) = data.split_first()?;
    let (desc, url) = match enc {
        1 | 2 => {
            let end = rest.chunks(2).position(|c| c == [0, 0])? * 2;
            (decode(enc, &rest[..end]), rest.get(end + 2..)?)
        }
        _ => {
            let end = rest.iter().position(|&b| b == 0)?;
            (decode(enc, &rest[..end]), rest.get(end + 1..)?)
        }
    };
    let url: String = url
        .iter()
        .take_while(|&&b| b != 0)
        .map(|&b| char::from(b))
        .collect();
    Some((desc, url.trim().to_owned()))
}

fn decode(enc: u8, bytes: &[u8]) -> String {
    match enc {
        0 => bytes.iter().map(|&b| char::from(b)).collect(),
        1 | 2 => {
            let mut units: Vec<u16> = bytes
                .as_chunks::<2>()
                .0
                .iter()
                .map(|&c| u16::from_be_bytes(c))
                .collect();
            let little = enc == 1 && units.first() == Some(&0xFFFE);
            if enc == 1 && matches!(units.first(), Some(0xFEFF | 0xFFFE)) {
                units.remove(0);
            }
            if little {
                units.iter_mut().for_each(|u| *u = u.swap_bytes());
            }
            String::from_utf16_lossy(&units)
        }
        _ => String::from_utf8_lossy(bytes).into_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build an ID3v2.4 tag from (id, encoding, payload) frames.
    fn tag(frames: &[(&[u8; 4], &[u8])]) -> Vec<u8> {
        let mut body = Vec::new();
        for (id, data) in frames {
            body.extend_from_slice(*id);
            let n = data.len();
            body.extend_from_slice(&[
                (n >> 21) as u8 & 0x7F,
                (n >> 14) as u8 & 0x7F,
                (n >> 7) as u8 & 0x7F,
                n as u8 & 0x7F,
            ]);
            body.extend_from_slice(&[0, 0]);
            body.extend_from_slice(data);
        }
        let n = body.len();
        let mut out = b"ID3\x04\x00\x00".to_vec();
        out.extend_from_slice(&[
            (n >> 21) as u8 & 0x7F,
            (n >> 14) as u8 & 0x7F,
            (n >> 7) as u8 & 0x7F,
            n as u8 & 0x7F,
        ]);
        out.extend(body);
        out
    }

    #[test]
    fn encodings() {
        let utf16le: Vec<u8> = [0xFF, 0xFE]
            .into_iter()
            .chain("Żółw".encode_utf16().flat_map(u16::to_le_bytes))
            .collect();
        let mut tit2 = vec![1];
        tit2.extend(utf16le);
        let mut tpe1 = vec![3];
        tpe1.extend("Björk\0Other".as_bytes());
        let t = tag(&[
            (b"TIT2", &tit2),
            (b"TPE1", &tpe1),
            (b"TALB", b"\x00Caf\xE9"),
        ]);
        let info = read_tags(&t);
        assert_eq!(info.title.as_deref(), Some("Żółw"));
        assert_eq!(info.artist.as_deref(), Some("Björk"));
        assert_eq!(info.album.as_deref(), Some("Café"));
    }

    #[test]
    fn consecutive_tags_and_garbage() {
        let mut data = tag(&[(b"PRIV", b"owner\0\x01\x02")]);
        data.extend(tag(&[(b"TIT2", b"\x03Song")]));
        data.extend_from_slice(b"\xFF\xF1audio...");
        assert_eq!(read_tags(&data).title.as_deref(), Some("Song"));
        assert!(read_tags(b"\xFF\xF1not id3").is_empty());
        assert!(
            read_tags(b"ID3\x04\x00\x00\xFF\xFF\xFF\xFF").is_empty(),
            "bad size"
        );
        // Truncated frame size larger than the data: stop, no panic.
        assert!(read_tags(&tag(&[(b"TIT2", b"\x03Song")])[..15]).is_empty());
    }

    #[test]
    fn wxxx_artwork() {
        let t = tag(&[(b"WXXX", b"\x00artworkURL_640x\0https://img/x.jpg")]);
        assert_eq!(
            read_tags(&t).artwork_url.as_deref(),
            Some("https://img/x.jpg")
        );
        let other = tag(&[(b"WXXX", b"\x00homepage\0https://x")]);
        assert_eq!(read_tags(&other).artwork_url, None);
    }
}
