#!/usr/bin/env bash
# GUI smoke test: launch Flare under Xvfb, wait for the main window, screenshot,
# optionally load a sample file, verify no crash, terminate cleanly.
# Usage: gui_smoke.sh [OUT_DIR]      (default OUT_DIR=./smoke_out)
# Env:   BUILD_DIR (default /home/user/bld)   FLARE_BIN (override binary path)
#        FLAREROOT (default <repo>/stuff)      SMOKE_TIMEOUT (secs, default 30)
#        SMOKE_FILE (file passed as the load argument in phase 2; default
#        tests/flash_fixtures/sample.fla; set to "" to skip phase 2)
# Exit: 0 pass, 1 crash/early exit, 2 hang (no window within timeout), 3 setup error,
#       4 ANY modal dialog open except the Startup popup (set SMOKE_ALLOW_DIALOG=1 to skip this check).
set -u
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BUILD_DIR="${BUILD_DIR:-/home/user/bld}"
OUT="${1:-./smoke_out}"; mkdir -p "$OUT"; OUT="$(cd "$OUT" && pwd)"
TIMEOUT="${SMOKE_TIMEOUT:-30}"
export FLAREROOT="${FLAREROOT:-$REPO/stuff}"
SMOKE_FILE="${SMOKE_FILE-$REPO/tests/flash_fixtures/sample.fla}"

BIN="${FLARE_BIN:-}"
if [ -z "$BIN" ]; then
  for c in "$BUILD_DIR/bin/Flare" "$BUILD_DIR/bin/flare" /opt/flare/bin/flare /opt/flare/bin/Flare; do
    [ -x "$c" ] && { BIN="$c"; break; }
  done
fi
[ -n "$BIN" ] && [ -x "$BIN" ] || { echo "gui_smoke: Flare binary not found (build first)"; exit 3; }
for t in Xvfb xdotool xprop; do command -v $t >/dev/null || { echo "gui_smoke: missing $t"; exit 3; }; done
SHOT=""; command -v import >/dev/null && SHOT=import
[ -z "$SHOT" ] && command -v scrot >/dev/null && SHOT=scrot
[ -n "$SHOT" ] || { echo "gui_smoke: need imagemagick (import) or scrot"; exit 3; }

# Isolated HOME so startup.log / crash dir are only from this run.
HOME_DIR="$(mktemp -d)"; export HOME="$HOME_DIR"
export XDG_DATA_HOME="$HOME/.local/share" XDG_CONFIG_HOME="$HOME/.config"
export FLARE_QUIET_IMPORT=1 LIBGL_ALWAYS_SOFTWARE=1 QT_QPA_PLATFORM=xcb
ulimit -c unlimited 2>/dev/null || true
LOG="$OUT/flare_stdout.log"; : >"$LOG"
STARTUP="$XDG_DATA_HOME/Flare/Flare/startup.log"

Xvfb :77 -screen 0 1600x900x24 >"$OUT/xvfb.log" 2>&1 &
XPID=$!
export DISPLAY=:77
APP_PID=""
cleanup() {
  [ -n "$APP_PID" ] && kill -KILL "$APP_PID" 2>/dev/null
  kill "$XPID" 2>/dev/null
  rm -rf "$HOME_DIR"
}
trap cleanup EXIT
for _ in $(seq 1 20); do xdotool getdisplaygeometry >/dev/null 2>&1 && break; sleep 0.25; done

fail() { # fail <code> <msg>
  echo "SMOKE FAIL: $2"
  echo "--- log tail ($LOG) ---"; tail -n 40 "$LOG"
  [ -f "$STARTUP" ] && { cp "$STARTUP" "$OUT/startup.log"; echo "--- startup.log tail ---"; tail -n 20 "$STARTUP"; }
  $SHOT -window root "$OUT/fail.png" 2>/dev/null || true
  exit "$1"
}


