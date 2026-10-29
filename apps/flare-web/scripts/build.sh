#!/bin/sh
# Builds the browser app into apps/flare-web/dist, ready for any static file server.
# Needs the wasm32-unknown-unknown target and the wasm-bindgen command-line tool.
set -eu
here=$(cd "$(dirname "$0")/.." && pwd)
root=$(cd "$here/../.." && pwd)
profile=${PROFILE:-release}
flags=""
[ "$profile" = release ] && flags="--release"

cargo build -p flare-web --target wasm32-unknown-unknown $flags
rm -rf "$here/dist"
mkdir -p "$here/dist/pkg"
wasm-bindgen --target web --out-dir "$here/dist/pkg" \
  "$root/target/wasm32-unknown-unknown/$profile/flare_web.wasm"
cp "$here/public/index.html" "$here/dist/index.html"
cp "$here/src/lib/main.js" "$here/dist/main.js"
echo "built $here/dist"
