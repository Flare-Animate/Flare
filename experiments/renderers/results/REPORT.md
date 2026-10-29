# CPU rasterizer comparison (tiny-skia 0.12.0, vello_cpu 0.3.0, agg-rust 1.2.0)

Experiment for the pack's renderer gate (WP05 / `perf(render)`). Isolated crate, own lockfile,
not part of the product dependency graph. Reproduce with `./run_bench.sh` (needs hyperfine).
Machine, toolchain and binary hash: `machine.txt`. Raw data: `hf-*.json` (hyperfine) and `warm.txt`
(in-process, 30 renders each, after the first). Same deterministic scene bytes for every backend.

## Scenes
- `adjacent`: 3200 opaque triangles sharing every edge (seam/conflation test).
- `blobs`: 1500 translucent (alpha 150) closed cubic shapes, some self-intersecting.
- `strokes`: 1500 cubic curves, widths 1-24, round caps and joins (heavy overdraw).
All: anti-aliased, non-zero fill, source-over, opaque white background, 8-bit sRGB, no gradients/masks/text.

## Speed and memory (hyperfine, 20 runs after 3 warmup; wall time includes process start + scene build)
| scene | size | backend | wall mean ± sd (ms) | min–max (ms) | peak RSS (MiB) | vs tiny-skia |
|---|---|---|---|---|---|---|
| adjacent | 1920x1080 | tiny-skia | 25.6 ± 2.2 | 23.4–30.7 | 18.0 | 1.00x |
| adjacent | 1920x1080 | vello-cpu | 10.0 ± 1.3 | 8.3–12.8 | 22.3 | 2.57x |
| adjacent | 1920x1080 | agg | 13.0 ± 2.2 | 11.3–19.9 | 9.8 | 1.97x |
| adjacent | 3840x2160 | tiny-skia | 50.8 ± 1.6 | 47.9–53.9 | 65.5 | 1.00x |
| adjacent | 3840x2160 | vello-cpu | 23.4 ± 11.2 | 18.5–69.1 | 74.4 | 2.17x |
| adjacent | 3840x2160 | agg | 41.4 ± 31.8 | 30.9–174.1 | 33.6 | 1.23x |
| blobs | 1920x1080 | tiny-skia | 98.4 ± 4.1 | 94.5–113.7 | 18.0 | 1.00x |
| blobs | 1920x1080 | vello-cpu | 16.7 ± 0.6 | 15.8–18.3 | 25.5 | 5.91x |
| blobs | 1920x1080 | agg | 69.0 ± 2.7 | 64.1–74.2 | 9.9 | 1.43x |
| blobs | 3840x2160 | tiny-skia | 118.0 ± 3.1 | 113.3–124.4 | 65.5 | 1.00x |
| blobs | 3840x2160 | vello-cpu | 25.1 ± 1.2 | 22.6–28.4 | 73.0 | 4.70x |
| blobs | 3840x2160 | agg | 84.9 ± 1.4 | 82.2–87.9 | 33.7 | 1.39x |
| strokes | 1920x1080 | tiny-skia | 375.5 ± 14.4 | 361.5–425.3 | 17.9 | 1.00x |
| strokes | 1920x1080 | vello-cpu | 73.6 ± 5.7 | 70.2–93.9 | 69.1 | 5.10x |
| strokes | 1920x1080 | agg | 178.3 ± 4.4 | 171.6–187.9 | 10.0 | 2.11x |
| strokes | 3840x2160 | tiny-skia | 830.4 ± 24.7 | 801.7–920.6 | 65.5 | 1.00x |
| strokes | 3840x2160 | vello-cpu | 136.9 ± 4.8 | 130.2–149.4 | 152.8 | 6.06x |
| strokes | 3840x2160 | agg | 441.0 ± 16.8 | 411.7–470.7 | 34.1 | 1.88x |

Warm in-process p50 / p95 (ms), 4K: adjacent tiny 42.9/48.4, vello 13.1/17.4, agg 26.2/29.1;
blobs tiny 109.2/118.5, vello 17.9/19.5, agg 79.1/97.9; strokes tiny 853.7/909.4, vello 138.1/182.8, agg 433.5/485.4.
All three ran single-threaded (user time ~ wall time in `/usr/bin/time -l`).
Outliers: adjacent 4K agg (max 174 ms) and vello (max 69 ms) had a slow run each; one-off, not investigated.

## Agreement with tiny-skia (not ground truth), 1080p
| scene | backend | mean abs err | max | px > 8 | px > 32 |
|---|---|---|---|---|---|
| adjacent | vello-cpu | 0.24 | 20 | 0.46% | 0 |
| adjacent | agg | 0.23 | 21 | 0.42% | 0 |
| blobs | vello-cpu | 0.50 | 42 | 1.23% | 0.002% |
| blobs | agg | 1.01 | 26 | 0.15% | 0 |
| strokes | vello-cpu | 2.39 | 132 | 9.67% | 1.53% |
| strokes | agg | 1.30 | 134 | 5.44% | 0.28% |
Strokes disagree most: different stroker/flattening plus heavy overdraw amplifying edge differences.
Eyeballed 1080p strokes and blobs output: visually equivalent. No backend is a reference, so this
shows disagreement, not error. Seam behaviour in `adjacent` was not inspected at pixel level.

## Findings
1. vello_cpu is 2.2-6.1x faster than tiny-skia and 1.7-5x faster than agg-rust on this corpus, at
   comparable output, but uses more memory (up to 153 MiB vs 66 MiB on 4K strokes) and 48 crates vs 26 (tiny-skia) and 1 (agg-rust).
2. agg-rust is 1.2-2x faster than tiny-skia, has the smallest footprint (~half the RSS) and zero dependencies.
3. agg-rust 1.2.0 attaches its buffer through `unsafe RowAccessor::new_with_buf(ptr, ..)`; the README
   example (`RenderingBuffer::new(&mut buf, ..)`) does not match the published API. Any use needs a
   small safe wrapper to satisfy the project rule against raw-pointer-dependent public APIs.
4. vello_cpu canvas size is `u16`, so max 65535 px per side (fine for 4K/8K exports).
5. Licenses: tiny-skia BSD-3-Clause, agg-rust BSD-3-Clause (crate metadata only; inherited AGG
   components not audited), vello_cpu Apache-2.0 OR MIT.

## Provisional decision (not final)
Primary CPU route: **vello_cpu**, on speed with acceptable agreement. Keep **tiny-skia** only as a
cross-check/reference if a second renderer is wanted. Do not adopt **agg-rust** yet; revisit if the
compound-rasterizer seam behaviour (Flash-style adjacent regions) proves to matter, since that
is its distinguishing feature and was NOT tested here.

## Not measured (still open gates)
Gradients, masks/clips, nested groups/blend modes, text, bitmap fills, perspective, tiled vs
untiled output, vello_cpu multithreading and `OptimizeQuality` mode, cold-start/first-render cost
beyond hyperfine, Vello GPU/wgpu (no GPU path run), Lyon/tess2, pixel-level seam inspection,
other platforms. The corpus is synthetic, not real Animate projects. One machine (Apple M1 Pro).
