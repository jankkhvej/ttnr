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
| Worldwide HLS pattern example: `http://as-hls-ww-live.akamaized.net/pool_904/live/ww/bbc_6music/bbc_6music.isml/bbc_6music-audio%3d96000.norewind.m3u8` | https://helpdesk.midwich.com/support/solutions/articles/50000141277 | STALE 2026-11-03: `pool_904` path returns 410 Gone; pool numbers change (now e.g. `pool_81827798`) |
| 320 kbps AAC streams are geo-locked to the UK; 96 kbps worldwide | https://moodeaudio.org/forum/showthread.php?pid=49213 | partly STALE 2026-11-03: from Poland the media selector labels the stream 320 but the `nonuk` master has one 96k HE-AAC variant |
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

### 2026-11-02 · Licensing (for ADR-007)

| Fact | Source | Status |
|---|---|---|
| mpv's meson option `gpl` defaults to `true` ("GPL (version 2 or later) build"); without GPL-only files mpv/libmpv is LGPLv2.1+, but `-Dgpl=false` "does not in itself create a LGPLv2.1+ license grant" | https://raw.githubusercontent.com/mpv-player/mpv/v0.41.0/meson.options · https://raw.githubusercontent.com/mpv-player/mpv/v0.41.0/Copyright | VERIFIED 2026-11-02 |
| Homebrew `mpv` formula passes no `-Dgpl=false` (→ GPL build); licence `all_of: GPL-2.0-or-later, LGPL-2.1-or-later` | https://github.com/Homebrew/homebrew-core/blob/HEAD/Formula/m/mpv.rb | VERIFIED 2026-11-02 |
| Homebrew `ffmpeg` is built with `--enable-gpl --enable-version3` (plus libx264/libx265) → GPL-3.0-or-later | https://github.com/Homebrew/homebrew-core/blob/HEAD/Formula/f/ffmpeg.rb | VERIFIED 2026-11-02 |
| FSF: FreeBSD licence (2-clause BSD), Modified BSD (3-clause) and ISC are lax permissive licences, compatible with the GNU GPL | https://www.gnu.org/licenses/license-list.html#FreeBSD | VERIFIED 2026-11-02 |
| FreeBSD project licence is the 2-clause BSD licence; licence text taken from SPDX `BSD-2-Clause` | https://www.freebsd.org/copyright/freebsd-license/ · https://github.com/spdx/license-list-data/blob/main/text/BSD-2-Clause.txt | VERIFIED 2026-11-02 |

### 2026-11-02 · Project name collisions (crates.io, PyPI, npm, Homebrew, Debian, GitHub, local `man`/`PATH`)

| Name | Finding | Status |
|---|---|---|
| `tnr` | **Taken on macOS:** Thunder Compute's official GPU-cloud CLI is the `tnr` command (Go binary, macOS `.pkg`, Homebrew tap `Thunder-Compute/tnr`); also crates.io `tnr` 0.2.0 = macOS notification CLI; PyPI `tnr` (deprecated Thunder CLI); npm `tnr` "Thunder command line utility" | VERIFIED 2026-11-02 · https://github.com/Thunder-Compute/thunder-cli · https://crates.io/crates/tnr |
| `ttnr` | No crates.io/PyPI/npm package, no Homebrew formula/cask, no Debian source package, not on local `PATH`/`man`; GitHub: only tiny unrelated repos (≤1 star) | VERIFIED 2026-11-02 |
| `tuner` | No crates.io crate, no Homebrew formula; npm `tuner` = "play a roster of internet radio stations" (same domain); PyPI `tuner` = Kivy instrument tuner | VERIFIED 2026-11-02 |

### 2026-11-02 · mpv 0.41.0 options and commands used by `MpvPlayer`

| Fact | Source | Status |
|---|---|---|
| All options set by `MpvPlayer` exist in the installed mpv 0.41.0: `vid` (`video` is an alias), `audio-display`, `force-window`, `ytdl`, `config`, `load-scripts`, `terminal`, `osc`, `input-default-bindings`, `input-vo-keyboard`, `input-media-keys`, `idle`, `audio-client-name`, `ao`, `audio-channels`, `volume`, `audio-device`, `http-header-fields` | `mpv --list-options` (local, 0.41.0) | VERIFIED 2026-11-02 |
| libmpv's built-in profile already sets `config=no idle=yes terminal=no input-terminal=no osc=no input-default-bindings=no input-vo-keyboard=no input-media-keys=no media-controls=no` (ttnr sets them explicitly anyway) | `mpv --show-profile=libmpv` | VERIFIED 2026-11-02 |
| `--vid=no` disables video playback; `--audio-display=no` disables cover-art display (which would otherwise appear as a video track) | https://github.com/mpv-player/mpv/blob/v0.41.0/DOCS/man/options.rst | VERIFIED 2026-11-02 |
| `--audio-channels`: default `auto-safe` = system's preferred layout (stereo if none); `auto` = whatever the device accepts, preferring the source layout, with a warning about HDMI issues | same | VERIFIED 2026-11-02 |
| `--volume` always controls mpv's internal (software) mixer since 0.18.1 → app volume independent of system volume (SPEC F3.6); `volume-max` default 130 | same; `mpv --list-options` | VERIFIED 2026-11-02 |
| `--audio-device=auto` follows the default device; names come from the `audio-device-list` property | same | VERIFIED 2026-11-02 |
| `--load-scripts` / `--config` can only be set before `mpv_initialize` | `libmpv2` docs for `Mpv::with_initializer` | VERIFIED 2026-11-02 |
| String-list options: `-append` adds one item verbatim (no escaping, may contain `,`); `-clr` clears; via API: `change-list <name> <op> <value>` (value `""` for ops without one) | https://github.com/mpv-player/mpv/blob/v0.41.0/DOCS/man/mpv.rst (List Options) · https://github.com/mpv-player/mpv/blob/v0.41.0/DOCS/man/input.rst | VERIFIED 2026-11-02 (and by test `headers_are_passed_verbatim_and_replaced_per_play`) |
| `loadfile <url> replace` stops the current file and starts the new one; returns before loading completes | input.rst | VERIFIED 2026-11-02 |
| `--media-controls` is Windows-only | options.rst | VERIFIED 2026-11-02 |
| Lead for M6: `--input-media-keys` (macOS/Windows) lets mpv receive media keys; whether this works in libmpv inside a windowless terminal app is unknown | options.rst | UNVERIFIED |
| `ao=coreaudio` opens on the owner's Mac (`current-ao` = `coreaudio`) | ignored test `coreaudio_output_opens` | VERIFIED 2026-11-02 |

### 2026-11-02 · mpv events, logs and HTTP errors (for engine events)

| Fact | Source | Status |
|---|---|---|
| FFmpeg network errors as seen through mpv 0.41.0 / FFmpeg 9.0.2 logs (prefix `ffmpeg`, level warn): `http: HTTP error 403 Forbidden`, `http: HTTP error 404 Not Found`, `http: HTTP error 500 Internal Server Error`; connection: `tcp: Connection to tcp://H:P failed: Connection refused` / `… Operation timed out`; DNS: `tcp: Failed to resolve hostname …`. Same `http:` line for an HLS playlist 403 and for HLS segment 403s (then `[ffmpeg/demuxer] hls: …` lines) | local experiment with `mpv --msg-level=all=warn` against a loopback HTTP server; regression tests in `crates/ttnr-engine/tests/events.rs` | VERIFIED 2026-11-02 |
| mpv exposes no HTTP status property; a failed open ends with `end-file` reason error, code `MPV_ERROR_LOADING_FAILED` (-13) | same experiment (`EndFile(Error(-13))`) | VERIFIED 2026-11-02 |
| `mpv_wait_event` returns queued events first, then property changes, then log messages → a log line explaining a failure can be delivered *after* the `end-file` event | https://github.com/mpv-player/mpv/blob/v0.41.0/player/client.c (`mpv_wait_event`) | VERIFIED 2026-11-02 |
| FFmpeg's log callback is process-global; mpv routes it only to the **first** mpv core initialised in the process (`log_mpv_instance`), re-registered only after that core is destroyed. Other concurrent cores get no `[ffmpeg]` log lines | https://github.com/mpv-player/mpv/blob/v0.41.0/common/av_log.c (`init_libav`, `mp_msg_av_log_callback`) | VERIFIED 2026-11-02 (reproduced: parallel tests lost the lines) |
| `libmpv2` 7.0: no safe wrapper for `mpv_request_log_messages` (has `Event::LogMessage`, marked "has not been tested"); `Mpv::ctx` is public so `libmpv2_sys::mpv_request_log_messages` can be called; `wait_event` returns an end-file *with error* as `Err(Error::Raw(code))`, losing the reason; `PropertyData::from_raw` panics (`unimplemented!`) for `MPV_FORMAT_NODE`, so never observe properties as `Format::Node` | crate source `libmpv2-7.0.0/src/mpv.rs`, `src/mpv/events.rs` | VERIFIED 2026-11-02 |
| `wait_event(-1.0)` blocks indefinitely (timeout < 0 → 1e20 s): no polling while idle | player/client.c | VERIFIED 2026-11-02 |
| The `quit` command makes mpv emit `MPV_EVENT_SHUTDOWN` to clients | test `drop_shuts_down_and_closes_channel` | VERIFIED 2026-11-02 |

