# Progress log

Newest entry on top. Each entry: date · milestone/task · what changed · how to try it · next step · open questions for the owner.

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
