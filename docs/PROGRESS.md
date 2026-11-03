# Progress log

Newest entry on top. Each entry: date · milestone/task · what changed · how to try it · next step · open questions for the owner.

## 2026-11-03 · M7 · Control socket – **M7 complete, waiting for owner review**
- TUI serves `$TMPDIR/ttnr-$UID.sock` (JSON lines: status, subscribe, toggle_pause, next, prev, mute, volume, play, quit); `ttnr now [--json]`, `ttnr ctl` (ADR-042); `Command::SetVolume`.
- Tests: status JSON + line, request parsing, a served socket (status, command, unknown station, subscribe updates, second instance backs off, socket removed); live pty round trip (now, ctl volume, now --json, bad command, ctl quit → TUI exits in 90 ms, socket gone, "not running").
- M7 summary: radio-browser directory search (`/`, `ttnr search`), exec sources from scripts (`[sources.<id>]`, docs/SOURCES.md, 32-line SomaFM example), control socket + `ttnr now`/`ctl`.
- Follow-up: move `ttnr play` onto `App` (gains reconnects, socket).

## 2026-11-03 · M7 · `exec` source
- `ExecSource` (list/resolve/now over stdin/stdout JSON), `[sources.<id>]` in config.toml, TUI adds script stations to the list, `ttnr search --source`, docs/SOURCES.md, example `examples/sources/somafm.py` (32 lines) (ADR-041).
- Tests: shell-script doubles (list/search, resolve with headers/ttl, unsupported → chain, errors with stderr, missing program, `now` lines + stop on receiver drop), config validation/path expansion, station de-duplication. Live: `ttnr search groove --source somafm`, `ttnr play groovesalad` (exec station) plays with titles from `now`.
- Next: M7 task 3 – `ttnr now --json` and the control socket.

## 2026-11-03 · M7 · `radio-browser` source
- Owner's config restored (test lines removed). `RadioBrowserSource` (search, click counting, resolved URL via `UrlSource`), TUI `/` search → results list, `ttnr search`, `ttnr play radio-browser:<uuid>`, `stations.toml` entries (ADR-040).
- Tests: recorded fixtures (search, empty, click; failed click still plays; uuid-only station), detail/bitrate normalisation, URL encoding; core search flow (prefill, stale answers, clamp, play, empty/error); snapshots for search prompt + results. Live: `ttnr search "groove salad"`, `ttnr play radio-browser:960cf833-…` plays with ICY titles.
- Next: M7 task 2 – `exec` source (external command protocol).

## 2026-11-03 · M6 complete – accepted by the owner
- Now Playing / Control Center + media keys (AppKit host on the main thread, ADR-036); output devices: default followed by mpv, name shown, pinned device falls back and returns (ADR-037); reconnect with backoff + wake (ADR-038); optional exclusive mode and sample-rate matching (ADR-039).
- Owner on exclusive mode: macOS reroutes other apps to another output instead of muting them – keep it that way (option 1); muting other outputs was rejected as too invasive.
- Open: sample-rate switching not explicitly confirmed in Audio MIDI Setup; the owner's config has `exclusive`/`match_sample_rate` enabled from the test.
- Next: M7 – radio-browser source, `exec` source, `ttnr now --json` + control socket. Follow-up: move `ttnr play` onto `App` (ADR-022; gains reconnects).

## 2026-11-03 · M6 · Exclusive mode and sample-rate matching
- Config `exclusive` (mpv `audio-exclusive`, CoreAudio hog mode) and `match_sample_rate` (mpv `coreaudio-change-physical-format`), both off by default, for TUI and `ttnr play` (ADR-039). Test: options read back from libmpv. Waiting for the owner's check by ear / Audio MIDI Setup – then M6 is complete.

## 2026-11-03 · M6 · Task 3 done – owner test
- Sleep: woke → immediate reconnect, works. Wi-Fi off ~20 s on BBC HLS: mpv buffered, FFmpeg's HLS demuxer resumed by itself and skipped expired segments (log), so no reconnect was needed – correct. The reconnect path (stream really lost) was verified with a local dropping server; visible live with an Icecast station and a longer outage. Next: M6 task 4 – optional exclusive mode and sample-rate matching.

## 2026-11-03 · M6 · Reconnect with backoff, wake from sleep
- Core reconnects a station that has played when it fails retryably or ends while not seekable (live): 1/2/4/8/16/30/60 s, reset on playing; stale retries ignored; never-played stations fail as before. Engine: `seekable` in `StreamInfo`. macOS: wake notification → immediate reconnect (ADR-038).
- Tests: 6 core scenarios (backoff sequence + reset, never played, live end vs file end, stale/superseded retries, wake, resolve error while reconnecting), translator `seekable`; end to end with a local server dropping a real-time stream (3 connections, 1 s retries). Waiting for the owner's sleep / Wi-Fi test.

