# C++ to Rust auto-translation trial (cpp2rust, crust)

Date: 2026-10-15. Scratch dir /tmp/c2r (build artifacts deleted). Note: the
source tree is `flare/sources/` (no `toonz/sources` in this repo).

## Verdict
Neither tool is a usable drafting aid for this codebase today. Hand-port, or
LLM-assisted port with the C++ as spec and golden tests, remains the workflow.

## crust (NishanthSpShetty/crust v0.2.0)
- Builds in ~6 s (`cargo build --release`, no deps beyond getopts). Run: `crust FILE.cpp` -> `FILE.rs`.
- It is a token-level translator, not a real C++ parser. Tested on 3 self-contained inputs:
  1. `stdfx/hsvutil.cpp` (includes stripped): output generated but garbage. `std::max({r,g,b})`
     became `max = std ;`, `(max - min) / max` became `* s = ( ;`, `(int)hue` became a bogus
     `let mut hue : &mut i32`. rustc: unbalanced delimiters. Unusable.
  2. clamp/lerp (2 simple free functions): generated; closest to usable. Emits `if (v < lo) == true { lo }`
     with doc comments as attributes on expressions (E0658) and missing `return`, so 3 compile errors.
     Fixable by hand in a minute, but it is trivial code anyway.
  3. struct + `unsigned char*` loop + `std::string`/`std::vector` code: parser panics
     (index out of bounds in parser.rs). No output.
- Pointer params become `&mut`, templates/STL/casts unsupported. Do not use.

## cpp2rust (Cpp2Rust/cpp2rust, PLDI'26, clang-AST based, safe `Ptr<T>` runtime libcc2rs)
- Not built: blocked by disk, not by network. Requirements: LLVM/clang 23 (libclang-23-dev, clang++-23),
  cmake, ninja, rustup nightly-2026-10-01 with rustc-dev + llvm-tools (rule-preprocessor), a stable toolchain, ruff.
- apt.llvm.org noble-23 repo is reachable via proxy (clang 23.1.4). I downloaded and unpacked the
  LLVM/clang 23 dev packages (1.1 GB unpacked) into /tmp, which left ~3 GB free. The nightly rustc-dev
  toolchain (~1.5 GB+) plus a stable install plus the clang-cpp link would not fit in 4 GB. Stopped, deleted everything.
- Not tested: output quality, whether libcc2rs output compiles. Output is expected to be
  verbose (Ptr<T>/Rc<RefCell>-style) and not idiomatic, which matters for a render hot path.
- To retry: needs a machine with >=15 GB free (or CI). Steps: `apt install libclang-23-dev clang++-23 ninja-build cmake`,
  `pip install ruff`, `cmake -GNinja .. && ninja`, then `cpp2rust --file=x.cpp -o=x.rs`.
  Qt-free leaf files still need their headers resolvable (clang needs a compile DB / -I paths); expect
  failures on `tcommon`/trop macros, so feed it preprocessed, include-stripped extracts.

## Recommended workflow
1. Pick leaf pure-math functions (e.g. stdfx/hsvutil.cpp, gammafx, posterize, blend modes).
2. Hand-port (or LLM-port) to Rust, using the C++ as reference.
3. Differential-test against the C++ (cc crate or a small harness) on random inputs.
4. Re-evaluate cpp2rust on a CI runner with disk; compare its safe output only as an extra reference.

## Example before/after (best case: crust on trivial code)
C++:
    int clamp(int v, int lo, int hi) { if (v < lo) return lo; if (v > hi) return hi; return v; }
crust output (does not compile):
    fn clamp(v: i32, lo: i32, hi: i32) -> i32 { if (v < lo) == true { /** doc */ lo } if (v > hi) == true { /** doc */ hi } v }
Correct hand port:
    fn clamp(v: i32, lo: i32, hi: i32) -> i32 { v.clamp(lo, hi) }  // caveat: panics if lo > hi; use v.max(lo).min(hi) to match C++
Second example (hsvutil `*s = (max - min) / max;`): crust gave `* s = ( ;`. Hand port: return a (h,s,v) tuple.
