#!/usr/bin/env bash
# Start Flare under Xvfb :77 with an isolated stuff/ copy and a chosen theme, then leave it
# running for xdotool/import driving. Usage: gui_theme_session.sh <Theme> <Layout> [extra ini lines...]
# Env: FLARE_BIN, SESS (session dir, default /tmp/lg-sess), QT_SCALE_FACTOR passthrough.
# Stop with: scripts/dev/gui_theme_session.sh stop
set -u
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SESS="${SESS:-/tmp/lg-sess}"
if [ "${1:-}" = stop ]; then kill "$(cat "$SESS/app.pid")" "$(cat "$SESS/xvfb.pid")" 2>/dev/null; exit 0; fi
THEME="${1:?theme}"; LAYOUT="${2:-Animate}"; shift 2
BIN="${FLARE_BIN:-${BUILD_DIR:-/home/user/bld}/bin/Flare}"
rm -rf "$SESS"; mkdir -p "$SESS/home"
export HOME="$SESS/home" XDG_DATA_HOME="$SESS/home/.local/share" XDG_CONFIG_HOME="$SESS/home/.config"
export USER=flare LIBGL_ALWAYS_SOFTWARE=1 QT_QPA_PLATFORM=xcb DISPLAY=:77
cp -r "$REPO/stuff" "$SESS/stuff"; export FLAREROOT="$SESS/stuff"
S="$FLAREROOT/profiles/layouts/settings.flare"; mkdir -p "$S"
{ printf '[General]\nCurrentRoomChoice=%s\nCurrentStyleSheetName=%s\n' "$LAYOUT" "$THEME"; for l in "$@"; do echo "$l"; done; } > "$S/preferences.ini"
# FONT_PX (default 12 = 100%; 15 = 125%) is the app font pixel size from env.ini.
if [ -n "${FONT_PX:-}" ]; then mkdir -p "$FLAREROOT/profiles/users/$USER"; echo "SoftwareCurrentFontSize \"$FONT_PX\"" > "$FLAREROOT/profiles/users/$USER/env.ini"; fi
Xvfb :77 -screen 0 1600x900x24 >/dev/null 2>&1 & echo $! > "$SESS/xvfb.pid"
sleep 1
"$BIN" >"$SESS/app.log" 2>&1 & echo $! > "$SESS/app.pid"
echo "started; shots with: DISPLAY=:77 import -window root out.png"