## 2026-11-03 · M6 · Task 2 done – owner confirmed
- Default-device following, pinned fallback/return and the device label work (owner: "everything is nice"). Next: M6 task 3 – sleep/wake and network changes: reconnect with backoff.

## 2026-11-03 · M6 · Output devices
- mpv already follows the system default (verified in its CoreAudio source). Added: pinned device as a preference (plays while present, falls back to the default, returns when replugged), config `output_device`, default device name in the view (`system default (MAJOR IV)`) from CoreAudio with a change listener (ADR-037).
- Tests: core fallback/return/no-op, picker; platform query + listener (ignored test needs a device); pty: label shown, quit 0.1 s. Waiting for the owner's AirPods test.

## 2026-11-03 · M6 · Task 1 done – owner confirmed
- Now Playing / Control Center and media keys work in the TUI in kitty and Terminal.app (keys, Control Center, headphones). Next: M6 task 2 – follow default-device changes, pinned device, device picker from mpv's list.

## 2026-11-03 · M6 · Next/previous greyed out where they do nothing
- Owner: next/previous "did not work in Terminal.app" (keys, Control Center, headphones). Log: those sessions were `ttnr play`, which receives every command but ignores next/previous by design (one target); the TUI switches stations. Fix: `NowPlayingInfo::can_skip` – Control Center/media next/previous are disabled in `ttnr play` and in the TUI without a station list. TUI logs the terminal program at start.

## 2026-11-03 · M6 · Now Playing + media keys in the app
- Owner's spike test: all four checks passed (Control Center entry, F8, F7/F9, Control Center buttons) with `.accessory` + embedded Info.plist. Owner approved the design (ADR-036).
- `ttnr-platform`: `run_app`, `MediaLink`, `NowPlayingInfo`, `RemoteCommand`; macOS host (NSApplication accessory, remote command targets, Now Playing with heartbeat). Binary: `src/media.rs` mapping, TUI and `ttnr play` integration, config `media_controls`, Info.plist in `ttnr`.
- Tests: platform link/no-host, media mapping (track/programme/station lines, states, keys); pty: TUI under the host draws and quits in 80 ms. Waiting for the owner's check in the real app.

## 2026-11-03 · M6 · Now Playing spike
- Research (RESEARCH.md): libmpv's own Now Playing is unusable for us; plan = own module in `ttnr-platform` on objc2 + objc2-media-player, AppKit host on the main thread, app on a worker thread (needs owner OK – large structural change), 1 s heartbeat.
- Spike `crates/ttnr-platform/examples/nowplaying_spike.rs` (+ embedded Info.plist `org.hcc07.ttnr`, opt-out `TTNR_NO_PLIST=1`): plays a stream, publishes Now Playing as a live stream, prints received remote commands. Waiting for the owner's test (Control Center + media keys, kitty and Terminal.app).

## 2026-11-03 · M5 complete – accepted by the owner
- Themes: `plain`, `pdp11-vt100` (+amber, +green), `pdp11-bsd`, `pdp11-teletype`, `zx80`, `zx-spectrum`, `bbc-mode7`; engine: logical screen + margins, text rules (case, ASCII, character map, octal, percent sign), chrome (banner, prompt, cursor, double-height bar, mosaic rule), status templates, printout mode, animations (boot sequences, tape border, ZX80 blank), serial line speed (`b`, `baud`) with cursor (ADR-033–035).
- Open, not blocking: host name in the `pdp11-bsd` banner (`ttnr`); sextant rule in Terminal.app unchecked; Teletype "everything on paper" is a proposal (THEMES.md).
- Next: M6 – macOS integration, task 1: Now Playing / Control Center + media keys.

## 2026-11-03 · M5 · Fix: boot's last frame could stay on screen (owner)
- Owner: `pdp11-vt100-amber` looked hung on the boot's last frame after switching. Cause: a race – the next wake-up was computed (and the boot expired) just after its last frame was drawn, so no further redraw was scheduled. `Fx::tick` no longer expires anything; an ended animation always gets one more frame. Regression test `an_ended_boot_still_gets_a_final_frame`; pty: 8/8 runs show the UI after the boot, quit in ~10 ms.

