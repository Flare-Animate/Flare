# Flipnote Studio files

Flare imports Nintendo Flipnote Studio animations. The reader is `crates/io/flare-import/src/flipnote/`
(the library; the desktop, browser, `flare` and `flare-mcp` reach it through `import_path` and
`import_bytes`, as they do every other format). This document says what the reader reads, what
the files hold, and how far each part was checked.

## Versions and files

| Format | Magic | Platform | Size | Status |
|---|---|---|---|---|
| PPM (`.ppm`) | `PARA` | DSi Flipnote Studio (the Hatena and Sudomemo archives of the same files should match; not checked) | 256 by 192, two 1-bit layers | Imported (frames, layers, paper, frame rate; sound and thumbnail not yet) |
| KWZ (`.kwz`) | `KFH` (the fourth byte varies: 0x14 in the samples) | 3DS Flipnote Studio 3D | 320 by 240, three layers, sound tracks | Imported: frames, three layers with their pens, paper colour, frame rate. Sound, layer depths and per-frame draw order are not imported yet |
| KWC (`.kwc`) | `KFH` | 3DS comment files | a KWZ without the `KSN` section | Imported as a one-note comment (its frames and layers, as a KWZ) |
| KIC (folder icon) | `KIC` | 3DS folder icons | as KWZ | Not recognised (not an animation) |

Flipnote Studio saved every file with an RSA-2048 signature. The reader does not verify it. A
file edited outside Flipnote Studio imports as readily as one made in it, and the import says so.

## PPM layout

All integers are little-endian unless marked.

| Offset | Field |
|---|---|
| `0x00` | magic `PARA` |
| `0x04` | frame data length (u32): from `0x06A0` to the end of the frame data |
| `0x08` | sound data length (u32) |
| `0x0C` | frame count minus one (u16) |
| `0x0E` | version (u16). The high nibble set means the metadata block is present |
| `0x10` | lock flag (u16, 1 means locked) |
| `0x12` | thumbnail frame index (i16) |
| `0x14`, `0x2A`, `0x40` | root, parent and current author names (11 UTF-16 units each) |
| `0x56`, `0x5E`, `0x8A` | parent, current and root FSIDs (8 bytes each, shown byte-reversed as hex) |
| `0x66`, `0x78` | parent and current filenames: 3 MAC bytes, 13 characters, a decimal edit count (u16) |
| `0x9A` | creation time (i32): seconds since 2000-01-01 UTC |
| `0xA0` | thumbnail: 1536 bytes, 4-bit tiles of 64 by 48 |
| `0x6A0` | animation section: offset table length (u16), then padding to `0x6A8` |
| `0x6A6` | flags (u16): bit 1 loops; bit 4 hides layer 1; bit 5 hides layer 2 |
| `0x6A8` | frame pointers, one u32 per frame, each relative to the end of the table |
| `0x6A0 + frame data length` | one flag byte per frame: bit 0 SE1, bit 1 SE2, bit 2 SE3 |
| next multiple of 4 after the flags | sound header |

The sound header is four u32 section lengths (BGM, then three sound effects), the frame speed
byte and the BGM speed byte, padded to 32 bytes. The tracks follow in that order. A track is a
4-byte block header (initial predictor i16, step index u8, one unused byte) and then 4-bit IMA
ADPCM samples, low nibble first.

Speeds: the stored byte is `8 - speed`. Frame rates by speed (flipnote.js's table, which the
reader uses): 0 and 1 are 0.5 a second, then 1, 2, 4, 6, 12, 20 and 30. Speed bytes above 8 are
refused.

### Frames

A frame starts with a header byte: bit 7 keyframe; bits 5 and 6 nonzero means two signed bytes
of translation follow (x, then y); bit 0 clear means the paper is black and the pens are
inverted; bits 1 and 2 choose layer 1's pen and bits 3 and 4 layer 2's pen, through the global
palette (white, black, red, blue).

Then, for layer 1 and then layer 2, 48 bytes of line types: four lines to a byte, two bits a
line, the lowest pair first. A zero byte means four blank lines. The line types are:

- 0: blank.
- 1: compressed. A big-endian u32 says which 8-pixel chunks are present (bit 31 is the first);
  each present chunk is one byte, least significant bit leftmost, and the rest of the chunk is
  zero.
- 2: compressed as 1, but every pixel starts set and each chunk writes all eight bits.
- 3: raw, 32 bytes of eight pixels each.

Layer 1's lines come before layer 2's. A delta frame is the XOR of its decoded layers with the
previous frame's layers. With a translation, a pixel takes the previous frame's pixel that the
translation moves it from; pixels the translation does not reach keep their decoded value.

The frame pointers, not the decoded length, bound a frame. Real files leave up to 12 bytes
after some frames that no line refers to (see Verification).

## KWZ layout

A KWZ is a run of sections, each `magic` (`KFH`, `KTN`, `KMC`, `KMI`, `KSN`; the fourth byte is
not part of the magic), a u32 length, then the data. The sections end where the 256-byte RSA
signature starts (not checked). Offsets below are from the start of a section, its magic included.

