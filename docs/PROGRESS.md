# Progress log

Newest entry on top. Each entry: date · milestone/task · what changed · how to try it · next step · open questions for the owner.

## 2026-11-03 · M3 · Snapshot tests – **M3 complete, waiting for owner review**
- 24 insta snapshots (8 view states × 3 sizes, theme `plain`) reviewed by hand; review fixed blank-line collapsing, `HE-AAC` label, URL prompt ellipsis (ADR-026).
- M3 summary: Elm-style app state machine in core; TUI runtime (UI task, redraw on change/resize, terminal restored on every exit); main view per SPEC F5.1 + stations / open URL / help / device picker; remappable keymap in `config.toml`; TOML theme engine with colour-depth fallback and user themes; snapshot + style tests.
- Open follow-up: `ttnr play` still has its own loop (ADR-022).
- Next after review: M4 – BBC source.

## 2026-11-03 · M3 · Theme engine
- `ttnr-themes`: TOML theme files (`themes/plain.toml` embedded), `ThemeSet` (built-ins + `~/.config/ttnr/themes/`), `ColorDepth` detection, downgrading with per-depth overrides; config `theme` and `color`; `t` cycles themes (ADR-025, THEMES.md "Theme files").
- Tests: colour parsing/downgrade/detection, theme parsing errors, set loading/overrides, view styles per depth, CLI unknown theme; pty: user theme with ASCII borders cycled in, `#ffb000` → 256-colour 214.
- Next: M3 task 5 – snapshot tests for all views at 3 sizes.

## 2026-11-03 · M3 · Remappable keymap
- `ttnr-tui::Keymap` (defaults per SPEC F5.3, overrides, validation, help/footer from bindings), `src/config.rs` + `examples/config.toml` (ADR-024).
- Tests: keymap defaults/overrides/errors/labels, config parsing and example validity, CLI: bad `[keys]` reported before the TUI starts.
- Try it: copy `examples/config.toml` to `~/.config/ttnr/`, uncomment e.g. `quit = "x"`, run `cargo run -q`, press `?`.
- Next: M3 task 4 – theme model + TOML loader + `plain` theme; colour-depth fallback.

## 2026-11-03 · M3 · Views
- `ttnr-tui::view`: main view per SPEC F5.1 (station, programme, artist – title, album, format, status, volume + output device, recent tracks, key help), overlays for stations, open URL, help, devices; priority-based layout down to 32×16 (ADR-023). Minimal `ttnr-themes::Theme` + `plain`, width-aware truncation.
- Tests: rendering at sizes 1×1…120×40 for every view (no panic), essentials at 32×16, footer growth, low-height priorities, error/empty states; pty run through the overlays.
- Next: M3 task 3 – keymap remappable in config.

## 2026-11-02 · M3 · App state machine + TUI runtime
- `ttnr_core::app` (Elm-style `App::update(Msg) -> Vec<Effect>`), `ttnr-tui` (`command_for` keymap, minimal `draw`), binary `src/tui.rs` runtime (ADR-022). `ttnr` alone now starts the TUI (station list opens if stations exist).
- Engine: device list events; error classification on `ErrorKind`.
- Tests: 11 state-machine tests (resolve/start, mirror → refresh → error, late answers, history + priority, volume/mute/pause, next/prev, lists, URL prompt, devices, help/quit), keymap tests, engine-event mapping, device JSON parsing; manual pty run (start, select, play, quit, terminal restored).
- Try it: `cargo run -q` (with `~/.config/ttnr/stations.toml`), Enter to play, `+`/`-`, `m`, space, `q`.
- Next: M3 task 2 – full views: main per SPEC F5.1, station list, open-URL prompt, help overlay, device picker.

## 2026-11-02 · M2 · Re-resolve on 403/404, `ttnr refresh` – **M2 complete, waiting for owner review**
- `ttnr_core::Failover` (next mirror → one refresh → give up), used by `ttnr play`; `ttnr refresh [id|url]` (ADR-021).
- Tests: failover units; CLI `stale_cache_self_heals` (cached URL 404 → re-resolve → plays, cache healed), `refresh_command`, and the M2 acceptance offline: `.pls` → ICY stream → `now:` line.
- M2 summary: source traits + registry + resolver chain + cache; `url` source (PLS/M3U/XSPF/ASX, HLS pass-through, mirror fail-over); `stations` source + `ttnr stations`; ICY and HLS-ID3 now-playing; re-resolve once + `ttnr refresh`.
- Next after review: M3 – minimal TUI + theme engine.

