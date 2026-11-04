# ttnr – text tuner

A small, open-source terminal player for **network radio** on macOS. It plays
any stream URL (Icecast/Shoutcast, HLS, playlists), BBC radio with live
programme and track information, stations from the radio-browser.info
directory, and sources you script yourself – with a minimal text UI and
faithful retro themes (DEC PDP-11 on a VT100 or a Teletype, 2.11BSD, Sinclair
ZX80 and ZX Spectrum, BBC Micro MODE 7), all in plain text.

Audio goes through libmpv (FFmpeg) to CoreAudio: every format FFmpeg
decodes, any channel layout, the macOS default output device followed as it
changes. Now Playing in Control Center and the media keys work; the player
reconnects after sleep or a dropped network.

## Install

Requirements: macOS on Apple Silicon (the only tested platform) and
Homebrew. ttnr is built from source either way (no prebuilt binaries:
see "Licence").

### With Homebrew (tap)

```zsh
brew install jankkhvej/ttnr/ttnr     # taps github.com/jankkhvej/homebrew-ttnr
```

This builds the latest release with Homebrew's mpv; `brew upgrade ttnr`
updates it, `brew install --HEAD jankkhvej/ttnr/ttnr` builds `main`.
Homebrew's mpv 0.41.0 has a CoreAudio bug on macOS 27, so until mpv 0.42
ttnr uses AVFoundation there (volume and mute react up to ~2 s late); the
source build below avoids it. Example configuration:
`$(brew --prefix)/share/ttnr/examples`; man page: `man ttnr`.

### From source

Needs Rust (<https://rustup.rs>; the toolchain is pinned in
`rust-toolchain.toml`).

```zsh
brew install mpv pkgconf ffmpeg
git clone https://github.com/jankkhvej/ttnr.git && cd ttnr
./scripts/build-libmpv.zsh     # recommended, see below (~20 s, no system changes)
cargo install --path .         # installs `ttnr` into ~/.cargo/bin
```

`scripts/build-libmpv.zsh` builds Homebrew's mpv 0.41.0 recipe plus an
upstream CoreAudio fix into `third_party/mpv/` and `ttnr` links to it.
Without it ttnr uses Homebrew's libmpv, whose CoreAudio output fails on
macOS 27 for many streams; it then falls back to AVFoundation, where volume
and mute react up to ~2 s late (ADR-015). Release builds: see
"Release build" below. The man page is `docs/ttnr.1`
(`man ./docs/ttnr.1`).

## Quick start

```zsh
ttnr play https://somafm.com/groovesalad.pls   # plain output, Ctrl-C stops
ttnr search "jazz"                             # radio-browser.info
mkdir -p ~/.config/ttnr && cp examples/stations.toml ~/.config/ttnr/
ttnr                                           # the TUI: pick a station, Enter
ttnr --theme bbc-mode7
```

## The TUI

| Key | Action | Key | Action |
|---|---|---|---|
| `space` | play / pause | `s` | station list |
| `+` `=` / `-` | volume | `/` | search radio-browser |
| `m` | mute | `o` | open a URL |
| `n` / `p` | next / previous station | `d` | output device |
| `t` | next theme | `b` | line speed (terminal themes) |
| `]` / `[` | track info later / earlier | `?` | help |
| `q` | quit | `i` | diagnostics line (optional) |

Keys are remappable (`[keys]` in `config.toml`). The view shows the station,
programme, artist – title, the stream format, status, volume, output device
and recent tracks; nothing else.

Media keys (F7/F8/F9), headphone buttons and Control Center control the
TUI and `ttnr play`. Set `media_controls = false` to turn that off.

## Command line

| Command | |
|---|---|
| `ttnr` | the TUI (`--theme <name>` picks a theme) |
| `ttnr play <url \| file \| station-id \| radio-browser:<uuid>>` | play with plain status lines |
| `ttnr search <text> [--source <id>] [--limit n]` | search a source (default radio-browser) |
| `ttnr stations [--search text]` | list `stations.toml` |
| `ttnr refresh [station-id \| url]` | re-discover stream URLs, bypassing the cache |
| `ttnr now [--json]` | what the running ttnr plays |
| `ttnr ctl <command> [arg]` | control it: `toggle_pause`, `next`, `prev`, `mute`, `quit`, `volume N`, `play <id>` |

Details: `man ./docs/ttnr.1`.

## Configuration

Everything is optional; ttnr runs without any configuration. Files live in
`~/.config/ttnr/` (`$XDG_CONFIG_HOME/ttnr/`): `config.toml`,
`stations.toml`, `themes/*.toml`. Start from `examples/config.toml` and
`examples/stations.toml`. Reference: [docs/CONFIG.md](docs/CONFIG.md).

## Themes

`plain` (default), `pdp11-vt100` (+ `-amber`, `-green`), `pdp11-bsd`,
`pdp11-teletype`, `zx80`, `zx-spectrum`, `bbc-mode7`. Retro themes have
boot sequences and machine-style messages; terminal themes can simulate a
slow serial line (`b`). Themes are TOML files – write your own:
[docs/THEME-GUIDE.md](docs/THEME-GUIDE.md). Design notes and sources:
[docs/THEMES.md](docs/THEMES.md).

## Sources

Built in: plain URLs, `stations.toml`, BBC (live national and local
stations; streams as BBC serves them for your location), radio-browser.info.
Any other source can be a script in any language speaking a small JSON
protocol – see [docs/SOURCES.md](docs/SOURCES.md) and
`examples/sources/somafm.py` (32 lines).

## Release build

```zsh
cargo build --release          # target/release/ttnr
```

The binary links libmpv by absolute path (the local build in
`third_party/mpv/prefix` or Homebrew's), so keep that library where it is.
The Homebrew formula is `packaging/homebrew/ttnr.rb`; the tap
repository `jankkhvej/homebrew-ttnr` carries a copy as `Formula/ttnr.rb`.
Releases: `CHANGELOG.md`.

## Development

The project is developed with Claude Code following `CLAUDE.md` and the
documents in `docs/` (spec, architecture, roadmap, research log, decision
records, progress log).

```zsh
./scripts/check.zsh    # the gate: cargo fmt --check, clippy -D warnings, tests, cargo deny
./start.zsh            # checks prerequisites, then Claude Code continues from docs/PROGRESS.md
```

Tests never use the network (fixtures under `tests/fixtures/`); live smoke
tests are `#[ignore]`d (`cargo test -- --ignored`). `cargo-deny` is needed
for the gate: `cargo install --locked cargo-deny`.

| Path | What |
|---|---|
| `docs/SPEC.md` | requirements and non-goals |
| `docs/ARCHITECTURE.md` | crates, data flow, stream resolution |
| `docs/ROADMAP.md` | milestones and acceptance criteria |
| `docs/DECISIONS.md` | architecture decision records |
| `docs/RESEARCH.md` | verified external facts with sources |
| `docs/PROGRESS.md` | session log |

## Licence

The source code is under the **BSD 2-Clause** licence (see `LICENSE`).
`ttnr` links at run time to libmpv and FFmpeg from Homebrew, which are built
as GPL (FFmpeg `--enable-gpl --enable-version3`). A compiled `ttnr` binary
is therefore, as a combined work, redistributable only under
GPL-3.0-or-later terms. Building from source for your own use is
unaffected.
Dependencies and their licences: [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)
(generated by `scripts/notices.py`).
