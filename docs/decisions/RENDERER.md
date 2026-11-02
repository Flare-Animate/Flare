# Decision: vello_cpu is the CPU rasterizer

Status: accepted for the CPU export and stage path. GPU rendering was tried (below) and is not adopted: vello_cpu with threads is faster.

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
scenes into an `Rgba8Unorm` texture on a wgpu 30 device (the version the workspace uses). Apple M1
Pro (10 cores), release build, 40 frames per run, p50 per frame; raw output is in
`experiments/renderers/results/gpu-matrix.txt`. Four ways of drawing:

- `cpu`: vello_cpu on one thread.
- `cpu x7`: vello_cpu with 7 worker threads, which is how the product uses it.
- `gpu`: vello_gpu, drawn and copied back to memory.
- `gpu, no copy`: vello_gpu, drawn and finished on the GPU but not copied back, which is what
  drawing straight into the texture the editor shows would cost.

The scenes are `light` (40 large shapes, what a frame usually is), `adjacent`, `blobs` (1500),
`blobs4` (6000), `small` (20000 shapes of 3 to 15 px) and `strokes` (1500).

| 1080p | cpu | cpu x7 | gpu | gpu, no copy |
|---|---|---|---|---|
| light | 3.6 ms | 2.6 ms | 4.0 ms | 2.5 ms |
| adjacent | 6.1 ms | 3.7 ms | 7.6 ms | 5.1 ms |
| blobs | 14.2 ms | 5.8 ms | 16.2 ms | 13.1 ms |
| blobs4 | 50.2 ms | 14.1 ms | 53.4 ms | 49.3 ms |
| small | 38.3 ms | 10.3 ms | 38.1 ms | 36.0 ms |
| strokes | 71.4 ms | 19.5 ms | 67.0 ms | 66.1 ms |

| 4K | cpu | cpu x7 | gpu | gpu, no copy |
|---|---|---|---|---|
| light | 6.9 ms | 5.0 ms | 11.1 ms | 3.1 ms |
| adjacent | 13.7 ms | 7.5 ms | 17.3 ms | 10.8 ms |
| blobs | 18.2 ms | 7.3 ms | 22.7 ms | 15.5 ms |
| blobs4 | 55.1 ms | 16.9 ms | 62.7 ms | 51.6 ms |
| small | 42.8 ms | 12.9 ms | 43.7 ms | 38.0 ms |
| strokes | 140.0 ms | 72.3 ms | 132.6 ms | 121.6 ms |

What it shows:

- The pictures agree: on all twelve scene and size pairs no pixel differs by more than 5 of 255.
- vello_gpu is not faster than vello_cpu with its threads on any scene: it is 2.5 to 4 times
  slower on the heavy 1080p ones (blobs4 14 ms against 53 ms, strokes 19 ms against 67 ms). It
  is level with the single-threaded CPU. Leaving out the copy back saves little except on
  `light` at 4K (11.1 to 3.1 ms), because vello_gpu builds its strips on one CPU thread and that
  is where the time goes; the GPU part is small.
- The first frame costs 30 to 180 ms for the device and the pipelines (one run, 930 ms).
- The one place it wins is a light 4K frame without the copy (3.1 ms against 5.0 ms), and a frame
  that light is already well inside the budget either way.

So: stay on vello_cpu for every surface, the editor stage included. Revisit when vello_gpu builds
its strips on several threads, or for a scene a CPU cannot draw in time (this machine's 72 ms for
4K strokes is the nearest).

It needs a GPU. It does not fall back by itself: with no adapter `request_adapter` returns an
error and nothing can be drawn. The backend therefore returns an error for any failure (no
adapter, no device, a size over 65535, a failed render) and the caller draws with vello_cpu
instead; `FLARE_NO_GPU=1` forces that path, and the output was then identical to vello_cpu's.
That is the shape to keep if the GPU is ever used: try for a GPU once at start, and keep
vello_cpu for every surface that has no GPU or no window.

Not measured: gradients, masks, text, images, other machines (this one has unified memory, so
the copy back is cheaper than on a discrete GPU), the GPU on Windows and Linux, and power use.
