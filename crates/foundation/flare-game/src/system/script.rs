//! The script system. The C++ scaffold ran a script per entity through an engine it never
//! started, and the port keeps scripts as data (`docs/PARITY.md`), so this system does nothing.
//! It keeps its place in the order so a script runner can be added without reordering the rest.

use crate::system::StepContext;
use crate::{GameError, World};

pub(crate) fn run(_world: &mut World, _ctx: &mut StepContext) -> Result<(), GameError> {
    Ok(())
}
