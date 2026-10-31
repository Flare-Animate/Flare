#!/usr/bin/env bash
# Builds Ruffle's desktop player from the pinned checkout in third_party/ruffle, for Test Movie.
# It needs the submodule (git submodule update --init) and takes a few minutes and about 1.5 GB.
# The player lands at target/ruffle-desktop/release/ruffle_desktop (.exe on Windows); point
# FLARE_RUFFLE at it, or put it on PATH as `ruffle`.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
CARGO_TARGET_DIR="$root/target/ruffle-desktop" cargo build --release --locked \
  --manifest-path "$root/third_party/ruffle/Cargo.toml" -p ruffle_desktop
echo "built: $root/target/ruffle-desktop/release/ruffle_desktop"
