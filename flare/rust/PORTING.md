# C++ -> Rust porting status (flare/rust, C ABI, gated by FLARE_WITH_RUST)

| C++ | Rust | Status |
|---|---|---|
| common/tsystem/tfilepath.cpp (standard mode: getType/getName/getLevelName/getDots/getFrame/getParentDir) | flare_formats/src/tfilepath.rs | partial, tested |
| common/tapptools/tenv.cpp (getSystemVarPathSetValue split) | flare_formats/src/tenv.rs | partial, tested |
| common/tapptools/tenv.cpp (rest: EnvGlobals/VariableSet/stuff dirs) | - | todo |
| xfl/swf/moho/fcp/symlib/xflrender (Flare formats) | flare_formats/src/{xfl,swf,moho,fcp,symlib,xflrender}.rs | done |
| tfilepath non-standard mode (analyzePath), withName/withParentDir | - | todo |