## 2026-11-03 · M5 · VT100 boot noise static (owner)
- The VT100 power-on noise no longer changes per frame. Research: DEC documents ~1 s reset, self-test, keyboard LEDs + bell, cursor at line 1 column 1; random characters only on fatal errors; per-character attributes need the AVO → static garbage without attributes kept as a short design choice (RESEARCH.md).

## 2026-11-03 · M5 · Slow line: no retyping on resize (owner)
- With a line speed set, resizing the window (and the first frame) no longer retypes the whole screen; the new layout appears at once and only later UI changes arrive slowly. Test `resize_redraws_at_once`.

## 2026-11-03 · M5 · `pdp11-bsd` theme (owner)
- New built-in `pdp11-bsd`: 2.11BSD on a VT100 console – lower case, decimal, ASCII borders, getty banner, `% ttnr play ` URL prompt, `ttnr: …` errors, VT100 self-test + 2.11BSD boot transcript, speed key. Facts from the 2.11BSD source tree and emulator logs (RESEARCH.md).
- Engine: `[effects] boot_text` (VT100 boot lines per theme; RSX lines moved into the `pdp11-vt100*` files; long transcripts speed up and scroll).

## 2026-11-03 · M5 · Cursor during slow redraws (owner)
- VT100 themes at a set line speed show a block cursor on the next cell to arrive while an update is in progress (owner: slow full redraws looked hung). Tests extended in `serial.rs`.

## 2026-11-03 · M5 · Line speed key (owner)
- `b` cycles the simulated line speed off → 9600 → 1200 → 300 → 110 baud on terminal themes (`[chrome] serial`: teletype, VT100 variants); shown as `9600 BAUD`; teletype prints at that speed, VT100 receives changed cells one by one (ADR-035). Config `baud` is the start value.
- Tests: core cycling, `SlowScreen` (row order, line speed, no banked idle time, switch-on keeps the screen), printout speed change, view label; pty: `b` on VT100 → only the label changes, at 110 baud a volume change arrives ~0.1 s per character; start with `baud = 1200` types the screen out.

## 2026-11-03 · M5 · Fix: list highlight showed through boot animations
- Owner: the station list's highlight was visible during every boot sequence. Cause: effect cells were style-*patched* over the UI, keeping its reverse-video modifier. Cells are now reset before painting; regression test `boot_frames_hide_the_ui_below` (fails without the fix).

## 2026-11-03 · M5 · Spectrum tweaks (owner)
- Boot: static garbage → black with red `¦` lines → copyright, 1.4 s (was 2.6 s with moving garbage).
- `[screen] margin_x`/`margin_y`: `zx-spectrum` has 2 columns / 1 row of paper around the text, so the tape stripes keep their distance (margins drop first below 36×26).

## 2026-11-03 · M5 · Owner feedback: Spectrum, animations, baud
- Owner: new `zx-spectrum` theme (tape-loading border stripes), boot animations on theme switch, optional Teletype line speed; "everything on paper" kept as a proposal (THEMES.md).
- Done: `VOL 144` without `%` in octal themes (`percent_sign`); `baud = 110|300|9600` for printout themes; `[effects]` with boot sequences (Spectrum RAM test → copyright, ZX80 K cursor, BBC banner, VT100 self-test → RSX-11M boot lines) and loading effects (Spectrum tape stripes, ZX80 blank); config `effects = false` disables all (ADR-034). Research via subagent logged in RESEARCH.md.
- Tests: fx time model (boot once per switch, skip, disabled, tape phases/tail/error/mono, ZX80 blank, stripe heights), printout line speed and backlog, animation frame snapshots, pty run (boot → `t` → next boot → quit).
- Open: the "exclamation marks" Spectrum boot pattern was not found in sources – which model/situation did you see it on?

## 2026-11-03 · M5 · Retro themes implemented (milestone done, awaiting owner review)
- Theme engine: optional `[screen]` (logical width/height, border colour), `[text]` (uppercase, ASCII-only transliteration, character map, octal), `[chrome]` (banner, prompt, cursor, double-height title bar, mosaic rule with quadrant fallback, printout mode), `[status]` templates (ADR-033). Border colour now follows colour-depth fallback; the title bar becomes inverse video in mono.
- Built-ins: `pdp11-vt100` (+`-amber`, `-green`), `pdp11-teletype`, `zx80`, `bbc-mode7`; snapshots for every view × 3 sizes × theme, plus a Teletype printout session. `plain` unchanged.
- Config: `mosaic = "auto|sextant|quadrant"`; CLI: `ttnr --theme <name>` (SPEC F7) overrides config.
- Next: owner review ("recognisable at a glance"), then M6.
- Open questions: does Terminal.app with your font show the sextant rule in `bbc-mode7` (else set `mosaic = "quadrant"`)? Octal applies to volume/sync only – also wanted for bitrate/sample rate?

