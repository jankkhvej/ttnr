# Architecture decision records

Format: `ADR-NNN · date · title` — Context / Decision / Consequences.

## ADR-001 · 2026-11-02 · Rust
Context: TUI + audio + macOS integration, open-source only, owner preference for a modern stack.
Decision: Rust (stable). Swift rejected: the natural playback path (AVPlayer) is a closed framework and Swift TUI libraries are immature.
Consequences: ratatui for UI; objc2 for the few macOS APIs we need.

## ADR-002 · 2026-11-02 · Playback via libmpv (FFmpeg) rather than a pure-Rust decoder
Context: requirement to play "any format", any channel layout, HLS/DASH/ICY, with CoreAudio output.
Decision: embed libmpv (from Homebrew `mpv`), audio-only configuration.
Consequences: broad codec support (incl. HE-AAC, AC-3, multichannel) and robust HLS; native dependency on Homebrew mpv; project licence must be GPL-compatible.

## ADR-003 · 2026-11-02 · Licence GPL-3.0-or-later (superseded by ADR-007)
Context: linking libmpv built as GPL.
Decision: GPL-3.0-or-later for the project.

## ADR-004 · 2026-11-02 · Resolve stream URLs at play time
Context: BBC (and others) change stream URLs without notice.
Decision: resolver chain cache → discovery → patterns → user override; auto re-resolve once on 403/404; `tuner refresh`.
Consequences: no hard-coded URL is ever load-bearing; small on-disk cache.

