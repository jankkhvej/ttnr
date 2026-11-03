# Roadmap

Work top to bottom. Tick `[x]` when a task meets its acceptance criteria and
`./scripts/check.zsh` is green. Stop at the end of each milestone for owner review.

## M0 — Bootstrap
- [x] Verify toolchain: Rust stable, libmpv via Homebrew (`pkg-config --modversion mpv`), ffmpeg. Log versions in RESEARCH.md.
- [x] Choose and verify the libmpv Rust binding (maintenance, licence, mpv version compatibility) → ADR.
- [x] Cargo workspace with the crates in ARCHITECTURE.md (empty but compiling), `rust-toolchain.toml`, `LICENSE` (BSD-2-Clause, ADR-007), `deny.toml`.
- [x] `scripts/check.zsh` gate (fmt, clippy -D warnings, test, deny) passes.
- [x] `ttnr --version` and `ttnr --help` work.
**Acceptance:** clean clone → `./scripts/check.zsh` green.

## M1 — Playback engine (headless)
- [x] `MpvPlayer`: audio-only config, play/stop/pause/volume via CoreAudio.
- [x] Engine events: buffering, playing, error kinds (incl. HTTP 403/404), EOF.
- [x] `StreamInfo` from mpv properties (codec, rate, format, channels, bitrate).
- [x] `ttnr play <url>` plays in the terminal (plain log output, Ctrl-C stops).
- [x] `scripts/gen-fixtures.zsh` + format-matrix test (see ARCHITECTURE.md) incl. 5.1 and 7.1.
**Acceptance:** owner hears an Icecast MP3 stream, an HLS AAC stream and a local 5.1 file correctly.

## M2 — Sources & in-stream metadata
- [x] `Source` / `MetadataProvider` traits, `SourceRegistry`, `ResolveCtx`, resolver chain + cache (cache/discovery/patterns/override).
- [x] `url` source: direct URLs and playlist files (`m3u`, `m3u8` master/media, `pls`, `xspf`, `asx`) with fixtures.
- [x] `stations` source from `stations.toml` (sample file in `examples/`).
- [x] ICY and HLS timed-ID3 metadata surfaced as `NowPlaying`.
- [x] Auto re-resolve once on 403/404; `ttnr refresh [station]`.
**Acceptance:** `ttnr play` with a `.pls` URL shows ICY titles; refresh path covered by tests.

## M3 — Minimal TUI + theme engine
- [x] App state machine in core; ratatui UI on its own task; redraw only on change.
- [x] Main view per SPEC F5.1; station list; open-URL prompt; help overlay; device picker.
- [x] Keymap (SPEC F5.3), remappable in config.
- [x] Theme model + TOML loader + `plain` theme; colour-depth fallback.
- [x] Snapshot tests for all views at 3 sizes.
**Acceptance:** owner uses it for an hour with no glitches; resize is clean.

## M4 — BBC source
- [x] Research & verify (log in RESEARCH.md): list of BBC national/local stations and IDs; how BBC's own player discovers live stream URLs (machine-readable endpoint); current URL patterns, regions (`ww`/`uk`) and bitrates; geo-restrictions for listeners outside the UK.
- [x] Resolver: discovery → patterns → override, with quality/region fallback; works from Poland (international streams).
- [x] Now-playing provider: current track + programme (BBC Radio & Music Services endpoints), handles empty responses (speech programmes), backoff, fixtures.
- [x] Sync offset default for BBC measured by ear with the owner; configurable. (Owner: default 0 – show as it comes; tunable, ADR-032.)
**Acceptance:** BBC Radio 6 Music plays from Kraków and shows show + artist/title; simulated URL change (stale cache) self-heals.

## M5 — Retro themes
- [x] Research each machine (THEMES.md checklist) and correct THEMES.md.
- [x] `pdp11-vt100`, `pdp11-teletype`, `zx80`, `bbc-mode7` themes + snapshots.
- [x] Octal numbers, double-height emulation, mosaic glyph fallback, uppercase mapping.
**Acceptance:** owner judges each theme recognisable at a glance. (Accepted 2026-11-03.)

## M6 — macOS integration
- [x] Now Playing / Control Center + media keys (MediaPlayer framework via `objc2`; terminal app has no window — solve and document).
- [x] Follow default-device changes; pinned device option; device picker uses mpv's device list.
- [ ] Sleep/wake and network change: reconnect with backoff.
- [ ] Optional exclusive mode and sample-rate matching.
**Acceptance:** AirPods connect/disconnect while playing → audio follows; media keys work; laptop sleep → resumes.

## M7 — Extensibility
- [ ] `radio-browser` source (search/browse the open directory).
- [ ] `exec` source: external command protocol (JSON over stdout) + one example script; documented in `docs/SOURCES.md`.
- [ ] `ttnr now --json`; control socket (JSON lines) with status/commands.
**Acceptance:** a new source added as a 30-line script, no recompilation.

## M8 — Polish & release
- [ ] README, man page, config reference, theme authoring guide.
- [ ] Release profile, `cargo install --path .` instructions, local Homebrew formula draft.
- [ ] Final licence audit (`cargo deny`), third-party notices file.

## M9 — Optional diagnostics indicators (owner request 2026-11-03; after M8)
- [ ] Optional, individually toggled indicators (all off by default, config + key): network activity, stream download rate (mpv cache/demuxer stats), metadata API requests (last poll, errors/backoff), a note when now-playing may be out of sync (e.g. BBC listing lag).
- [ ] No extra redraws when disabled; when enabled, refresh at most ~1 Hz and only on change.
**Acceptance:** with indicators off the UI is unchanged (snapshots identical); each indicator can be enabled alone.