### 2026-11-02 · Stream format properties (for `StreamInfo`)

| Fact | Source | Status |
|---|---|---|
| Documented in 0.41.0: `audio-params/{format,samplerate,channels,hr-channels,channel-count}` (decoder output), `audio-out-params` (same, as sent to the audio API), `audio-bitrate` (packet-level estimate, bits/s, throttled updates), `file-format` (may be a comma list, e.g. `mov,mp4,m4a,3gp,3g2,mj2`), `current-tracks/audio/…` → `track-list/N/{codec,codec-desc,codec-profile,demux-bitrate,hls-bitrate,demux-samplerate,demux-channels}` | https://github.com/mpv-player/mpv/blob/v0.41.0/DOCS/man/input.rst | VERIFIED 2026-11-02 |
| `audio-codec-name` still works but is not described in the 0.41.0 property list (only in interface-changes as the replacement of `audio-format`) → ttnr uses `current-tracks/audio/codec` | input.rst, https://github.com/mpv-player/mpv/blob/v0.41.0/DOCS/interface-changes.rst | VERIFIED 2026-11-02 |
| mpv reports no source bit depth: 24-bit FLAC/ALAC decode to `s32`/`s32p` (ffprobe `bits_per_raw_sample=24`), 16-bit FLAC to `s16`, lossy codecs to `floatp` → bit depth is shown only for `u8`/`s16` | local test files from `ffmpeg 9.0.2` + `mpv --term-playing-msg` | VERIFIED 2026-11-02 |
| Measured on test files: AAC-LC → `codec=aac prof=LC`; AC-3 5.1 → `hr-channels=5.1(side)`, `demux-bitrate=448000`; ALAC/AAC in MP4 have `demux-bitrate`, FLAC does not | same | VERIFIED 2026-11-02 |
| During end-of-file teardown, stream properties become unavailable one by one *before* the `end-file` event is delivered | events test trace, 2026-11-02 | VERIFIED 2026-11-02 |
| Homebrew FFmpeg 9.0.2 has the `aac_at` (AudioToolbox) encoder besides native `aac`; native `aac` cannot encode HE-AAC; whether `aac_at` can produce HE-AAC v1/v2 fixtures is to be checked in M1 task 5 | `ffmpeg -encoders` | UNVERIFIED (HE-AAC part) |

### 2026-11-02 · CLI, logging and smoke-test streams (for `ttnr play`)

