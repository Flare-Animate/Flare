//! A run: a description stepped for a number of frames under a script, with the events it
//! produced and the digest of the final state. Pure: the same inputs always give the same run.

use crate::limits::{MAX_RUN_EVENTS, MAX_RUN_FRAMES};
use crate::{Event, GameError, GameSpec, Input, World};

/// The result of a run.
#[derive(Debug, Clone, PartialEq)]
pub struct Playback {
    pub frames: u32,
    /// Each event with the 0-based frame it happened on, in order.
    pub events: Vec<(u32, Event)>,
    /// The digest of the world after the last frame.
    pub digest: u64,
}

/// Steps `spec` for `frames` frames at `frames_per_second`. Frame `i` takes `script[i]`, and
/// frames past the end of the script take no input. A script longer than the run is refused,
/// because the extra frames would be silently ignored.
pub fn play(
    spec: &GameSpec,
    frames_per_second: f64,
    frames: u32,
    script: &[Input],
) -> Result<Playback, GameError> {
    if frames > MAX_RUN_FRAMES {
        return Err(GameError::TooManyFrames);
    }
    if script.len() > frames as usize {
        return Err(GameError::BadInput(format!(
            "the script has {} frames but the run has {frames}",
            script.len()
        )));
    }
    let mut world = World::from_spec(spec, frames_per_second)?;
    let none = Input::none();
    let mut events = Vec::new();
    for frame in 0..frames {
        let input = script.get(frame as usize).unwrap_or(&none);
        for event in world.step(input)? {
            if events.len() >= MAX_RUN_EVENTS {
                return Err(GameError::TooManyEvents);
            }
            events.push((frame, event));
        }
    }
    Ok(Playback {
        frames,
        events,
        digest: world.digest(),
    })
}
