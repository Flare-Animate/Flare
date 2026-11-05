//! Edits of the game description. A game edit is a step of its batch like any other: a refused
//! one fails the batch, and the batch is one undo step.

use flare_game::GameSpec;

use crate::{Command, EditError};

pub(crate) fn step(game: &mut Option<GameSpec>, command: Command) -> Result<(), EditError> {
    let spec = game.get_or_insert_with(GameSpec::new);
    match command {
        Command::AddGameEntity(entity) => spec.add_entity(entity)?,
        Command::RemoveGameEntity(id) => {
            spec.remove_entity(id)?;
        }
        Command::SetGameComponent { entity, component } => spec.set_component(entity, component)?,
        Command::RemoveGameComponent { entity, kind } => spec.remove_component(entity, kind)?,
        Command::SetGameGravity { x, y } => *spec = spec.clone().with_gravity(x, y)?,
        _ => return Err(EditError::NotAGameEdit),
    }
    Ok(())
}
