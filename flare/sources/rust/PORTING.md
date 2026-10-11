# C++ -> Rust porting status (C ABI, gated by FLARE_WITH_RUST)

Rust modules live beside the C++ they port, at `flare/sources/common/<subsystem>/rust/`.
Crate manifest: `flare/sources/Cargo.toml`. Tests: `python tests/run_rust.py`.

| C++ | Rust | Status |
|---|---|---|
| common/tsystem/tfilepath.cpp (standard mode: getType/getName/getLevelName/getDots/getFrame/getParentDir) | common/tsystem/rust/tfilepath.rs | partial, tested |
| common/tapptools/tenv.cpp (getSystemVarPathSetValue split) | common/tapptools/rust/tenv.rs | partial, tested |
| common/tapptools/tenv.cpp (rest: EnvGlobals/VariableSet/stuff dirs) | - | todo |
| common/flash (swf/xfl/xflrender/symlib/fcp/framecount) | common/flash/rust/*.rs | done |
| common/moho (Moho reader) | common/moho/rust/moho.rs | done |
| flare/sources/remote (remote control) | remote/rust/src/lib.rs | done |
| tfilepath non-standard mode (analyzePath), withName/withParentDir | - | todo |