# Any open modal/dialog window fails the run (D18). Only the benign "Flare Startup"
# popup is allow-listed (QA_DIALOG_ALLOW overrides). See qa/dialogs.sh.
. "$REPO/scripts/dev/qa/dialogs.sh"
detect_modal_error() { find_open_dialogs "$1" "$2"; }

# run_phase <name> [args...]: launch, wait for window, screenshot, hold, terminate.
run_phase() {
  local name="$1"; shift
  echo "== phase $name: $BIN $*"
  "$BIN" "$@" >>"$LOG" 2>&1 &
  APP_PID=$!
  local deadline=$(( $(date +%s) + TIMEOUT )) win=""
  while [ "$(date +%s)" -lt "$deadline" ]; do
    if ! kill -0 "$APP_PID" 2>/dev/null; then
      wait "$APP_PID"; local rc=$?
      APP_PID=""; fail 1 "Flare exited early during $name (rc=$rc, 139=SEGV 134=ABRT)"
    fi
    # Main window: a large visible window (the splash and tool popups are smaller).
    for w in $(xdotool search --onlyvisible --pid "$APP_PID" 2>/dev/null); do
      local g; g="$(xdotool getwindowgeometry --shell "$w" 2>/dev/null)"
      local W H; W=$(echo "$g" | sed -n 's/^WIDTH=//p'); H=$(echo "$g" | sed -n 's/^HEIGHT=//p')
      if [ "${W:-0}" -ge 800 ] && [ "${H:-0}" -ge 500 ]; then win="$w"; break; fi
    done
    [ -n "$win" ] && break
    sleep 0.5
  done
  [ -n "$win" ] || { kill -KILL "$APP_PID" 2>/dev/null; APP_PID=""; fail 2 "no main window within ${TIMEOUT}s during $name"; }
  echo "main window id=$win title='$(xdotool getwindowname "$win" 2>/dev/null)'"
  sleep 5   # let panels / deferred init paint
  kill -0 "$APP_PID" 2>/dev/null || { wait "$APP_PID"; local rc=$?; APP_PID=""; fail 1 "Flare died after window appeared during $name (rc=$rc)"; }
  $SHOT -window root "$OUT/$name.png" || fail 3 "screenshot failed"
  if [ "${SMOKE_ALLOW_DIALOG:-0}" != 1 ]; then
    local dlg; dlg="$(detect_modal_error "$APP_PID" "$win")" \
      && { kill -KILL "$APP_PID" 2>/dev/null; APP_PID=""; fail 4 "dialog open during $name: $dlg"; }
  fi
  sleep 3
  kill -0 "$APP_PID" 2>/dev/null || { wait "$APP_PID"; local rc=$?; APP_PID=""; fail 1 "Flare died after screenshot during $name (rc=$rc)"; }
  kill -TERM "$APP_PID" 2>/dev/null
  for _ in $(seq 1 20); do kill -0 "$APP_PID" 2>/dev/null || break; sleep 0.5; done
  if kill -0 "$APP_PID" 2>/dev/null; then kill -KILL "$APP_PID"; echo "note: needed SIGKILL after SIGTERM in $name"; fi
  wait "$APP_PID" 2>/dev/null; APP_PID=""
}

run_phase main
if [ -n "$SMOKE_FILE" ] && [ -f "$SMOKE_FILE" ]; then
  cp "$SMOKE_FILE" "$OUT/" 2>/dev/null
  run_phase load_sample "$OUT/$(basename "$SMOKE_FILE")"
fi

if grep -qiE "fatal|segmentation|terminate called|abort" "$LOG" "$STARTUP" 2>/dev/null; then
  grep -iE "fatal|segmentation|terminate called|abort" "$LOG" "$STARTUP" | head -5
  fail 1 "fatal marker in logs"
fi
if ls core* "$HOME"/.local/share/Flare/Flare/crash/* >/dev/null 2>&1; then fail 1 "core/crash file produced"; fi
[ -f "$STARTUP" ] && cp "$STARTUP" "$OUT/startup.log"
echo "SMOKE PASS (screenshots in $OUT)"
exit 0
