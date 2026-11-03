# ttnr — product specification

## Goal

A small, fast, **open-source** terminal player for **network music streams**
on macOS (Apple Silicon first). Think "VLC's *Open Network Stream*", but audio
only, with a minimal text UI that shows exactly what is playing, and with
faithful retro **text themes**.

The first showcase source is **BBC radio** (e.g. BBC Radio 6 Music) with live
track and programme information — which the BBC streams themselves do not carry.

## Users and context

- Owner: one technical user on macOS, listening while working.
- Runs in any modern terminal (Terminal.app, iTerm2, WezTerm, Ghostty, kitty).
- Must work with any monospace font; themes degrade gracefully.

## Functional requirements

### F1 Sources (modular, extensible)
- F1.1 Play any **URL** the user enters: Icecast/Shoutcast (ICY), plain HTTP
  audio files/streams, **HLS** (`.m3u8`), **MPEG-DASH** (`.mpd`), and playlist
  files (`.m3u`, `.pls`, `.xspf`, `.asx`) that resolve to streams.
  *DASH is deferred* (owner, 2026-11-02; ADR-016): 0 of 62,042 radio-browser
  stations use it and Homebrew's FFmpeg has no DASH demuxer.
- F1.2 A **source plugin system**: each source (generic URL, BBC, radio
  directories, …) implements one Rust trait and is registered in one place.
  Adding a new source must not require touching the engine or the TUI.
- F1.3 Built-in sources, in roadmap order: `url` (generic), `stations`
  (user's `stations.toml`), `bbc`, `radio-browser` (open directory
  radio-browser.info), `exec` (external command speaking a small JSON
  protocol, so sources can be scripted in any language without recompiling).
- F1.4 **Audio only.** Video tracks are never decoded or shown.

### F2 Formats and audio quality
- F2.1 Decode **every format FFmpeg can decode**: MP3, AAC-LC, HE-AAC v1/v2,
  xHE-AAC where FFmpeg supports it, Opus, Vorbis, FLAC, ALAC, AC-3/E-AC-3,
  PCM, etc., in any common container (TS, fMP4, ADTS, Ogg, Matroska, …).
- F2.2 **Any channel layout**: mono, stereo, 5.1, 7.1 — output via CoreAudio
  with correct channel mapping; downmix only when the device needs it.
- F2.3 Bit-perfect-friendly options: optional exclusive (hog) mode and
  optional device sample-rate matching. Off by default.
- F2.4 Show the actual stream format: codec, sample rate, bit depth (if
  known), channel layout, bitrate.

### F3 macOS integration ("works seamlessly with Apple's sound system")
- F3.1 Output through **CoreAudio**.
- F3.2 Follow the **system default output device** when it changes
  (e.g. AirPods connect) unless the user pinned a device.
- F3.3 Optional pinned output device, chosen in-app.
- F3.4 **Now Playing / Control Center** integration and **media keys**
  (play/pause, next/previous station).
- F3.5 Survive sleep/wake and network changes: reconnect with backoff, no
  crash, no stuck UI.
- F3.6 App volume is independent of system volume.

### F4 Now-playing metadata
- F4.1 Unified `NowPlaying` model: station, programme/show, artist, title,
  (album, label, artwork URL when available), source of the info, timestamp.
- F4.2 Metadata providers: in-stream (ICY `StreamTitle`, HLS timed ID3) and
  external pollers (e.g. BBC's now-playing endpoint).
- F4.3 Configurable **sync offset** between metadata and audio.
- F4.4 Short recent-tracks history (last N, default 5).

### F5 Minimal TUI
- F5.1 Shows only: station, programme, artist — title, stream format line,
  status (playing/buffering/reconnecting/error), volume, output device,
  recent tracks, one-line key help.
- F5.2 **No FFT, no spectrum, no visualisers.** Optional diagnostics indicators
  (network activity, download rate, metadata polling, sync note) are allowed
  only as opt-in extras, off by default (owner, 2026-11-03; ROADMAP M9).
- F5.3 Keys (defaults, remappable): `space` play/pause, `+`/`-` volume,
  `m` mute, `n`/`p` next/prev station, `s` station list, `o` open URL,
  `d` output device, `t` cycle theme, `?` help, `q` quit; added later:
  `]`/`[` sync offset, `b` line speed of terminal themes.
- F5.4 Works from 32×16 up to any size; adapts layout; no flicker.

### F6 Themes (text-only, as faithful as possible)
- F6.1 Theme engine driven by TOML theme files (built-ins embedded, user
  themes in `~/.config/ttnr/themes/`).
- F6.2 A theme controls: palette, character case rules, border glyphs,
  allowed glyph set (ASCII-only / box-drawing / block mosaics), logical
  screen width (e.g. 32 or 40 columns) and centring, banner/boot text,
  prompt string, cursor style, number formatting (e.g. octal), status
  message style, text effects (bold/reverse/blink/double-height emulation).
- F6.3 Built-in themes: `plain` (default), `pdp11-vt100`, `pdp11-teletype`,
  `pdp11-bsd`, `zx80`, `zx-spectrum` (owner, 2026-11-03), `bbc-mode7` (BBC Micro by
  Acorn). See THEMES.md. Optional animations (boot sequences, tape-loading
  border) are text and colour only and can be turned off.
- F6.4 Colour depth fallback: truecolor → 256 → 16 → monochrome.

### F7 CLI and scripting
- `ttnr` (TUI), `ttnr play <url|station-id>`, `ttnr --theme <name>`,
  `ttnr stations`, `ttnr now --json` (current NowPlaying as JSON),
  and a local control socket so other tools (e.g. a menu-bar widget) can
  read status and send commands.

### F8 Configuration
- `~/.config/ttnr/config.toml` (respect `$XDG_CONFIG_HOME`),
  `stations.toml`, `themes/*.toml`. Sensible defaults; app runs with no config.

## Non-functional requirements

- **Licence:** project source BSD-2-Clause (GPL-compatible). A binary linked
  against Homebrew's libmpv/FFmpeg (GPL builds) is, as a combined work,
  distributed under GPL-3.0-or-later terms (ADR-007). All deps OSI-approved,
  enforced by `cargo deny`.
- **Privacy:** no telemetry, no accounts, no analytics.
- **Resources:** idle CPU near zero while playing; TUI redraws only on change.
- **Robustness:** a broken source or metadata provider never crashes the app.
- **Testability:** parsers and providers tested against fixtures; TUI via
  snapshot tests; a format-matrix test proves F2 with generated audio files.

## Non-goals

- Video. Visualisers. Music library management. Podcast management.
- Recording/downloading streams. DRM-protected services.
- Non-macOS platforms are not a goal now — but don't make Linux impossible
  (keep macOS-specific code behind a `platform` module).
