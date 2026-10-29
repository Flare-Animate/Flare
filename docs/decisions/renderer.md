# Decision: vello_cpu is the CPU rasterizer

Status: accepted for the CPU export and stage path. GPU rendering is not decided.

## Decision

Use `vello_cpu` for rasterizing frames. Keep `tiny-skia` only as a cross-check if a second
renderer is wanted. Do not adopt `agg-rust` for now.

## Evidence

An isolated experiment (`experiments/renderers`, reproducible with `run_bench.sh`) rendered
the same deterministic scenes with all three on an Apple M1 Pro: 3200 shared-edge triangles,
1500 translucent cubic blobs, and 1500 thick round-cap strokes, at 1080p and 4K, timed with
hyperfine plus in-process runs. Full tables and raw data are in
`experiments/renderers/results/REPORT.md`.

- vello_cpu was 2.2 to 6.1 times faster than tiny-skia and 1.7 to 5 times faster than
  agg-rust, with output that is visually equivalent.
- It used more memory (up to 153 MiB against 66 MiB for tiny-skia on 4K strokes) and pulls in
  more crates (48 against 26 and 1).
- agg-rust attaches its buffer through an `unsafe` raw-pointer call, so using it would need a
  safe wrapper. Its compound rasterizer, the reason to consider it for adjacent Flash-style
  regions, was not tested.
- Licenses: vello_cpu is Apache-2.0 OR MIT; tiny-skia and agg-rust are BSD-3-Clause.

## Not measured

Gradients, masks and clips, blend modes, text, bitmap fills, perspective, tiled against
untiled output, vello_cpu multithreading and its quality mode, any GPU path, real Animate
projects, other machines. Revisit this decision if one of these changes the answer; the
most likely candidate is seam behaviour between adjacent regions.
