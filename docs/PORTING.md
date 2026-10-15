# C++ -> Rust porting status (C ABI, gated by FLARE_WITH_RUST)

Rust modules live beside the C++ they port: `<name>.cpp` becomes `<name>.rs` in the same directory (e.g. `common/tsystem/tfilepath.rs`). There is no `rust/` subfolder; see RUST_REWRITE.md.
Crate manifest: `flare/sources/Cargo.toml`. Tests: `python tests/run_rust.py`.

| C++ | Rust | Status |
|---|---|---|
| common/tsystem/tfilepath.cpp (standard mode: getType/getName/getLevelName/getDots/getFrame/getParentDir) | common/tsystem/tfilepath.rs | partial, tested |
| common/tapptools/tenv.cpp (getSystemVarPathSetValue split) | common/tapptools/tenv.rs | partial, tested |
| common/tapptools/tenv.cpp (rest: EnvGlobals/VariableSet/stuff dirs) | - | todo |
| common/flash (swf/xfl/xflrender/symlib/fcp/framecount) | common/flash/*.rs | done |
| common/moho (Moho reader) | common/moho/moho.rs | done |
| flare/sources/remote (remote control) | remote/remote.rs | done |
| tfilepath non-standard mode (analyzePath), withName/withParentDir | - | todo |

## Retired C++ files (cpp deleted, header = thin forwarder into Rust)

| Module | C++ retired | Rust file | Tests | Grain |
|---|---|---|---|---|
| tnzcore | common/tcore/tmathutil.cpp | common/tcore/tmathutil.rs | 8 | fine (scalar math; array helpers coarse) |
| tnzbase | common/tparam/tcubicbezier.cpp | common/tparam/tcubicbezier.rs | 6 | fine |
| tnzcore | common/tcore/trandom.cpp | common/tcore/trandom.rs | 7 | fine |
| tnzcore | common/tcolor/tcolorvalue.cpp | common/tcolor/tcolorvalue.rs | 6 | fine |
| tnzcore | common/trop/runsmap.cpp | common/trop/runsmap.rs | 4 | coarse |
| stdfx | stdfx/igs_color_rgb_hls.cpp | stdfx/igs_color_rgb_hls.rs | 6 | fine (per-pixel scalar; used by the coarse kernels below directly in Rust) |
| stdfx | stdfx/igs_color_rgb_hsv.cpp | stdfx/igs_color_rgb_hsv.rs | 6 | fine (same) |
| stdfx | stdfx/igs_math_random.cpp | stdfx/igs_math_random.rs | 4 | fine (per-sample; see note) |
| stdfx | stdfx/igs_gauss_distribution.cpp (was `#include`d as source; new igs_gauss_distribution.h) | stdfx/igs_gauss_distribution.rs | 4 | coarse |
| stdfx | stdfx/igs_negate.cpp | stdfx/igs_negate.rs | 7 | coarse |
| stdfx | stdfx/igs_density.cpp | stdfx/igs_density.rs | 4 | coarse |
| stdfx | stdfx/igs_warp_hori.cpp | stdfx/igs_warp_hori.rs | 7 | coarse (whole float image + reference) |
| stdfx | stdfx/igs_warp_vert.cpp | stdfx/igs_warp_vert.rs | 7 | coarse |
| stdfx | stdfx/igs_hls_adjust.cpp | stdfx/igs_hls_adjust.rs | 8 | coarse |
| stdfx | stdfx/igs_hsv_adjust.cpp | stdfx/igs_hsv_adjust.rs | 8 | coarse |
| stdfx | stdfx/igs_hls_add.cpp | stdfx/igs_hls_add.rs | 8 | coarse |
| stdfx | stdfx/igs_hsv_add.cpp | stdfx/igs_hsv_add.rs | 8 | coarse |
| stdfx | stdfx/igs_level_auto.cpp | stdfx/igs_level_auto.rs | 8 | coarse (8/16-bit LUT) |
| stdfx | stdfx/igs_level_auto_in_camera.cpp | stdfx/igs_level_auto_in_camera.rs | 7 | coarse (8/16-bit LUT + float) |
| stdfx | stdfx/igs_levels.cpp | stdfx/igs_levels.rs | 12 | coarse (8/16-bit LUT + float LUT) |
| stdfx | stdfx/igs_median_filter.cpp | stdfx/igs_median_filter.rs | 8 | coarse |
| stdfx | stdfx/igs_median_filter_smooth.cpp | stdfx/igs_median_filter_smooth.rs | 5 | coarse |
| stdfx | stdfx/igs_hsv_noise.cpp | stdfx/igs_hsv_noise.rs | 11 | coarse (also hosts the noise plane / limit helpers shared with hls_noise) |
| stdfx | stdfx/igs_hls_noise.cpp | stdfx/igs_hls_noise.rs | 7 | coarse |
| sound | sound/tsioutils.cpp | sound/tsioutils.rs | (earlier) | coarse |
| tnzbase | tnzbase/permissionsmanager.cpp | tnzbase/permissionsmanager.rs | (earlier) | fine |

Notes: the C++ `throw`s (TMathException, std::domain_error) stay in the inline header
forwarders; Rust returns a status code. Modules that call the forwarders link `flare_formats`
(tnzcore, tnzbase, stdfx, colorfx, tnztools, flarelib, Flare).

Batch 2 notes (13 `stdfx/igs_*` kernels, +104 tests, workspace 802 -> 906):
- Every entry point is a whole-buffer call (`extern "C"` in the Rust file, inline forwarder in the unchanged-signature header). Rust validates the raw arguments (null, negative size, overflow, misaligned 16-bit/float buffers, overlapping in/out for the median filters) and returns a status code; the header rethrows `std::domain_error` (the old texts) or `std::invalid_argument`. No `slice::from_raw_parts` on null; nothing panics across the boundary.
- `bits` for float images is `std::numeric_limits<float>::digits`, i.e. **24**, not 32 (`ino::bits()`); `igs_levels` and `igs_level_auto_in_camera` select float on 24.
- Quirks of the C++ kept on purpose (found by differential testing against the old code, 1670 bit-identical outputs over 8/16-bit/float, rgba/rgb/gray, with/without reference): `igs_level_auto*` builds table `n` (user R,G,B,A order) from the min/max of memory position `n` (B,G,R,A); grayscale `igs_levels` reads the blue table; float `igs_levels` clamps the reference weight only for RGBA; `igs_median_filter_smooth` interpolation branches collapse to "first sorted pixel whose accumulated weight reaches half" (its loop increment reads the next element); `igs_warp_*` interpolates `float*double + float*float`.
- Deliberate deviations (only where the C++ was undefined behaviour): integer `igs_levels` with `clamp_sw == false` saturates instead of wrapping on x86 (both callers pass `true`); `igs_level_auto_in_camera` clamps the camera rectangle to the image; hue wrap loops skip non-finite / |hue| >= 1e7 values instead of spinning.
- Rejected candidates: see "Rejected" in RUST_REWRITE.md (Ported so far, wave 4).

