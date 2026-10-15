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

| Module | C++ retired | Rust file | Tests |
|---|---|---|---|
| tnzcore | common/tcore/tmathutil.cpp | common/tcore/tmathutil.rs | 8 |
| tnzbase | common/tparam/tcubicbezier.cpp | common/tparam/tcubicbezier.rs | 6 |
| tnzcore | common/tcore/trandom.cpp | common/tcore/trandom.rs | 7 |
| tnzcore | common/tcolor/tcolorvalue.cpp | common/tcolor/tcolorvalue.rs | 6 |
| tnzcore | common/trop/runsmap.cpp | common/trop/runsmap.rs | 4 |
| stdfx | stdfx/igs_color_rgb_hls.cpp | stdfx/igs_color_rgb_hls.rs | 6 |
| stdfx | stdfx/igs_color_rgb_hsv.cpp | stdfx/igs_color_rgb_hsv.rs | 6 |
| stdfx | stdfx/igs_math_random.cpp | stdfx/igs_math_random.rs | 4 |
| stdfx | stdfx/igs_gauss_distribution.cpp (was `#include`d as source; new igs_gauss_distribution.h) | stdfx/igs_gauss_distribution.rs | 4 |
| stdfx | stdfx/igs_negate.cpp | stdfx/igs_negate.rs | 7 |
| stdfx | stdfx/igs_density.cpp | stdfx/igs_density.rs | 4 |
| sound | sound/tsioutils.cpp | sound/tsioutils.rs | (earlier) |
| tnzbase | tnzbase/permissionsmanager.cpp | tnzbase/permissionsmanager.rs | (earlier) |

Notes: the C++ `throw`s (TMathException, std::domain_error) stay in the inline header
forwarders; Rust returns a status code. Modules that call the forwarders link `flare_formats`
(tnzcore, tnzbase, stdfx, colorfx, tnztools, flarelib, Flare).
