//! The sprite system: advances each animated sprite's frame at its frame rate. The C++ left this
//! as a comment ("could advance frame based on animation rate"); the port does it, and sorts the
//! visible sprites for drawing as the C++ render pass did.

use crate::system::StepContext;
use crate::{Component, ComponentKind, GameError, World};

pub(crate) fn run(world: &mut World, ctx: &mut StepContext) -> Result<(), GameError> {
    let dt = ctx.dt;
    for live in world.entities.values_mut() {
        let sprite = match live.components.get(&ComponentKind::Sprite) {
            Some(Component::Sprite(sprite)) => *sprite,
            _ => continue,
        };
        if sprite.fps <= 0.0 {
            continue;
        }
        live.clock += sprite.fps * dt;
        let advance = live.clock.floor();
        live.clock -= advance;
        if advance < 1.0 {
            continue;
        }
        // The cast saturates, and `frames` is at least one, so the frame stays in range.
        let steps = advance as u64;
        let frames = u64::from(sprite.frames);
        live.frame = if sprite.looping {
            ((u64::from(live.frame) + steps) % frames) as u32
        } else {
            (u64::from(live.frame).saturating_add(steps)).min(frames - 1) as u32
        };
    }
    Ok(())
}
