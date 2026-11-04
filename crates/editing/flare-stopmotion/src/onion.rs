use crate::error::StopMotionError;

/// Most earlier or later frames the onion skin may show each way.
pub const MAX_ONION_FRAMES: u8 = 5;
/// Highest ghost opacity, in percent.
pub const MAX_ONION_OPACITY: u8 = 100;

const DEFAULT_OPACITY: u8 = 50;

/// How many earlier and later frames are drawn faintly under the live frame, and how faint.
/// Off by default: nothing before or after is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OnionSkin {
    before: u8,
    after: u8,
    opacity: u8,
}

impl Default for OnionSkin {
    fn default() -> Self {
        Self {
            before: 0,
            after: 0,
            opacity: DEFAULT_OPACITY,
        }
    }
}

impl OnionSkin {
    pub fn new(before: u8, after: u8, opacity: u8) -> Result<Self, StopMotionError> {
        if before > MAX_ONION_FRAMES {
            return Err(StopMotionError::OnionFrames {
                got: before,
                max: MAX_ONION_FRAMES,
            });
        }
        if after > MAX_ONION_FRAMES {
            return Err(StopMotionError::OnionFrames {
                got: after,
                max: MAX_ONION_FRAMES,
            });
        }
        if opacity > MAX_ONION_OPACITY {
            return Err(StopMotionError::OnionOpacity {
                got: opacity,
                max: MAX_ONION_OPACITY,
            });
        }
        Ok(Self {
            before,
            after,
            opacity,
        })
    }

    pub fn before(&self) -> u8 {
        self.before
    }

    pub fn after(&self) -> u8 {
        self.after
    }

    pub fn opacity(&self) -> u8 {
        self.opacity
    }

    /// True when some frames are shown and they are not invisible.
    pub fn is_enabled(&self) -> bool {
        (self.before > 0 || self.after > 0) && self.opacity > 0
    }
}

/// The positions of the frames drawn as ghosts while the frame at `current` is live: up to
/// `before` earlier ones, nearest first, then up to `after` later ones, nearest first. Stops at
/// the ends of the session, so no position is returned twice or out of range.
pub fn ghost_frames(
    onion: &OnionSkin,
    len: usize,
    current: usize,
) -> Result<Vec<usize>, StopMotionError> {
    if current >= len {
        return Err(StopMotionError::IndexOutOfRange {
            index: current,
            len,
        });
    }
    if !onion.is_enabled() {
        return Ok(Vec::new());
    }
    let earlier = usize::from(onion.before).min(current);
    let later = usize::from(onion.after).min(len - 1 - current);
    let mut ghosts: Vec<usize> = (1..=earlier).map(|step| current - step).collect();
    ghosts.extend((1..=later).map(|step| current + step));
    Ok(ghosts)
}
