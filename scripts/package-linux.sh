#!/usr/bin/env bash
# Builds dist/flare-<version>-linux-<arch>.tar.gz holding the app, the command line tool and the
# MCP server. Needs the libraries listed in CONTRIBUTING.md.
set -euo pipefail
cd "$(dirname "$0")/.."
version=$(cargo metadata --no-deps --format-version 1 --locked |
  python3 -c 'import json,sys; print(next(p["version"] for p in json.load(sys.stdin)["packages"] if p["name"]=="flare-desktop"))')
cargo build --release --locked -p flare-desktop -p flare-cli -p flare-mcp
name="flare-$version-linux-$(uname -m)"
rm -rf "dist/$name" && mkdir -p "dist/$name"
cp target/release/flare-desktop target/release/flare-cli target/release/flare-mcp "dist/$name/"
cp LICENSE README.md CHANGELOG.md "dist/$name/"
tar -C dist -czf "dist/$name.tar.gz" "$name"
echo "dist/$name.tar.gz"
