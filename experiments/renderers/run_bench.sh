#!/usr/bin/env bash
# Reproduce: ./run_bench.sh   (needs hyperfine; run on an otherwise idle machine)
set -euo pipefail
cd "$(dirname "$0")"
cargo build --release --locked
B=target/release/renderer-bench
{
  echo "date: $(date -u +%FT%TZ)"
  echo "rustc: $(rustc --version)"
  echo "hyperfine: $(hyperfine --version)"
  echo "os: $(uname -srm)"
  echo "cpu: $(sysctl -n machdep.cpu.brand_string 2>/dev/null || grep -m1 'model name' /proc/cpuinfo)"
  echo "cores: $(sysctl -n hw.ncpu 2>/dev/null || nproc)"
  echo "mem_bytes: $(sysctl -n hw.memsize 2>/dev/null || true)"
  echo "binary_sha256: $(shasum -a 256 $B | cut -d' ' -f1)"
} > results/machine.txt
for size in 1920x1080 3840x2160; do
  for scene in adjacent blobs strokes; do
    hyperfine --warmup 3 --runs 20 --export-json "results/hf-$scene-$size.json" \
      -n tiny-skia "$B render tiny-skia $scene $size" \
      -n vello-cpu "$B render vello-cpu $scene $size" \
      -n agg "$B render agg $scene $size" > "results/hf-$scene-$size.txt"
    for backend in tiny-skia vello-cpu agg; do
      $B render $backend $scene $size --repeat 30 | tee -a results/warm.txt
    done
  done
done
