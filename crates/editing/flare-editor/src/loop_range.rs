//! Loop playback over part of the timeline, as Animate's loop range does: with Loop on, a
//! bracket on the ruler names the frames that play round and round, so one stretch of a long
//! scene can be watched while it is worked on. Without a range the whole scene loops.
//!
//! The range is a view setting kept with the open document; it is never saved or exported.

/// The frames a loop plays over, both ends included, counted from 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoopRange {
    from: u32,
    to: u32,
}

impl LoopRange {
    /// The frames between `a` and `b`, in either order.
    pub fn new(a: u32, b: u32) -> Self {
        Self {
            from: a.min(b),
            to: a.max(b),
        }
    }

    pub fn from(self) -> u32 {
        self.from
    }

    pub fn to(self) -> u32 {
        self.to
    }

    /// How many frames the loop holds.
    pub fn len(self) -> u32 {
        self.to - self.from + 1
    }

    /// A range always holds at least one frame.
    pub fn is_empty(self) -> bool {
        false
    }

    /// The range as it applies to a scene of `frames` frames: cut to the scene's last frame, or
    /// `None` when the scene has no frames or the range starts past its end. A scene that
    /// shrank under a range keeps whatever of the range is left.
    pub fn within(self, frames: u32) -> Option<Self> {
        let last = frames.checked_sub(1)?;
        (self.from <= last).then(|| Self::new(self.from, self.to.min(last)))
    }

    /// The range a drag of one end to `frame` makes, in a scene of `frames` frames. Dragging an
    /// end past the other swaps them, as a bracket's ends do. `None` when the range would be the
    /// whole scene, which is what no range means.
    pub fn dragged(current: Option<Self>, end: LoopEnd, frame: u32, frames: u32) -> Option<Self> {
        let last = frames.checked_sub(1)?;
        let current = current
            .and_then(|r| r.within(frames))
            .unwrap_or(Self::new(0, last));
        let frame = frame.min(last);
        let moved = match end {
            LoopEnd::Start => Self::new(frame, current.to),
            LoopEnd::End => Self::new(current.from, frame),
        };
        (moved != Self::new(0, last)).then_some(moved)
    }
}

/// An end of the loop range, as its handle on the ruler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopEnd {
    Start,
    End,
}

/// Where playback stands after `steps` more frames from `frame`, in a scene of `frames` frames,
/// and whether it is still playing.
///
/// Not looping, it stops on the scene's last frame. Looping, it goes round the loop range, or
/// the whole scene without one. A playhead before the range plays on into it; one after the
/// range goes to its start, as Animate's does when Play is pressed outside the bracket.
pub fn advance(
    frame: u32,
    steps: u64,
    frames: u32,
    looping: bool,
    range: Option<LoopRange>,
) -> (u32, bool) {
    let Some(last) = frames.checked_sub(1) else {
        return (0, looping);
    };
    if steps == 0 {
        return (frame.min(last), true);
    }
    let next = u64::from(frame) + steps;
    if !looping {
        return if next > u64::from(last) {
            (last, false)
        } else {
            (frame + steps as u32, true)
        };
    }
    let range = range
        .and_then(|r| r.within(frames))
        .unwrap_or(LoopRange::new(0, last));
    let (from, to, len) = (
        u64::from(range.from),
        u64::from(range.to),
        u64::from(range.len()),
    );
    // Past the range, the first step lands on its start.
    let at = if u64::from(frame) > to {
        from + steps - 1
    } else {
        next
    };
    let at = if at <= to {
        at
    } else {
        from + (at - from) % len
    };
    // `at` is at most `to`, a `u32`.
    (u32::try_from(at).unwrap_or(range.to), true)
}

#[cfg(test)]
#[path = "loop_range_tests.rs"]
mod tests;
