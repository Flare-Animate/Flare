#!/usr/bin/env bash

set -euo pipefail

if [ "$#" -ne 1 ] || [ ! -d "$1" ]; then
  echo "Usage: $0 /path/to/Flare.app" >&2
  exit 2
fi

APP_BUNDLE="$(cd "$1" && pwd -P)"
FRAMEWORKS_DIR="$APP_BUNDLE/Contents/Frameworks"
MACOS_DIR="$APP_BUNDLE/Contents/MacOS"
BREW_CELLAR="$(brew --cellar)"

mkdir -p "$FRAMEWORKS_DIR"

list_dependencies() {
  local target="$1"
  local self_id
  self_id="$(otool -D "$target" 2>/dev/null | awk 'NR == 2 { print $1; exit }')"
  otool -L "$target" | awk -v self_id="$self_id" 'NR > 1 && $1 != self_id { print $1 }'
}

find_brew_dylib() {
  local target="$1"
  local dependency="$2"
  local relative_path="${dependency#@rpath/}"
  local runpath
  local runpath_dir
  local candidate
  local name

  while IFS= read -r runpath; do
    case "$runpath" in
      @loader_path/*)
        runpath_dir="$(dirname "$target")/${runpath#@loader_path/}"
        ;;
      @executable_path/*)
        runpath_dir="$MACOS_DIR/${runpath#@executable_path/}"
        ;;
      *)
        runpath_dir="$runpath"
        ;;
    esac
    candidate="$runpath_dir/$relative_path"
    if [ -f "$candidate" ]; then
      printf '%s\n' "$candidate"
      return 0
    fi
  done < <(otool -l "$target" | awk '/cmd LC_RPATH/ { getline; getline; print $2 }')

  name="$(basename "$relative_path")"
  find -L "$BREW_CELLAR" -type f -name "$name" -print -quit 2>/dev/null
}

resolve_target() {
  local target="$1"
  local dependency
  local name
  local bundled_path
  local source_path
  local replacement

  while IFS= read -r dependency; do
    bundled_path=""
    case "$dependency" in
      /usr/lib/*|/System/*)
        continue
        ;;
      @rpath/*.dylib)
        name="$(basename "$dependency")"
        bundled_path="$FRAMEWORKS_DIR/$name"
        if [ ! -f "$bundled_path" ]; then
          source_path="$(find_brew_dylib "$target" "$dependency")"
          if [ -z "$source_path" ] || [ ! -f "$source_path" ]; then
            echo "ERROR: cannot locate $dependency required by $target" >&2
            return 1
          fi
          cp -L "$source_path" "$bundled_path"
          chmod u+w "$bundled_path"
          install_name_tool -id "@executable_path/../Frameworks/$name" "$bundled_path"
          echo "Bundled $dependency from $source_path"
          COPIED=1
        fi
        ;;
      /usr/local/*.dylib|/opt/homebrew/*.dylib)
        name="$(basename "$dependency")"
        bundled_path="$FRAMEWORKS_DIR/$name"
        if [ ! -f "$bundled_path" ]; then
          source_path="$dependency"
          if [ ! -f "$source_path" ]; then
            source_path="$(find_brew_dylib "$target" "@rpath/$name")"
          fi
          if [ -z "$source_path" ] || [ ! -f "$source_path" ]; then
            echo "ERROR: cannot locate Homebrew dependency $dependency required by $target" >&2
            return 1
          fi
          cp -L "$source_path" "$bundled_path"
          chmod u+w "$bundled_path"
          install_name_tool -id "@executable_path/../Frameworks/$name" "$bundled_path"
          echo "Bundled $dependency from $source_path"
          COPIED=1
        fi
        ;;
    esac

    if [ -n "${bundled_path:-}" ] && [ -f "$bundled_path" ]; then
      replacement="@executable_path/../Frameworks/$(basename "$bundled_path")"
      if [ "$dependency" != "$replacement" ]; then
        install_name_tool -change "$dependency" "$replacement" "$target"
      fi
    fi
    bundled_path=""
  done < <(list_dependencies "$target")
}

is_macho() {
  file -b "$1" | grep -q 'Mach-O'
}

while :; do
  COPIED=0
  for target in "$FRAMEWORKS_DIR"/*.dylib "$MACOS_DIR"/*; do
    [ -f "$target" ] || continue
    is_macho "$target" || continue
    resolve_target "$target"
  done
  [ "$COPIED" -eq 1 ] || break
done

for target in "$FRAMEWORKS_DIR"/*.dylib "$MACOS_DIR"/*; do
  [ -f "$target" ] || continue
  is_macho "$target" || continue
  while IFS= read -r dependency; do
    case "$dependency" in
      @rpath/*.dylib|/usr/local/*.dylib|/opt/homebrew/*.dylib)
        echo "ERROR: unresolved dependency $dependency in $target" >&2
        exit 1
        ;;
    esac
  done < <(list_dependencies "$target")
done

echo "Homebrew dylib dependencies are bundled and resolved."
