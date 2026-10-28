#!/bin/sh
# Restart the app under Xvfb :99 for UI smoke tests. Native dialogs are off
# (typed-path dialog instead), autosave is fast, settings go to out/cfg.
pkill -x flare; sleep 0.5
cd "$(dirname "$0")/.." && mkdir -p out/cfg && rm -f out/cfg/recovery* && (DISPLAY=:99 AC_NO_NATIVE_DIALOGS=1 AC_NO_STARTUP=${AC_NO_STARTUP-1} AC_AUTOSAVE_SECS=${AC_AUTOSAVE_SECS:-3} AC_CONFIG_DIR=$PWD/out/cfg nohup target/release/flare "$@" > out/app.log 2>&1 &)
sleep 3
