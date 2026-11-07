# Media formats: import and export

The target is that every image and video format in the table below imports and exports, on every
surface that can have it (desktop, browser, `flare-cli`, `flare-mcp`). This document is the audit:
where each format stands, what carries it, and the slice that moves it. A row changes when its
behaviour lands, in the same commit. Gaps that stay are recorded in [SURFACES.md](SURFACES.md) with
their reason.

Status words: **yes** (works, tested), **partial** (works with a stated limit), **no** (not
done), **gap** (no permissive way to do it yet; the reason is given).

## Current state (read from the code)

| Format | Import | Export | Carried by | Notes | Slice |
|---|---|---|---|---|---|
| PNG still | yes | yes | `png` crate (`flare-import/bitmap.rs`, `flare-render/image.rs`) | one-frame project on import | done |
| APNG (animated PNG) | no | no | `png` 0.18 supports it | decode and encode both available in the crate already in use | 2 |
| JPEG | yes (one-frame project) | yes (opaque only) | `jpeg_decoder` (`flare-import/bitmap.rs`); `image` encodes | claimed in `file_kinds.rs`; desktop, web and OS filters list it | done (import) |
| GIF still | yes | yes | `gif` crate; export via the movie path | | done |
| GIF animated | yes (delays become frame rate and holds) | yes (`flare-runtime` movie) | `gif` crate | | done |
| BMP | yes (one-frame project) | yes | `image` (bmp); import bounded by the `image` limits | round trip tested in `flare-import` | done (import) |
| TGA | yes (one-frame project) | yes | `image` (tga), written uncompressed on purpose | round trip tested | done (import) |
| TIFF | no | no | none | refused as `.tif`/`.tiff` in `file_kinds.rs` | 2 |
| WebP lossless | no | no | `image` `webp` feature (pure-Rust lossless) | | 2 |
| WebP lossy | no | no | to check: image-webp decodes lossy VP8 | encode may be lossless only | 2 |
| WebP animated | no | no | image-webp decode | | 5 (video slice) |
| AVIF | no | no | rav1e (BSD-2) for encode; decode needs a permissive AV1 decoder | heavy; checked before adding | 2 (if licence and size allow) |
| ICO | no | no | `image` `ico` feature | | 2 |
| PNM (PBM/PGM/PPM) | no | no | `image` `pnm` feature | `.ppm` is already claimed as Flipnote's PPM animation, so generic PNM needs a decision | 2 |
| QOI | no | no | `image` `qoi` feature | | 2 |
| HDR (Radiance) | no | no | `image` `hdr` feature | | 2 |
| OpenEXR | no | no | `image` `exr` feature (exr crate, pure Rust) | | 2 |
| DDS | no | no | `image` `dds` (decode only) | write only if cheap | 2 |
| SVG | yes (`usvg`) | **no** | import in `flare-import/svg.rs` | export from the vector stage | 3 |
| PSD | no | no | psd crate (read) | layered export needs own writer | 4 |
| HEIC | no | no | no permissive pure-Rust decoder found | recorded as a gap if none is found | 4 |

| Video | Import | Export | Carried by | Notes | Slice |
|---|---|---|---|---|---|
| MP4 (H.264 + AAC) | no | yes, with sound | `rusty_h264`, `oxideav-aac`, `muxide` (all pure Rust), `flare-video` | the only video export that exists today | done (export) |
| GIF video | yes | yes | see GIF above | | done |
| APNG video | no | no | `png` crate | | 5 |
| animated WebP video | no | no | image-webp | | 5 |
| WebM (VP8/VP9/AV1) | no | no | pure Rust AV1 encode (rav1e) + WebM mux if permissive and small; else ffmpeg | | 5 |
| MOV | no | no | ffmpeg bridge | | 5 |
| AVI | no | no | ffmpeg bridge | | 5 |
| MKV | no | no | ffmpeg bridge | | 5 |
| MPEG | no | no | ffmpeg bridge | | 5 |
| FLV | no | no | ffmpeg bridge; refused today as `.flv` (the Flash video is not an animation) | | 5 |

## Policy

- Prefer permissive pure-Rust crates. A new crate must be MIT, Apache-2.0, BSD or similar, and its
  size is measured with `cargo tree` before it is added. Heavy codecs go behind an optional feature.
- Video encoders with no permissive pure-Rust production implementation use the system `ffmpeg`
  as an optional external tool, run as a subprocess with argv only. Flare never links or bundles
  it (it is LGPL or GPL). Without ffmpeg, the format reports a typed error that names it and how to
  install it.
- Every new decoder is bounded: dimensions, frame count, decoded bytes and time. Input is untrusted.
