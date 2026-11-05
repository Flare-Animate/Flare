//! The audio system. The C++ scaffold's `playSound` was empty; the port reports each sound that
//! starts. Decoding and output belong to the surface, which plays the event.

use crate::system::StepContext;
use crate::{Audio, Component, ComponentKind, Event, GameError, World};

pub(crate) fn run(world: &mut World, ctx: &mut StepContext) -> Result<(), GameError> {
    let mut started = Vec::new();
    for (id, live) in world.entities.iter_mut() {
        let audio = match live.components.get(&ComponentKind::Audio) {
            Some(Component::Audio(audio)) => *audio,
            _ => continue,
        };
        if live.sounded {
            continue;
        }
        live.sounded = true;
        if let Some(event) = cue(*id, audio) {
            started.push(event);
        }
    }
    for event in started {
        ctx.emit(event)?;
    }
    Ok(())
}

fn cue(entity: crate::EntityId, audio: Audio) -> Option<Event> {
    if !audio.play_on_start {
        return None;
    }
    Some(Event::Sound {
        entity,
        sound: audio.sound?,
        volume: audio.volume,
        looping: audio.looping,
    })
}