## 2026-11-02 · M2 · ICY and HLS ID3 now-playing
- Engine `EngineEvent::Tags` (ICY title/name, tags); core `StreamTags`, `NowPlaying::from_tags`, `Merger`; sources `id3` parser + `HlsId3Provider` (segment-head poller, because mpv/FFmpeg drop packed-audio ID3 updates); `ttnr play` prints `now: Artist – Title` (ADR-020).
- Tests: ICY titles from a loopback Icecast-style server, translator/merger/ICY-split units, ID3 parser (encodings, consecutive tags, garbage) + real recorded segment head, poller with virtual time (variant selection, change-only, stop without ID3, non-HLS), sized media read.
- Verified live: SomaFM (ICY) and a StreamS HLS test stream (ID3) show titles; FIP's timed ID3 is empty (no titles from the stream itself).
- Try it: `cargo run -q -- play https://somafm.com/groovesalad.pls`, `cargo run -q -- play https://db2.indexcom.com/bucket/ram/00/05/05.m3u8`.
- Next: M2 task 5 – auto re-resolve once on 403/404; `ttnr refresh [station]`.

## 2026-11-02 · M2 · `stations` source
- `StationsSource` (load/validate/list/find), `ttnr play <station-id>`, `ttnr stations [--search]`; `examples/stations.toml` now has real, checked SomaFM/FIP entries plus BBC placeholders (ADR-019).
- Tests: parser/validation units, example file validity, CLI (list, search, play by id, BBC-before-M4 message, unknown id, broken file).
- Try it: `mkdir -p ~/.config/ttnr && cp examples/stations.toml ~/.config/ttnr/ && cargo run -q -- stations && cargo run -q -- play fip`.
- Next: M2 task 4 – ICY and HLS timed-ID3 metadata as `NowPlaying`.

## 2026-11-02 · M2 · `url` source + playlists + HTTP client
- `ttnr-sources`: `playlist` (detect/parse M3U, HLS pass-through, PLS, XSPF, ASX), `ReqwestHttp` (native TLS, stream-aware, capped), `UrlSource` (ADR-018). `ttnr play` now resolves via the registry, caches in `~/.cache/ttnr/resolve.json`, and fails over between playlist mirrors.
- Fixtures: 10 real playlists recorded from popular stations. Tests: parser units, real-playlist table, HTTP client vs loopback (endless stream, redirect, cap, ICY status), `UrlSource` through the chain, CLI fail-over (404 mirror → next).
- Try it: `cargo run -q -- play https://somafm.com/groovesalad.pls` (also `.xspf`, `.asx`, `.m3u` URLs in RESEARCH.md).
- Next: M2 task 3 – `stations` source from `stations.toml` (+ sample in `examples/`).

## 2026-11-02 · M2 · Source traits, registry, resolver chain + cache
- `ttnr-core`: `Station`, `NowPlaying`/`InfoSource`, extended `StreamDescriptor` (+`ResolverTrace`), `Http` trait, `Source`/`MetadataProvider` (async-trait), `ResolveCtx`, `ResolveCache` (JSON, TTL), `resolve_chain`, `SourceRegistry` (ADR-017).
- Tests: cache (TTL, persistence, corrupt file), chain (cache hit/miss, expiry, pattern probing order, override last and uncached, exhausted trace, refresh replaces stale URL, failed refresh does not resurrect it, unknown source).
- Next: M2 task 2 – `url` source with playlist parsers + real HTTP client.

