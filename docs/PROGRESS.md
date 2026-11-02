# Progress log

Newest entry on top. Each entry: date · milestone/task · what changed · how to try it · next step · open questions for the owner.

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
