#!/usr/bin/env bash
# Regenerate flare/sources/common/tvrender/mm_html5_runtime.js from mm_html5_runtime.ts.
#   scripts/build-mm-html5-runtime.sh          compile and overwrite the .js
#   scripts/build-mm-html5-runtime.sh --check  fail if the committed .js differs from compiled output (CI)
# include_str!("mm_html5_runtime.js") in mm_html5.rs is unchanged; the .js stays the committed artifact.
set -euo pipefail
TS_VERSION="6.0.2"   # pinned so output is reproducible (tsconfig relies on 6.x's ignoreDeprecations)
root="$(cd "$(dirname "$0")/.." && pwd)"
dir="$root/flare/sources/common/tvrender"
if command -v tsc >/dev/null 2>&1 && [ "$(tsc -v)" = "Version $TS_VERSION" ]; then
  TSC=(tsc)
else
  TSC=(npx --yes "-p" "typescript@$TS_VERSION" tsc)
fi
out="$(mktemp -d)"
trap 'rm -rf "$out"' EXIT
"${TSC[@]}" -p "$dir/tsconfig.mm.json" --outDir "$out"
if [ "${1:-}" = "--check" ]; then
  if ! diff -u "$dir/mm_html5_runtime.js" "$out/mm_html5_runtime.js"; then
    echo "mm_html5_runtime.js is stale: run scripts/build-mm-html5-runtime.sh and commit the result" >&2
    exit 1
  fi
  echo "mm_html5_runtime.js is up to date"
else
  cp "$out/mm_html5_runtime.js" "$dir/mm_html5_runtime.js"
  echo "wrote $dir/mm_html5_runtime.js"
fi
