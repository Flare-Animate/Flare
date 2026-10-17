#!/usr/bin/env bash
# sync-animatecraft.sh - watch the upstream AnimateCraft repo for new commits and
# write, per commit, a note (.md), a patch (.patch) and an INDEX.md line into
# OUT_DIR so the changes can be ported into Flare by hand.
#
# Never runs git commit/push on Flare and never writes outside OUT_DIR, STATE_FILE
# and CLONE_DIR. Patches use upstream paths; apply them via the mapping table.
set -euo pipefail

# ---------------------------------------------------------------------------
# Configuration (environment overrides)
# ---------------------------------------------------------------------------
UPSTREAM_URL=${UPSTREAM_URL:-https://github.com/Flare-Animate/RustConversion-AnimateCraft}
BRANCH=${BRANCH:-main}
CLONE_DIR=${CLONE_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}/flare/animatecraft-upstream}
STATE_FILE=${STATE_FILE:-$CLONE_DIR/../animatecraft-last-synced-sha}
INTERVAL=${INTERVAL:-900}
OUT_DIR=${OUT_DIR:-}

export GIT_LFS_SKIP_SMUDGE=1

STOP=0
SLEEP_PID=""
MODE=loop
LAST_MAPPED=0
LAST_UNMAPPED=0
REPO_ROOT=""

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------
usage() {
  cat <<'EOF'
Usage: scripts/sync-animatecraft.sh [--once | --loop] [--help]

  --once   Run a single sync pass and exit (non-zero on failure).
  --loop   Run passes forever, sleeping INTERVAL seconds between them (default).
           SIGINT/SIGTERM stop the loop after the current pass.
  --help   Show this help.

Environment (defaults in parentheses):
  UPSTREAM_URL  (https://github.com/Flare-Animate/RustConversion-AnimateCraft)
  BRANCH        (main)
  CLONE_DIR     (${XDG_CACHE_HOME:-$HOME/.cache}/flare/animatecraft-upstream)
  STATE_FILE    ($CLONE_DIR/../animatecraft-last-synced-sha)
  INTERVAL      (900, seconds, loop mode only)
  OUT_DIR       ($(git rev-parse --show-toplevel)/docs/upstream-sync)

First run records the current upstream tip as the baseline and lists the last
20 commits without generating per-commit files.
EOF
}

die() {
  printf 'sync-animatecraft: %s\n' "$*" >&2
  exit 1
}

warn() {
  printf 'sync-animatecraft: %s\n' "$*" >&2
}

# Retry a command up to 4 times with 2/4/8/16s backoff.
retry() {
  local -a delays=(2 4 8 16)
  local i
  for i in 0 1 2 3 4; do
    if "$@"; then
      return 0
    fi
    if (( i < 4 )); then
      warn "network step failed (attempt $((i + 1))/5), retrying in ${delays[i]}s"
      sleep "${delays[i]}"
    fi
  done
  return 1
}

flat() {
  printf '%s' "${1//\//_}"
}

# ---------------------------------------------------------------------------
# Upstream clone
# ---------------------------------------------------------------------------
clone_once() {
  mkdir -p -- "$(dirname -- "$CLONE_DIR")"
  git clone --quiet --filter=blob:none --no-checkout --single-branch \
    --branch "$BRANCH" -- "$UPSTREAM_URL" "$CLONE_DIR"
}

fetch_upstream() {
  retry git -C "$CLONE_DIR" fetch --quiet --filter=blob:none origin \
    "+refs/heads/$BRANCH:refs/remotes/origin/$BRANCH"
}

ensure_clone() {
  if [[ -e $CLONE_DIR/.git ]]; then
    fetch_upstream || { warn "fetch of origin/$BRANCH failed after retries"; return 1; }
    return 0
  fi
  if [[ -d $CLONE_DIR ]]; then
    if [[ -n $(ls -A -- "$CLONE_DIR") ]]; then
      warn "$CLONE_DIR exists, is not empty, and is not a git clone; refusing to touch it"
      return 1
    fi
    rmdir -- "$CLONE_DIR"
  elif [[ -e $CLONE_DIR ]]; then
    warn "$CLONE_DIR exists and is not a directory"
    return 1
  fi
  if ! retry clone_once; then
    warn "clone of $UPSTREAM_URL failed after retries"
    return 1
  fi
}

# ---------------------------------------------------------------------------
# Path mapping (upstream -> Flare)
# ---------------------------------------------------------------------------
# Looks up a basename in the #[path = "..."] list of flarelib/ac_core.rs and
# prints the resolved repo-relative Flare path. Returns 1 when not found.
ac_core_map() {
  local base=$1 p rel
  local lib="$REPO_ROOT/flare/sources/flarelib/ac_core.rs"
  [[ -n $REPO_ROOT && -f $lib ]] || return 1
  while IFS= read -r p; do
    if [[ $(basename -- "$p") == "$base" ]]; then
      rel=$(cd "$REPO_ROOT/flare/sources/flarelib" && realpath -m --relative-to="$REPO_ROOT" -- "$p")
      printf '%s\n' "$rel"
      return 0
    fi
  done < <(sed -n 's/^#\[path = "\([^"]*\)"\].*/\1/p' "$lib")
  return 1
}

map_path() {
  local p=$1 rest base
  case $p in
    crates/ac-core/src/lib.rs)
      printf '%s\n' flare/sources/flarelib/ac_core.rs ;;
    crates/ac-core/src/*)
      rest=${p#crates/ac-core/src/}
      base=$(basename -- "$rest")
      if ! ac_core_map "$base"; then
        printf '%s\n' "flare/sources/flarelib/$rest"
      fi ;;
    crates/ac-render/src/*)
      printf '%s\n' "flare/sources/common/tvrender/ac_$(flat "${p#crates/ac-render/src/}")" ;;
    crates/ac-xfl/src/lib.rs)
      printf '%s\n' common/flash/ac_xfl.rs ;;
    crates/ac-xfl/src/*)
      printf '%s\n' "common/flash/ac_xfl_$(flat "${p#crates/ac-xfl/src/}")" ;;
    crates/ac-audio/src/lib.rs)
      printf '%s\n' sound/ac_audio.rs ;;
    crates/ac-audio/src/*)
      printf '%s\n' "sound/ac_audio_$(flat "${p#crates/ac-audio/src/}")" ;;
    crates/ac-script/src/lib.rs)
      printf '%s\n' common/expressions/ac_script.rs ;;
    crates/ac-script/src/*)
      printf '%s\n' "common/expressions/ac_script_$(flat "${p#crates/ac-script/src/}")" ;;
    crates/ac-package/src/lib.rs)
      printf '%s\n' common/tsystem/ac_package.rs ;;
    crates/ac-package/src/*)
      printf '%s\n' "common/tsystem/ac_package_$(flat "${p#crates/ac-package/src/}")" ;;
    crates/ac-app/src/main.rs)
      printf '%s\n' flare/ac_app_main.rs ;;
    crates/ac-app/src/*)
      printf '%s\n' "flare/ac_app/${p#crates/ac-app/src/}" ;;
    crates/ac-cli/src/main.rs)
      printf '%s\n' flareqt/ac_cli_main.rs ;;
    docs/*)
      printf '%s\n' "docs/${p#docs/}" ;;
    *)
      printf '%s\n' UNMAPPED ;;
  esac
}

# ---------------------------------------------------------------------------
# Per-commit output
# ---------------------------------------------------------------------------
set_state() {
  mkdir -p -- "$(dirname -- "$STATE_FILE")"
  printf '%s\n' "$1" > "$STATE_FILE.tmp"
  mv -f -- "$STATE_FILE.tmp" "$STATE_FILE"
}

# Writes OUT_DIR/<short>.md, <short>.patch and the INDEX.md line. Sets
# LAST_MAPPED / LAST_UNMAPPED. Safe to re-run for the same commit.
write_commit() {
  local sha=$1 short subject author date link files f target rows=""
  local mapped=0 unmapped=0 index="$OUT_DIR/INDEX.md"

  short=$(git -C "$CLONE_DIR" rev-parse --short=12 "$sha") || return 1
  subject=$(git -C "$CLONE_DIR" log -1 --format=%s "$sha") || return 1
  author=$(git -C "$CLONE_DIR" log -1 --format=%an "$sha") || return 1
  date=$(git -C "$CLONE_DIR" log -1 --format=%aI "$sha") || return 1
  link="${UPSTREAM_URL%.git}/commit/$sha"
  files=$(git -C "$CLONE_DIR" diff-tree --root --no-commit-id --name-only -r -M "$sha") || return 1

  while IFS= read -r f; do
    [[ -z $f ]] && continue
    target=$(map_path "$f")
    if [[ $target == UNMAPPED ]]; then
      unmapped=$((unmapped + 1))
    else
      mapped=$((mapped + 1))
    fi
    rows+="| \`$f\` | $target |"$'\n'
  done <<< "$files"

  mkdir -p -- "$OUT_DIR"

  {
    printf '# %s %s\n\n' "$short" "$subject"
    printf -- '- Subject: %s\n' "$subject"
    printf -- '- Author: %s\n' "$author"
    printf -- '- Date: %s\n' "$date"
    printf -- '- Link: %s\n' "$link"
    printf -- '- Mapped files: %d\n' "$mapped"
    printf -- '- UNMAPPED files: %d\n\n' "$unmapped"
    printf '## git show --stat\n\n```\n'
    git -C "$CLONE_DIR" show --stat --format= "$sha" || return 1
    printf '```\n\n'
    printf '## Changed files (upstream -> Flare)\n\n'
    printf '| Upstream | Flare |\n|---|---|\n'
    printf '%s' "$rows"
  } > "$OUT_DIR/$short.md.tmp" || return 1
  mv -f -- "$OUT_DIR/$short.md.tmp" "$OUT_DIR/$short.md" || return 1

  git -C "$CLONE_DIR" format-patch --stdout -1 "$sha" > "$OUT_DIR/$short.patch.tmp" || return 1
  mv -f -- "$OUT_DIR/$short.patch.tmp" "$OUT_DIR/$short.patch" || return 1

  if [[ ! -f $index ]]; then
    printf '# AnimateCraft upstream sync index\n\nUpstream paths; see each .md for the Flare mapping.\n\n' > "$index" || return 1
  fi
  if ! grep -q -- "^- $short " "$index"; then
    printf -- '- %s %s %s (mapped=%d, unmapped=%d)\n' \
      "$short" "${date%%T*}" "$subject" "$mapped" "$unmapped" >> "$index" || return 1
  fi

  LAST_MAPPED=$mapped
  LAST_UNMAPPED=$unmapped
}

# ---------------------------------------------------------------------------
# One sync pass
# ---------------------------------------------------------------------------
run_pass() {
  local remote_sha last sha count=0 total_mapped=0 total_unmapped=0
  local -a new_commits=()

  ensure_clone || return 1

  if ! remote_sha=$(git -C "$CLONE_DIR" rev-parse --verify -q "refs/remotes/origin/$BRANCH^{commit}"); then
    warn "origin/$BRANCH not found in clone"
    return 1
  fi

  if [[ ! -s $STATE_FILE ]]; then
    set_state "$remote_sha" || return 1
    echo "baseline recorded: $BRANCH @ ${remote_sha:0:12}; last 20 upstream commits (not generated):"
    git -C "$CLONE_DIR" log --format='  %h %ad %s' --date=short -20 "$remote_sha" || return 1
    return 0
  fi

  last=$(tr -d '[:space:]' < "$STATE_FILE")
  if ! git -C "$CLONE_DIR" cat-file -e "$last^{commit}" 2>/dev/null; then
    warn "last synced sha $last is not in upstream history; not guessing. Delete $STATE_FILE to rebaseline."
    return 1
  fi
  if [[ $last == "$remote_sha" ]]; then
    echo "."
    return 0
  fi
  if ! git -C "$CLONE_DIR" merge-base --is-ancestor "$last" "$remote_sha"; then
    warn "upstream history was rewritten (last synced $last is not an ancestor of $remote_sha); not guessing. Delete $STATE_FILE to rebaseline."
    return 1
  fi

  mapfile -t new_commits < <(git -C "$CLONE_DIR" rev-list --reverse "$last..$remote_sha") || return 1

  for sha in "${new_commits[@]}"; do
    if ! write_commit "$sha"; then
      warn "failed writing files for $sha; STATE_FILE not advanced past it"
      return 1
    fi
    set_state "$sha" || return 1
    count=$((count + 1))
    total_mapped=$((total_mapped + LAST_MAPPED))
    total_unmapped=$((total_unmapped + LAST_UNMAPPED))
    echo "  $(git -C "$CLONE_DIR" rev-parse --short=12 "$sha") $(git -C "$CLONE_DIR" log -1 --format=%s "$sha")"
  done

  echo "animatecraft sync: $count new commit(s), mapped files $total_mapped, UNMAPPED files $total_unmapped -> $OUT_DIR"
}

run_loop() {
  while (( STOP == 0 )); do
    run_pass || warn "pass failed; retrying after ${INTERVAL}s"
    (( STOP == 0 )) || break
    sleep "$INTERVAL" &
    SLEEP_PID=$!
    wait "$SLEEP_PID" || true
    SLEEP_PID=""
  done
  echo "animatecraft sync: stopped"
}

on_signal() {
  STOP=1
  if [[ -n $SLEEP_PID ]]; then
    kill "$SLEEP_PID" 2>/dev/null || true
  fi
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------
for arg in "$@"; do
  case $arg in
    --once) MODE=once ;;
    --loop) MODE=loop ;;
    -h|--help) usage; exit 0 ;;
    *) die "unknown argument: $arg (try --help)" ;;
  esac
done

command -v git >/dev/null 2>&1 || die "git not found in PATH"
[[ $INTERVAL =~ ^[0-9]+$ ]] && (( INTERVAL > 0 )) || die "INTERVAL must be a positive integer (seconds)"

REPO_ROOT=$(git rev-parse --show-toplevel 2>/dev/null || true)
if [[ -z $OUT_DIR ]]; then
  [[ -n $REPO_ROOT ]] || die "not inside the Flare git repo; set OUT_DIR explicitly"
  OUT_DIR="$REPO_ROOT/docs/upstream-sync"
fi

if [[ $MODE == once ]]; then
  run_pass
else
  trap on_signal INT TERM
  run_loop
fi
