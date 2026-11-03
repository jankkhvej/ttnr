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

## ADR-019 · 2026-11-02 · `stations` source is a catalogue
Context: M2 task 3. Stations in `stations.toml` belong to different resolving sources (`url`, `bbc`, …).
Decision: `StationsSource` loads `$XDG_CONFIG_HOME/ttnr/stations.toml` (default `~/.config/ttnr/`), validates it (unique non-empty ids, name, source; `url` required for source `url`; `source = "stations"` rejected; unknown top-level keys rejected) and only lists/looks up; each station resolves through its own `source` via the registry. Missing file = empty list. `params` is an explicit inline table (no flattening magic). CLI: `ttnr play <url|path|station-id>` (URL/path detection: `://`, leading `/ . ~`, or an existing path), `ttnr stations [--search]`. A broken `stations.toml` does not block playing a plain URL.
Consequences: stations of sources not yet built (e.g. `bbc` before M4) fail with "needs the `bbc` source, which is not available yet".

## ADR-020 · 2026-11-02 · In-stream now-playing: ICY via mpv, HLS ID3 via our own segment poller
Context: M2 task 4. ICY reaches mpv's `metadata`; but `metadata/by-key/*` is not change-notified, libmpv2 cannot decode `Node` events, and mpv/FFmpeg drop ID3 now-playing in HLS packed-audio segments (RESEARCH.md).
Decision:
- Engine observes `metadata` as a *string* purely as a change signal and re-reads `metadata/by-key/{icy-title, icy-name, title, artist, album}` (also on file-loaded/playback-restart); emits coalesced `EngineEvent::Tags(StreamTags)` (not sticky: an empty `StreamTitle` clears).
- Core: `StreamTags`, `NowPlaying::from_tags` (tags win over ICY; `StreamTitle` split at the first ` - `), `InfoSource::{Icy, Tags, External}` (renamed from `TimedId3`), `Merger` (latest per source, highest priority shown, repeats ignored).
- `ttnr-sources::id3` (ID3v2.3/2.4 text + WXXX artwork, consecutive tags, never panics) and `HlsId3Provider`: master → first variant → media playlist; for each new newest segment a ranged GET of 8 KiB; emits `NowPlaying` on change; polls every target duration (2–30 s); exponential backoff on errors (≤ 60 s); stops after 3 segments without ID3 or when the receiver is dropped. `UrlSource::metadata` attaches it to `.m3u8` URLs. `Source::metadata` now takes `&ResolveCtx` (providers need HTTP).
- `ReqwestHttp` treats a media type as an endless stream only without `Content-Length` (sized segments, incl. 206, are read).
Consequences: ID3 comes from the live edge, ahead of playback by the buffer (seconds) – the sync offset (F4.3, M4) will delay display. TS timed-ID3 is left to FFmpeg/mpv (untested). One extra small request per segment for HLS stations.

## ADR-021 · 2026-11-02 · Failover policy in core; `ttnr refresh`
Context: M2 task 5 – "auto re-resolve once on 403/404; `ttnr refresh`" (ADR-004). The TUI (M3) needs the same behaviour as `ttnr play`.
Decision: `ttnr_core::Failover` (pure): on a failure try the next candidate URL; when candidates are exhausted and the error hints at a stale URL (HTTP 403/404, unreachable), return `Action::Refresh` **once**; then `GiveUp`. The caller runs `SourceRegistry::refresh` (bypasses and replaces the cache) and feeds the result back. Mirror-worthy errors: 403, 404, other HTTP, unreachable, load failed; others (unsupported, audio output) give up immediately. `ttnr refresh [id|url]` re-discovers one station or all from `stations.toml`, prints URLs + trace, exits non-zero if any failed.
Consequences: a stale cached URL self-heals within one failed attempt (covered by `stale_cache_self_heals`). Live-stream reconnect with backoff after network loss/sleep is separate (M6).

