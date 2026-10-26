# Shared by gui_smoke.sh and qa/run_all.sh. Source it; needs xdotool, xprop.
# find_open_dialogs <pid> <main_win> : print one line per open dialog-like window of the
# app (id, title, why) and return 0 if any, 1 if none.
# A window counts as a dialog when it is not the main window, is visible, and
#  - _NET_WM_STATE contains _NET_WM_STATE_MODAL, or
#  - _NET_WM_WINDOW_TYPE contains _NET_WM_WINDOW_TYPE_DIALOG, or
#  - it is a transient-for window (WM_TRANSIENT_FOR set), or
#  - it is a plain top-level at least 120x60 (not menu/tooltip/splash/combo/dnd/notification).
# QA_DIALOG_ALLOW (egrep on title, default '^Flare Startup$') lists benign windows to ignore.
find_open_dialogs() {
  local pid="$1" main="$2" w name why props state type found=1 g W H
  local allow="${QA_DIALOG_ALLOW-^Flare Startup\$}"
  for w in $(xdotool search --onlyvisible --pid "$pid" 2>/dev/null); do
    [ "$w" = "$main" ] && continue
    name="$(xdotool getwindowname "$w" 2>/dev/null)"
    [ -n "$allow" ] && echo "$name" | grep -qE "$allow" && continue
    props="$(xprop -id "$w" _NET_WM_STATE _NET_WM_WINDOW_TYPE WM_TRANSIENT_FOR 2>/dev/null)"
    state="$(echo "$props" | grep '^_NET_WM_STATE')"
    type="$(echo "$props" | grep '^_NET_WM_WINDOW_TYPE')"
    why=""
    echo "$state" | grep -q MODAL && why="modal"
    [ -z "$why" ] && echo "$type" | grep -q DIALOG && why="type=DIALOG"
    [ -z "$why" ] && echo "$props" | grep -q '^WM_TRANSIENT_FOR.*window id' && why="transient"
    if [ -z "$why" ] && ! echo "$type" | grep -qE 'MENU|TOOLTIP|SPLASH|COMBO|DND|NOTIFICATION|DROPDOWN'; then
      g="$(xdotool getwindowgeometry --shell "$w" 2>/dev/null)"
      W=$(echo "$g" | sed -n 's/^WIDTH=//p'); H=$(echo "$g" | sed -n 's/^HEIGHT=//p')
      [ "${W:-0}" -ge 120 ] && [ "${H:-0}" -ge 60 ] && why="toplevel ${W}x${H}"
    fi
    [ -n "$why" ] && { echo "window $w title='$name' ($why)"; found=0; }
  done
  return $found
}
