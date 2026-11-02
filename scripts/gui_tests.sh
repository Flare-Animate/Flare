#!/usr/bin/env bash
# A scripted run of the real desktop app, for what only real windows can show: Test Movie opens in
# a window of its own, that window closes, and the editor keeps drawing until it quits. Needs a
# display and a GPU, so CI does not run it; run it before changing the windows or the player.
# The app (built with the `autotest` feature, which no release has) does the steps on timers.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build -p flare-desktop --features autotest
python3 - <<'PY'
import subprocess, sys

try:
    run = subprocess.run(
        ["target/debug/flare-desktop"], capture_output=True, text=True, timeout=90
    )
except subprocess.TimeoutExpired:
    sys.exit("the app did not quit within 90 seconds: it hung")
output = run.stdout + run.stderr
if run.returncode != 0 or "panicked" in output:
    sys.exit(f"the scripted run failed (exit {run.returncode}):\n{output}")
print("the scripted run opened Test Movie, closed it, and quit cleanly")
PY
