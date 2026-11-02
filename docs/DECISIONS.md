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

## ADR-003 · 2026-11-02 · Licence GPL-3.0-or-later (proposed — owner to confirm in M0)
Context: linking libmpv built as GPL.
Decision: GPL-3.0-or-later for the project.

## ADR-004 · 2026-11-02 · Resolve stream URLs at play time
Context: BBC (and others) change stream URLs without notice.
Decision: resolver chain cache → discovery → patterns → user override; auto re-resolve once on 403/404; `tuner refresh`.
Consequences: no hard-coded URL is ever load-bearing; small on-disk cache.
