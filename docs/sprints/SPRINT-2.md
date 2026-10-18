# Sprint 2 (2026-10-17): Rust port batch 5 — motion wind + attenuation

Goal: continue the Rust rewrite with FX kernel ports, following the Sprint 1 DoD.

## Backlog
| # | Item | State |
|---|---|---|
| 1 | `igs_motion_wind_table.cpp` port | ✅ 3 tests |
| 2 | `igs_attenuation_distribution.cpp` port | ✅ 5 tests |
| 3 | `igs_motion_wind_pixel.cpp` port | ✅ 4 tests |
| 4 | Module registration (`flare_formats.rs`) | ✅ 3 `#[path]` entries |
| 5 | C++ bridge (headers, CMake) | ✅ |
| 6 | Full Flare build on macOS | ✅ `ninja` exit 0, app launches |
| 7 | Docs (PORTING.md, RUST_REWRITE.md, this file) | ✅ |

## Ports

**`stdfx/igs_motion_wind_table.rs`** — `table_size()` + `make_table_inplace()` (gamma/bias decay table). No C++ caller remains, so `igs_motion_wind_table.h` was deleted and the `.cpp` retired; the table is now internal to the pixel port.

**`stdfx/igs_attenuation_distribution.rs`** — `attenuation_distribution()` (circle / N-gon conical mask, normalized to sum 1). C ABI `flare_igs_attenuation_distribution` marshals a flat `f64` matrix + `i32` offsets/sizes allocated by Rust; `flare_igs_attenuation_free` releases them. `igs_attenuation_distribution.h` rebuilds the original `vector<vector<double>>` + `vector<double*>` signature inline, so `igs_fog.cpp` only changed its include from `.cpp` to `.h`.

**`stdfx/igs_motion_wind_pixel.rs`** — stateful per-pixel wind trail (`MotionWindPixel`). The original API is a C++ class, so `igs_motion_wind_pixel.h` is now an inline class over an opaque Rust handle: `flare_igs_motion_wind_pixel_{new,change,clear,free}`. `igs_motion_wind.cpp` still walks the scanlines; only the per-pixel decision moved to Rust.

## Deviations (only where C++ was UB / would throw)
- `make_table_inplace` returns 0 instead of `std::out_of_range` when `ceil(length) > table.len()`.
- `pixel::clear` resets table length/position instead of `table_.clear()` (the C++ left `table_array_` dangling; `clear()` only ran immediately before destruction).
- Every FFI entry point is panic-guarded with `crate::guard` (0 / NULL on panic).

## Build integration (what actually broke the build)
The ports alone did not build; four separate faults, in order:
1. `stdfx/CMakeLists.txt` still listed the three deleted `.cpp` files and the deleted table header → CMake "Cannot find source file".
2. `igs_fog.cpp` did `#include "igs_attenuation_distribution.cpp"` → switched to the new header.
3. `igs_motion_wind.cpp` also references the C++ `pixel` class (ctor / `change` / `clear`) → undefined symbols at link; fixed by the handle-based header forwarder.
4. The old pixel header had been supplying `igs_ifx_common.h` transitively (`igs::image::rgba`, `igs::color::ref_value`) → added that include directly in `igs_motion_wind.cpp`.

Environment fault, unrelated to the ports: macOS must pin `-DOpenCV_DIR="$(brew --prefix opencv@4)/lib/cmake/opencv4"`. A stale `build/` configured against OpenCV 5 fails on `iwa_bloomfx.cpp` (`cv::getRotationMatrix2D` is gone in 5.x).

## Verification
```
cargo test --manifest-path flare/sources/Cargo.toml --lib   # 294 passed, 0 failed
ninja -C build -j2                                          # exit 0
python3 scripts/atlas_check.py                              # 0 hard, 0 soft
```
- `Flare.app/Contents/MacOS/Flare` (11 MB) built; binary newer than every touched source.
- `nm -g libtnzcore.dylib` shows `T _flare_igs_motion_wind_pixel_{new,change,clear,free}` and `T _flare_igs_attenuation_{distribution,free}`. `libtnzstdfx.dylib` carries them as `U` and resolves at load — expected, the Rust staticlib links into `libtnzcore`.
- Smoke test: app launched, stayed alive 14 s, no crash report, empty log.

## Differential verification (Sprint 1 DoD #3)
Goldens captured from the archived C++ (`git show <pre-deletion>:…`), replayed by Rust
tests — the same pattern as `igs_color_blend` / `BlurMatrix`:

| Port | Harness | Golden | Compare |
|---|---|---|---|
| `igs_attenuation_distribution` | `tests/native/igs_attenuation_ref.cpp` + `gen_igs_attenuation_cases.py` | `igs_attenuation_cases.txt` (62 cases) | diameter/offset/size/`lens_starts` exact, matrix ≤1e-12 |
| `igs_motion_wind_pixel` (+ table) | `tests/native/igs_motion_wind_ref.cpp` + `gen_igs_motion_wind_cases.py` | `igs_motion_wind_cases.txt` (3 cfg × 400 px) | `inside` exact, channels bit-exact (2 cfg) / ≤1e-12 (gamma cfg) |

`msvc_reference.build_and_run_host` was added so the generators also run on POSIX
(`-ffp-contract=off`: without it clang fuses mul+add into an FMA and the reference
records a 1-ULP artifact that Rust never had).

### Bugs the harness caught (all shipped silently by a green build)
1. **`lens_starts`** pointed at the row start, not the first covered pixel. `igs_fog.cpp`
   feeds it straight into the per-row lens loop with `lens_sizes`, so every sample was
   offset — a wrong render, not a crash. Now `matrix[y].data() + offset`, NULL when the
   row is empty.
2. **`rgb_to_lightness`** was "cleaned up" to `(max+min)/2`. The original is
   channel-order dependent and the `re < gr` branch divides only `max(gr, bl)` by two.
   Different `inside` decisions from step 4 onward.
3. **Channel order** assumed r,g,b; the tree is Toonz6 — `enum num { blu = 0, gre, red, alp }`
   (`igs_ifx_common.h:23`). `get_lightness` now reads `pixel[RED/GRE/BLU]`.

## Open
- `ninja` re-runs the Corrosion byproduct copy on every invocation (`[2/19] Copying byproducts libflare_formats.a`) — exits 0, pre-existing churn, not a failure.
- Goldens were recorded with Apple clang/libm; MSVC's CRT may differ in the last bits for
  `sqrt`/`pow`/`cos`/`sin`, hence the 1e-12 tolerance on matrix/channel values. Structural
  fields and the two arithmetic-only pixel configs are compared bit-exactly.
- Follow-up (not this commit): `BLU/GRE/RED` are local constants in `igs_motion_wind_pixel.rs`.
  Promoting them (or a shared `igs::image` channel-order helper) would stop the next port
  from re-inventing the wrong order. Audited the other ref-taking ports for the same defect:
  `igs_levels.rs` already indexes through `RED/GRE/BLU`, `igs_color_blend`/`median_filter*`
  take r,g,b as parameters, `igs_hls_*`/`igs_hsv_*` use memory position 0 deliberately.
- Not pinned by a test: the new FFI error paths (`-3` null out-parameter, `-4` allocation
  failure) are verified by inspection only. `igs_fog.cpp` never passes NULL and the
  diameters are small, so neither branch is reachable from the shipping call site, but the
  all-or-nothing publish contract should get a direct unit test before it is relied on.
- Separate defect found while running the gate, not touched here: `tests/check_issue_fixes.py:154`
  unpacks 3 values from a 4-tuple (`for num, title, desc, detail in open_items`), so the whole
  "issue map" suite dies with `ValueError` before checking anything. Two-line fix, own commit.
- `tests/native/igs_attenuation_wrapper_probe.cpp` exists because the golden encodes where the
  *original* `lens_starts` points, but the line that was actually fixed lives in the C++ wrapper —
  nothing in CI executes it. The probe asserts that contract (null rows, in-bounds sizes,
  normalisation, exception mapping); it is manual, not a suite target.
- The three `u64::from` calls sit behind `#[allow(clippy::useless_conversion)]` on purpose:
  `c_ulong` is 64-bit here and 32-bit on Windows, so removing them "because clippy says so"
  would break the Windows build.
- `crate::guard`'s panic default for the attenuation FFI is `ERR_ALLOC`, which the header
  reports as `std::bad_alloc`. Unreachable from `igs_fog.cpp` today, but a panic would be
  mislabelled as an allocation failure; give it a distinct status if that path ever matters.
- Follow-up: nothing in CI runs `gen_igs_*_cases.py --check`, so a hand-edited golden would
  make a red test green with no gate noticing. Wire both generators into `tests/run_all.py`,
  skipping cleanly when no host C++ compiler is present.
- Follow-up (not this commit): the `TRANSITIONAL FFI` marker was applied only to the three
  batch-5 modules. `metnum.rs`, `predict3d.rs`, `CIL.rs`, `CallCircle.rs`, `BlurMatrix.rs` and
  the wave-3/4 stdfx kernels have the same shape — a C ABI export plus an inline forwarder
  header that only exists because the caller is still C++. Sweep them in their own commit so
  this diff stays reviewable: `rg 'TRANSITIONAL FFI'` finds the ones already marked.
