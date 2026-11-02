# Research log — verified external facts

Every external fact the code or docs rely on goes here: **fact · source URL ·
date checked · status** (`VERIFIED`, `UNVERIFIED`, `STALE`). Re-check
anything older than ~3 months before relying on it.

## Seeded on 2026-11-02 (from the planning chat — re-verify in M0/M4)

| Fact | Source | Status |
|---|---|---|
| BBC live radio streams carry no track metadata in-stream (HLS); third-party players get track info from a separate API | https://community.roonlabs.com/t/bbc-6-music-stream-url-with-metadata/314302 | seen 2026-11-02 |
| BBC now-playing endpoint used by community tools: `https://rms.api.bbc.co.uk/v2/services/bbc_6music/segments/latest`; returns empty `data` during programmes without song info | https://community.roonlabs.com/t/bbc-radio-6-metadata-addition/124273 | seen 2026-11-02, unofficial/undocumented |
| BBC schedule endpoint used as fallback for programme info: `https://ess.api.bbci.co.uk/schedules?serviceId=bbc_6music` | same thread | seen 2026-11-02, unofficial |
| Worldwide HLS pattern example: `http://as-hls-ww-live.akamaized.net/pool_904/live/ww/bbc_6music/bbc_6music.isml/bbc_6music-audio%3d96000.norewind.m3u8` | https://helpdesk.midwich.com/support/solutions/articles/50000141277 | seen 2026-11-02; URLs changed in 2023, expect change |
| 320 kbps AAC streams are geo-locked to the UK; 96 kbps worldwide | https://moodeaudio.org/forum/showthread.php?pid=49213 | seen 2026-11-02 |
| Reports of BBC restricting HLS/DASH to authorised third parties | https://forum.minimserver.com/showthread.php?pid=53243 | seen 2026-11-02 |
| BBC is gradually restricting BBC Sounds for non-UK listeners | https://www.music-assistant.io/music-providers/bbc-sounds | seen 2026-11-02 |
| Homebrew `mpv` (0.41.0 at check time) licence: GPL-2.0-or-later AND LGPL-2.1-or-later; installs `mpv.pc` for pkg-config | https://formulae.brew.sh/formula/mpv.html | seen 2026-11-02 |
| Symphonia (pure-Rust decoder) has AAC-LC but not HE-AAC → reason to use libmpv/FFmpeg instead | https://github.com/pdeljanov/Symphonia/wiki | seen 2026-11-02 |
| `souvlaki` macOS media controls require a window (issue #23); a terminal app must run the AppKit loop itself (cf. `plavno` using objc2) | https://github.com/Sinono3/souvlaki · https://docs.rs/crate/plavno/latest/source/src/macos_media_regression.rs | seen 2026-11-02 |
| Open-source prior art: LMS BBC Sounds plugin (Perl), Music Assistant BBC Sounds provider + `auntie-sounds` Python library — useful references for BBC endpoints | https://github.com/expectingtofly/LMS_BBC_Sounds_Plugin · https://auntie-sounds.readthedocs.io/en/latest/ | seen 2026-11-02 |

## Findings

(append below: date · fact · source · status)
