//! Experimental features. **Unstable: APIs here may change or vanish without notice, and none of
//! it is wired into the document model, the command batches, the project file or any front end.**
//!
//! A feature lives here while its design is being proven. When one is promoted it moves to the
//! layer that owns it (and gets its surfaces, see `docs/SURFACES.md`).
//!
//! - [`viewspace`]: ViewSpace Keys, multi-view 2D interpolation (see `docs/experimental/VIEWSPACE.md`).

pub mod viewspace;
