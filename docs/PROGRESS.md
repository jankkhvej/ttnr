# Progress log

Newest entry on top. Each entry: date · milestone/task · what changed · how to try it · next step · open questions for the owner.

## 2026-11-06 · Branch protection on `main`
- `main` requires a PR (0 approvals – one account cannot approve its own PRs) whose branch is up to date with `main` and a passing `check` job (so the check ran on top of the newest `main`; merge `origin/main` into a stale branch first); `check` must run on every PR (no path filters); applies to admins too (the agent works with the owner's admin account); force-push and deletion blocked (issue #20). Owner can switch it off under Settings → Branches if ever needed.
- Next: nothing scheduled. Open: the curl backend (#16).

## 2026-11-06 · Release 0.1.3
- Version bump, CHANGELOG 0.1.3: mpv 0.42 support (`curl-enabled=no`, HTTP errors recognised again – #15), mpv ≥ 0.42 recommended, local libmpv build retired; the embedded Info.plist now carries the crate version (was 0.1.0). Release issue #17.
- Try: `brew upgrade mpv && brew upgrade jankkhvej/ttnr/ttnr` (after the tap update), `ttnr --version` → `ttnr 0.1.3`.
- Done: tag `v0.1.3`, GitHub release, formula (#19, with an mpv ≥ 0.42 caveat), tap commit `ttnr 0.1.3`, `brew audit --strict --online` from the tap. Next: branch protection on `main`.

## 2026-11-06 · CI on GitHub Actions
- `.github/workflows/ci.yml` (ADR-050, issue #12): `macos-26`, Homebrew mpv (after `brew update`), the unchanged gate; free for this public repo. First run green in ~4 min.
- Try: open any PR – the `CI / check` status appears. Next: branch protection requiring it; release 0.1.3.

## 2026-11-06 · Homebrew mpv 0.42: local libmpv build retired
- Homebrew's mpv 0.42.0 has the CoreAudio fix (ADR-049); its new curl backend broke HTTP error detection (2 CLI tests failed, 1 hung) → `curl-enabled=no`. `scripts/build-libmpv.zsh`, the build-script preference and `third_party/mpv/` removed (issue #14).
- Try: `brew upgrade mpv` (≥ 0.42), `cargo build --release`, play BBC 6 Music and a 5.1 file – log `audio output ao="coreaudio"`; volume/mute react at once.
- Next: release 0.1.3 (0.1.2 from the tap builds against mpv 0.42 without `curl-enabled=no`), then CI (#13). Open: the curl backend as a follow-up.

Older entries: [PROGRESS-archive.md](PROGRESS-archive.md).