## 2026-11-03 · M5 · Retro machines researched
- THEMES.md rewritten from sources (VT100 manual, RT-11/RSX-11 manuals, ASR-33, ZX80 manual, SAA5050/teletext, Unicode charts); UNVERIFIED items marked. Owner confirmed BBC track info now appears at the right time (M4 timing fix).
- Next: M5 task 2 – theme engine extensions + the four themes.

## 2026-11-03 · M4 complete; M9 added
- M4 accepted by the owner: BBC source (web-player flow, refusals final), now-playing with on-air timing (default offset 0), resolver/failover shared with other sources.
- Owner request: optional diagnostics indicators → new ROADMAP M9 (after M8), SPEC F5.2 note.
- Next: M5 – retro themes (owner most interested).

## 2026-11-03 · M4 · Now-playing timing fix
- Owner test: offset keys seemed to do nothing; titles late. Fixed: BBC polling 10 s, updates stamped with on-air time (broadcast start + segment offset), shown at `at + offset` with one re-timable pending update; `ttnr play` uses the same rule (ADR-032). Default offset 0 (owner's preference).
- Tests: timed display, re-timing on `]`/`[`, stale timers ignored, RFC 3339 parser, provider timestamps from recorded fixtures.

## 2026-11-03 · M4 · Sync offset mechanism
- Per-source sync offset (`[sync_offset]`, live `]`/`[` keys, shown in the view; applied in TUI and `ttnr play`) (ADR-031). BBC default awaits the owner's ears.
- Tests: delayed/dropped provider updates, key adjustment per source, view, config; help snapshots updated (two new bindings, cut first at 32×16).

## 2026-11-03 · M4 · BBC now-playing
- `BbcNowPlaying` provider (programme + current track, speech handling, backoff), `ttnr play` prints `on air:` (ADR-030).
- Tests: recorded fixtures (6 Music music/just-ended, Radio 4 speech, both programme formats), provider with virtual time (change-only, speech → programme only, backoff 60/120/240 s, stop).
- Live from Kraków: 6 Music shows programme + track; Radio 4 shows programme during speech.
- Next: M4 task 4 – sync offset (needs the owner's ears).

## 2026-11-03 · M4 · BBC resolver
- `ttnr-sources::bbc`: `BbcSource` (list from `/v2/networks`, one station per service; discovery = token → media selector v3; highest bitrate, HLS, https, one URL per CDN as failover candidates; refusals final with clear messages; no patterns – ADR-029). Registered for `play`, `refresh` and the TUI.
- Tests: recorded fixtures (networks, 6 Music selection, geolocation and unavailable refusals; fake token), registry chain (bearer token sent, cached, refusal → override only, no probing), ignored live smoke tests (pass from Kraków).
- Live: `ttnr play bbc_6music` from Kraków → HE-AAC 48 kHz stereo ~102 kbps.
- Next: M4 task 3 – BBC now-playing provider.

## 2026-11-03 · M4 · BBC research
- Verified from Kraków: station list (`rms /v2/networks`, 66 networks), stream discovery as BBC's web player does it (token → media selector v3 → `nonuk` HLS, 96k HE-AAC, plays in mpv), 5 Sports Extra refused (`403 geolocation`), now-playing (`segments/latest`) and programme (`broadcasts/latest`) without keys. Old `pool_904` URLs → 410.
- Owner decision: use the web-player flow, refusals final, no patterns/spoofing (ADR-029).
- Next: M4 task 2 – BBC resolver.

## 2026-11-03 · M3 review follow-up
- Owner: Terminal.app looks as intended; kitty colours off (secondary text brown); Terminal.app jumped a line on vertical resize; kitty slight flicker at resize start; everything else OK.
- Fixed: `faint` colour value, `plain` dim = faint; synchronized output around every frame; cursor parked at (0,0) after each frame (ADR-027). Terminal.app resize verified by screen recording before/after (recordings deleted).
- To check by the owner: kitty and iTerm2 resize/colours.
- iTerm2 (owner's recording): reflowed garbage stayed after a width change → full repaint on every resize event (ADR-028); verified with an AppleScript-driven iTerm2 recording: clean as soon as the drag stops; during the drag iTerm2's own reflow remains (normal terminal behaviour).
- Owner's kitty recording analysed frame by frame: the black rectangles are kitty's size overlay; the start-of-resize flicker is one fully blank frame drawn by kitty before ttnr is told about the resize (kitty `resize_debounce_time`), so not fixable in ttnr (RESEARCH.md).

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
