#!/usr/bin/env bash
# Configure + build Flare into a persistent out-of-tree dir (default /home/user/bld).
# Mirrors .github/workflows/workflow_linux.yml. Prints only errors; exit code = build result.
# Env: BUILD_DIR (default /home/user/bld), JOBS (default 2), CC/CXX (default gcc/g++),
#      VERBOSE=1 to stream full output. Full logs: $BUILD_DIR/{cmake_configure,build}.log
set -u
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BUILD_DIR="${BUILD_DIR:-/home/user/bld}"
JOBS="${JOBS:-2}"
CC="${CC:-gcc}"; CXX="${CXX:-g++}"
mkdir -p "$BUILD_DIR"

# Bundled libtiff private headers (same as CI).
if [ ! -f "$REPO/thirdparty/tiff-4.0.3/tif_config.h" ]; then
  (cd "$REPO/thirdparty/tiff-4.0.3" && chmod +x configure && ./configure --disable-lzma) >"$BUILD_DIR/tiff_configure.log" 2>&1 \
    || { echo "tiff configure failed"; tail -20 "$BUILD_DIR/tiff_configure.log"; exit 1; }
fi

LAUNCH=()
if command -v ccache >/dev/null 2>&1; then
  LAUNCH=(-DCMAKE_CXX_COMPILER_LAUNCHER=ccache -DCMAKE_C_COMPILER_LAUNCHER=ccache)
fi

run() { # run <logfile> cmd...
  local log="$1"; shift
  if [ "${VERBOSE:-0}" = 1 ]; then "$@" 2>&1 | tee "$log"; return "${PIPESTATUS[0]}"; fi
  "$@" >"$log" 2>&1
}

if [ ! -f "$BUILD_DIR/build.ninja" ] || [ "${RECONFIGURE:-0}" = 1 ]; then
  run "$BUILD_DIR/cmake_configure.log" cmake -S "$REPO/flare/sources" -B "$BUILD_DIR" -G Ninja \
      "${LAUNCH[@]}" -DCMAKE_C_COMPILER="$CC" -DCMAKE_CXX_COMPILER="$CXX" \
      -DCMAKE_C_FLAGS=-w -DCMAKE_CXX_FLAGS=-w \
      -DWITH_TRANSLATION=OFF -DWITH_SYSTEM_LZO=ON -DCMAKE_BUILD_TYPE=Release
  rc=$?
  if [ $rc -ne 0 ]; then echo "CMake configure FAILED ($rc):"; tail -40 "$BUILD_DIR/cmake_configure.log"; exit $rc; fi
fi

start=$(date +%s)
run "$BUILD_DIR/build.log" ninja -C "$BUILD_DIR" -j"$JOBS" "$@"
rc=$?
el=$(( $(date +%s) - start ))
if [ $rc -ne 0 ]; then
  echo "BUILD FAILED (rc=$rc, ${el}s). Errors:"
  grep -E -B1 -A6 "error:|FAILED:|undefined reference|ninja: error" "$BUILD_DIR/build.log" | head -80
  exit $rc
fi
echo "build ok in ${el}s -> $BUILD_DIR"
