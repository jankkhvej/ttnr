# Progress log

Newest entry on top. Each entry: date · milestone/task · what changed · how to try it · next step · open questions for the owner.

## 2026-11-06 · CI on GitHub Actions
- `.github/workflows/ci.yml` (ADR-050, issue #12): `macos-26`, Homebrew mpv (after `brew update`), the unchanged gate; free for this public repo. First run green in ~4 min.
- Try: open any PR – the `CI / check` status appears. Next: branch protection requiring it; release 0.1.3.

## 2026-11-06 · Homebrew mpv 0.42: local libmpv build retired
- Homebrew's mpv 0.42.0 has the CoreAudio fix (ADR-049); its new curl backend broke HTTP error detection (2 CLI tests failed, 1 hung) → `curl-enabled=no`. `scripts/build-libmpv.zsh`, the build-script preference and `third_party/mpv/` removed (issue #14).
- Try: `brew upgrade mpv` (≥ 0.42), `cargo build --release`, play BBC 6 Music and a 5.1 file – log `audio output ao="coreaudio"`; volume/mute react at once.
- Next: release 0.1.3 (0.1.2 from the tap builds against mpv 0.42 without `curl-enabled=no`), then CI (#13). Open: the curl backend as a follow-up.

## 2026-11-06 · Release 0.1.2
- Version bump, CHANGELOG 0.1.2 (live pause from the live edge and small mpv cache – #3; `ttnr play` Ctrl-C/SIGTERM – #8), man page (pausing live radio). Tracked in the release issue #9.
- Try: `brew upgrade jankkhvej/ttnr/ttnr` (after the tap update) or `cargo install --path . --locked`, then `ttnr --version` → `ttnr 0.1.2`; pause a live station for a minute and resume.
- Done: tag `v0.1.2` (on the merge of #10), GitHub release, formula `url`/`sha256` (#11), tap `jankkhvej/homebrew-ttnr` commit `ttnr 0.1.2`; `brew audit --strict --online` from the tap passed (no install). Open: CI (owner: only if free), branch protection, release automation.

## 2026-11-06 · Flaky test fixed, Ctrl-C fix, PR workflow
- Workflow (owner): every change via a GitHub issue + PR, `/code-review`, merge (CLAUDE.md step 6). PRs #3 (live pause, issue #1), #4 (keep-alive race in the HTTP test server, #2), #6 (workflow docs, #5).
- Issue #7: `ttnr play` ignored Ctrl-C about 1 in 10 runs – a new `tokio::signal::ctrl_c()` per `select!` loses a signal delivered while another branch wins; now signal streams created before setup, as in the TUI (0/60 hangs under load, was 4/40); SIGTERM and Ctrl-C during start-up stop cleanly; every exit path stops metadata and tells control-socket clients; CLI tests wait with a deadline.
- Known, not changed: the `pls_with_icy_titles` test server is single-threaded with a blocking read, so a reconnect can wait on it; harmless now that Ctrl-C always works.

## 2026-11-06 · Live radio: pause stops, small cache (memory check)
- Owner asked for a memory check of a long-running release build: no leak (`leaks`: 80 bytes), but 260 MB held by mpv's demuxer cache, and after a ~25 min pause playback ran ~25 min behind live (`buf 1489 s`).
- Fix (ADR-048): pausing a live stream stops it, resuming starts at the live edge; mpv cache `demuxer-seekable-cache=no`, `cache-secs=10`, `demuxer-max-bytes=8MiB`. Measured 33–34 MB playing and paused on BBC 6 Music.
- Tests: core pause/resume (live, file, unknown stream, pending resolution, pause while resuming, first-buffering pause, failed command; guard mutations fail); ICY test caught that `demuxer-max-back-bytes=0` loses ICY titles. Review of PR #3: metadata provider stopped too, `[resumed]` in `ttnr play`.
- Try: `cargo install --path .` (or the release build), play BBC 6 Music, press `i`: `buf` stays around 10–20 s; pause for a minute, resume – short buffering, then live. Owner: no grace period for short pauses. Known: live HLS with a DVR window that mpv reports as seekable still pauses in mpv (ADR-048).

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
