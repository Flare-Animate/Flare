#!/usr/bin/env bash
# The tests that need a GPU adapter: Ruffle playing movies inside the process (frames over time,
# the sound we publish) and drawing them for comparison with Flare's own render. They are ignored
# in `cargo test --workspace` because a machine with no GPU would fail them. Needs Java (Ruffle's
# ActionScript library is built with it) and a GPU wgpu can open.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo test -p flare-ruffle --features embedded --test embedded -- --ignored
cargo test -p flare-ruffle --features render-check --test render
cargo test -p flare-desktop -- --ignored the_window_shows
