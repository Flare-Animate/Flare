#!/usr/bin/env bash
# Regenerate flare/sources/common/expressions/jsfl.js from jsfl.ts.
#   scripts/build-jsfl.sh          compile and overwrite the .js
#   scripts/build-jsfl.sh --check  fail if the committed .js differs from compiled output (CI)
# include_str!("jsfl.js") in ac_script.rs is unchanged; the .js stays the committed artifact.
set -euo pipefail
TS_VERSION="6.0.2"   # pinned so output is reproducible (tsconfig relies on 6.x's ignoreDeprecations)
root="$(cd "$(dirname "$0")/.." && pwd)"
dir="$root/flare/sources/common/expressions"
if command -v tsc >/dev/null 2>&1 && [ "$(tsc -v)" = "Version $TS_VERSION" ]; then
  TSC=(tsc)
else
  TSC=(npx --yes "-p" "typescript@$TS_VERSION" tsc)
fi
out="$(mktemp -d)"
trap 'rm -rf "$out"' EXIT
"${TSC[@]}" -p "$dir/tsconfig.json" --outDir "$out"
if [ "${1:-}" = "--check" ]; then
  if ! diff -u "$dir/jsfl.js" "$out/jsfl.js"; then
    echo "jsfl.js is stale: run scripts/build-jsfl.sh and commit the result" >&2
    exit 1
  fi
  echo "jsfl.js is up to date"
else
  cp "$out/jsfl.js" "$dir/jsfl.js"
  echo "wrote $dir/jsfl.js"
fi
