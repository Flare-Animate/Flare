#!/usr/bin/env bash
# Automated GUI QA for Flare under Xvfb with an isolated HOME. Haiku-friendly: just run it.
#   scripts/dev/qa/run_all.sh [--tools|--import|--export|--prefs|--rooms|--all] [--dir DIR] [--out OUT]
#   (no section flag = --all). DIR: extra directory whose importable files are also imported.
# Needs a built Flare (BUILD_DIR, default /home/user/bld, or FLARE_BIN), Xvfb, xdotool, xprop,
# ImageMagick `import`, python3 + PIL, node + playwright (/opt/node-tools) and chromium.
# Output: OUT/results.json, OUT/results.md, OUT/shots/*.png, OUT/logs/*.log
# Exit: 0 all PASS/UNTESTED, 1 any FAIL, 3 setup error.
set -u
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SECS=(); DIR=""; OUT="./qa_out"
while [ $# -gt 0 ]; do
  case "$1" in
    --tools) SECS+=(tools);; --import) SECS+=(import);; --export) SECS+=(export);;
    --prefs) SECS+=(prefs);; --rooms) SECS+=(rooms);; --all) SECS=(tools import export prefs rooms);;
    --dir) DIR="$2"; shift;; --out) OUT="$2"; shift;;
    -h|--help) sed -n 2,9p "$0"; exit 0;;
    *) [ -d "$1" ] && DIR="$1" || { echo "unknown arg $1"; exit 3; };;
  esac; shift
done
[ ${#SECS[@]} -eq 0 ] && SECS=(tools import export prefs rooms)
BIN="${FLARE_BIN:-${BUILD_DIR:-/home/user/bld}/bin/Flare}"
[ -x "$BIN" ] || { echo "run_all: Flare binary not found: $BIN (build first, set BUILD_DIR/FLARE_BIN)"; exit 3; }
for t in Xvfb xdotool xprop import python3 node; do command -v $t >/dev/null || { echo "run_all: missing $t"; exit 3; }; done
python3 -c "import PIL" 2>/dev/null || { echo "run_all: python3 PIL missing"; exit 3; }
mkdir -p "$OUT"; OUT="$(cd "$OUT" && pwd)"
export FLARE_BIN="$BIN"
# pick a free display number
D=88; while [ -e "/tmp/.X11-unix/X$D" ]; do D=$((D+1)); done
ARGS=("${SECS[@]}" --out "$OUT" --display ":$D")
[ -n "$DIR" ] && ARGS+=(--dir "$DIR")
python3 "$HERE/qa_driver.py" "${ARGS[@]}"
rc=$?
echo; cat "$OUT/results.md" 2>/dev/null | head -150
echo "results: $OUT/results.json  $OUT/results.md"
exit $rc
