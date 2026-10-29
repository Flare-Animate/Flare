# Performance

How the editor stays responsive, what was measured, and the rules new work follows. Numbers
are from one development machine (release profile, criterion, medians); they show where time
goes and what a change bought, not what any other machine will do.

## Rules for new work

1. **Measure before optimizing, and keep the benchmark.** A change that is justified by speed
   comes with a benchmark that shows the before and after (`cargo bench -p <crate>`), and the
   commit message quotes both. Benchmarks use deterministic fixtures built outside the timed
   section. A result that regresses a neighbouring case is reported, not hidden.
2. **Nothing slow runs on the UI thread.** Drawing a frame, reading or writing a project, and
   publishing run on workers (`flare-ui/src/background.rs`). The UI hands over an immutable
   snapshot and polls for the result. A new slow operation (export, import, publish, a new
   effect) is a new job type there, not a blocking call in a handler.
3. **Share, do not copy.** Documents are immutable snapshots made of shared parts (`Arc`
   shapes, spans, names, layer lists). An edit copies the part it changes. Take a snapshot
   with `EditorSession::snapshot` to give work to another thread.
4. **Draw what is visible, at the resolution shown.** The stage texture is the visible part of
   the stage at screen resolution, never the whole stage at one pixel per unit. Anything new
   that draws takes a `Viewport`.
5. **Reuse identical work.** A frame is keyed by what it draws (`Frame::signature`); held
   exposures and loops are rendered once. Caches have a byte budget and a size cap, and drop
   entries of document states that can no longer be shown.
6. **Every unit of work is bounded** (see `engineering.md`) and uses threads only above a
   measured size: below it, starting workers costs more than it saves.
7. **Stale results are dropped, not shown.** Jobs carry ids; only the newest request's result
   is installed. A result for a document that changed must not replace the user's work.
8. **Tests run both ways.** Background code has an inline mode (same code path, no threads) for
   deterministic tests, plus tests that exercise the real workers.

## What was measured

| Case | Before | After | Change |
|---|---|---|---|
| Rename one layer, 5 000-layer document | 5.3 ms | 0.2 ms | structural sharing, one validation per batch, pre-sized validation sets |
| Move 100 spans, 5 000-layer document | 620 ms | 0.5 ms | same |
| Validate a 5 000-layer document | 376 us | 136 us | sets sized up front |
| Idle UI frame, 5 000 layers | 3.2 ms | 0.1 ms | timeline paints only visible rows |
| Idle UI frame, 1 000 selected spans | 0.86 ms | 0.14 ms | selection bounds cached per document state, one pass for validation |
| Render 1280x720, 100 shapes | 1.36 ms | 0.9 ms | premultiplied path, no extra conversion passes |
| Render 1280x720, 5 000 shapes | 6.3 ms | 3.4 ms | 4 rasterizer threads (only above 800 shapes) |
| Render 1280x720, 20 000 shapes | 23 ms | 8.6 ms | same |
| Open / save 10 000 shapes | 31 ms / 61 ms | unchanged | measured; not worth a new format yet |

Seven threads were no better than four (20 000 shapes: 9.2 ms against 8.6 ms) and slower on
small frames (100 shapes: 2.0 ms against 0.9 ms), so the cap is four and the threshold 800.

## Libraries: where they are used, and why not elsewhere

- **bytemuck** is used for pixel buffers (`flare-render`): reinterpreting RGBA bytes as pixels
  without copying. It is the right tool wherever a buffer changes shape without changing
  content.
- **Threads and SIMD for rasterizing** come from `vello_cpu` (its `multithreading` feature,
  which uses rayon internally, and its own SIMD kernels). Flare does not repeat them in its
  own code, so there is no separate use of `rayon` or `wide` yet. They get a site when a
  benchmark shows one: for example parallel validation of very large documents, or a SIMD
  compositing kernel for onion skin if it ever shows up in a profile.
- **zerocopy, rkyv, encase, half** have no site yet. The project file stores JSON; at 10 000
  shapes it opens in 31 ms, so a binary format is not justified by measurement. When it is
  (very long paths, or large libraries), `zerocopy` is the planned way to read the numbers.
  `encase` and GPU-layout types arrive with a GPU renderer.
- **winit** is a direct dependency because the desktop app owns the event loop and window. It
  must stay in the same version family as `egui-winit`; the tree has one copy.

## Not yet done

- Region-limited recomputation inside a frame (only the changed area) and tile caching.
- A priority scheduler and cancellation of a frame already being drawn (a newer request
  currently waits for the current one to finish; stale results are dropped).
- Memory budgeting across all caches and history together (each has its own limit).
