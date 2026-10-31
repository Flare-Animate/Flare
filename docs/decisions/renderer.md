# Decision: vello_cpu is the CPU rasterizer

Status: accepted for the CPU export and stage path. GPU rendering was tried (below) and is not adopted yet.

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

## GPU spike: vello_gpu 0.3.0

The experiment has a `vello-gpu` backend (`experiments/renderers/src/gpu.rs`) that draws the same
scenes into an `Rgba8Unorm` texture on a wgpu 30 device (the version the workspace uses) and reads
the pixels back. Apple M1 Pro, release build, 15 frames per run, vello_cpu single threaded as in
the first experiment. Times are per frame including the readback; `first` is the cold frame with
the device and pipelines being made.

| scene, size | vello_cpu p50 | vello_gpu p50 | vello_gpu first | largest channel difference |
|---|---|---|---|---|
| adjacent, 1080p | 6.7 ms | 7.1 ms | 934 ms | 2 of 255 |
| blobs, 1080p | 14.0 ms | 15.0 ms | 42 ms | 4 |
| strokes, 1080p | 70.1 ms | 66.6 ms | 97 ms | 3 |
| adjacent, 4K | 12.6 ms | 17.2 ms | 42 ms | 2 |
| blobs, 4K | 17.4 ms | 22.6 ms | 43 ms | 4 |
| strokes, 4K | 133.5 ms | 128.6 ms | 159 ms | 3 |

What it shows:

- The pictures agree: no pixel differs by more than 4 of 255 on any of the six scenes.
- With the readback included, the GPU is not faster on these scenes. vello_gpu still builds its
  strips on the CPU, which is where the time goes. The first frame costs 40 ms to 930 ms.
- So there is no reason to move export, `flare-cli` or `flare-mcp` to it. The case that is not
  measured is the editor stage: drawing straight into the texture egui shows, with no readback,
  and with several threads for the CPU side. That is the next measurement, not a decision.

It needs a GPU. It does not fall back by itself: with no adapter `request_adapter` returns an
error and nothing can be drawn. The backend therefore returns an error for any failure (no
adapter, no device, a size over 65535, a failed render) and the caller draws with vello_cpu
instead; `FLARE_NO_GPU=1` forces that path, and the output was then identical to vello_cpu's.
If the editor ever uses vello_gpu it keeps the same shape: try for a GPU once at start, and keep
vello_cpu for every surface that has no GPU or no window.

Not measured: gradients, masks, text, images, a frame with the readback left out, vello_cpu with
its threads, other machines, and the GPU on Windows and Linux.