| Fact | Source | Status |
|---|---|---|
| XDG Base Directory spec 0.8: `$XDG_STATE_HOME` default `$HOME/.local/state` when unset or empty; relative paths in `XDG_*` must be ignored | https://specifications.freedesktop.org/basedir-spec/latest/ | VERIFIED 2026-11-02 |
| `tracing-appender` 0.2.5: `RollingFileAppender::builder().rotation(Rotation::DAILY).filename_prefix(..).filename_suffix(..).max_log_files(n).build(dir)`; implements `MakeWriter` (sync writes); file name `ttnr.YYYY-MM-DD.log` | crate source `tracing-appender-0.2.5/src/rolling{,/builder}.rs`; CLI test | VERIFIED 2026-11-02 |
| `tracing-subscriber` 0.3.23 `Targets` implements `FromStr` (filter without the regex-based `env-filter` feature) | crate source `src/filter/targets.rs` | VERIFIED 2026-11-02 |
| mpv 0.41.0 built-in scripts are controlled by `load-auto-profiles`, `load-commands`, `load-console`, `load-context-menu`, `load-positioning`, `load-select`, `load-stats-overlay` (plus `osc`, `ytdl`); `auto_profiles` exits mid-hook without conditional profiles → `[cplayer] client removed during hook handling` warning | `mpv --list-options`, `mpv -v` trace | VERIFIED 2026-11-02 |
| Lead for M6: Homebrew mpv 0.41.0 is built with the `macos-media-player` feature (mpv's own Now Playing / remote-command integration); whether it works inside libmpv in a windowless terminal app, and how to toggle it, is unknown | `mpv -v` "List of enabled features" | UNVERIFIED |
| Smoke-test streams (played with `ttnr play`, `ao=null`): SomaFM Groove Salad Icecast MP3 `https://ice6.somafm.com/groovesalad-128-mp3` (from `https://somafm.com/groovesalad.pls`; MP3 44.1 kHz stereo 128 kbps); FIP (Radio France) HLS `https://stream.radiofrance.fr/fip/fip.m3u8?id=radiofrance` (master with lofi/midfi/hifi AAC-LC variants 107/185/280 kbit/s; mpv picked hifi: AAC LC 48 kHz stereo 280 kbps) | manual run 2026-11-02 | seen 2026-11-02 (third-party URLs, may change) |

### 2026-11-02 · Format matrix (fixtures from Homebrew FFmpeg 9.0.2, played by mpv 0.41.0)

| Fact | Source | Status |
|---|---|---|
| Encoders available: `libmp3lame`, `aac` (native, LC only), `aac_at` (AudioToolbox), `libopus`, `vorbis` (native, experimental → `-strict experimental`), `flac`, `alac`, `ac3`, `eac3`, `pcm_s24le`; **no** `libvorbis`, no `libfdk_aac` | `ffmpeg -encoders` | VERIFIED 2026-11-02 |
| HE-AAC fixtures: `aac_at` takes numeric profiles – `-profile:a 4` (AV_PROFILE_AAC_HE) → ffprobe/mpv `HE-AAC`, `-profile:a 28` (AV_PROFILE_AAC_HE_V2) → `HE-AACv2`; names like `aac_he` are rejected | `/opt/homebrew/include/libavcodec/defs.h:72-73`; generated files probed with ffprobe and mpv | VERIFIED 2026-11-02 |
| FFmpeg's `eac3` encoder supports at most 5.1 → no E-AC-3 7.1 fixture can be generated locally; 7.1 is covered by AAC-LC 7.1 and Opus 7.1 | `ffmpeg -h encoder=eac3` (channel layouts) | VERIFIED 2026-11-02 |
| **No MPEG-DASH demuxer** in Homebrew FFmpeg 9.0.2 (`ffmpeg -demuxers` lists only `hls` and `webm_dash_manifest`; the formula does not pass `--enable-libxml2`); mpv 0.41.0 has no own DASH demuxer (`mpv --demuxer=help`) → `.mpd` cannot be played with the current Homebrew stack (SPEC F1.1 gap) | `ffmpeg -demuxers`, `mpv --demuxer=help`, https://github.com/Homebrew/homebrew-core/blob/HEAD/Formula/f/ffmpeg.rb | VERIFIED 2026-11-02 |
| mpv reports for the fixtures: codec/profile/rate/channels/container as in `crates/ttnr-engine/tests/format_matrix.rs` (e.g. `ac3_51` → `5.1(side)`, `aac_71`/`opus_71` → `7.1`, local HLS → container `hls`, ADTS → `aac`, Ogg → `ogg`) | format-matrix test | VERIFIED 2026-11-02 |
| `track-list/N/codec-profile` is "available only if the track has been already decoded" → it can arrive after the other stream facts | input.rst 0.41.0 | VERIFIED 2026-11-02 |
| **mpv 0.41.0 observer quirk:** playing an AAC file right after another AAC file (`loadfile … replace`), the observer gets `audio-params/*` and `codec-profile` as unavailable during teardown and is never re-notified for the new file, although `mpv_get_property` returns the values (reproduced: `aac_lc.m4a` → `he_aac_v2.m4a`; after an MP3 it works). Workaround in ttnr: re-read stream properties on `file-loaded`, `playback-restart` and `audio-reconfig` | local reproduction, 2026-11-02; no matching upstream issue found by a web search of mpv issues (2026-11-02) | VERIFIED locally; not reported upstream |

### 2026-11-02 · CoreAudio output bug in mpv 0.41.0 on macOS 27; channel-check fixtures

| Fact | Source | Status |
|---|---|---|
| mpv 0.41.0 `ao=coreaudio` fails AO init on macOS 27 for planar-float or mono sources: `unable to set the input channel layout on the audio unit (-50)`; reproduced on the owner's Mac (output "MAJOR IV", Bluetooth) for MP3/AAC/Vorbis 44.1 kHz floatp and mono, while packed float, s16 and Opus work | local runs 2026-11-02; https://github.com/mpv-player/mpv/issues/18384 ("unable to set the input channel layout on coreaudio and macOS 27", closed 2026-11-01) | VERIFIED 2026-11-02 |
| Cause: v0.41.0 `init_audiounit()` passes an `AudioChannelLayout` to `kAudioOutputUnitProperty_ChannelMap` (expects a channel-index map). Fixed on master by PR #18679 "ao_coreaudio: fixes for channel and layout mapping" (merged 2026-11-01, milestone v0.42.0): master only *reads* the channel map. Not in any release yet | https://github.com/mpv-player/mpv/pull/18679 · https://github.com/mpv-player/mpv/blob/v0.41.0/audio/out/ao_coreaudio.c · https://github.com/mpv-player/mpv/blob/master/audio/out/ao_coreaudio.c | VERIFIED 2026-11-02 |
| Homebrew `mpv` stable is still 0.41.0 revision 12 (no #18679 backport); `brew install --HEAD mpv` would build master | https://formulae.brew.sh/api/formula/mpv.json | VERIFIED 2026-11-02 (HEAD build not tried) |
| `--ao` is a priority list (`--ao=<driver1,driver2,...[,]>`); `ao=coreaudio,avfoundation` falls back to `avfoundation` (AVSampleBufferAudioRenderer) when CoreAudio init fails – verified: MP3/Vorbis/AAC/mono play, 5.1/7.1 downmixed to the stereo headset | https://github.com/mpv-player/mpv/blob/v0.41.0/DOCS/man/ao.rst; local runs | VERIFIED 2026-11-02 |
| Known `avfoundation` issue: the tail of every file is dropped at EOF (open) – irrelevant for live streams | https://github.com/mpv-player/mpv/issues/18461 | VERIFIED 2026-11-02 |
| `--coreaudio-change-physical-format` (default no): "multichannel audio output will actually work" but changes system-wide device format – candidate for SPEC F2.3 options in M6 | ao.rst 0.41.0 | VERIFIED 2026-11-02 |
| Channel-ID test practice: ALSA `speaker-test` steps channel by channel, default pink noise, optional SMPTE ST 2095 band-limited pink noise at -18.5 dBFS; ttnr's `channel_check_*.flac` follow that (1.5 s pink noise per channel, LFE 60 Hz sine, ~-20 dBFS, 0.5 s gaps, 2 cycles); FFmpeg orders: `5.1(side)` FL FR FC LFE SL SR, `7.1` FL FR FC LFE BL BR SL SR; levels/windows verified per channel with `volumedetect` | https://manpages.debian.org/testing/alsa-utils/speaker-test.1.en.html · `ffmpeg -layouts` | VERIFIED 2026-11-02 (Dolby/ITU channel-ID specs not consulted) |
| FFmpeg `join` maps inputs by channel name: a mono input is FC and lands in the FC slot unless `map=` is given | observed while building the channel-check fixtures | VERIFIED 2026-11-02 |

### 2026-11-02 · Owner review of M1: downmix, DASH prevalence, AVFoundation vs CoreAudio, local libmpv build

| Fact | Source | Status |
|---|---|---|
| FFmpeg swresample downmix defaults: `lfe_mix_level` 0 (LFE dropped), `center_mix_level` 0.707107, `surround_mix_level` 0.707107 (−3 dB) – explains the owner's by-ear result for `channel_check_51.flac` on a stereo headset: L, R, C (both ears), silence (LFE), quieter SL, SR | `ffmpeg -h full` (FFmpeg 9.0.2) | VERIFIED 2026-11-02 |
| radio-browser.info `/json/stations?hidebroken=true&limit=1000000`: 62,042 stations; 0 DASH (`.mpd`) URLs; 4,086 HLS (6.6 %); 117 playlist URLs; codecs MP3 42,171 · AAC+ 8,861 · AAC 8,320 · OGG 622 | https://de1.api.radio-browser.info/json/stations (fetched 2026-11-02) | VERIFIED 2026-11-02 |
| `ao_avfoundation` (0.41.0): `device_buffer = samplerate * 2` ("read ahead aggressively"); supports `audio-device` (`setAudioOutputDeviceUniqueID`) and `list_devs`, no `hotplug_init` (CoreAudio AO has hotplug); interleaved formats only; no S/PDIF | https://github.com/mpv-player/mpv/blob/v0.41.0/audio/out/ao_avfoundation.m · ao_coreaudio.c | VERIFIED 2026-11-02 |
| "Muting is delayed with ao_avfoundation" (open); a `--avfoundation-buffer` option was proposed but does not exist in 0.41.0 (`mpv --list-options`) | https://github.com/mpv-player/mpv/issues/15014 | VERIFIED 2026-11-02 |
| Homebrew mpv formula: tarball `v0.41.0` sha256 `ee21092a…6209`; backports 75b2ccfe (#17731) sha256 `3906b98b…0c1f` and c5d391ad (#18383) sha256 `769b218d…e924`; PR #18679 patch (8 commits) sha256 `ec23e46f…e55d` at fetch time | https://github.com/Homebrew/homebrew-core/blob/HEAD/Formula/m/mpv.rb · https://github.com/mpv-player/mpv/pull/18679.patch | VERIFIED 2026-11-02 |
| mpv 0.41.0 requires meson ≥ 1.3.0; meson 1.12.1 and ninja 1.13.2 from PyPI (Apache-2.0) build it; configure with `-Dlibmpv=true -Dcplayer=false` and Lua/JS/etc. disabled works with Homebrew's ffmpeg/libplacebo/libass; ~18 s build; enabled features include coreaudio, avfoundation, macos-media-player | `meson.build`; https://pypi.org/project/meson/ · https://pypi.org/project/ninja/; local build | VERIFIED 2026-11-02 |
| With the patched local libmpv, MP3/Vorbis/AAC 44.1 kHz, mono and AC-3 5.1 open `ao=coreaudio` on the owner's Mac (log `audio output ao="coreaudio"`) | local runs | VERIFIED 2026-11-02 |
| Without Lua, the options `load-auto-profiles`, `load-console`, … do not exist (`MPV_ERROR_OPTION_NOT_FOUND`, −5) | local build | VERIFIED 2026-11-02 |
| `git apply` run inside a subdirectory of another git repo silently skips patch paths outside that subdirectory (reported success, applied nothing) → use `patch -p1` | observed 2026-11-02 | VERIFIED 2026-11-02 |

### 2026-11-02 · M2 source traits

| Fact | Source | Status |
|---|---|---|
| Rust 1.99.1: a trait with `async fn` is still not dyn-compatible (`E0038 … because method is async`) → `async-trait` 0.1.92 for `dyn Source` / `dyn Http` / `dyn MetadataProvider` | `rustc --edition 2024` on a minimal example | VERIFIED 2026-11-02 |

### 2026-11-02 · Playlists, HTTP client, `url` source

| Fact | Source | Status |
|---|---|---|
| mpv's playlist demuxer reads 8 KiB to probe, strips a BOM, forces format by MIME when it matches, needs `#EXTM3U` on the first line for M3U (headerless only by extension, "unsafe" level), `[playlist]` (case-insensitive) for PLS; no ASX/XSPF parsing there | https://github.com/mpv-player/mpv/blob/master/demux/demux_playlist.c (via research subagent) | seen 2026-11-02 |
| HLS = `#EXTM3U` + any `#EXT-X-` tag (RFC 8216); `.m3u8`/`application/vnd.apple.mpegurl` alone does not imply HLS (UTF-8 plain playlists use `.m3u8` too) | https://www.rfc-editor.org/rfc/rfc8216 | VERIFIED 2026-11-02 (spec), behaviour pinned by tests |
| Real playlists (recorded to `tests/fixtures/playlists/real/`): PLS with CRLF and lowercase `numberofentries` (StreamTheWorld, SomaFM; `audio/x-scpls`), headerless and extended M3U (`audio/x-mpegurl`), ASX with `href = "…"` spacing (`video/x-ms-asx`, `video/x-ms-asf`), XSPF with `&amp;` (`application/xspf+xml`), HLS master with relative variant and HLS media playlist (`application/vnd.apple.mpegurl`) | recorded from radio-browser's most-voted playlist URLs | VERIFIED 2026-11-02 |
| reqwest 0.13.5: default TLS is rustls + aws-lc-rs (+ rustls-platform-verifier); `native-tls` feature uses Security.framework on macOS (security-framework, MIT/Apache-2.0), no C/cmake build; `Response::chunk()` reads incrementally, dropping the response aborts the body; `ClientBuilder::{read_timeout, connect_timeout, redirect, user_agent}` exist | crate source `reqwest-0.13.5/Cargo.toml`, `src/async_impl/{client,response}.rs` | VERIFIED 2026-11-02 |
| A Shoutcast v1 `ICY 200 OK` status line makes reqwest/hyper fail the request → `UrlSource` hands such URLs straight to mpv | test `shoutcast_v1_status_line_is_an_error` (loopback server) | VERIFIED 2026-11-02 |
| HEAD is unreliable on Icecast mounts → probe with GET and drop after the headers | research subagent (general knowledge, not from a primary source) | UNVERIFIED (GET+drop used regardless) |
| With all targets, `native-tls` pulls `openssl` (+ `openssl-macros` → duplicate `syn` 2) for Linux; on macOS it uses security-framework only | `cargo tree -d --target all` | VERIFIED 2026-11-02 |
| `ttnr play` resolves and plays SomaFM `.pls`, WDVX `.xspf`, TranceBase `.asx`, 101 Smooth Jazz `.m3u` | manual run 2026-11-02 | seen 2026-11-02 |

### 2026-11-02 · stations.toml

| Fact | Source | Status |
|---|---|---|
| `toml` 1.1.8 implements TOML spec 1.1.0 (crate version suffix `+spec-1.1.0`) | https://crates.io/crates/toml | VERIFIED 2026-11-02 |
| BBC service ids `bbc_6music`, `bbc_radio_three` in `examples/stations.toml` | `rms.api.bbc.co.uk/v2/networks` | VERIFIED 2026-11-03 |

### 2026-11-02 · In-stream metadata: ICY and HLS ID3

| Fact | Source | Status |
|---|---|---|
| FFmpeg http requests ICY by default (`icy=1` → `Icy-MetaData: 1`); mpv's `stream_lavf.c` `read_icy()` turns `icy_metadata_headers` into tags (e.g. `icy-name`, `icy-genre`, `icy-br`) and `StreamTitle` into `icy-title` (charset guessed, converted to UTF-8) | https://github.com/FFmpeg/FFmpeg/blob/master/libavformat/http.c · https://github.com/mpv-player/mpv/blob/v0.41.0/stream/stream_lavf.c (via research subagent); behaviour confirmed by test `icy_titles_become_tags` | VERIFIED 2026-11-02 |
| mpv notifies `metadata`, `filtered-metadata`, `media-title` on `MP_EVENT_METADATA_UPDATE`; `metadata/by-key/<key>` sub-properties are not in the event table → observe `metadata` and re-read keys | https://github.com/mpv-player/mpv/blob/v0.41.0/player/command.c (property/event table, via subagent) | VERIFIED 2026-11-02 (works in our engine test) |
| At playback start `icy-title` is unavailable until the first metadata block (after `icy-metaint` bytes); `icy-name`/`icy-genre` are there from the headers (SomaFM) | `mpv --term-playing-msg` on SomaFM | VERIFIED 2026-11-02 |
| FFmpeg 9.0.2 `hls.c`: TS segments' timed-ID3 data streams are propagated as metadata updates (`timed_id3_metadata`, `metadata_has_changed`); in packed-audio segments only the initial ID3 is used, later changes log "Changing ID3 metadata in HLS audio elementary stream" (missing feature) | https://github.com/FFmpeg/FFmpeg/blob/n9.0.2/libavformat/hls.c (lines ~187, 1312, 2677, 2844) | VERIFIED 2026-11-02 |
| mpv 0.41.0 shows **no** metadata keys for a packed-AAC HLS stream whose segments start with ID3 TIT2/TPE1 (`metadata/list/count` = 0) | `mpv --term-playing-msg` on https://db2.indexcom.com/bucket/ram/00/05/05.m3u8 | VERIFIED 2026-11-02 |
| That stream (StreamS encoder): each `.aac` segment starts with two ID3v2.4 tags – PRIV `com.apple.streaming.transportStreamTimestamp` (73 B), then TIT2/TPE1/TALB/TRSN/WXXX `artworkURL_640x`… (984 B); audio from byte 1057 | recorded segment, `tests/fixtures/id3/streams_packed_aac.id3` | VERIFIED 2026-11-02 |
| FIP (Radio France) HLS uses MPEG-TS segments with an AAC stream and a `timed_id3` data stream, but the ID3 packets are empty (10-byte header, no frames) → no now-playing via ID3 | ffprobe/ffmpeg on a live segment | VERIFIED 2026-11-02 |
| A TS timed-ID3 stream with real frames reaching mpv's `metadata` is untested (no public example found) | – | UNVERIFIED |
| ID3v2.4 / v2.3 structure (syncsafe sizes in v2.4 frames, text encodings 0–3, WXXX layout) | https://id3.org/id3v2.4.0-structure · https://id3.org/id3v2.3.0 | from spec knowledge; pinned by unit tests | 

### 2026-11-02 · TUI stack (M3)

| Fact | Source | Status |
|---|---|---|
| ratatui 0.30.2 (MIT, MSRV 1.88) – apps depend on the `ratatui` facade (split into ratatui-core/-widgets/-crossterm internally); default `crossterm` feature uses crossterm 0.29; `TestBackend` re-exported without extra features | https://crates.io/api/v1/crates/ratatui · https://docs.rs/ratatui/0.30.2 | VERIFIED 2026-11-02 |
| crossterm 0.29.0 (MIT); `EventStream` needs feature `event-stream` (+ `futures`); same version unifies with ratatui's re-export `ratatui::crossterm` | https://crates.io/api/v1/crates/crossterm · https://ratatui.rs/tutorials/counter-async-app/async-event-stream/ | VERIFIED 2026-11-02 (builds) |
| insta 1.49.0 (Apache-2.0); futures 0.3.34 (MIT OR Apache-2.0) | crates.io API | VERIFIED 2026-11-02 |
| `ratatui::init/try_init` enter raw mode + alternate screen and install a panic hook that restores the terminal; `restore()` must be called on normal exit; Ctrl-C arrives as a key in raw mode; SIGTERM is the app's job | https://github.com/ratatui/ratatui/blob/main/ratatui/src/init.rs (via research subagent) | VERIFIED 2026-11-02 (pty run shows `?1049h`/`?1049l`) |
| No built-in redraw-on-change; ratatui diffs buffers so a redraw only writes changed cells (observed: "Local Test" drawn over "no station" writes only changed letters) | research subagent; pty run | VERIFIED 2026-11-02 |
| `TestBackend` Display (used by insta snapshots) contains symbols only, no colours/styles | https://ratatui.rs/recipes/testing/snapshots/ | VERIFIED 2026-11-02 (subagent, docs) |
| No colour-depth detection in crossterm/ratatui; convention: `NO_COLOR` → mono, `COLORTERM=truecolor|24bit` → truecolor, `TERM=*256color*` → 256, else 16; Terminal.app gained 24-bit colour in macOS 26 but is not known to set `COLORTERM` | https://no-color.org · https://www.macrumors.com/2025/06/16/apples-terminal-app-macos-tahoe/ | partly UNVERIFIED (Terminal.app env to be checked on the owner's Mac in task 4) |
| mpv `audio-device-list` read as a string is JSON `[{"name":"coreaudio/…","description":"…"}, …]` incl. `auto` and `avfoundation/…` duplicates | engine scratch run on the owner's Mac | VERIFIED 2026-11-02 |

### 2026-11-03 · Colour depth on the owner's Mac

| Fact | Source | Status |
|---|---|---|
| Owner's terminal: kitty – `TERM=xterm-kitty`, `COLORTERM=truecolor` | env in the session | VERIFIED 2026-11-03 |
| Terminal.app on this Mac: build 488.9, version 2.15 (macOS 26 brought 24-bit colour per press reports); whether it sets `COLORTERM` was not checked (needs the GUI app) → ttnr treats `TERM_PROGRAM=Apple_Terminal` as 256 colours unless `color = "truecolor"` | `PlistBuddy` on `/System/Applications/Utilities/Terminal.app/Contents/Info.plist`; https://www.macrumors.com/2025/06/16/apples-terminal-app-macos-tahoe/ | UNVERIFIED (truecolor detection) |
| xterm default RGB for ANSI 0–15 and the 6×6×6 cube levels (0, 95, 135, 175, 215, 255) + grey ramp 8 + 10·i are the usual 256-colour mapping | xterm `256colres.pl` / common knowledge | from knowledge; pinned by unit tests |

### 2026-11-03 · Owner review of M3: colours and resizing

| Fact | Source | Status |
|---|---|---|
| The owner's kitty scheme maps ANSI 8 (`bright_black`) to `#5a3625` (dark brown), so `plain`'s secondary text/borders looked brown; palette-relative colours vary per terminal scheme → secondary text now uses SGR 2 (faint) on the default colour | `~/.config/kitty/kitty.conf` (read-only); owner screenshot | VERIFIED 2026-11-03 |
| ratatui-core 0.1.2: on any size change `Terminal::autoresize` → `resize` → `clear_viewport` (`ClearType::All` for fullscreen) before redrawing → a blank flash between clear and redraw | `ratatui-core-0.1.2/src/terminal/{resize,buffers}.rs` | VERIFIED 2026-11-03 |
| crossterm 0.29 provides `BeginSynchronizedUpdate` / `EndSynchronizedUpdate` (DEC private mode 2026; frame shown only when complete; ignored by terminals without support) | `crossterm-0.29.0/src/terminal.rs` | VERIFIED 2026-11-03 |
| Terminal.app (build 488.9): while the window is resized vertically the UI was shifted by one line until the next redraw; screen recording of an automated resize showed the top row missing on shrink and an extra row on grow. Parking the hidden cursor at (0,0) after each frame (plus synchronized output) removed it in the same recording | `screencapture -v` of a Terminal.app window resized via AppleScript (recordings deleted after analysis) | VERIFIED 2026-11-03 (Terminal.app); kitty/iTerm2 to be checked by the owner |
| kitty 0.49.2 on the owner's Mac: during a resize the program is not told about the new size until the drag pauses/ends (`resize_debounce_time`, default `0.1 0.5`; where the OS reports resize start/end – macOS – "redraw is immediate after end of resize"); meanwhile kitty draws its own size overlay (the black boxes). The owner's recording shows exactly one fully blank frame at the start of each resize (window not yet resized, ttnr not yet signalled) – kitty's own rendering, not caused by ttnr, which already uses synchronized output | https://sw.kovidgoyal.net/kitty/conf/ (resize_debounce_time); frame analysis of the owner's screen recording 2026-11-03 (frames deleted after analysis) | VERIFIED 2026-11-03 (cause attributed by elimination; compare with another full-screen TUI in kitty to confirm) |
| iTerm2 3.7.6: shrinking the window width rewraps (reflows) the on-screen lines while dragging; the owner's recording showed the garbled frame *staying* ~1–2 s at a constant size afterwards, because ratatui's diff renderer assumed the screen still held its last frame (a resize event for an already-drawn size produced no output). Forcing a full repaint on every resize event (`Terminal::resize`, inside a synchronized update) makes the screen clean as soon as the drag stops; during the drag itself iTerm2 still shows its own reflow (terminal behaviour). Growing is clean throughout | owner's recording + own AppleScript-driven recording of iTerm2 (both analysed frame by frame, frames/recordings deleted) | VERIFIED 2026-11-03 |

### 2026-11-03 · BBC live radio (M4 research; checked from Kraków, Poland)

| Fact | Source | Status |
|---|---|---|
| Station list: `GET https://rms.api.bbc.co.uk/v2/networks?limit=100` (no auth) → 66 networks (19 national, 7 regional, 40 local), each `{id, coverage, short_title, long_title, default_service_id, services:[{id, pid, short_title}]}`; stream by **service id** (e.g. network `bbc_radio_four` → services `bbc_radio_fourfm`, `bbc_radio_fourlw`) | live request; https://github.com/kieranhogg/auntie-sounds/blob/main/src/auntie_sounds/endpoints.py | VERIFIED 2026-11-03 |
| World Service language services (`bbc_arabic_radio`, `bbc_hindi_radio`, …) are not in `/v2/networks` (auntie-sounds hard-codes them) | https://github.com/kieranhogg/auntie-sounds/blob/main/src/auntie_sounds/stations.py | seen 2026-11-03 (subagent), not re-tested |
| Old flow `mediaselector/6/select/version/2.0/mediaset/{pc,audio-syndication,audio-syndication-dash,apple-ipad-hls}/vpid/<id>` from Poland: `selectionunavailable` for 6 Music, Radio 1/3/4; World Service works (320k HLS) | live requests | VERIFIED 2026-11-03 |
| BBC web-player flow: `GET https://rms.api.bbc.co.uk/v2/sign/token/<service_id>` (no auth) → `{"token": JWT}` (~24 h, claims `cvids: ["urn:bbc:pips:pid:<id>"]`); then `GET https://open.live.bbc.co.uk/mediaselector/6/select/version/3.0/mediaset/pc/cvid/urn:bbc:pips:pid:<id>/format/json` with `Authorization: Bearer <JWT>` → `media[]` (`bitrate`, `encoding`), each with `connection[]` `{protocol, supplier (akamai_audio_live_ww/cloudfront_audio_live_ww), transferFormat (hls/dash), href}` | live requests; auntie-sounds `src/auntie_sounds/playback.py` `get_live_streams()` | VERIFIED 2026-11-03 |
| From Poland that flow returns `…/audio/simulcast/hls/nonuk/pc_hd_abr_v2/{aks,cfs}/<id>.m3u8` for 6 Music, Radio 4 FM (subagent also: Radio 1, 2, 3, 5 Live, Scotland FM, London); the `nonuk` master has one variant `BANDWIDTH=101760, CODECS="mp4a.40.5"` → `as-hls-ww-live.akamaized.net/pool_*/live/ww/<id>/<id>.isml/<id>-audio%3d96000.norewind.m3u8`; mpv plays it as HE-AAC 48 kHz stereo | live requests + `mpv` run | VERIFIED 2026-11-03 |
| BBC refuses rights-restricted services abroad even with a token: `bbc_radio_five_live_sports_extra` → `403 {"result":"geolocation"}` | live request | VERIFIED 2026-11-03 |
| Every media selector response carries a disclaimer ("part of the BBC iPlayer content protection system … unauthorised use … circumvention") | live responses | VERIFIED 2026-11-03 – owner decided to use the web-player flow (ADR-029) |
| BBC Sounds app/on-demand closed to non-UK users on 21 July 2025; live national, nations and local radio remains listenable abroad via BBC's web pages (bbc.com "Audio > Radio" → BBC Sounds live page), TuneIn, Apple Music; on-demand not; sports rights can remove stations (all but World Service close abroad during the Olympics) | https://james.cridland.net/blog/2025/how-to-listen-to-bbc-radio-overseas/ (updated 2026-04-04) · https://en.wikipedia.org/wiki/BBC_Sounds | VERIFIED 2026-11-03 (secondary sources + own requests) |
| auntie-sounds README warns BBC "is turning off non-UK access soon" (no date) | https://github.com/kieranhogg/auntie-sounds | risk, UNVERIFIED |
| Now playing: `GET https://rms.api.bbc.co.uk/v2/services/<id>/segments/latest?limit=1` (no auth) → `data[0]` `{segment_type:"music", titles:{primary: artist, secondary: title}, image_url (with {recipe}), offset:{start, end, label:"Now Playing", now_playing:true}}`; empty `data` during speech (seed, not re-tested) | live request | VERIFIED 2026-11-03 |
| Current programme: `GET https://rms.api.bbc.co.uk/v2/broadcasts/latest?service=<id>&on_air=now` → `data[0]` `{start, end, programme:{titles:{primary: presenter, secondary: show, display_title}}}` | live request | VERIFIED 2026-11-03 |
| `segments/latest` items carry `offset.now_playing`; right after a track ends the latest item has `now_playing: false` (label "Less Than a Minute Ago") – not a current track. Radio 4 during "Farming Today" returned `data: []` (speech). `broadcasts/latest` titles: music shows `primary` = presenter, `secondary` = show; speech `primary` = programme, `secondary` = episode | recorded fixtures `tests/fixtures/bbc/{segments,broadcast}_*.json` | VERIFIED 2026-11-03 |
| BBC `nonuk` media playlists carry `EXT-X-PROGRAM-DATE-TIME`, but the timestamps run ~30 s ahead of the wall clock at fetch time (5 × 6.4 s segments starting 06:00:06 at 06:00:08 UTC) → not usable to compute the audio delay; the sync offset is set by ear | media playlist fetched 2026-11-03 | VERIFIED 2026-11-03 |
| BBC `segments/latest` `offset.start`/`end` are seconds since the broadcast's `start` (06:13:09 UTC: broadcast 05:00:00, `progress` 4390 s; a track 4139–4322 s = 06:08:59–06:12:02, label "1 Minute Ago"). BBC lists the current track with a lag (for minutes only the previous, ended track was listed) | live requests 2026-11-03 | VERIFIED 2026-11-03 |

### 2026-11-03 · Retro themes (M5 research; via two research subagents + own checks)

| Fact | Source | Status |
|---|---|---|
| VT100: 80×24 characters, 132×14 (132×24 with Advanced Video Option); 12″ CRT, P4 phosphor; 7×9 dot matrix | VT100 User Guide ch. 1–2: https://vt100.net/docs/vt100-ug/chapter1.html · https://vt100.net/docs/vt100-ug/chapter2.html | VERIFIED 2026-11-03 |
| VT100 SGR 1 bold, 4 underscore, 5 blink, 7 reverse; DECSCNM reverse screen; DEC Special Graphics table (◆ ▒ ␉␌␍␊ ° ± ␤␋ ┘┐┌└┼ ⎺⎻─⎼⎽ ├┤┴┬│ ≤ ≥ π ≠ £ ·); DECDHL `ESC # 3`/`ESC # 4`, DECDWL `ESC # 6`, DECSWL `ESC # 5` | https://vt100.net/docs/vt100-ug/chapter3.html (Unicode mapping is ours) | VERIFIED 2026-11-03 |
| VT220 offered white, green or amber tubes (green/amber are not VT100 facts) | https://terminals-wiki.org/wiki/index.php/DEC_VT220 | seen 2026-11-03 (secondary) |
| Emulator colours (not measurements): cool-retro-term amber `#ff8100`, green `#0ccc68` on `#000000` | https://github.com/Swordfish90/cool-retro-term/blob/master/app/qml/ApplicationSettings.qml | VERIFIED 2026-11-03 |
| RT-11: monitor prompt `.`; `DATE [dd-mmm-yy]` (example `.DATE 18-MAY-91`); KMON errors `?KMON-F-…` (e.g. `?KMON-F-Illegal command`) | RT-11 Quick Reference Manual V5.6 (bitsavers AA-M241D-TC); https://www.5volts.ch/pages/dcj11sbc/200-dcj11-rt11/ | VERIFIED (date, prompt) / seen (KMON texts) 2026-11-03 |
| RSX-11M: implicit prompt `>`, explicit `MCR>` (CTRL/C); boot banner e.g. `RSX-11M V4.6 BL56 1912.K MAPPED` | RSX-11M Beginner's Guide DEC-11-OMBGA-A-D; System Generation and Installation Guide AA-H625D-TC (bitsavers); https://github.com/JamesHagerman/nankervis-pdp11-js/blob/master/ExampleBoots.md | VERIFIED via manual summaries 2026-11-03 |
| PDP-11/70 front panel: 22 address + 16 data lamps, octal labelling | https://www.pcjs.org/machines/dec/pdp11/panel/1170/ | seen 2026-11-03 |
| Teletype Model 33 ASR: upper case only, 72 characters per line, 10 cps (110 baud); 1963 ASCII ↑/← at today's `^`/`_` | https://www.pdp8online.com/asr33/asr33.shtml · https://gunkies.org/wiki/Teletype | VERIFIED (72/10 cps) / seen (↑←) 2026-11-03 |
| ZX80: 32×24 cells of 8×8; black on white, inverse = white on black, no colour, no lower case; 64 glyphs + inverse; 2×2 block graphics + 50 % grey; listing top / edit line bottom; inverse `K`/`L` cursor, inverse `S` for syntax errors; screen blank while computing (no FAST/SLOW); reports `n/m`, codes 0–9 (0 OK/BREAK, 1 NEXT without FOR, 2 variable not found, 3 subscript, 4 no room, 5 screen full, 6 overflow, 7 RETURN without GOSUB, 8 INPUT as command, 9 STOP) | ZX80 Operating Manual https://cdn.hackaday.io/files/1707077303528288/ZX80_OperatingManual.pdf · http://blog.tynemouthsoftware.co.uk/2023/10/how-the-zx80-generates-video.html · https://en.wikipedia.org/wiki/ZX80_character_set | VERIFIED 2026-11-03 (subagent read the manual) |
| SAA5050 / MODE 7: 40×25; 3-bit RGB colours; control codes &80–&9F occupy a cell; double height top/bottom rows; flash; contiguous/separated mosaics; hold graphics | http://mdfs.net/Info/Comp/Teletext/Controls · https://en.wikipedia.org/wiki/Mullard_SAA5050 | seen 2026-11-03 (secondary) |
| Teletext English set: `#`→£, `[`→←, `\`→½, `]`→→, `^`→↑, `_`→#, `` ` ``→—, `{`→¼, `\|`→‖, `}`→¾, `~`→÷ | https://en.wikipedia.org/wiki/Teletext_character_set | seen 2026-11-03 (secondary) |
| BBC Micro banner after hard reset `BBC Computer 32K`, `BASIC`, prompt `>` (`Acorn DFS` with discs); MODE 7 cursor end line 19 of a 20-line cell (bottom of the cell) | https://stardot.org.uk/forums/viewtopic.php?t=8884 · https://beebwiki.mdfs.net/CRTC | seen 2026-11-03; blank lines and cursor start line UNVERIFIED |
| Sextants U+1FB00–U+1FB3B (Unicode 13.0); not encoded: empty, ▌, ▐, █; separated mosaics in U+1CC00 block (Unicode 16.0) | https://www.unicode.org/charts/PDF/U1FB00.pdf · https://www.unicode.org/charts/PDF/U1CC00.pdf | VERIFIED (charts) 2026-11-03 |
| Menlo / SF Mono / Monaco lack sextants; kitty, Ghostty, WezTerm draw legacy-computing symbols themselves | https://github.com/circle-platforms/circle/issues/388 · https://arewelegacycomputingyet.com/ | seen 2026-11-03 (secondary) – needs a check in the owner's Terminal.app |

### 2026-11-03 · Why the built-in themes look like this (M5)

- `pdp11-vt100` (+`-amber`, `-green`): 80×24 logical screen, P4 white-on-black, one colour with bold for emphasis; borders and status glyphs (◆ ▒) from the DEC Special Graphics set; RSX-11M banner style `RSX-11M V4.6 BL56` and `MCR>` prompt; upper case and octal figures as DEC system output; errors `?TTNR-F-…` after the `?KMON-F-` / RSX message style. Amber/green = later DEC tubes (VT220), offered as variants. Sources: rows above (VT100 user guide, RT-11/RSX-11M manuals).
- `pdp11-teletype`: 72 columns, upper case, ASCII only, no borders, printed lines appended (Model 33 ASR); `^`/`_` as ↑/← (1963 ASCII). Paper and ink colours: design choice, UNVERIFIED.
- `zx80`: 32×24 black on white with a white border, upper case, no colour, inverse-video station bar, block glyphs █ ▌ ▒, status as `n/m` report codes (0 = OK, 2 = not found, 4 = no room → network, 9 = STOP → paused) per the ZX80 Operating Manual. Exact code choice for resolving/loading/buffering (`0/10` …) is a design choice.
- `bbc-mode7`: 40×25 on black, teletext colours, yellow double-height bar for the station, `BBC Computer 32K` / `BASIC` banner, `>` prompt with a flashing underline cursor, English teletext character map (`#`→£ …), sextant rule with a quadrant fallback.

### 2026-11-03 · ZX Spectrum, power-on sequences (M5 additions; research subagent, sources read through a summarising fetch tool)

| Fact | Source | Status |
|---|---|---|
| ROM message `© 1982 Sinclair Research Ltd` at 0x1539 (© = 0x7F), printed by NEW/START_NEW on the bottom line | https://skoolkid.github.io/rom/asm/1539.html · https://skoolkid.github.io/rom/asm/11B7.html | VERIFIED (ROM disassembly) 2026-11-03 |
| START_NEW: border white (`OUT ($FE),7`), RAM_FILL writes 2 to every byte down to $4000, RAM_READ changes 2→1→0; then PAPER 7, INK 0, BORDER 7, CLS | https://skoolkid.github.io/rom/asm/11B7.html | VERIFIED 2026-11-03 |
| Visible power-on: garbage, black screen with vertical red bars during the RAM check, then white with the copyright line (attributes 2 = red ink on black paper explains the bars – our inference) | https://spectrumcomputing.co.uk/forums/viewtopic.php?t=1216 | seen 2026-11-03 (forum); duration UNVERIFIED |
| No source for an "exclamation marks" power-on pattern (the System Test ROM flashes black/white; 128K/+2 not researched) | https://worldofspectrum.org/pub/sinclair/games-info/s/SinclairZXSpectrumTestROM.txt | UNVERIFIED |
| LD-BYTES border: white at entry, red while waiting for a leader, cyan on an edge (red/cyan stripes), blue/yellow from the sync on (`XOR $03`) | https://skoolkid.github.io/rom/asm/0556.html | VERIFIED 2026-11-03 |
| Tape timing (T-states): pilot 2168, sync 667/735, bit 0 = 2 × 855, bit 1 = 2 × 1710; header pilot 8063 pulses, data 3223; 224 T-states per line, 312 lines, 69,888 per frame | https://worldofspectrum.org/faq/reference/48kreference.htm | VERIFIED 2026-11-03 |
| Character set: 0x5E ↑, 0x60 £, 0x7F ©; 128–143 2 × 2 block graphics | https://en.wikipedia.org/wiki/ZX_Spectrum_character_set | seen 2026-11-03 (secondary) |
| Reports 0–9, A–R with texts (`0 OK`, `D BREAK - CONT repeats`, `8 End of file`, `R Tape loading error` …), shown at the bottom until a key is pressed; cursor modes K/L/C/E/G as a flashing letter | https://worldofspectrum.org/ZXBasicManual/zxmanappb.html · https://worldofspectrum.org/ZXBasicManual/zxmanchap1.html | VERIFIED 2026-11-03; `, n:m` suffix from memory, UNVERIFIED |
| RGB: no primary source; emulator palettes use 0xD7/0xD8/0xC8 for non-bright, 0xFF bright | https://www.turbodither.com/palettes/zx-spectrum · https://lospec.com/palette-list/zx-spectrum | seen 2026-11-03 – 0xD7 chosen |
| ZX80: picture goes blank (white/grey) while busy; at power-on an inverse K cursor bottom left on a blank screen | http://blog.tynemouthsoftware.co.uk/2023/10/how-the-zx80-generates-video.html · ZX80 Operating Manual (search snippet) | seen 2026-11-03 |
| BBC Micro power-on: `BBC Computer 32K`, `BASIC`, `>` (soft BREAK omits `32K`); beep via VDU 7 (single/double UNVERIFIED) | https://stardot.org.uk/forums/viewtopic.php?t=8884 | seen 2026-11-03 |
| VT100 power-up self-test shows test data, erases, rings the bell and puts the cursor top left; errors show a code character top left | https://vt100.net/docs/vt100-ug/chapter1.html · https://vt100.net/docs/vt102-ug/chapter9.html | seen 2026-11-03 (secondary summary) |
| RSX-11M V4.6 BL56 console boot (SIMH): `RSX-11M V4.6 BL56 128.K MAPPED`, `>RED DU:=SY:`, `>RED DU:=LB:`, `>MOU DU:RSXM56`, `>@DU:[1,2]STARTUP` (device names vary) | https://forum.vcfed.org/index.php?threads/booting-rsx11m-4-6-on-a-kdf11-be.46047/ · http://ancientbits.blogspot.com/2012/07/installing-rsx-11m-46-from-scratch-part_12.html | seen 2026-11-03 |
| Teletype Model 33: no text-visible power-on behaviour found | – | UNVERIFIED |
| Printing terminals after the ASR-33 at 300 baud (e.g. DECwriter) | – | UNVERIFIED (only used as an example speed) |

### 2026-11-03 · 2.11BSD on the PDP-11 (pdp11-bsd theme; research subagent read the source tree)

| Fact | Source | Status |
|---|---|---|
| 2.11BSD tree, patch level 431 (April 2000): `/etc/ttys` console `"/usr/libexec/getty std.9600" vt100 on secure`; gettytab banner `2.11 BSD UNIX (%h) (%t)` then `login: ` | https://github.com/RetroBSD/2.11BSD (`usr/etc/ttys`, `usr/etc/gettytab`) | VERIFIED (source) 2026-11-03 |
| csh prompt `% ` (`# ` for uid 0), sh `$ `; no host name in default prompts | same repo, `usr/bin/csh/sh.c`, `usr/bin/sh/msg.c` | VERIFIED 2026-11-03 |
| Errors: csh `foo: Command not found.`; sh `foo: not found`; `ls foo` → `foo not found` | same repo, `usr/bin/csh/sh.exec.c`, `sh.err.c`, `usr/bin/sh/msg.c`, `usr/bin/ls/ls.c` | VERIFIED 2026-11-03 |
| `/etc/rc` messages: `Automatic reboot in progress...`, `checking quotas: done.`, `checking for core dump...`, `preserving editor files`, `clearing /tmp`, `standard daemons: update cron accounting.`, `starting lpd` | same repo, `usr/etc/rc` | VERIFIED 2026-11-03 |
| Boot transcript: `70Boot from xp(0,1,0) at 0176700`, `Press <CR> to boot, or any other key to abort: 0`, `: xp(0,1,0)unix`, `Boot: bootdev=05010 bootcsr=0176700`, `2.11 BSD UNIX #115: Sat Apr 22 19:07:25 PDT 2000` / `sms1@curly.2bsd.com:/usr/src/sys/GENERIC`, `phys mem  = 3915776`, `avail mem = 3684480`, `user mem  = 307200`, probe lines `rl 0 csr 174400 vector 160 attached` … | https://github.com/JamesHagerman/nankervis-pdp11-js/blob/master/ExampleBoots.md · https://sergev.org/pdp11/211bsd | seen 2026-11-03 (emulator logs) |
| `fsck -p` prints nothing on a clean disk (two identical dates around it in the log); preen line format UNVERIFIED | same logs | seen 2026-11-03 |

### 2026-11-03 · VT100 power-on screen (owner question; research subagent)

| Fact | Source | Status |
|---|---|---|
| Power-up: 8224 reset releases ~1 s after power-on; self-test: ROM checksums (failing ROM on keyboard LEDs), RAM test, NVR checksum (bell several times on failure), keyboard scan (LEDs ~0.5 s + one bell); then "the cursor appears at column 1, line 1" | https://vt100.net/docs/vt100-tm/chapter4.html §4.2.8 | VERIFIED 2026-11-03 |
| Fatal self-test error: no intelligible information, "most likely … a random pattern of characters"; non-fatal errors as a single character on screen | https://vt100.net/docs/vt100-ug/chapter1.html | seen 2026-11-03 (search summary) |
| A normal power-on showing garbage is not documented; firmware fills character memory with blanks at power-on (VT101 forum answer) | https://forum.vcfed.org/index.php?threads/vt101-powers-on-but-displays-gibberish.48618/ | seen 2026-11-03 (secondary) – the static garbage in `vt100` boot is a design choice |
| Per-character attributes (reverse, blink, bold, underline) need the Advanced Video Option; the base VT100 has none | https://vt100.net/docs/vt100-ug/chapter1.html | seen 2026-11-03 |
| Earlier "test data, then erase" wording (VT102 guide summary) not confirmed for the VT100 | https://vt100.net/docs/vt102-ug/chapter9.html | UNVERIFIED |

### 2026-11-03 · macOS Now Playing / media keys for a windowless process (M6 research; subagent, sources older than macOS 27)

| Fact | Source | Status |
|---|---|---|
| libmpv's built-in Now Playing (`macos-media-player`, `remote_command_center.swift`) starts only with `input-media-keys=yes`, which the libmpv profile forces off; it maps next/prev to mpv keys, reports media type video and elapsed time, and needs a main-thread run loop host anyway → not used | local mpv 0.41.0 source: `etc/builtin.conf:21,30`, `input/input.c:1698-1712`, `osdep/mac/app_hub.swift:93-99`, `osdep/mac/remote_command_center.swift` | VERIFIED (source) 2026-11-03 |
| Remote commands are delivered on the main thread's run loop; a worker-thread run loop sent media keys to Music.app | https://github.com/LargeModGames/spotatui/pull/528 · https://github.com/emoeem/voicefox/pull/46 | seen 2026-11-03 |
| Working terminal players host NSApplication (`.accessory`/`.prohibited`) on the main thread with tokio elsewhere; a 1 s heartbeat re-asserting `playbackState` keeps the claim | https://github.com/Spotifyd/spotifyd/pull/1430 · https://docs.rs/crate/spotatui/0.38.0/source/src/infra/macos_media.rs | seen 2026-11-03 |
| On macOS `MPNowPlayingInfoCenter.playbackState` "must be set every time the app begins or halts playback" | objc2-media-player 0.3.2 generated docs (Apple header comment) | VERIFIED 2026-11-03 |
| A bare executable can carry an Info.plist via `-sectcreate __TEXT __info_plist` | https://redsweater.com/blog/2083/the-power-of-plist | seen 2026-11-03 – embedded in the spike, verified with `otool -s __TEXT __info_plist` |
| Crates: objc2 0.6.5 (MIT), objc2-foundation 0.3.2 (MIT), objc2-app-kit / objc2-media-player / objc2-core-foundation 0.3.2 (Zlib OR Apache-2.0 OR MIT), block2 0.6.2 (MIT); objc2-media-player exposes `MPNowPlayingInfoCenter::setPlaybackState`, `MPNowPlayingInfoPropertyIsLiveStream`, `MPRemoteCommand::addTargetWithHandler` | crates.io API; crate sources | VERIFIED 2026-11-03 (cargo deny passes) |
| Whether a bare executable from kitty/Terminal.app appears in Control Center and gets media keys on macOS 27 | spike `crates/ttnr-platform/examples/nowplaying_spike.rs` | UNVERIFIED – owner test pending |
| mpv 0.41.0 CoreAudio AO: `hotplug_cb` on `kAudioHardwarePropertyDevices` / `kAudioHardwarePropertyDefaultOutputDevice` re-inits the device and requests an AO reload when it changed (default device followed with `audio-device=auto`); the selected device's name is only logged at verbose level | local source `third_party/mpv/src/audio/out/ao_coreaudio.c:476-498`, `ao_coreaudio_utils.c:105-130` | VERIFIED (source) 2026-11-03 |
| objc2-core-audio 0.3.2 (Zlib OR Apache-2.0 OR MIT) exposes `AudioObjectGetPropertyData`, `AudioObjectAddPropertyListener`, `kAudioHardwarePropertyDefaultOutputDevice`, `kAudioObjectPropertyName`; default device name read on the owner's Mac ("MAJOR IV") | crates.io API; crate source; pty run | VERIFIED 2026-11-03 |
| mpv 0.41.0 (Homebrew CLI, `ao=null`): SomaFM Icecast `duration=00:00:00 seekable=no`; BBC 6 Music live HLS `duration=00:00:06 seekable=no`; local AAC file `duration=00:00:03 seekable=yes` → `seekable` tells live from files, `duration` does not | local runs 2026-11-03 | VERIFIED 2026-11-03 |
| After a dropped HTTP stream mpv logs `[ffmpeg] http: Stream ends prematurely …`, plays out its buffer, then ends the file (EOF) | local test server 2026-11-03 | VERIFIED 2026-11-03 |
| mpv 0.41.0: `--audio-exclusive` works for `coreaudio` (redirect to `coreaudio_exclusive`, "direct device access and exclusive mode (bypasses the sound server)"); `--coreaudio-change-physical-format` (default no) "will change the system-wide audio settings", equivalent to the Format setting in Audio MIDI Setup; values read back from libmpv | `DOCS/man/options.rst:2053-2060`, `DOCS/man/ao.rst:130-150`, `audio/out/ao_coreaudio.c:158-170` (local source); engine test | VERIFIED 2026-11-03 |
| Exclusive (hog) mode on macOS 27: other apps are not muted; macOS reroutes them to another available output while ttnr holds the device and moves them back afterwards (Safari/YouTube: external speakers → MacBook speakers → back) | owner's test 2026-11-03 | VERIFIED (observation) 2026-11-03 |

### 2026-11-03 · radio-browser.info API (M7)

| Fact | Source | Status |
|---|---|---|
| Server selection: DNS lookup of `all.api.radio-browser.info` + reverse lookup, done by the client (`/json/servers` only if DNS is impossible); descriptive User-Agent `appname/appversion`; `/json/url/<stationuuid>` "should be called everytime when a user starts playing a stream"; search params `name`, `tag`, `country`, `countrycode`, `language`, `order`, `reverse`, `limit`, `offset`, `hidebroken`; station fields `stationuuid`, `name`, `url`, `url_resolved`, `codec`, `bitrate`, `hls`, `lastcheckok`, `countrycode`, `tags`, `favicon` | https://docs.radio-browser.info/ | VERIFIED 2026-11-03 |
| `all.api.radio-browser.info` → 91.98.4.78 → `de1.api.radio-browser.info` (one server at check time); TLS certificate SAN `*.api.radio-browser.info`, `*.radio-browser.info`; HTTPS to `all.api…` verifies | `dig`, `openssl s_client`, `curl` | VERIFIED 2026-11-03 |
| Recorded answers: search "groove salad" (5 stations; one entry has `bitrate` 320000 – user-entered bit/s), `/json/url/<uuid>` → `{"ok":true,"message":"retrieved station url",…,"url":…}`, empty search `[]` | `tests/fixtures/radio-browser/` | VERIFIED 2026-11-03 |
| mpv 0.41.0: HTTP file from a server without range support (`python3 -m http.server`): `file-size=48.602 KiB seekable=no`; SomaFM Icecast: `file-size=(unavailable) seekable=no`; a dropped Icecast-like test stream reports `duration` = what was read (3 s) at its end → finite = `seekable` or known `file-size` (non-HLS) | local runs 2026-11-03 | VERIFIED 2026-11-03 |

