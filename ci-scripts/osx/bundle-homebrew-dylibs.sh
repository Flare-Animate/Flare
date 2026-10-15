#!/usr/bin/env bash
# Bundle every non-system dylib that Flare.app links (FFmpeg, OpenCV, ... from
# Homebrew) into Contents/Frameworks and rewrite the references to @rpath.
#
# Issue #98: a released bundle shipped libavdevice.63.dylib that still pointed at
# a versioned Homebrew Cellar path (ffmpeg 9.0.1, libavfilter.12), which only
# existed on the build machine, so dyld aborted at launch on every other Mac.
#
# Nothing here hard-codes a Homebrew version or prefix. Libraries are located by
# (in order): the absolute install name, the target's own LC_RPATH entries,
# `brew --prefix <formula>` for the formula named in a Cellar path (so a version
# bump 9.0.1 -> 9.0.2 still resolves), then `brew --prefix`/lib, opt/*/lib and
# the Cellar. The finished bundle is verified: any dependency that is not a
# system library or @rpath/@loader_path/@executable_path reference fails the
# script (and so the CI job) instead of producing an app that cannot start.
#
# Usage: bundle-homebrew-dylibs.sh /path/to/Flare.app

set -euo pipefail

if [ "$#" -ne 1 ] || [ ! -d "$1" ]; then
  echo "Usage: $0 /path/to/Flare.app" >&2
  exit 2
fi

APP_BUNDLE="$(cd "$1" && pwd -P)"
CONTENTS_DIR="$APP_BUNDLE/Contents"
FRAMEWORKS_DIR="$CONTENTS_DIR/Frameworks"
MACOS_DIR="$CONTENTS_DIR/MacOS"
PLUGINS_DIR="$CONTENTS_DIR/PlugIns"
APP_RPATH="@executable_path/../Frameworks"

mkdir -p "$FRAMEWORKS_DIR"

# ---- Homebrew locations (no hard-coded Cellar path) -------------------------
BREW_PREFIX=""
BREW_CELLAR=""
if command -v brew >/dev/null 2>&1; then
  BREW_PREFIX="$(brew --prefix 2>/dev/null || true)"
  BREW_CELLAR="$(brew --cellar 2>/dev/null || true)"
fi
# brew missing or unusable: fall back to the two standard prefixes.
SEARCH_PREFIXES=()
[ -n "$BREW_PREFIX" ] && SEARCH_PREFIXES+=("$BREW_PREFIX")
for p in /opt/homebrew /usr/local; do
  [ -d "$p" ] && SEARCH_PREFIXES+=("$p")
done
CELLARS=()
[ -n "$BREW_CELLAR" ] && CELLARS+=("$BREW_CELLAR")
for p in "${SEARCH_PREFIXES[@]:-}"; do
  [ -n "$p" ] || continue
  [ -d "$p/Cellar" ] && CELLARS+=("$p/Cellar")
done

is_macho() {
  file -b "$1" | grep -q 'Mach-O'
}

# Install names a dylib/executable links, excluding its own id.
list_dependencies() {
  local target="$1" self_id
  self_id="$(otool -D "$target" 2>/dev/null | awk 'NR == 2 { print $1; exit }')"
  otool -L "$target" | awk -v self_id="$self_id" 'NR > 1 && $1 != self_id { print $1 }'
}

list_rpaths() {
  otool -l "$1" | awk '/cmd LC_RPATH/ { getline; getline; print $2 }'
}

