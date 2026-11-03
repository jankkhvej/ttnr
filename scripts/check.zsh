#!/usr/bin/env zsh
# Quality gate: must pass before every commit.
emulate -L zsh; setopt err_exit pipe_fail
cd "${0:A:h}/.."
if [[ ! -f Cargo.toml ]]; then print "check: not bootstrapped yet (M0) — nothing to check."; exit 0; fi
print "▸ fmt";    cargo fmt --all --check
print "▸ clippy"; cargo clippy --workspace --all-targets -- -D warnings
print "▸ test";   ./scripts/gen-fixtures.zsh >/dev/null && cargo test --workspace
if command -v cargo-deny >/dev/null; then
  print "▸ deny";   cargo deny check
else
  print "✗ cargo-deny missing: cargo install --locked cargo-deny"; exit 1
fi
# Third-party notices must match the dependency graph (needs Homebrew for
# the native-library part; ADR-045).
if command -v brew >/dev/null; then
  print "▸ notices"; python3 -I scripts/notices.py --check
else
  print "▸ notices skipped (no Homebrew)"
fi
print "✓ all checks passed"