## ADR-005 · 2026-11-02 · libmpv binding: `libmpv2` crate
Context: we embed libmpv (ADR-002) and need a maintained, OSI-licensed Rust binding that matches the installed libmpv (client API 2.5, mpv 0.41.0). Candidates: `libmpv2` (kohsine), `libmpv`/`libmpv-sys` (ParadoxSpiral), `mpv-client-dyn`, or our own FFI.
Decision: `libmpv2` 7.x with `default-features = false` (drops the video `render` feature). Its `-sys` crate ships pregenerated bindings for client API 2.5, so no bindgen/libclang is needed. Because `libmpv2-sys` emits only `-lmpv`, `tuner-engine/build.rs` adds the search path via the `pkg-config` crate (reads Homebrew's `mpv.pc`). In `deny.toml` the deprecated licence id `LGPL-2.1` is clarified to `LGPL-2.1-only` (pinned to the LICENSE file hash).
Rejected: ParadoxSpiral `libmpv` (last release 2020, pre-API-2); `mpv-client-dyn` (plugin API, not embedding); hand-written FFI (more unsafe code for no gain – keep as fallback if `libmpv2` stalls).
Consequences: unsafe code stays inside the binding; `MpvPlayer` wraps `libmpv2::Mpv`. A future libmpv API 3 would need a new binding release. If the clarify hash breaks after a `libmpv2` upgrade, re-check the LICENSE file and update the hash.

## ADR-006 · 2026-11-02 · Workspace layout and toolchain
Context: ARCHITECTURE.md lists one binary and six library crates; M8 wants `cargo install --path .`.
Decision: the repo root is the `tuner` binary package *and* the workspace root; libraries live in `crates/tuner-*`. Shared versions/lints in `[workspace.*]`. Edition 2024, resolver 3. `rust-toolchain.toml` pins `1.99.1` (current stable) with rustfmt + clippy. Workspace lints: `missing_docs` and clippy `unwrap_used`/`expect_used` warn (→ errors under `-D warnings`), allowed in tests via `clippy.toml`.
Consequences: `cargo install --path .` works as-is; bumping Rust is an explicit one-line change.

## ADR-007 · 2026-11-02 · Licence BSD-2-Clause for the project source (supersedes ADR-003)
Context: owner prefers a modern FreeBSD-style licence. We link libmpv; Homebrew builds mpv with its default `-Dgpl=true` and FFmpeg with `--enable-gpl --enable-version3`, so the libraries we run against are effectively GPL-3.0-or-later. The FSF lists the 2-clause ("FreeBSD") licence as GPL-compatible.
Decision: all tuner source is BSD-2-Clause (`LICENSE`, `license = "BSD-2-Clause"` in the workspace). We distribute source only; a compiled binary linked against GPL libmpv/FFmpeg is, as a combined work, redistributable under GPL-3.0-or-later terms – stated in README. `GPL-3.0-or-later` removed from the `cargo deny` allow-list (no Rust dependency needs it).
Rejected: GPL-3.0-or-later for the source (not required by our dependencies, against owner preference); an LGPL-only libmpv/FFmpeg build to make even binaries non-GPL (custom native build outside Homebrew – revisit only if binary redistribution under BSD terms becomes a goal).
Consequences: anyone can reuse tuner code under BSD terms. A future Homebrew bottle / release binary must ship GPL-3.0 text and offer corresponding source of the GPL parts (M8 third-party notices).

## ADR-008 · 2026-11-02 · Project name `ttnr`
Context: "tuner" was a working name; owner wanted a terse, early-BSD-style command name (`ls`, `rm`). `tnr` collides with Thunder Compute's macOS CLI; `ttnr` had no collisions (RESEARCH.md, 2026-11-02).
Decision: the project, binary and crates are `ttnr` / `ttnr-*` ("text tuner"). Config/cache/state dirs and the control socket use `ttnr` (`~/.config/ttnr/`, `$TMPDIR/ttnr-$UID.sock`). Owner domain hcc07.org is used for reverse-DNS identifiers (e.g. macOS bundle id `org.hcc07.ttnr`, M6). Earlier ADRs and log entries keep the old name as history.
Consequences: the repo folder on disk is still `~/workspace/tuner` (outside the repo; owner may rename it).

## ADR-009 · 2026-11-02 · `MpvPlayer` shape and `audio-channels=auto-safe` default
Context: M1 task 1. ARCHITECTURE.md sketched `audio-channels=auto` and a `Player` trait with infallible `stop`/`pause`/`set_volume`. The mpv 0.41.0 manual defines `auto-safe` (default) as "the system's preferred channel layout", which is what "respect device layout / downmix only when the device needs it" (SPEC F2.2) means on macOS (the layout set in Audio MIDI Setup); `auto` sends whatever the device accepts and is flagged by mpv for HDMI problems (e.g. 5.1 into an 8-output interface wired as stereo).
Decision: `EngineConfig::audio_channels` defaults to `auto-safe`, configurable (`auto`, explicit layouts). All `Player` methods return `Result<(), EngineError>`. One `Arc<mpv>` handle (libmpv2's `Mpv` is `Send + Sync`) so an event thread can share it (M1 task 2). HTTP headers go through `change-list http-header-fields clr/append` so values may contain commas. Volume is clamped to 0–100 (NaN → 0). `StreamDescriptor` lives in `ttnr-core` with only `urls` + `headers` until M2 adds resolver metadata.
Consequences: 5.1/7.1 output depends on the device's configured speaker layout; the owner checks this by ear at M1 acceptance. Switch to `auto` per device if `auto-safe` proves too conservative.

## ADR-010 · 2026-11-02 · Engine events: event thread, log-based HTTP error classification
Context: M1 task 2 needs buffering/playing/EOF/error events, with HTTP 403/404 distinguished so core can re-resolve (ADR-004). mpv has no HTTP status property; the only signal is FFmpeg's log line. Facts in RESEARCH.md (2026-11-02, "mpv events, logs and HTTP errors").
Decision:
- `MpvPlayer` spawns one thread blocking in `mpv_wait_event(-1)`; `Drop` sends `quit` and joins on `Shutdown`.
- Raw mpv events → owned `RawEvent` → pure `Translator` (unit-tested) → `EngineEvent` {`Loading`, `Playing`, `Buffering{percent}`, `Paused(bool)`, `Eof`, `Stopped`, `Error{kind, detail}`, `Shutdown`}. Observed: `pause`, `paused-for-cache`, `cache-buffering-state`.
- Warn-level mpv logs are requested via `libmpv2_sys::mpv_request_log_messages` (one documented `unsafe`). The last `[ffmpeg] http: HTTP error N` / `tcp:` / `tls:` line of the current file classifies a failure as `Http403`, `Http404`, `Http(n)` or `Unreachable`; otherwise the mpv error code gives `Unsupported`, `AudioOutput`, `LoadFailed` or `Other(code)`.
- Because logs are delivered after queued events, an error `end-file` triggers a non-blocking drain (`wait_event(0)`): logs first, then the error, then other drained events in order.
- Channel: `tokio::sync::mpsc::unbounded_channel` (tokio `sync` feature only) – usable from sync code (`blocking_recv`/`try_recv`) and async. `Player::take_events()` hands the receiver out once (refines ARCHITECTURE's `events()`).
- **One mpv core per process** (FFmpeg logs reach only the first core). Tests that need logs serialise with a mutex.
Consequences: classification depends on FFmpeg's English log wording – pinned by tests against a loopback server, re-check on FFmpeg/mpv upgrades. Mid-stream HLS segment 403s that mpv survives don't produce an error (only if the file ends with one).

## ADR-011 · 2026-11-02 · `StreamInfo`: sticky facts, coalesced events, honest bit depth
Context: M1 task 3. mpv exposes stream format as ~13 separate properties that change in bursts while a stream opens and become unavailable one by one when it ends; a naive mapping produced ~16 `StreamInfo` events per file, ending with an empty one.
Decision: `ttnr-core::StreamInfo` (codec, desc, profile, container, rate, sample format, bit depth, channels, count, bitrate, output rate/channels) with `summary()` for the format line. The engine observes scalar sub-properties (never `Format::Node`, see ADR-010) and (1) keeps facts sticky within a file – unavailable/empty never clears, `StartFile` resets; (2) emits `EngineEvent::StreamInfo` only when mpv's queue is drained (non-blocking drain after every event) and the info changed. Bitrate = container nominal → HLS variant → measured (rounded to kbps). Bit depth only when unambiguous (`u8`, `s16`); the raw decoder format is kept in `sample_format`.
Consequences: 1–4 info events per stream open. A field that genuinely disappears mid-stream keeps its last value until the next file. 24-bit sources show no bit depth (mpv cannot tell) – revisit if FFmpeg/mpv expose `bits_per_raw_sample`.

## ADR-012 · 2026-11-02 · `ttnr play` and file logging
Context: M1 task 4 – headless playback with plain output; CLAUDE.md requires `tracing` to a file, never to the TUI's stdout.
Decision: clap subcommand `play <url> [--volume 0-100]` (+ hidden `--audio-output null` for tests). The binary runs a current-thread tokio runtime (`rt`, `macros`, `signal`); `tokio::select!` over engine events and `ctrl_c()`. Status lines on stdout (`[loading]`, `[playing]`, `format: …`, `[buffering N%]`, `[end of stream]`, `[stopped]`); errors on stderr with exit code 1; EOF and Ctrl-C exit 0. Logs: `tracing-appender` daily rotation, 7 files, `$XDG_STATE_HOME/ttnr/ttnr.YYYY-MM-DD.log` (default `~/.local/state/ttnr/`), filter `TTNR_LOG` (`Targets` syntax, default `info`; no regex dependency). Logging failures never stop the app. mpv's built-in scripts are disabled (`load-*=no`).
Consequences: CLI tests run the real binary with `XDG_STATE_HOME` in a temp dir. Reconnect on EOF/error for live streams is not done here (M2 refresh, M6 sleep/wake).

## ADR-013 · 2026-11-02 · Format matrix and `StreamInfo` as a mirror of mpv state (amends ADR-011)
Context: M1 task 5. The matrix exposed (a) mpv not re-notifying unchanged or stale-unavailable properties across files (RESEARCH.md), which broke ADR-011's "reset on StartFile"; (b) toolchain limits: no E-AC-3 7.1 encoder, no DASH demuxer.
Decision:
- `Translator` keeps a *mirror* of mpv's stream properties that is never reset; `StartFile` only forgets what was *sent*. `StreamInfo` is emitted only while a file is loaded (not during teardown), coalesced per queue drain, sticky per file at emission level (a vanished field keeps its last sent value until the next file).
- The event thread re-reads all stream-info properties on `file-loaded`, `playback-restart` and `audio-reconfig` (works around the mpv observer quirk).
- `scripts/gen-fixtures.zsh` generates 17 playable fixtures + a DASH sample into `tests/fixtures/generated/` (git-ignored), idempotent via a script-hash stamp, failing loudly on a missing encoder. The `format_matrix` test runs the script itself, then plays every fixture through one `MpvPlayer` and compares codec, profile, rate, channels, count, container and bit depth. ARCHITECTURE's "E-AC-3 7.1" becomes E-AC-3 5.1 + AAC 7.1 + Opus 7.1. DASH is excluded until FFmpeg has a DASH demuxer (owner decision pending).
Consequences: fixtures need Homebrew ffmpeg on macOS (`aac_at`). `cargo test` on a clean clone generates fixtures (~2 s).

## ADR-014 · 2026-11-02 · Audio output `coreaudio,avfoundation`; smoothed measured bitrate
Context: mpv 0.41.0 CoreAudio AO fails on macOS 27 for planar-float/mono sources (mpv #18384, fixed in master by #18679, unreleased; Homebrew still 0.41.0). That covers most MP3/AAC/Vorbis radio. Separately, measured bitrate of VBR/lossless streams flooded the format line.
Decision: `AudioOutput::CoreAudio` sets `ao=coreaudio,avfoundation` (mpv's documented priority list): CoreAudio when it works, AVFoundation otherwise. No forced `audio-format`/`audio-samplerate` workaround (would cost precision or resample). Measured bitrate: not shown for lossless codecs; for lossy streams without a nominal rate, kbps-rounded with 10 % hysteresis.
Consequences: with the fallback, multichannel is downmixed by AVFoundation; device pinning and exclusive mode (M6) must be re-checked per AO. Drop the fallback (or keep it as a safety net) once Homebrew ships mpv ≥ 0.42. *Amended by ADR-015: a locally built, fixed libmpv is now preferred; the fallback stays as a safety net.*

## ADR-015 · 2026-11-02 · Locally built libmpv (0.41.0 + CoreAudio fix), no system changes
Context: owner asked whether AVFoundation is a significant regression; if so, install the fixed library locally (not via brew). Evidence (RESEARCH.md): `ao_avfoundation` read-ahead buffer = 2 s → software volume/mute changes lag (mpv #15014, open; no `--avfoundation-buffer` in 0.41.0); no hotplug (device-change following, M6); tail dropped at EOF (#18461). For an interactive player with `+`/`-`/`m` keys that is significant.
Decision: `scripts/build-libmpv.zsh` reproduces Homebrew's mpv 0.41.0 build (same tarball and two backports, SHA-256 pinned) plus PR #18679 (8 commits, the upstream fix), libmpv only, without Lua/JS/Blu-ray/Vulkan/etc., using meson/ninja from a private venv and Homebrew's libraries. Output in `third_party/mpv/` (git-ignored). `ttnr-engine/build.rs` prefers `third_party/mpv/prefix` via `PKG_CONFIG_PATH`, else Homebrew. Built-in-script options are optional (absent without Lua). `ao=coreaudio,avfoundation` stays as a safety net; `ttnr play` logs the active AO.
Consequences: binaries link the repo-local `libmpv.2.dylib` by absolute path – fine for development; M8 packaging must revisit (Homebrew mpv ≥ 0.42 should make the local build unnecessary). Re-run the script after `brew upgrade` of ffmpeg/libplacebo/libass if their ABI changes. A clean clone without the local build still works (Homebrew libmpv + AVFoundation fallback).
Rejected: forcing `audio-format=s16`/`audio-samplerate` (precision loss/resampling); mpv master (unreleased, API churn); a different playback library (ADR-002 still holds – the bug is one function in one AO).

## ADR-016 · 2026-11-02 · MPEG-DASH deferred
Context: Homebrew FFmpeg 9.0.2 has no DASH demuxer (no libxml2); mpv has none of its own. Owner asked whether DASH can be skipped.
Decision: defer DASH (SPEC F1.1 annotated). Evidence: radio-browser.info, 62,042 non-broken stations on 2026-11-02 – 0 with `.mpd` URLs, 4,086 HLS (6.6 %), the rest direct Icecast/Shoutcast/HTTP. BBC offers HLS as well (M4 research will confirm).
Consequences: `.mpd` URLs fail with "no playable audio" for now. The DASH fixture stays in `gen-fixtures.zsh` for when support arrives (FFmpeg built with libxml2 – could be added to the local build in the future).

## ADR-017 · 2026-11-02 · Source traits: hooks + one shared resolver chain
Context: M2 task 1. ARCHITECTURE sketched `Source::resolve/refresh` per source; the chain (cache → discovery → patterns → override) must behave the same for every source and be testable without network.
Decision:
- `Source` (async-trait) offers hooks: `id`, `capabilities`, `list`, `discover` (authoritative lookup, `Ok(None)` if n/a), `patterns` (templates, probed in order), `metadata` (optional `MetadataProvider`). `ttnr_core::resolve_chain` / `SourceRegistry::{resolve, refresh}` run the chain, record a `ResolverTrace`, and cache discovery/pattern hits in `ResolveCache` (JSON, atomic write, default TTL 24 h, corrupt file = empty). The user override (`Station::url`) is last and never cached, so a recovered discovery wins again. `Mode::Refresh` deletes the cached entry first, so a failed refresh cannot resurrect a known-bad URL.
- Network behind `ttnr_core::Http` (`get`, `probe`); the real client comes with the `url` source; tests use fakes. `ResolveCtx` carries HTTP, cache and an injectable clock.
- `Station` {id, name, source, url override, params}; `StreamDescriptor` gains `resolved_at`, `ttl`, `trace`; `NowPlaying` + `InfoSource` (priority External > TimedId3 > Icy) defined for the metadata tasks.
Consequences: a source with fully custom resolution implements it inside `discover`. Adding a source = implement hooks + one `register` call.

## ADR-018 · 2026-11-02 · `url` source: content-sniffed playlists, reqwest + native TLS
Context: M2 task 2. Requirements: any URL (SPEC F1.1), never download an endless stream, HLS untouched, playlist files expanded, tests offline. Research (RESEARCH.md) via a lighter-model subagent, key facts re-verified.
Decision:
- `ttnr-sources::playlist`: pure functions. Detection by content first (`#EXTM3U` + `#EXT-X-` → HLS; `#EXTM3U` → M3U; `[playlist]` → PLS; `<asx` → ASX; XSPF namespace), MIME/extension only as a hint (a headerless M3U must actually list URLs). UTF-8 with BOM strip, Latin-1 fallback. Lenient tag scanning for ASX/XSPF (no XML dependency; ASX is often malformed). Relative URLs resolved against the final URL (`url` crate); only network schemes accepted from remote playlists (no `file:`), duplicates removed.
- `ReqwestHttp` (reqwest 0.13, `default-features = false, features = ["native-tls"]`): redirects ≤ 10, connect 10 s / read 15 s timeouts, body cap 256 KiB, body not read at all for `audio/*`/`application/ogg`/`icy-*` responses (`HttpResponse::streaming`); `probe` = GET dropped after headers. Error text includes the cause chain.
- `UrlSource::discover`: non-HTTP → direct; transport error (e.g. `ICY 200 OK`) → direct; non-2xx → error (chain falls back to the URL as override, so mpv still classifies the failure); streaming / HLS / unknown → direct; playlist → entries as ordered candidates (TTL 1 h).
- `ttnr play` resolves through the registry with the on-disk cache (`$XDG_CACHE_HOME/ttnr/resolve.json`) and fails over to the next candidate on HTTP/unreachable/load errors.
- `cargo deny` graph restricted to `aarch64-apple-darwin` and `x86_64-apple-darwin` (the shipped platforms); avoids judging Linux-only branches like native-tls → openssl.
Consequences: nested playlists (a playlist entry that is itself a PLS/M3U) are left to mpv, which parses M3U/PLS itself. Extend the deny targets if Linux becomes a goal.
