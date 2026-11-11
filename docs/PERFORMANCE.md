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
   publishing run on workers (`flare-runtime`). The UI hands over an immutable
   snapshot and polls for the result. A new slow operation (export, import, publish, a new
   effect) is a new job type there, not a blocking call in a handler.
3. **Share, do not copy.** Documents are immutable snapshots made of shared parts (`Arc`
   shapes, spans, names, layer lists). An edit copies the part it changes. Take a snapshot
   with `EditorSession::snapshot` to give work to another thread.
4. **Draw what is visible, at the resolution shown.** The stage texture is the visible part of
   the stage at screen resolution, never the whole stage at one pixel per unit. Anything new
   that draws takes a `Viewport`.
5. **Reuse identical work.** A frame is keyed by what it draws (`Frame::signature`) and by the
   very document snapshot it was drawn from, so held exposures and loops are rendered once and
   undo and redo bring back pictures already drawn without drawing them again (the cache holds
   the snapshot, which also keeps its address from being reused). Caches have a byte budget and a size cap, and drop
   entries of document states that can no longer be shown.
6. **Every unit of work is bounded** (see `ENGINEERING.md`) and uses threads only above a
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
| Undo then redo, 5 000-layer document (`cargo bench -p flare-editor`) | 6.6 ns | unchanged | a pointer swap of shared snapshots; what undo costs on screen is the redraw, which the picture cache now skips for states shown before |
| Open / save 10 000 shapes | 31 ms / 61 ms | unchanged | measured; not worth a new format yet |
| Brush outline rebuilt for the live preview each frame (`BrushStroke::sides`, release build): 2 000 samples / the 8 000 cap, shaky line, Smoothing 0 | n/a | 0.6 ms / 3.0 ms | the preview draws the committed outline instead of a ribbon of the raw points; a spiral at the cap is the worst seen, 4.2 ms, and Smoothing 2 roughly halves each |

Seven threads were no better than four (20 000 shapes: 9.2 ms against 8.6 ms) and slower on
small frames (100 shapes: 2.0 ms against 0.9 ms), so the cap is four and the threshold 800.

## Libraries: where they are used, and why not elsewhere

- **bytemuck** is used for pixel buffers (`flare-render`): reinterpreting RGBA bytes as pixels
  without copying. It is the right tool wherever a buffer changes shape without changing
  content.
- **Threads and SIMD for rasterizing** come from `vello_cpu` (its `multithreading` feature,
  which uses rayon internally, and its own SIMD kernels).
- **rayon** has one site of Flare's own: GIF export (`flare-runtime`), where every frame is
  drawn and colour-quantized independently. Frames are made a few at a time on a pool of every
  core but one (at most eight; the window keeps one), and written in order, so the bytes are the
  same whatever the thread count (a test checks 1, 2, 3 and 8). Measured on a busy 30-frame
  movie in release: 0.171 s on one thread, 0.088 s on four. It is a desktop-only path; the browser
  has no threads to give and makes the frames one at a time.
  **MP4 export does not use it, by measurement:** per frame at 1280 x 720 the picture is drawn in
  1.4 ms (7%), converted to YUV in 2.7 ms (14%) and encoded in 15 ms (79%), and the encoder has
  to see the frames in order. `wide` for the conversion would save at most a tenth of an export
  and is not worth a dependency yet.
- **rtrb** carries sound commands from the window to the audio callback (`flare-platform`): a
  lock-free queue of 256 commands. The callback owns the mixer outright, takes the commands
  waiting at the start of each buffer in order, and never waits for anything. Before this the
  mixer sat behind a mutex the window also took, and a buffer that met the lock held was
  silent (a dropout). A full queue drops the newest command and returns false rather than make
  the window wait. One thing is still on the callback thread: a sound that finishes drops its
  `Arc`, which frees memory only if it was the last owner (the window keeps the decoded sound).
- **rubato** resamples a sound whose rate is not the device's (`Audio::resampled`, an FFT
  resampler over the whole sound). The mixer takes only sounds at its rate, so a sound of
  another rate is resampled on a worker thread (`flare-platform`) before it is queued, and
  the copy is kept (64 MiB at most) for the next play; the window thread never resamples. The
  first play of such a sound starts late by as long as the resampling takes; the SWF and MP4
  exports resample on their own worker threads. This replaced three hand-written linear
  interpolation loops, which aliased when lowering a rate. Not measured: the resampling time
  of a ten-minute track.
- **zerocopy, rkyv, lz4_flex, encase, half** are not used and not planned. The project file's
  records are CBOR, compressed with zstd (see ARCHITECTURE.md); at 10 000 shapes the old JSON
  opened in 31 ms and the new records are a seventh the size, so reading numbers in place is
  not justified by measurement. `encase` and GPU-layout types would arrive with a GPU renderer.
- **kurbo** is already in the tree through `vello_cpu` and is used as `vello_cpu::kurbo`; it is
  not a direct dependency. The code that converts curves for XFL and SWF is hand-written and
  tested, and is the place to look if a curve routine is ever wrong or slow. **tracing** is what
  the performance work will want for spans around render, export and save, and goes in with it.
  **smallvec, indexmap, slotmap, hashbrown, bumpalo** have no site yet.
- **winit** is a direct dependency because the desktop app owns the event loop and window. It
  must stay in the same version family as `egui-winit`; the tree has one copy.

## Not yet done

- Region-limited recomputation inside a frame (only the changed area) and tile caching.
- A priority scheduler and cancellation of a frame already being drawn (a newer request
  currently waits for the current one to finish; stale results are dropped).
- Memory budgeting across all caches and history together (each has its own limit).
