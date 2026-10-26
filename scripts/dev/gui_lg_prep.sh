#!/usr/bin/env bash
# Start a themed session, dismiss the startup popup, maximise main window. Usage: gui_lg_prep.sh Theme [Layout]
cd "$(dirname "$0")/../.." || exit 1
scripts/dev/gui_theme_session.sh stop 2>/dev/null; sleep 1
scripts/dev/gui_theme_session.sh "$1" "${2:-Animate}" "${@:3}" >/dev/null
export DISPLAY=:77; sleep 25; xdotool key Escape; sleep 1
W=$(xdotool search --onlyvisible --pid "$(cat /tmp/lg-sess/app.pid)" | while read w; do echo "$(xdotool getwindowgeometry --shell $w | sed -n 's/WIDTH=//p') $w"; done | sort -n | tail -1 | cut -d' ' -f2)
echo "$W" > /tmp/lg-sess/win
xdotool windowmove "$W" 0 0 windowsize "$W" 1600 900; sleep 4
