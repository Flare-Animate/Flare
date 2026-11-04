//! Toonz files as Flare projects (filled in by the Toonz conversion).

use std::path::Path;

use crate::xfl::Imported;

pub(crate) fn import(_name: &Path, _bytes: &[u8]) -> Result<Imported, String> {
    Err("Toonz import is not built yet".into())
}
