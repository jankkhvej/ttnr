# CLAUDE.md — instructions for Claude Code

You are developing **ttnr**: a minimal, themeable terminal (TUI) player for
**network music streams** on macOS, written in Rust, built only from
open-source components. Owner: Sergii. Name: `ttnr` ("text tuner", ADR-008).

Read these before doing anything, every session:

- @docs/SPEC.md — what to build (requirements, non-goals)
- @docs/ARCHITECTURE.md — how it is structured (crates, traits, data flow)
- @docs/ROADMAP.md — milestones, tasks and acceptance criteria
- @docs/PROGRESS.md — where the previous session stopped
- @docs/THEMES.md — theme design spec (PDP-11, ZX80, BBC Micro)
- @docs/RESEARCH.md — verified external facts (with sources)
- @docs/DECISIONS.md — architecture decision records

## Session workflow (follow every time)

1. **Orient.** Read PROGRESS.md and ROADMAP.md. Run `./scripts/check.zsh`
   to see the current state of the build. Fix a red build before anything else.
2. **Pick** the first unchecked task of the current milestone. Do not jump
   ahead to later milestones. Do not start more than one task at a time.
3. **Plan briefly** (a few lines in your reply), then implement in small steps.
   Write or update tests with the code, not after it.
4. **Gate.** `./scripts/check.zsh` must pass: `cargo fmt --check`,
   `cargo clippy --all-targets -- -D warnings`, `cargo test`,
   `cargo deny check` (licences/advisories).
5. **Record.** Tick the task in ROADMAP.md, append a dated entry to
   PROGRESS.md (what changed, what's next, open questions), add an ADR to
   DECISIONS.md for any non-trivial design choice.
6. **Commit** locally with a Conventional Commit message
   (`feat(engine): …`, `fix(bbc): …`). Never push; the owner pushes.
7. **Continue** with the next task. At the **end of a milestone**: stop,
   summarise what the owner should try by ear/eye (exact commands), and wait.

## Hard rules

- **Open source only.** Every dependency (crate, library, tool) must have an
  OSI-approved licence and pass `cargo deny`. No closed SDKs, no telemetry,
  no accounts. Apple system frameworks (CoreAudio, MediaPlayer) are allowed
  only as the OS layer we must talk to.
- **Verify, don't invent.** Any external fact (API endpoints, stream URLs,
  codec support, historical details for themes, crate capabilities) must be
  checked with WebSearch/WebFetch or by running code, and logged in
  RESEARCH.md with source URL and date. If you cannot verify it, mark it
  `UNVERIFIED` in code comments and docs and tell the owner. Never present
  guesses as facts.
- **Audio streams only.** Video is always disabled (no video output, ever).
- **No network in unit tests.** Use recorded fixtures under `tests/fixtures/`.
  Network smoke tests live behind `--ignored` / a `net` feature.
- **Stay inside this repo.** Don't modify files outside it. Ask the owner
  before: installing system packages (brew), adding a new system/native
  dependency, changing the licence, renaming the project, or any large
  architectural change not covered by ARCHITECTURE.md.
- **Keep the TUI minimal.** No spectrum/FFT/visualisers. Only the info
  listed in SPEC.md.
- **Themes are text-only.** Characters, case, colours, borders, layout width,
  prompts and banners — never images, never sixel/kitty graphics.

## Conventions

- Rust stable (pinned in `rust-toolchain.toml`), edition 2021+, workspace layout.
- Errors: `thiserror` in libraries, `anyhow` in the binary. No `unwrap()` /
  `expect()` outside tests except with a `// SAFETY:`-style justification.
- Logging: `tracing`, to a file (never to the TUI's stdout).
- Config: TOML under `$XDG_CONFIG_HOME/ttnr/` (default `~/.config/ttnr/`).
- Public traits and modules get rustdoc comments.
- Snapshot-test TUI rendering with ratatui's `TestBackend` (+ `insta`).
- Prefer small, boring, well-maintained crates; record why in DECISIONS.md.

## Communicating with the owner

The owner is an experienced engineer (software + radio). Be concise and
precise. When something needs his ears (audio quality, device switching),
give exact commands to run and what to listen for.
