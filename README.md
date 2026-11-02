# ttnr – text tuner

A minimal, themeable, open-source terminal player for network music streams
on macOS — BBC radio with live track info first, any stream URL after that.
Rust + ratatui + libmpv (FFmpeg) → CoreAudio. Themes: plain, PDP-11 (VT100 /
Teletype), Sinclair ZX80, BBC Micro MODE 7 — in text.

**Status:** scaffold only. The code is written by Claude Code following the
docs in this folder.

## Start / continue development

```zsh
cd ~/workspace/tuner
./start.zsh            # checks prerequisites, then Claude Code continues from docs/PROGRESS.md
./start.zsh --resume   # reopen the last Claude Code conversation in this folder
./start.zsh --auto     # fewer permission prompts (Claude Code auto mode)
./start.zsh --check    # prerequisites only
```

Prerequisites (the script checks and offers to install the brew ones):
Homebrew, Claude Code CLI, git, Rust (https://rustup.rs), and
`brew install mpv pkgconf ffmpeg`. Later `cargo-deny` is needed for the
licence gate (`cargo install --locked cargo-deny`).

Recommended: `./scripts/build-libmpv.zsh` builds a fixed libmpv into
`third_party/mpv/` (no system changes, ~20 s). Homebrew's mpv 0.41.0 has a
CoreAudio bug on macOS 27; without the local build ttnr falls back to
AVFoundation (volume/mute react up to ~2 s late). See docs/DECISIONS.md ADR-015.

Claude works milestone by milestone and **stops at the end of each one** for
you to try it — the exact commands will be in its last message and in
`docs/PROGRESS.md`. Run `./start.zsh` again to continue.

## Files

| Path | What |
|---|---|
| `CLAUDE.md` | Rules and session workflow for Claude Code |
| `docs/SPEC.md` | Requirements and non-goals |
| `docs/ARCHITECTURE.md` | Crates, traits, stream resolution, engine config, tests |
| `docs/ROADMAP.md` | Milestones M0–M8 with acceptance criteria |
| `docs/THEMES.md` | Theme spec (to be verified against sources) |
| `docs/RESEARCH.md` | Verified external facts with sources |
| `docs/DECISIONS.md` | Architecture decision records |
| `docs/PROGRESS.md` | Session log — where things stand |
| `scripts/check.zsh` | Quality gate: fmt, clippy, tests, licences |
| `.claude/settings.json` | Claude Code permissions (no push, no sudo) |
| `examples/stations.toml` | Sample station list |

## Licence

The source code is under the **BSD 2-Clause** licence (see `LICENSE`).
`ttnr` links at runtime to libmpv and FFmpeg from Homebrew, which are built
as GPL (FFmpeg `--enable-gpl --enable-version3`). A compiled `ttnr` binary
is therefore, as a combined work, redistributable only under
GPL-3.0-or-later terms. Building from source for your own use is unaffected.

