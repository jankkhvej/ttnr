# Changelog

## 0.1.1 – 2026-11-04

### Fixed
- Script sources (`[sources.<id>]`): while a source's `now` script ran, a
  concurrent `list` or `resolve` call to a script could hang until its 15 s
  timeout and fail. Script processes are now started one at a time.

### Documentation
- Homebrew install: `brew tap jankkhvej/ttnr`, `brew trust jankkhvej/ttnr`
  (Homebrew loads third-party taps only once trusted), then
  `brew install jankkhvej/ttnr/ttnr`.

## 0.1.0 – 2026-11-04

First release. Source only: build with `cargo install --path .` (see
README); a compiled binary links GPL builds of libmpv and FFmpeg and is,
as a combined work, GPL-3.0-or-later.

### Playback
- Any stream URL: Icecast/Shoutcast (ICY titles), HLS (timed ID3 titles),
  plain HTTP audio, local files, playlists (`.pls`, `.m3u`, `.m3u8`,
  `.xspf`, `.asx`) with mirror fail-over.
- Every format FFmpeg decodes, any channel layout, through libmpv to
  CoreAudio; the stream format is shown (codec, rate, bit depth, channels,
  bitrate).
- Stream URLs resolved at play time with a small cache; a stale URL is
  re-discovered once; dropped live streams reconnect with backoff; the
  computer waking from sleep reconnects at once.

### Sources
- `stations.toml` station list.
- BBC radio (national and local) through BBC's own web-player flow, with
  programme and track information; stations BBC does not offer for your
  location fail with a clear message.
- radio-browser.info directory search (`/` in the TUI, `ttnr search`).
- Scripted sources in any language over a small JSON protocol
  (`[sources.<id>]`, docs/SOURCES.md, example `examples/sources/somafm.py`).

### Terminal UI
- Minimal view: station, programme, artist – title, format, status, volume,
  output device, recent tracks, key help; station list, URL prompt, device
  picker, help; remappable keys; works from 32×16.
- Themes as TOML files, user themes in `~/.config/ttnr/themes/`; built in:
  `plain`, `pdp11-vt100` (+ amber, green), `pdp11-bsd`, `pdp11-teletype`,
  `zx80`, `zx-spectrum`, `bbc-mode7` – boot sequences, ZX Spectrum tape
  border, simulated serial line speed (`b`), colour-depth fallback.
- Track-info sync offset per source (`]` / `[`).
- Optional diagnostics line (`i`): network, download rate, last request,
  sync note.

### macOS
- Now Playing / Control Center and media keys.
- Follows the default output device; pinned device with fallback.
- Optional exclusive (hog) mode and sample-rate matching.

### Command line
- `ttnr play`, `ttnr search`, `ttnr stations`, `ttnr refresh`, `ttnr now
  [--json]`, `ttnr ctl`; control socket (JSON lines) for other tools.

### Known limits
- macOS on Apple Silicon only (tested on macOS 27).
- MPEG-DASH (`.mpd`) is not supported (no DASH demuxer in Homebrew's
  FFmpeg).
- Homebrew's mpv 0.41.0 has a CoreAudio bug on macOS 27; build the fixed
  libmpv with `scripts/build-libmpv.zsh`, or ttnr falls back to AVFoundation
  (volume/mute react up to ~2 s late) until mpv 0.42.
