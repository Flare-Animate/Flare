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
| stdfx | stdfx/hsvutil.cpp | stdfx/hsvutil.rs | 5 | fine (scalar OLDRGB2HSV/OLDHSV2RGB; 2M random cases bit-identical to C++) |
| stdfx | stdfx/igs_perlin_noise.cpp | stdfx/igs_perlin_noise.rs | 6 | coarse (8/16-bit + float; carries a private bit-identical copy of `Noise1234::noise(x,y,z)`; `iwa_noise1234.cpp` stays C++ because its per-pixel callers are C++; 150 random images bit-identical) |
| flare | flare/metnum.cpp | flare/metnum.rs | 6 | coarse (Approx/Cholesky/CholForw/CholBack over `double**`; Alloc/Free stay inline in the header) |
| flare | flare/predict3d.cpp | flare/predict3d.rs | 4 | coarse (whole point arrays; uses `metnum` directly in Rust) |
| flarelib | flarelib/sandor_fxs/CIL.cpp | flarelib/sandor_fxs/CIL.rs | 4 | coarse (string -> sorted index list; `CCIL::set` forwards, `isIn` inline) |
| flarelib | flarelib/sandor_fxs/CallCircle.cpp | flarelib/sandor_fxs/CallCircle.rs | 4 | coarse (builds the sorted offset list; class, ctor, draw and templates inline in the header) |
| sound | sound/tsioutils.cpp | sound/tsioutils.rs | (earlier) | coarse |
| tnzbase | tnzbase/permissionsmanager.cpp | tnzbase/permissionsmanager.rs | (earlier) | fine |
| stdfx | stdfx/naru_maxflow.cpp | stdfx/naru_maxflow.rs | 6 | coarse (whole graph; opaque C-ABI handle, naru_graph.h forwards) |

Notes: the C++ `throw`s (TMathException, std::domain_error) stay in the inline header
forwarders; Rust returns a status code. Modules that call the forwarders link `flare_formats`
(tnzcore, tnzbase, stdfx, colorfx, tnztools, flarelib, Flare).

Batch 2 notes (13 `stdfx/igs_*` kernels, +104 tests, workspace 802 -> 906):
- Every entry point is a whole-buffer call (`extern "C"` in the Rust file, inline forwarder in the unchanged-signature header). Rust validates the raw arguments (null, negative size, overflow, misaligned 16-bit/float buffers, overlapping in/out for the median filters) and returns a status code; the header rethrows `std::domain_error` (the old texts) or `std::invalid_argument`. No `slice::from_raw_parts` on null; nothing panics across the boundary.
- `bits` for float images is `std::numeric_limits<float>::digits`, i.e. **24**, not 32 (`ino::bits()`); `igs_levels` and `igs_level_auto_in_camera` select float on 24.
- Quirks of the C++ kept on purpose (found by differential testing against the old code, 1670 bit-identical outputs over 8/16-bit/float, rgba/rgb/gray, with/without reference): `igs_level_auto*` builds table `n` (user R,G,B,A order) from the min/max of memory position `n` (B,G,R,A); grayscale `igs_levels` reads the blue table; float `igs_levels` clamps the reference weight only for RGBA; `igs_median_filter_smooth` interpolation branches collapse to "first sorted pixel whose accumulated weight reaches half" (its loop increment reads the next element); `igs_warp_*` interpolates `float*double + float*float`.
- Deliberate deviations (only where the C++ was undefined behaviour): integer `igs_levels` with `clamp_sw == false` saturates instead of wrapping on x86 (both callers pass `true`); `igs_level_auto_in_camera` clamps the camera rectangle to the image; hue wrap loops skip non-finite / |hue| >= 1e7 values instead of spinning.
- Rejected candidates: see "Rejected" in RUST_REWRITE.md (Ported so far, wave 4).

Batch 3 notes (`stdfx/hsvutil`, `stdfx/igs_perlin_noise`, +11 tests):
- Both verified against the old C++ with a differential harness (original .cpp from git linked next to `libflare_formats.a`): 2,000,000 random RGB/HSV cases and 150 random perlin images (8/16/float, 1/3/4 channels, wrap > width, with/without alpha rendering) gave 0 mismatches.
- `igs_perlin_noise` header forwarder throws `std::domain_error("Bad bits,Not uchar/ushort")` (status 1) or `std::invalid_argument` (status 2: null image, negative size, channels <= 0, misaligned 16-bit/float buffer).
- Deviations only where the C++ was UB: `OLDRGB2HSV` with NaN input returns hue 0 (C++ read an uninitialised value); null output pointers are skipped.
- Skipped: `perlinnoise.cpp` (TRaster/TSpectrumParam), `igs_attenuation_distribution`/`igs_motion_wind_table`/`igs_maxmin*` (already rejected, see RUST_REWRITE.md), `gammafx`/`posterizefx` and other `*fx.cpp` (TRasterFx classes, Qt/TRasterFxPort bound).

Batch 4 notes (`metnum`, `predict3d`, `CIL`, `CallCircle`, +18 tests):
- Differential test against the original C++ from git (old and new in separate TUs, new linked to `libflare_formats.a`): Approx 20,000, Cholesky 20,000, CholForw/CholBack 40,000 (incl. aliased b/x and zero diagonals), Predict3D 20,000 (incl. collinear), CCIL::set 200,000 random/edge strings, CCallCircle 400 radii: 0 mismatches (bitwise on doubles).
- Deviations only where the C++ was UB: CCIL tokens >= 100 chars overflowed a 100-byte buffer (Rust has no limit); `CCallCircle` with unusable radius (NaN, negative, > 5000, int overflow) yields an empty list.
- `CallCircle` order of equal-distance offsets depended on `qsort`; it is now defined: stable sort of row-major generation order (y then x), which matches glibc 2.39 (0 mismatches) but may differ from old MSVC/macOS (unstable qsort) tie order. Pinned by a radius-1 unit test. The 5000 radius cap is kept (CCallCircle radius comes from the user-set `CallParam::m_thickness`, so no static bound on callers).
- `CCIL` `atoi` now clamps to the `i32` range (old code truncated a saturated `long` to `int`); the old overflow was `strcpy` of the whole remaining list tail into a 100-byte buffer, so any list > 100 chars could overflow.
- Skipped: `CallParam.cpp` (atof argv parsing + embeds two `CCIL`; wrapper would exceed the logic, strtod parity not worth it), `BlurMatrix.cpp` (`rand()`-driven createRandom and `vector<vector<SXYD>>` class graph), `PatternPosition.cpp` (rand(), SPOINT/SRECT vectors, CSTColSelPic templates), `namebuilder.cpp` (class hierarchy with `std::wstring` virtual interface; Windows-only invalid-name list; wrapper bigger than the logic).