| Section | Use |
|---|---|
| `KFH` | Header. Creation and modified times (u32 at 12 and 16, Nintendo epoch), root, parent and current FSIDs (10 bytes each, at 24, 34, 44), author names (11 UTF-16 units at 54, 76, 98), filenames (28 bytes at 120, 148, 176). At 204: frame count (u16), thumbnail index, flags (lock bit 0, loop bit 1), frame speed (u8, an index into the table below), layer flags (u8: layer 1 hidden by bit 0, layer 2 by bit 1; layer 3 is shown only when both bits are clear, as flipnote.js reads it) |
| `KTN` | Thumbnail (not read) |
| `KMI` | One 28-byte record per frame: palette word (u32 at 0: paper, then two pens per layer, four bits each at bits 0, 8, 12, 16, 20, 24, 28), layer sizes (three u16 at 4, 6, 8), layer depths (three bytes at 0x14) |
| `KMC` | The layer bits: four bytes, then each frame's three layers in order, each as the size its `KMI` record gives |
| `KSN` | Sound: BGM speed (u32), five track lengths (u32 each, BGM then four effects), then the tracks after 4 bytes of padding. Absent in comments |

Frame speeds in frames a second: 1/5, 1/2, 1, 2, 4, 6, 8, 12, 20, 24, 30.

A layer's bits are 16-bit little-endian words read least significant bit first. The layer is
320 by 240 in 128-pixel tiles of 8 by 8 blocks, read tile by tile (a tile's rows top to bottom).
Each block starts with a 3-bit type:

- 0: one common line (5 bits), all eight rows. 1: one full line (13 bits), all eight rows.
- 2 and 3: a line (5 or 13 bits), alternate rows its line and its shifted line.
- 4: eight bits, one per row: a set bit takes a common line (5 bits), a clear bit a full line (13 bits).
- 5: skip the next N blocks (5 bits), which keep their pixels from the layer before.
- 7: a 2-bit pattern and a flag: two lines (5 bits each for common, or 13 each), in one of four row
  orders. Type 6 is not used and leaves the block as it was.

A line is eight pixels of value 0 to 2, coded as a base-3 number (3^8 = 6561 full lines); the
32 common lines are listed in flipnote.js. A layer the frame does not change is 38 bytes long and
keeps its pixels. A pixel of 0 is transparent; 1 and 2 are the layer's two pens.

## What the importer makes

A PPM becomes one Flare document at 256 by 192 with three layers, top to bottom: Layer 1, Layer 2
(each a transparent RGBA bitmap per frame, the pen colour where the bit is set) and Paper (a solid
bitmap of the paper colour). A KWZ is the same with Layer 3 and 320 by 240; its layers stack 1 over
2 over 3 over the paper, and the per-frame draw order and the layer depths (stereo offsets) are not
imported. Identical pictures share one bitmap in the library. A run of the same
bitmap is one span. Hidden layers import hidden. The frame rate is the speed's. The limits:
4096 frames, and 512 MiB of distinct pictures (about 2700 distinct frames at 256 by 192).

The diagnostics say what was not imported: the sound (with the number of tracks that have
samples), the thumbnail, the lock (as information), the author names and creation time (as
information), and the unchecked signature.

## Verification

Checked against real files:

- The seven PPM files in the flipnote.js test corpus (`test/samples/`, MIT) import and every frame
  decodes. Each frame's data ends inside the span up to the next frame in the file, with at most
  12 unread bytes after any frame. Those bytes look like leftovers in Flipnote's own buffer: the
  decoder reads the frame's line data completely, and the frames that follow decode as coherent
  drawings (rendered as ASCII for a few frames of `knight.ppm`, including a keyframe).
- The eight KWZ files and the one KWC comment in the same corpus decode every frame. For each
  changed layer, the bits read end within two bytes of the layer's declared size, which a misread
  tile would break. Their pixels were not compared with a reference decoder.

Not checked against real files, or only as a spec:

- The pixels were not compared with a reference decoder. An attempt to run flipnote.js on the
  samples was refused by the environment, so no digest comparison was made.
- Sound: the ADPCM decoder is tested with hand-computed values and a range test, not against the
  audible result.
- The thumbnail and the metadata strings were read from the layout only; the metadata test uses
  a synthetic file.
- KWZ pixels: not compared with a reference decoder (the same refused oracle run as for PPM).
- KWZ FSIDs are the raw hex of the ten bytes, not flipnote.js's region-formatted ids; the layer 3 visibility follows flipnote.js.
- Signatures are not verified (by design).

Tests: `crates/io/flare-import/src/flipnote/` (unit tests on synthetic files written by a test-only
writer in `ppm/synth.rs`, with every failure path: bad magic, truncation, zero frames, oversized
frame counts, frame pointers outside the file, short frame tables, bad speed bytes, tracks outside
the file, frames that run past the file, and the KWZ refusal) and `crates/io/flare-import/tests/flipnote_samples.rs`,
which reads the real files only when `FLARE_FLIPNOTE_SAMPLES` names their directory. The samples
are not committed.

## Sources

- flipnote.js (`jaames/flipnote.js`, MIT, Copyright (c) 2018 James Daniel): the PPM and KWZ
  parsers. The PPM offsets, line types, XOR rule, palette bits, frame rates and ADPCM follow its
  `src/parsers/PpmParser.ts`. See [ENGINEERING.md](ENGINEERING.md) for the attribution.
- The Flipnote Collective format wikis (`Flipnote-Collective/flipnote-studio-docs`, GPL-2.0, and
  `flipnote-studio-3d-docs`, CC BY-SA 4.0): read as facts only, no text or code copied.
- Hatenatools (`pbsds/Hatenatools`, AGPL-3.0): not read for code; no code used.

The sources are listed with their licences in [BACKLOG.md](BACKLOG.md).