expand_rpath() {
  local target="$1" rp="$2"
  case "$rp" in
    @loader_path/*)     printf '%s\n' "$(dirname "$target")/${rp#@loader_path/}" ;;
    @loader_path)       printf '%s\n' "$(dirname "$target")" ;;
    @executable_path/*) printf '%s\n' "$MACOS_DIR/${rp#@executable_path/}" ;;
    *)                  printf '%s\n' "$rp" ;;
  esac
}

# Find the real file for `$2` (an install name) as required by `$1`.
find_dylib() {
  local target="$1" dependency="$2"
  local name rel rp dir candidate formula prefix
  name="$(basename "$dependency")"

  # 1. An absolute install name that exists (any prefix, any version).
  case "$dependency" in
    /*) [ -f "$dependency" ] && { printf '%s\n' "$dependency"; return 0; } ;;
  esac

  # 2. Relative to the target's runtime search paths (@rpath/...).
  case "$dependency" in
    @rpath/*) rel="${dependency#@rpath/}" ;;
    @loader_path/*) rel="${dependency#@loader_path/}" ;;
    *) rel="$name" ;;
  esac
  while IFS= read -r rp; do
    [ -n "$rp" ] || continue
    dir="$(expand_rpath "$target" "$rp")"
    candidate="$dir/$rel"
    [ -f "$candidate" ] && { printf '%s\n' "$candidate"; return 0; }
  done < <(list_rpaths "$target")

  # 3. The formula named in a Cellar path, at whatever version is installed now.
  case "$dependency" in
    */Cellar/*/*)
      formula="${dependency#*/Cellar/}"
      formula="${formula%%/*}"
      if [ -n "$formula" ] && command -v brew >/dev/null 2>&1; then
        prefix="$(brew --prefix "$formula" 2>/dev/null || true)"
        if [ -n "$prefix" ] && [ -f "$prefix/lib/$name" ]; then
          printf '%s\n' "$prefix/lib/$name"; return 0
        fi
      fi
      for c in "${CELLARS[@]:-}"; do
        [ -n "$c" ] || continue
        candidate="$(find -L "$c/$formula" -type f -name "$name" -print -quit 2>/dev/null || true)"
        [ -n "$candidate" ] && { printf '%s\n' "$candidate"; return 0; }
      done
      ;;
  esac

  # 4. Any Homebrew lib dir, then the whole Cellar, by file name.
  for p in "${SEARCH_PREFIXES[@]:-}"; do
    [ -n "$p" ] || continue
    for dir in "$p/lib" "$p"/opt/*/lib; do
      [ -f "$dir/$name" ] && { printf '%s\n' "$dir/$name"; return 0; }
    done
  done
  for c in "${CELLARS[@]:-}"; do
    [ -n "$c" ] || continue
    candidate="$(find -L "$c" -type f -name "$name" -print -quit 2>/dev/null || true)"
    [ -n "$candidate" ] && { printf '%s\n' "$candidate"; return 0; }
  done
  return 1
}

ensure_rpath() {
  local target="$1"
  if ! list_rpaths "$target" | grep -qx "$APP_RPATH"; then
    install_name_tool -add_rpath "$APP_RPATH" "$target" 2>/dev/null || true
  fi
}

# Dependencies we leave alone: the OS, and references that already resolve
# relative to the bundle.
is_system_or_relative() {
  case "$1" in
    /usr/lib/*|/System/*|@executable_path/*|@loader_path/*) return 0 ;;
  esac
  return 1
}

COPIED=0

resolve_target() {
  local target="$1" dependency name bundled source_path

  while IFS= read -r dependency; do
    [ -n "$dependency" ] || continue
    is_system_or_relative "$dependency" && continue

    name="$(basename "$dependency")"
    bundled="$FRAMEWORKS_DIR/$name"

    case "$dependency" in
      @rpath/*)
        # Already bundled -> nothing to do; the rpath added below resolves it.
        if [ -f "$bundled" ]; then ensure_rpath "$target"; continue; fi
        ;;
    esac

    if [ ! -f "$bundled" ]; then
      if ! source_path="$(find_dylib "$target" "$dependency")" || [ -z "$source_path" ]; then
        # Qt frameworks etc. are macdeployqt's job; only complain about things
        # that are definitely not going to resolve.
        case "$dependency" in
          @rpath/Qt*|@rpath/*.framework/*) continue ;;
        esac
        echo "ERROR: cannot locate $dependency required by $target" >&2
        return 1
      fi
      cp -L "$source_path" "$bundled"
      chmod u+w "$bundled"
      install_name_tool -id "@rpath/$name" "$bundled"
      ensure_rpath "$bundled"
      echo "Bundled $dependency from $source_path"
      COPIED=1
    fi

    if [ "$dependency" != "@rpath/$name" ]; then
      install_name_tool -change "$dependency" "@rpath/$name" "$target"
    fi
    ensure_rpath "$target"
  done < <(list_dependencies "$target")
}

collect_targets() {
  local f
  for f in "$FRAMEWORKS_DIR"/*.dylib "$MACOS_DIR"/*; do
    [ -f "$f" ] && is_macho "$f" && printf '%s\n' "$f"
  done
  if [ -d "$PLUGINS_DIR" ]; then
    find "$PLUGINS_DIR" -type f -name '*.dylib' -print
  fi
}

# Iterate to a fixed point: newly copied dylibs bring their own dependencies.
while :; do
  COPIED=0
  while IFS= read -r target; do
    resolve_target "$target"
  done < <(collect_targets)
  [ "$COPIED" -eq 1 ] || break
done

# ---- verification -----------------------------------------------------------
fail=0
while IFS= read -r target; do
  while IFS= read -r dependency; do
    [ -n "$dependency" ] || continue
    is_system_or_relative "$dependency" && continue
    case "$dependency" in
      @rpath/*)
        name="${dependency#@rpath/}"
        if [ -f "$FRAMEWORKS_DIR/$name" ]; then continue; fi
        case "$name" in Qt*|*.framework/*) continue ;; esac
        echo "ERROR: unresolved $dependency in $target" >&2; fail=1 ;;
      *)
        echo "ERROR: non-portable dependency $dependency in $target" >&2; fail=1 ;;
    esac
  done < <(list_dependencies "$target")
  # a target that links a bundled @rpath/ dylib must carry the app rpath
  uses_bundled=0
  while IFS= read -r dependency; do
    case "$dependency" in
      @rpath/*) [ -f "$FRAMEWORKS_DIR/${dependency#@rpath/}" ] && uses_bundled=1 ;;
    esac
  done < <(list_dependencies "$target")
  if [ "$uses_bundled" -eq 1 ] && ! list_rpaths "$target" | grep -qx "$APP_RPATH"; then
    echo "ERROR: $target links a bundled @rpath dylib but lacks rpath $APP_RPATH" >&2; fail=1
  fi
done < <(collect_targets)

[ "$fail" -eq 0 ] || exit 1
echo "Homebrew dylib dependencies are bundled and resolved (@rpath)."