## 2026-11-02 · M1 review follow-up (owner feedback)
- Owner heard Icecast MP3 and HLS AAC fine. Channel check on a stereo headset: L, R, C, gap, quieter L/R… – exactly the FFmpeg downmix (LFE dropped, centre/surround −3 dB); explained in RESEARCH.md.
- DASH deferred (owner OK; 0 of 62,042 radio-browser stations use it) – ADR-016.
- AVFoundation judged a significant regression (volume/mute lag up to ~2 s, no hotplug) → `scripts/build-libmpv.zsh` builds Homebrew's mpv 0.41.0 recipe + upstream fix #18679 into `third_party/mpv/` (no system changes); engine prefers it (ADR-015). CoreAudio now opens for all formats; AVFoundation remains a safety net.
- Engine errors now name the failing option and use mpv's error text; `ttnr play` logs the active audio output.
- Try it: `./scripts/build-libmpv.zsh && cargo build && ./target/debug/ttnr play https://ice6.somafm.com/groovesalad-128-mp3`; log line `audio output ao="coreaudio"` in `~/.local/state/ttnr/`.
- Next: M2 – Sources & in-stream metadata (after owner's go).

## 2026-11-02 · M1 · Format matrix – **M1 complete, waiting for owner review**
- `scripts/gen-fixtures.zsh` (19 fixtures + DASH sample, incl. by-ear `channel_check_51/71.flac`) and `format_matrix` test (ADR-013).
- Found and fixed/worked around: mpv observer not re-notifying across AAC files (re-read on load/restart/reconfig); mpv 0.41.0 CoreAudio failing planar/mono sources on macOS 27 (upstream #18384, fixed unreleased) → `ao=coreaudio,avfoundation` (ADR-014); measured-bitrate churn (ADR-014).
- Research delegated to a lighter-model subagent where useful (owner request); facts re-verified before logging.
- Open questions for the owner: (1) DASH `.mpd` needs FFmpeg with libxml2 – Homebrew's has no DASH demuxer; defer or change the FFmpeg build? (2) Keep the AVFoundation fallback until Homebrew ships mpv 0.42, or try `brew install --HEAD mpv`?
- Next after review: M2 – Sources & in-stream metadata.

## 2026-11-02 · M1 · `ttnr play <url>`
- `ttnr play <url|file> [--volume N]`: plain status lines, Ctrl-C stops (exit 0), errors to stderr (exit 1). File logging via `tracing` to `~/.local/state/ttnr/` (ADR-012). mpv built-in scripts disabled (removed a warning on every file).
- Tests: CLI end-to-end (WAV to EOF with format line, HTTP 404 via loopback, volume validation, log file lands in `$XDG_STATE_HOME`); unit tests for paths and status lines.
- Smoke-tested by hand: SomaFM Icecast MP3 and FIP HLS AAC both play (RESEARCH.md).
- Try it: `cargo run -q -- play https://ice6.somafm.com/groovesalad-128-mp3` (Ctrl-C to stop).
- Next: M1 task 5 – `scripts/gen-fixtures.zsh` + format-matrix test (incl. 5.1/7.1).

## 2026-11-02 · M1 · `StreamInfo`
- `ttnr-core::StreamInfo` + `summary()` (e.g. `AAC LC · 48 kHz · stereo · 128 kbps`, shows `→ 44.1 kHz · stereo` when resampled/downmixed).
- Engine observes 13 scalar mpv properties; sticky facts per file, one coalesced `EngineEvent::StreamInfo` per burst (ADR-011). Event loop now drains non-blocking after each event and flushes when idle.
- Tests: translator units (coalescing, stickiness, bitrate precedence, reset); WAV integration test asserts codec/container/rate/format/bit depth/channels/bitrate/output.
- Next: M1 task 4 – `ttnr play <url>` with plain log output, Ctrl-C stops.

## 2026-11-02 · M1 · Engine events
- `ttnr-engine::event`: `EngineEvent`, `ErrorKind`, pure `Translator`; `MpvPlayer` event thread (blocks in `mpv_wait_event`, no polling), `take_events()`, clean shutdown on drop (ADR-010).
- HTTP 403/404/other and unreachable are classified from FFmpeg log lines; verified against a loopback HTTP server (plain and HLS), unit tests for the translator.
- Found and documented two mpv behaviours: logs arrive after queued events (drain on error), and FFmpeg logs reach only the first mpv core per process (→ one `MpvPlayer` per process; event tests serialised).
- Fixed a race in `plays_local_file_audio_only` (AO opens after the decoder). `tracing` without proc-macro attributes (avoids duplicate `syn`).
- Next: M1 task 3 – `StreamInfo` from mpv properties.

## 2026-11-02 · M1 · `MpvPlayer` (audio-only, play/stop/pause/volume)
- `ttnr-core::StreamDescriptor` (urls + headers, minimal); `ttnr-engine`: `Player` trait, `EngineConfig` (`AudioOutput::{CoreAudio, Null}`, `audio_channels` default `auto-safe`, initial volume), `MpvPlayer` (ADR-009). All mpv options verified against 0.41.0 (RESEARCH.md).
- Tests (`crates/ttnr-engine/tests/player.rs`): generated WAV via `ao=null` – audio-only (no VO), pause/resume, clamped software volume, stop → idle, empty descriptor error, HTTP headers verbatim, device pin/unpin. Ignored test opens real CoreAudio silently: `cargo test -p ttnr-engine -- --ignored coreaudio`.
- Diagrams in docs are Mermaid now (owner request, CLAUDE.md convention).
- Next: M1 task 2 – engine events (buffering, playing, error kinds incl. HTTP 403/404, EOF).

## 2026-11-02 · M0 follow-up – renamed to `ttnr`
- Owner chose `ttnr` (ADR-008): crates `ttnr-*`, binary `ttnr`, paths `~/.config/ttnr/` etc. across code, CLAUDE.md, SPEC, ARCHITECTURE, ROADMAP, README, examples, start.zsh.
- Licence BSD-2-Clause confirmed by owner. A project page on hcc07.org will come later → set Cargo `homepage` then.
- Try it: `./scripts/check.zsh` · `cargo run -q -- --version` (prints `ttnr 0.1.0`).
- Next: M1 – `MpvPlayer` (audio-only config, play/stop/pause/volume).

## 2026-11-02 · M0 follow-up – owner decisions
- Licence changed to **BSD-2-Clause** (owner preference; ADR-007 supersedes ADR-003). Binaries linked to Homebrew's GPL libmpv/FFmpeg are GPL-3.0-or-later as a combined work – documented in README/SPEC.
- Name: owner OK with "tuner", asked for a short BSD-style name. `tnr` is taken (Thunder Compute CLI on macOS); `ttnr` is free (RESEARCH.md). Waiting for the owner's pick before renaming.
- No public git repo yet → Cargo `repository`/`homepage` stay unset. Owner's domain **hcc07.org** is available for identifiers, e.g. macOS bundle id `org.hcc07.<name>` for Now Playing (M6).
- Next: rename if chosen, then M1.

## 2026-11-02 · M0 Bootstrap – complete
- Toolchain verified and logged in RESEARCH.md: Rust 1.99.1, mpv 0.41.0 (libmpv client API 2.5), FFmpeg 9.0.2, Homebrew 7.0.12. Installed `cargo-deny` 0.20.2 via `cargo install --locked cargo-deny` (per README).
- libmpv binding chosen: `libmpv2` 7.0.0 (ADR-005). `tuner-engine/build.rs` finds libmpv via pkg-config (the binding only emits `-lmpv`).
- Workspace: root = `tuner` bin, libraries in `crates/tuner-{core,engine,sources,themes,tui,platform}` (ADR-006); `rust-toolchain.toml` pins 1.99.1; `LICENSE` = GPL-3.0 text from gnu.org; `deny.toml` with OSI allow-list and a licence clarification for `libmpv2`.
- Tests: CLI definition check; engine test that creates an mpv core and reads `mpv-version` (proves linking + init).
- Try it: `./scripts/check.zsh` · `cargo run -- --version` · `cargo run -- --help`.
- Next: M1 – `MpvPlayer` (audio-only config, play/stop/pause/volume).
- Open questions for the owner: (1) confirm licence GPL-3.0-or-later (ADR-003 still "proposed"); (2) confirm the name "tuner"; (3) `repository` URL for Cargo metadata (left out, not invented).

## 2026-11-02 · Project scaffolded (by Claude in chat)
- Spec, architecture, themes, roadmap, research seeds and decisions written.
- No code yet. Next: M0 — Bootstrap, starting with toolchain verification.
- Open question for owner: confirm licence GPL-3.0-or-later and the name "tuner".
