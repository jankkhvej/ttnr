#!/usr/bin/env zsh
# ttnr — start/continue autonomous development with Claude Code.
#
#   ./start.zsh            continue the project (new session, reads progress from docs/)
#   ./start.zsh --resume   reopen the most recent Claude Code conversation here
#   ./start.zsh --auto     run in Claude Code "auto" permission mode (fewer prompts)
#   ./start.zsh --plan     start in plan mode (Claude proposes, doesn't edit)
#   ./start.zsh --check    only check prerequisites, don't start Claude
#
emulate -L zsh
setopt err_exit pipe_fail no_unset
cd "${0:A:h}"

mode=acceptEdits; resume=0; check_only=0
for a in "$@"; do
  case $a in
    --auto)   mode=auto ;;
    --plan)   mode=plan ;;
    --resume|-c) resume=1 ;;
    --check)  check_only=1 ;;
    -h|--help) sed -n '2,8p' "$0"; exit 0 ;;
    *) print -u2 "unknown option: $a"; exit 2 ;;
  esac
done

say()  { print -P "%F{cyan}▸%f $*"; }
warn() { print -P "%F{yellow}!%f $*"; }
die()  { print -P "%F{red}✗%f $*"; exit 1; }
ask()  { local r; read -q "r?$1 [y/N] " || { print; return 1; }; print; }

# --- prerequisites ---------------------------------------------------------
[[ $(uname -s) == Darwin ]] || die "macOS only."
command -v brew   >/dev/null || die "Homebrew not found (https://brew.sh)."
command -v claude >/dev/null || die "Claude Code CLI not found (https://code.claude.com/docs)."
command -v git    >/dev/null || die "git not found."
if ! command -v cargo >/dev/null; then
  die "Rust not found. Install the official toolchain from https://rustup.rs, then re-run."
fi

typeset -a missing
for f in mpv pkgconf ffmpeg; do
  brew list --formula $f >/dev/null 2>&1 || missing+=$f
done
if (( ${#missing} )); then
  warn "Missing Homebrew packages: ${missing[*]}"
  warn "(mpv provides libmpv for playback; ffmpeg generates test audio; pkgconf finds libmpv)"
  if ask "Install them now with brew?"; then brew install ${missing[@]}
  else die "Cannot continue without: ${missing[*]}"; fi
fi
pkg-config --exists mpv || die "pkg-config cannot find libmpv (mpv.pc). Try: brew reinstall mpv"

say "macOS $(sw_vers -productVersion) · $(cargo --version) · libmpv $(pkg-config --modversion mpv) · claude $(claude --version 2>/dev/null | head -1)"

# --- git -------------------------------------------------------------------
if [[ ! -d .git ]]; then
  git init -q && git add -A && git commit -qm "chore: project scaffold (spec, roadmap, workflow)"
  say "Initialised git repository."
fi

if (( check_only )); then say "Prerequisites OK."; exit 0; fi

# --- launch Claude Code ----------------------------------------------------
kickoff='Follow CLAUDE.md. Read docs/PROGRESS.md and docs/ROADMAP.md, state in two lines where the project stands and which task you take next, then continue with the session workflow until the current milestone is complete or you need the owner.'

if (( resume )); then
  exec claude --continue --permission-mode $mode
else
  exec claude -n ttnr --permission-mode $mode "$kickoff"
fi
