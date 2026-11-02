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

### 2026-11-02 · M0 toolchain (checked by running commands on the owner's Mac)

| Fact | Source | Status |
|---|---|---|
| macOS: Darwin 27.2.0, Apple Silicon (`aarch64-apple-darwin`) | `uname`, `rustup show` | VERIFIED 2026-11-02 |
| Rust stable 1.99.1 (2026-10-13), cargo 1.99.1; rustfmt + clippy installed | `rustc --version`, `rustup component list --installed` | VERIFIED 2026-11-02 |
| Homebrew 7.0.12; `mpv` 0.41.0_12 (licence GPL-2.0-or-later AND LGPL-2.1-or-later), built against FFmpeg 9.0.2 | `brew info --json=v2 mpv`, `mpv --version` | VERIFIED 2026-11-02 |
| `pkg-config --modversion mpv` = **2.5.0** – this is the libmpv *client API* version (`MPV_CLIENT_API_VERSION` = 2.5 in `client.h`), not the mpv release number | `mpv.pc`, `/opt/homebrew/Cellar/mpv/0.41.0_12/include/mpv/client.h:251` | VERIFIED 2026-11-02 |
| `mpv.pc` puts libmpv in `/opt/homebrew/Cellar/mpv/<ver>/lib`, which is not on the default linker search path | `mpv.pc` | VERIFIED 2026-11-02 |
| libmpv's install name is absolute (`/opt/homebrew/opt/mpv/lib/libmpv.2.dylib`), so binaries need no rpath and survive `brew upgrade mpv` while the ABI major stays 2 | `otool -D libmpv.dylib` | VERIFIED 2026-11-02 |
| `ffmpeg` 9.0.2_1 (Homebrew, GPL-3.0-or-later) – used only to generate test fixtures, not linked | `ffmpeg -version`, `brew info` | VERIFIED 2026-11-02 |
| `cargo-deny` 0.20.2 installed with `cargo install --locked cargo-deny` | `cargo deny --version` | VERIFIED 2026-11-02 |
| cargo-deny's `[[licenses.clarify]]` file hash is XxHash32 (seed 0) of the raw file bytes | `cargo-deny-0.20.2/src/lib.rs:439` | VERIFIED 2026-11-02 |

### 2026-11-02 · libmpv Rust bindings (crates.io API + crate sources)

| Fact | Source | Status |
|---|---|---|
| `libmpv2` 7.0.0, released 2026-10-30 (6.0.1 on 2026-10-29, 6.0.0 on 2026-05-12), ~52k recent downloads, licence field `LGPL-2.1`, repo kohsine/libmpv-rs; requires libmpv ≥ 2.0 (mpv ≥ 0.35) | https://crates.io/api/v1/crates/libmpv2 · https://github.com/kohsine/libmpv-rs | VERIFIED 2026-11-02 |
| `libmpv2-sys` 4.0.1 (2025-12-29) ships pregenerated bindings for client API **2.5** (same as installed mpv); its `build.rs` only emits `cargo:rustc-link-lib=mpv` (no search path); optional `use-bindgen` feature | crate source `libmpv2-sys-4.0.1/build.rs`, `include/client.h:251` | VERIFIED 2026-11-02 |
| `libmpv2` checks only the client API *major* at `Mpv::new()`; exposes `observe_property`, `wait_event`, `get/set_property`, `command`; `render` feature (default) is only for video rendering | crate source `libmpv2-7.0.0/src/{lib,mpv}.rs` | VERIFIED 2026-11-02 |
| `libmpv2`/`libmpv2-sys` LICENSE file is the plain LGPL-2.1 text (the "any later version" sentence appears only in the "How to Apply" appendix template) → treat as LGPL-2.1-only | crate source `LICENSE` | VERIFIED 2026-11-02 |
| `libmpv` 2.0.1 / `libmpv-sys` 3.1.0 (ParadoxSpiral) last released 2020 – unmaintained, predates libmpv API 2.x | https://crates.io/api/v1/crates/libmpv | VERIFIED 2026-11-02 |
| `mpv-client-dyn` 0.5.0 (2023-11-16) is for writing mpv *plugins* (cplugins), not embedding | https://crates.io/api/v1/crates/mpv-client-dyn | VERIFIED 2026-11-02 |
| Linking + init verified by test `tuner-engine::tests::libmpv_links_and_reports_version` (`mpv-version` property starts with `mpv `) | `cargo test` | VERIFIED 2026-11-02 |
