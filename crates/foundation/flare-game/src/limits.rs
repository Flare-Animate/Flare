//! Bounds on everything a game may grow to. Each one is checked where the value enters, so a
//! description or a script cannot make a step arbitrarily large.

/// Most entities in one game.
pub const MAX_ENTITIES: usize = 4096;
/// Most entities with a body; the collision pass compares pairs, so this is the cost bound.
pub const MAX_BODIES: usize = 1024;
/// Longest entity name or script name, in characters.
pub const MAX_NAME_CHARS: usize = 128;
/// Longest text on a UI element, in characters.
pub const MAX_TEXT_CHARS: usize = 256;
/// Most frames in one sprite animation.
pub const MAX_SPRITE_FRAMES: u32 = 4096;
/// Fastest animation or step rate, in frames per second.
pub const MAX_FPS: f64 = 240.0;
/// Largest absolute coordinate, in pixels. Positions that leave this range are refused.
pub const MAX_COORD: f64 = 1.0e6;
/// Largest size of a body, a UI element or a viewport, in pixels.
pub const MAX_SIZE: f64 = 16_384.0;
/// Fastest a controlled body may move, in pixels per second.
pub const MAX_SPEED: f64 = 10_000.0;
/// Most particles a particle component may emit per second.
pub const MAX_EMIT_RATE: f64 = 1000.0;
/// Most frames a single run may step.
pub const MAX_RUN_FRAMES: u32 = 100_000;
/// Most events one step may produce.
pub const MAX_EVENTS_PER_STEP: usize = 4096;
/// Most events one run may keep.
pub const MAX_RUN_EVENTS: usize = 100_000;
/// Most keys or actions named in one frame of input.
pub const MAX_INPUT_TOKENS: usize = 16;
