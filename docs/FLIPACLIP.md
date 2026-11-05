# FlipaClip projects

FlipaClip (the Android animation app) saves a project as a `.fc` file. Flare imports it through
`flare-import` (`crates/io/flare-import/src/flipaclip/`), by the extension, on every surface: File >
Open on the desktop and in the browser, `flare import` and `import_project` (MCP). Export is not
written: Flare does not save FlipaClip files.

The support article (support.flipaclip.com, "Backing up projects") says only that a project is
shared as a `.fc` file, that importing is free and that the file can be large. It describes no
internals. The layout below was read from one real sample; what is not seen there is marked
**assumed**.

## Sample provenance

- Archive: `drive-download-20240930T151824Z-001.zip` (9.9 MB, from archive.org). It holds two
  files: `V1/BFDI+assets.fc` (app version 4, 136 frames) and `V1.1/BFDI+assets.fc` (app version 7,
  141 frames). Both are 1920 x 1080 at 24 fps, named "BFDI assets".
- The animation is a third party's and its licence is unknown, so no sample is committed. The
  reader's test suite builds synthetic projects in the same layout; the ignored test
  `a_real_sample_imports` reads a file named by `FLIPACLIP_SAMPLE`.
- Three web searches found no specification, no other sample and no open-source parser for `.fc`.
  Nothing was copied from any source.

## Container

A `.fc` file is a zip archive. Its entries have no extensions:

| Entry | Content |
| --- | --- |
| `meta` | JSON object (below) |
| `bg` | optional, one picture: the background, full canvas |
| `<layer>/<frame>` | one picture per drawn frame; both numbers count from 0 |

Frames with nothing drawn have no entry (layer 1 of the sample has entries for frames 90 and
121 to 135 only). The zip's own entries are stored or deflated; the pictures inside are already
compressed.

### meta

Seen in both samples: `format` (1), `canvasWidth` (1920), `canvasHeight` (1080), `fps` (24),
`frameCount` (136 or 141), `name`, `version` (4 or 7, the app version), `atState` (empty),
`selectedBrushesState`, `toolsState` (per-tool settings), and `layersState`.

`layersState` is a JSON array held in a JSON string. Each layer has `id` (1, 2), `name` ("Capa 1",
"Capa 2"), `blendMode` (3), `active` (true on one layer), `alphaLockEnabled` (version 7),
`glowAlpha`, `glowChoke`, `glowColor` (an ARGB integer), `glowSize`, and `pixelatorSize`.

Read by the importer: `format` (must be 1; another value is refused), the canvas size (at most
8192 a side and 16 Mpx), `fps` (1 to 120), `frameCount` (1 to 2000), `name`, `version` (only
reported), and the layer names. Everything else in `meta` is not read.

### Picture

Each picture is a 24-byte header and a zstd stream. The header, from the sample:

| Offset | Size | Field |
| --- | --- | --- |
| 0 | 8 | signature `89 46 43 49 0d 0a 1a 0a` (`\x89FCI\r\n\x1a\n`) |
| 8 | 2 | version, little-endian; 1 |
| 10 | 2 | header length, little-endian; 24 |
| 12 | 3 | length of the whole entry, big-endian; equal to the entry's size in all samples |
| 15 | 1 | 0 |
| 16 | 2 | width, big-endian; the canvas width |
| 18 | 2 | height, big-endian; the canvas height |
| 20 | 4 | **not identified**; not a CRC-32 or Adler-32 of the decoded or the stored bytes; not checked |
| 24 | rest | zstd stream (magic `28 b5 2f fd`) |

The stream decodes to exactly `width * height * 4` bytes: RGBA8, rows top to bottom, 1920 x 1080 x 4
= 8 294 400 bytes for every picture in the sample.

The alpha is **premultiplied**: across 83 834 partly transparent pixels in six sampled pictures,
no colour channel was above its alpha. The importer un-premultiplies (`c * 255 / a`, rounded) so
the bitmap is straight alpha, as `flare-document` bitmaps are.

