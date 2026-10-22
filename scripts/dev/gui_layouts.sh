#!/usr/bin/env bash
# Screenshot every room of a layout profile under Xvfb with an isolated copy of stuff/.
# Usage: gui_layouts.sh <Profile> <OUT_DIR> [nrooms-to-click]
# Needs: Xvfb xdotool imagemagick, built binary (BUILD_DIR, default /home/user/bld).
# The repo's stuff/ is copied to a temp dir so the app cannot litter the checkout.
set -u
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BIN="${FLARE_BIN:-${BUILD_DIR:-/home/user/bld}/bin/Flare}"
PROFILE="${1:?profile}"; OUT="${2:?outdir}"; mkdir -p "$OUT"
TMP="$(mktemp -d)"; export HOME="$TMP/home"; mkdir -p "$HOME"
export XDG_DATA_HOME="$HOME/.local/share" XDG_CONFIG_HOME="$HOME/.config"
export USER=flare LIBGL_ALWAYS_SOFTWARE=1 QT_QPA_PLATFORM=xcb DISPLAY=:77
cp -r "$REPO/stuff" "$TMP/stuff"; export FLAREROOT="$TMP/stuff"
S="$FLAREROOT/profiles/layouts/settings.flare"; mkdir -p "$S"
printf '[General]\nCurrentRoomChoice=%s\n' "$PROFILE" > "$S/preferences.ini"
Xvfb :77 -screen 0 1600x900x24 >/dev/null 2>&1 & XPID=$!
trap 'kill $APP $XPID 2>/dev/null; rm -rf "$TMP"' EXIT
sleep 1
"$BIN" >"$OUT/$PROFILE.log" 2>&1 & APP=$!
sleep 25
# main window = biggest visible window of the app
WIN=""; best=0
for w in $(xdotool search --onlyvisible --pid "$APP" 2>/dev/null); do
  g="$(xdotool getwindowgeometry --shell "$w")"
  a=$(( $(echo "$g" | sed -n 's/^WIDTH=//p') * $(echo "$g" | sed -n 's/^HEIGHT=//p') ))
  [ "$a" -gt "$best" ] && { best=$a; WIN=$w; }
done
shot() { import -window root "$OUT/$PROFILE-$1.png"; }
shot 0-startpopup
xdotool key Escape; sleep 1
xdotool windowmove "$WIN" 0 0 windowsize "$WIN" 1600 900; sleep 4
shot 1-main
# TABS="Name:x Name:x ..." room-tab centres on the tab bar (y=10); clicked in order.
for t in ${TABS:-}; do
  xdotool mousemove "${t#*:}" 10 click 1; sleep 4; shot "room-${t%%:*}"
done
# MENUS="Name:x ..." menubar entries: open, screenshot, close.
for t in ${MENUS:-}; do
  xdotool mousemove "${t#*:}" 10 click 1; sleep 1.5; shot "menu-${t%%:*}"; xdotool key Escape; sleep 0.5
done
echo "win=$WIN tmp=$TMP"
sleep "${KEEP:-0}"
