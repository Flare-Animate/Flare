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
| flarelib | flarelib/sandor_fxs/PatternPosition.cpp | flarelib/sandor_fxs/PatternPosition.rs | 5 + 272-case golden | coarse (random + distance-driven stamp positions; `rand()` injected, positions returned as a Rust-owned block freed by `flare_pattern_position_free`; template ctor stays in the header) |
| flarelib | flarelib/sandor_fxs/SDirection.cpp | flarelib/sandor_fxs/SDirection.rs | 5 + 840-case golden | coarse (three in-place passes over the byte map: prepare, doDir, doRadius; class and `m_dir` stay in the header) |
| flarelib | flarelib/sandor_fxs/PatternMapParam.cpp | flarelib/sandor_fxs/PatternMapParam.rs | 4 + 400-case golden | coarse (argv 1..11 parsing via libc `atof`/`atoi` into a `repr(C)` struct; ink list stays `CCIL::set`) |
| sound | sound/tsioutils.cpp | sound/tsioutils.rs | (earlier) | coarse |
| tnzbase | tnzbase/permissionsmanager.cpp | tnzbase/permissionsmanager.rs | (earlier) | fine |
| stdfx | stdfx/naru_maxflow.cpp | stdfx/naru_maxflow.rs | 6 | coarse (whole graph; opaque C-ABI handle, naru_graph.h forwards) |
| flarelib | flarelib/skeletonlut.cpp | flarelib/skeletonlut.rs | 3 | data (8 constant LUTs exported as C symbols; `skeletonlut.h` rebinds `SkeletonLut::*` as array references; hashes pinned to the original) |
| flarelib | flarelib/iknode.cpp | flarelib/iknode.rs | 7 | coarse (`IKNode::computeS` ancestor chain + `rotatePoint`; class stays in `iknode.h`; bit-identical to a literal transcription of the C++ walk) |
| tnzcore | common/tvectorimage/cornerdetector.cpp | common/tvectorimage/cornerdetector.rs | 4 + 40 recorded C++ cases | coarse (polyline -> corner indexes; globals became locals; header repeats the adjacent-dup pass) |
| tnzext | tnzext/Types.cpp | tnzext/Types.rs | 1 | fine (odd/even bit test; the `range_error` throws stay inline in ext/Types.h) |
| flarefarm | flarefarm/tfarmclient/casmfileinfo.cpp | flarefarm/tfarmclient/casmfileinfo.rs | 3 + 63 recorded C++ cases | coarse (file -> frame range; tfarmclient has no CMake target yet) |
| tnzcore | common/psdlib/psdutils.cpp | common/psdlib/psdutils.rs | 7 + 1,603 recorded C++ cases | coarse, pure parts only (PackBits `unpackrow`, zlib inflate into a fixed buffer, 8/16-bit predictor undo; goldens from the original rebuilt with g++ + zlib by `tests/native/gen_psdutils_cases.py --check`). `mymalloc`/`mycalloc` (callers `free()` them, so the allocator must stay in the caller's C runtime), `readrow`, `skipBlock`, `read2UBytes`/`read2Bytes`/`read4Bytes` stay C++ as inline functions in `psdutils.h`: they are bare `fseek`/`fread`/`fgetc` on a caller-owned `FILE*`. UB-only deviations and the one zlib-vs-miniz difference (corrupt back-reference exactly where the output buffer runs out) are listed in the .rs header |

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

Batch 5 notes (`stdfx/igs_motion_wind_table`, `stdfx/igs_attenuation_distribution`, `stdfx/igs_motion_wind_pixel`, +14 tests):
- The two files wave 4 rejected as "not worth bridging" are ported after all: `igs_attenuation_distribution` now returns flat `f64`/`i32` arrays allocated by Rust (`flare_igs_attenuation_free` frees them) and `igs_attenuation_distribution.h` rebuilds the old `vector<vector<double>>` + `vector<double*>` API inline, so `igs_fog.cpp` only changed its `#include` from `.cpp` to `.h`.
- `igs_motion_wind_pixel` is a stateful class, so `igs_motion_wind_pixel.h` keeps the original constructor / `change()` / `clear()` API as an inline class over an opaque Rust handle (`flare_igs_motion_wind_pixel_new` / `_change` / `_clear` / `_free`). `igs_motion_wind.cpp` still owns the scanline walk; only the per-pixel decision moved to Rust.
- `igs_motion_wind_table.h` was deleted: `table_size` / `make_table` are now an internal helper of the pixel port (`make_table_inplace`), with no remaining C++ caller. `igs_motion_wind.cpp` now includes `igs_ifx_common.h` directly (the old pixel header used to supply it transitively).
- Deviations only where the C++ was UB or would throw: `make_table_inplace` returns 0 instead of `std::out_of_range` when `ceil(length) > table.size()`; `pixel::clear` resets the table length/position instead of `table_.clear()` (the C++ left `table_array_` dangling, and `clear()` only ran immediately before destruction); every FFI entry point is panic-guarded through `crate::guard` (0 / NULL on panic).
- Build note: the crate's staticlib is linked into `libtnzcore.dylib`, so `libtnzstdfx.dylib` carries `U _flare_igs_*` and resolves them at load. Also `-DOpenCV_DIR="$(brew --prefix opencv@4)/lib/cmake/opencv4"` is required on macOS: a stale `build/` configured against OpenCV 5 fails to compile `iwa_bloomfx.cpp` (`cv::getRotationMatrix2D`).
- Differential parity for batch 5 (goldens committed, replayed by Rust tests — see `docs/sprints/SPRINT-2.md`): `igs_attenuation_cases.txt` (62 cases, structural fields exact) and `igs_motion_wind_cases.txt` (3 configs × 400 pixels). The harness found three real defects that unit tests had missed: `lens_starts` pointed at the row start instead of the first covered pixel; `rgb_to_lightness` had been normalised to `(max+min)/2` although the original is channel-order dependent; and the port assumed r,g,b while the tree is Toonz6 (`blu=0, gre, red, alp`). `tests/native/msvc_reference.build_and_run_host` builds the reference on POSIX with `-ffp-contract=off` so clang FMA contraction does not record a 1-ULP artifact.

- Both C++ bridges are **transitional**: `igs_attenuation_distribution.h` + the two
  `flare_igs_attenuation_*` exports exist only because `igs_fog.cpp` is still C++, and
  `igs_motion_wind_pixel.h` + the four `flare_igs_motion_wind_pixel_*` exports exist only
  because `igs_motion_wind.cpp` is. Each side carries a `TRANSITIONAL FFI` marker naming
  the deletion set; when those two callers are ported, remove both halves together (and
  `tests/native/igs_attenuation_wrapper_probe.cpp`). The `*_cases.txt` goldens stay — they
  keep pinning the Rust against the archived C++ (same as `igs_color_blend`).

### Build notes (macOS, Ninja + Corrosion)

- Pin OpenCV 4: `-DOpenCV_DIR="$(brew --prefix opencv@4)/lib/cmake/opencv4"`. A `build/`
  cached against OpenCV 5 fails in `iwa_bloomfx.cpp` (`cv::getRotationMatrix2D` is gone in 5.x).
- **Corrosion's byproduct copy preserves mtime, so a Rust-only edit can leave the C++
  binaries stale while `ninja` still exits 0.** After touching a `.rs` file, `ninja` runs
  `cargo rustc`, copies `libflare_formats.a`, then prunes the dependent link edges because
  the archive's timestamp looks unchanged; `libtnzcore`/`libtnzstdfx`/`Flare` stay one edit
  behind. Force it with `touch build/libflare_formats.a && ninja -C build -j2`, and confirm
  the artifacts' mtimes are newer than the `.rs` you changed before trusting the build.
- Verify the FFI reached the shipping binary rather than assuming: the entry points are
  `U` (undefined) in `libtnzstdfx.dylib` and `T` in `libtnzcore.dylib` — the crate's
  staticlib links into tnzcore, and tnzstdfx resolves at load.
- `igs_math_random` follows the C++ ABI: `unsigned long` is 64-bit on LP64 (macOS, Linux)
  and 32-bit on LLP64 (Windows), which changes both the wrap modulus and the `next_d`
  denominator. There are therefore two motion-wind goldens, both recorded on a POSIX host:
  `igs_motion_wind_cases.txt` (`--rng-bits 64`) and `igs_motion_wind_cases_32.txt`
  (`--rng-bits 32`). The replay picks one on `size_of::<c_ulong>()`, not on
  `cfg(target_os)`, so i686 Linux is covered too. Do not "fix" the mismatch by widening the
  RNG to `u64` — that would diverge from what the Windows C++ original actually did.
  `gen_igs_motion_wind_cases.py` builds the reference against a generated
  `igs_math_random.h` stub of the requested width, so it needs no `libflare_formats.a` and
  both goldens regenerate on any POSIX host. The 32-bit stream is not a guess: it reruns
  the original recurrence with a 32-bit word, and it differs from the 64-bit golden in 724
  of 1,206 lines, so a Windows run is testing something real.
- Golden drift is gated. `tests/run_stdfx_goldens.py` (a suite in `tests/run_all.py`)
  re-derives every golden from the archived C++ with `--check` and fails on any byte of
  difference, so a hand-edited golden cannot turn a red test green. It skips (exit 0)
  without a host C++ compiler. In CI it runs only on macOS, because the recorded bytes
  include that machine's libm; the port's own tests are what execute on all three
  platforms (`.github/workflows/rust-tests.yml`, `cargo test --lib` on ubuntu, macos and
  windows).
- The attenuation FFI guards the matrix size: anything above `MAX_MATRIX_BYTES` (1 GiB,
  ~11.5k px, radius ~5.8k), and any diameter that does not fit the C ABI's `c_int`, is
  refused with `ERR_TOO_LARGE` (-5) *before* allocating. That is a deliberate divergence
  from the original, which would attempt the allocation; it is compatible because
  `igs_fog.cpp` never asks for a lens near that size, and it converts a process abort into
  a mapped `std::invalid_argument`. `-4` now means only "malloc failed" (`std::bad_alloc`).

Batch 6 notes (`PatternPosition`, `SDirection`, `PatternMapParam`, +14 tests, 1,512 golden cases):
- Differential goldens come from the original C++ rebuilt from git with the host compiler (`tests/native/gen_sandor_fxs_cases.py`, `--check` detects drift). They carry their own inputs (masks, argument lists). `PatternPosition` is platform independent: the reference TU redirects `rand()` to a fixed LCG that the Rust test replays, and `RAND_MAX` is an argument (2147483647 in the golden). `PatternMapParam` uses the host `atof`/`atoi` on both sides, so its golden is exact against glibc only; `0x..`/`inf`/`nan` tokens are the ones another libc may parse differently. The full chain (new inline headers + `libflare_formats.a`) was also run against the same harnesses and reproduces the goldens byte for byte.
- **`SDirection::equalizeDir` only ever writes `m_dir[0]`** (its output pointer is never advanced), so `doDir` returns the raw `makeDir` angles plus one overwrite. Almost certainly an upstream bug, preserved on purpose because fixing it changes every render; `do_dir` evaluates only the pixel whose value survives. Direction sums are `short` and wrap, as before.
- Deviations only where the C++ was UB or could not finish: `SDirection` no longer materialises the `(2*sens+1)^2` filter (computed on the fly; `sens > 4096` refused as the old out-of-memory, negative `sens` acts as 0) and clips its box scans to the image instead of looping `dBlur^2` times; `PatternPosition` returns an error (header: `SMemAllocError`) for a distance disc that cannot clear its own centre (NaN radius or radius <= -2, detected when a stamp fails to clear its own pixel, where the C++ stamped one pixel until `bad_alloc`) and for radii above 1024 cells; `lX*lY` overflow is refused; `PatternMapParam` returns -1 on a null argv entry (the C++ crashed) and the header keeps the `null()` defaults.
- Header changes: dead private helpers of `CPatternPosition` (`getPosAroundThis`, `findEmptyPos`, ... all internal to the algorithm) and `CSDirection`'s `m_df`/`m_lDf` are gone; `CSDirection` gained `m_sens`. `SDirection.h` now includes `<algorithm>` (the deleted `.cpp` was where callers got it transitively).
- Skipped: `Params.cpp` and `SError.cpp` are one-line includes of header-only templates/classes (`CParams<T>`, the `SError` hierarchy); there is no non-header code to port, so a Rust file would be a stub. Both stay in `flarelib/CMakeLists.txt`.