## ADR-022 · 2026-11-02 · App state machine (Elm-style) and TUI runtime
Context: M3 task 1 – app state machine in core; UI on its own task; redraw only on change.
Decision:
- `ttnr_core::app`: `App { state: AppState, failover, merger }`, `App::update(Msg) -> Vec<Effect>`. `AppState` is plain comparable data (station, status, now, history (last 5), stream info, volume/mute, device, devices, stations, view, quit). `Msg` = play/player events (core's own `PlayerEvent`, mapped from the engine by the runtime)/resolution results/now-playing/devices/commands; `Effect` = resolve/refresh/play/stop/pause/volume/device/start metadata/theme/quit. Failover and merging reuse `Failover`/`Merger`; late resolution answers for another station are ignored.
- Binary `src/tui.rs` (`ttnr` without subcommand): one loop over engine events, messages from spawned tasks (resolution, metadata provider – aborted on station change) and SIGTERM; effects performed on the player; state published via `tokio::sync::watch::send_if_modified` only when it changed. UI task: `ratatui::try_init` (panic hook restores the terminal), crossterm `EventStream`, draws only when the state changed or on resize, `restore()` on every exit. Keys → commands in `ttnr-tui::keys` (SPEC F5.3; Ctrl-C quits; text input in the URL prompt).
- Engine: `EngineEvent::Devices` from `audio-device-list` (observed as JSON string, CoreAudio entries only); `ErrorKind::{is_retryable, is_stale_hint}` shared by CLI and TUI.
Consequences: `ttnr play` still has its own loop (same `Failover`/`Merger` logic); migrating it onto `App` is a follow-up when its line output can be derived from state changes.

## ADR-023 · 2026-11-03 · Views: priority-based layout, minimal theme model first
Context: M3 task 2 – main view (SPEC F5.1), station list, open-URL prompt, help, device picker; usable from 32×16; views must not hard-code colours/glyphs (CLAUDE.md), but the theme engine is task 4.
Decision:
- `ttnr-themes::Theme` starts as plain data (palette roles fg/bg/accent/dim/warn/error as `ThemeColor` Reset/Ansi/Indexed/Rgb; glyphs: borders or none, ellipsis, dash, separator, status symbols, list marks; `uppercase`; title) with built-in `plain`; `Theme::fit` cuts to terminal *columns* (`unicode-width`, wide CJK = 2) ending in the theme's ellipsis. Task 4 adds TOML + depth fallback on top.
- Main view lines carry priorities; when the height is short the least important go first (blank lines → older history → "recent" → album → programme → format); station, track and status always stay. One column margin from 40 columns. The key-help line shows as many default bindings as fit, always ending with `? help · q quit`.
- Overlays are centred, cleared, framed popups; lists scroll to keep the highlight visible, mark the current item, highlight with reverse video.
Consequences: snapshot tests (task 5) will freeze the layouts per theme and size; the help table comes from the keymap once it is remappable (task 3).

## ADR-024 · 2026-11-03 · Remappable keymap in `config.toml`
Context: M3 task 3 – keymap (SPEC F5.3), remappable in config (SPEC F8).
Decision: `~/.config/ttnr/config.toml` (`$XDG_CONFIG_HOME` respected; missing = defaults; unknown fields rejected) gets `[keys]`: `action = "key"` or a list; giving an action replaces its defaults. Actions: toggle_pause, volume_up, volume_down, mute, next_station, prev_station, stations, open_url, devices, cycle_theme, help, quit. Key names: one character, space/enter/tab/backspace/esc/arrows/home/end/pageup/pagedown/f1–f24, `ctrl-`/`alt-` prefixes (letters normalised to lower case, as terminals report them). Validation: unknown action, unparsable key, a key bound to two actions, or remapping a reserved key (arrows/enter/esc, ctrl-c) is an error reported before the TUI takes the terminal. Fixed: navigation keys, j/k in lists, ctrl-c quits. Help overlay and footer are generated from the active keymap. `examples/config.toml` documents it (tested for validity).
Consequences: further settings (theme, colour depth, volume, history length, sync offset) join `Config` as their tasks land.

## ADR-025 · 2026-11-03 · Theme engine: TOML themes, depth fallback, cycling in state
Context: M3 task 4 – theme model + TOML loader + `plain`; colour-depth fallback (SPEC F6.1, F6.4).
Decision: themes are TOML (schema in THEMES.md), `deny_unknown_fields`; built-ins embedded with `include_str!` (`plain` now), user themes from `~/.config/ttnr/themes/` override by name; broken user themes are skipped, logged and shown once in the status line. `ColorDepth` detected (`NO_COLOR` → mono, `COLORTERM` truecolor/24bit → truecolor, `TERM` dumb/empty → mono, `256color`/kitty/Terminal.app → 256, else 16) or set by `color` in config. `Theme::for_depth` applies `[palette_256]`/`[palette_16]` overrides, then downgrades (nearest xterm cube/grey, nearest ANSI 16, mono = terminal default; emphasis then relies on bold/reverse). Config `theme = "…"` (unknown name = error before the TUI starts). `t` increments `AppState::theme_index` (no effect needed); the UI resolves all themes for the depth once and picks by index.
Consequences: retro themes (M5) add files plus the remaining controls (logical width, prompt, cursor, number format, effects, banner).

## ADR-026 · 2026-11-03 · Snapshot tests per theme × view × size
Context: M3 task 5 (THEMES.md: 32×16, 40×25, 100×30).
Decision: `crates/ttnr-tui/tests/snapshots.rs` renders 8 cases (main idle/playing/reconnecting/error, stations, open URL, help, devices) for every built-in theme at the three sizes into insta snapshots (`view@<theme>_<case>_<w>x<h>.snap`, characters only; colours are asserted separately in `views.rs`). Fixtures are deterministic (no clock). New/changed snapshots are reviewed by hand (`cargo insta review` or the `.snap.new` files, git-ignored) before committing; the gate fails on any difference.
Review findings fixed on the way: collapsed duplicate/edge blank lines, codec label drops the codec when the profile names it (`HE-AAC`, but `AAC LC`), a cut-off URL in the prompt starts with the theme's ellipsis.
Consequences: each M5 theme automatically gets 24 snapshots once its file is added to the built-ins.

## ADR-027 · 2026-11-03 · Terminal-independent dim text; flicker-free resize
Context: owner review of M3 – `plain` looked brown in kitty (ANSI 8 = `#5a3625` there); Terminal.app jumped one line during vertical resize; kitty flickered at the start of a resize.
Decision: theme colour value `"faint"` = default foreground + SGR 2; `plain` uses it for `dim` (secondary text, borders, footer) instead of `bright_black`. Every frame is wrapped in synchronized output (DEC 2026) so ratatui's clear-on-resize never shows as a blank frame, and the hidden cursor is parked at (0,0) after each frame so terminals that scroll to keep the cursor visible on shrink (Terminal.app) do not shift the UI.
Consequences: accents remain ANSI colours (they follow the user's scheme by design); custom themes can choose any style. Verified by recording in Terminal.app; kitty and iTerm2 rely on the same mechanisms (both document mode 2026 support – to be confirmed by eye).

## ADR-028 · 2026-11-03 · Full repaint on every resize event
Context: iTerm2 reflows the screen on width changes; the diff renderer then kept the garbled content until the next state change (owner's recording).
Decision: on each `Event::Resize(w, h)` the UI opens a synchronized update and calls `Terminal::resize(Rect(0, 0, w, h))` (clears the screen and resets ratatui's buffers without querying the cursor – `clear()` would race with the input stream), so the next draw repaints everything; the draw closes the synchronized update.
Consequences: a resize costs one full repaint (cheap, no visible flash). What terminals draw *during* a live drag (iTerm2 reflow, kitty blank frame) is outside the app's control.

## ADR-029 · 2026-11-03 · BBC streams via BBC's own web-player flow; refusals are final
Context: M4 research (RESEARCH.md). From Poland the old media selector v2 refuses national stations; BBC's current web player gets an anonymous token from `rms.api.bbc.co.uk/v2/sign/token/<id>` and calls media selector v3 with it, receiving BBC's dedicated `nonuk` streams; rights-restricted services are refused (`403 geolocation`). Media selector responses carry a content-protection disclaimer; third-party use is a grey area. Hard-coded Akamai pool URLs are dead (410).
Decision (owner, 2026-11-03): the `bbc` source discovers streams exactly like BBC's web player (token → media selector v3), plays what BBC serves for the listener's location, and treats every refusal (`selectionunavailable`, `geolocation`, 4xx) as final: **no guessed URL patterns, no region spoofing, no proxies**. ADR-004's "patterns" step is not used for BBC; a user-pinned `url` in `stations.toml` remains the last resort (the user's responsibility). Station list and now-playing come from `rms.api.bbc.co.uk` (no auth).
Consequences: BBC can change or close this at any time (auntie-sounds warns of further non-UK restrictions) – the source must fail clearly ("BBC does not offer this station in your location") rather than work around it. Revisit if BBC publishes official terms for third-party players.

## ADR-030 · 2026-11-03 · BBC now-playing provider
Decision: `BbcNowPlaying` (attached by `BbcSource::metadata`) polls every 30 s `broadcasts/latest?service=<id>&on_air=now` (programme: "Presenter – Show" or "Programme – Episode") and `segments/latest?limit=1` (a track only if `offset.now_playing` and music); emits `NowPlaying` (source External, station name, artwork 320×320 from `{recipe}`) only on change; speech or a just-ended track give programme only; failures back off silently 60 → 120 → … ≤ 300 s; stops when the receiver closes. `ttnr play` prints `on air: …` when the programme changes.
Consequences: display timing still follows the API (live edge); the sync offset (task 4) delays it to match the audio.
