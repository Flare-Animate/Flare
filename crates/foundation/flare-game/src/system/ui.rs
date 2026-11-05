//! The UI system: a press inside an interactive element is a click on it. This is the C++ hit
//! test (`rect.contains(pos)`) that the scaffold left as a comment.

use crate::system::StepContext;
use crate::{Component, ComponentKind, Event, GameError, World};

pub(crate) fn run(world: &mut World, ctx: &mut StepContext) -> Result<(), GameError> {
    if !ctx.input.pressed() {
        return Ok(());
    }
    let Some((px, py)) = ctx.input.pointer() else {
        return Ok(());
    };
    let mut clicks = Vec::new();
    for (id, live) in &world.entities {
        let (Some(Component::Ui(ui)), Some(Component::Transform(t))) = (
            live.components.get(&ComponentKind::Ui),
            live.components.get(&ComponentKind::Transform),
        ) else {
            continue;
        };
        if !ui.interactive {
            continue;
        }
        if px >= t.x && px <= t.x + ui.width && py >= t.y && py <= t.y + ui.height {
            clicks.push(Event::Click {
                entity: *id,
                x: px,
                y: py,
            });
        }
    }
    for event in clicks {
        ctx.emit(event)?;
    }
    Ok(())
}