The importer crops each picture to the bounding box of its non-transparent pixels and places the
bitmap at that corner. A picture with no drawn pixel adds no span, which leaves a gap.

## Import

The `meta` and the pictures are checked before anything is built; a failure names the entry and
the reason and changes nothing. The bounds are in `flipaclip/limits.rs`:

- 4096 entries; 16 MiB per entry; 256 MiB of entries together; 1 MiB of `meta`; 64 layers;
  2000 frames.
- A picture must match the canvas; its stream must inflate to exactly its size. The bytes
  inflated across the whole import are capped at 4 GiB (work, counted before each picture is
  cropped); the bytes kept after cropping at 512 MiB, the bitmap importer's bound.
- Entry names are checked as paths before an entry is read: an absolute name, a drive letter, a
  backslash, an empty part or a `.` or `..` part is refused, as is a name listed twice.

Output: one layer per FlipaClip layer, one bitmap span per drawn frame, a background layer (when
`bg` draws something) at the bottom, the canvas as the stage, and the frame rate from `fps`. Two
diagnostics say what was read and what was not. Identical pictures share one bitmap.

Not imported, and named as unsupported or warned in the diagnostics:

- Audio and fonts: no sound entry appeared in the samples, and the format for either is unknown.
  Any entry that is not `meta`, `bg` or `<layer>/<frame>` is listed as not imported.
- Layer settings in `meta`: blend modes, glow, alpha lock and the tool settings. Every layer is
  drawn normal and visible.
- Onion skin: no setting for it was found in `meta`.

## Assumed, not seen

These are guesses that the sample cannot confirm. Each one changes a result a person would see.

1. **The `<layer>` number is an index into `layersState`**, not the layer's `id`. In the sample the
   ids are 1 and 2 and the layers are 0 and 1, so the two readings cannot be told apart.
2. **`layersState` lists the bottom layer first**, so the importer reverses it (the timeline lists
   the top layer first). Check this against the app before relying on the stacking order.
3. **`blendMode` 3 is normal.** It is not read.
4. **`active` marks the layer selected in the app**, not visibility. It is not read.
5. **Visibility and opacity** are not in these samples. Every layer is taken as visible at full
   opacity.
6. **The 4 bytes at header offset 20** are not checked, and their meaning is not known.
7. **Pixel (0, 0) of a picture is the top-left of the canvas**, and a picture is placed by its
   offset within the canvas as a bitmap at that corner.
8. **The background is a layer**: `bg` is read as one full-canvas picture held for the whole timeline.
9. **Newer app versions keep this format.** Version 4 and 7 both write `format` 1. Any other
   `format` is refused; a future version may write another one.

## Measured

Both real samples, through `import_foreign_path`, in the dev profile with `flare-import` and
`zstd-sys` at opt-level 2 (set in the root `Cargo.toml`): about 1.0 s each (152 and 157 pictures).
With those two at the default opt-level 0 the same import took about 17 s; the cost is the
unoptimised zstd and the per-pixel crop, not the file reading (about 0.1 s) or the document. The
un-premultiply lookup table changed the unoptimised run from 17.6 s to 17.1 s, which is within noise,
so it was kept for its exactness rather than its speed.

## Surfaces

- Desktop and browser: File > Open, and the open dialogs list `fc` (`flare-platform`
  `dialogs_native.rs`, `flare-web` `app.rs`). The freedesktop MIME data (`packaging/flare-mime.xml`) now
  lists `*.fc` beside `*.fcp`; the `.fcp` glob was there before and no source seen here confirms it.
- Command line: `flare import file.fc out.flare` (`apps/flare-cli` test `import_reads_a_flipaclip_project_by_its_extension`,
  a meta-only project).
- MCP: `import_project` with a `.fc` in the export directory (`apps/flare-mcp` test
  `a_flipaclip_project_in_the_export_directory_imports_by_its_extension`).
