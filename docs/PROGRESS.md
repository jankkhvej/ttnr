# Progress log

Newest entry on top. Each entry: date · milestone/task · what changed · how to try it · next step · open questions for the owner.

## 2026-11-04 · Release 0.1.0 published; 0.1.1 prepared
- 0.1.0: GitHub release, tap github.com/jankkhvej/homebrew-ttnr (formula verified: strict audit, source install, `brew test`, fresh install from the public tap; all cleaned up afterwards).
- 0.1.1: the exec spawn fix (`dd56d89`) found after tagging 0.1.0; version bump, CHANGELOG, local tag `v0.1.1`. Next: owner pushes the tag and creates the release; then the formula's url/sha256 and the tap update.

## 2026-11-04 · Release 0.1.0 prepared
- Owner created github.com/jankkhvej/ttnr (public) and pushed `main`; local backup refs of the old history removed.
- `CHANGELOG.md` (0.1.0, source-only release), README install section with the Homebrew tap (`brew install jankkhvej/ttnr/ttnr`, formula – not a cask, since ttnr is built from source), local annotated tag `v0.1.0`. Next: owner pushes the tag and creates the GitHub release; then the formula's SHA-256 from GitHub's archive and the tap repo `jankkhvej/homebrew-ttnr`.

## 2026-11-04 · Publication prep
- Fresh-clone gate fixed (notices check deterministic). Commit author rewritten to the owner's GitHub noreply address (backup branch `backup/pre-author-rewrite`); repo-local `user.email` set.
- Repository URL `https://github.com/jankkhvej/ttnr` in Cargo.toml, README and the formula draft; `origin` set locally. Creating the public GitHub repo was blocked by Claude Code's permission classifier → the owner creates it and pushes.

## 2026-11-04 · Whole-repo security and code review – fixes
- Security (ADR-047): CLI output of network text (ICY titles, directory names, server messages) passed terminal control sequences through (verified OSC 0 / OSC 52); now neutralised. TUI was safe (ratatui drops control characters).
- Code review fixes: old station's metadata provider stopped at once and its late updates dropped (keyed messages); resolve cache key includes the station URL; empty ICY title clears the track; radio-browser refresh prefers the fresh click URL; teletype banner from the active theme; exec `now` stderr drained; superseded resolution answers ignored (refresh vs reconnect race); future `at` never waits longer than the sync offset, config offsets clamped to 120 s; CoreAudio listener callback never freed (no use-after-free at exit); socket/system-event plumbing shared by TUI and `play`.
- Tests: a regression test per fix (security and exec tests confirmed failing without the fix); teletype banner checked in a pty run.

## 2026-11-04 · M9 complete – roadmap M0–M9 done
- Owner: `i` works. All milestones accepted.
- Open (owner decisions, nothing scheduled): public repository / release archive, project page on hcc07.org (formula homepage), publishing the Homebrew formula (tap + `brew audit`), binary-distribution duties (GPL-3.0 text, source offer, objc2 licence texts); Teletype "everything on paper" proposal (THEMES.md); drop the local libmpv build once Homebrew ships mpv ≥ 0.42.

## 2026-11-04 · M9 · Diagnostics indicators – **M9 complete, waiting for owner review**
- `[diagnostics]` network / download / metadata / sync_note, `i` toggles (none configured = all); engine `Transfer` from `cache-speed` / `demuxer-cache-duration`; `ObservedHttp` records the last request; hidden = no state change (ADR-046).
- Tests: translator rounding, ObservedHttp, core (hidden ignores data, toggle, config subset), each indicator alone, snapshots (only help changed with diagnostics off; new `main_diagnostics` case). Live (muted, local paced stream): `diag: … 16 kB/s buf 4 s · 127.0.0.1:18768 3 s ago ok`.

Older entries: [PROGRESS-archive.md](PROGRESS-archive.md).
