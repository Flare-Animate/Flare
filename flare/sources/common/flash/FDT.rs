//! Port of `common/flash/FDT.cpp`.
//!
//! `FDT` is a low-level abstract base class for Flash definition tags; `FDT.cpp` contained
//! nothing but an empty implementation, because every derived class lives in its own
//! `FDTxxx.cpp`. There is consequently nothing to port here -- this module exists only so
//! the original source file has a Rust sibling (see `docs/PORTING.md`), and so that the
//! `#[path]` registry in `flare_formats.rs` stays exhaustive. The declaration of the
//! abstract class itself remains in `FDT.h` until the SWF reader (`common/flash/swf.rs`)
//! needs it.
