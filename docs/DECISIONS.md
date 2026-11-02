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

