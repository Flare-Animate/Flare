//! BVH motion files, the Biovision Hierarchy text format mocap tools share: a joint hierarchy
//! with offsets and channels, then the motion as one row of values per frame. This is how a
//! rig's bones and their motion leave for other tools.
//!
//! Supported: ROOT and JOINT with OFFSET and CHANNELS, End Site, MOTION with Frames and Frame
//! Time. Any other keyword is refused rather than skipped, so nothing is dropped silently.

mod import;
mod read;
mod write;

pub use import::{MAX_POSES, to_document};
pub use read::parse;
pub use write::to_text;

use thiserror::Error;

/// Largest BVH text read.
pub const MAX_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_JOINTS: usize = 1_024;
/// Deepest joint nesting, the root counted as level 1.
pub const MAX_DEPTH: usize = 64;
pub const MAX_FRAMES: usize = 1_000_000;
/// Values in the motion block (frames times channels).
pub const MAX_MOTION_VALUES: usize = 4_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
    Z,
}

/// One motion channel of a joint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motion {
    Position(Axis),
    Rotation(Axis),
}

impl Motion {
    /// The name the file uses, such as `Xposition`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Position(Axis::X) => "Xposition",
            Self::Position(Axis::Y) => "Yposition",
            Self::Position(Axis::Z) => "Zposition",
            Self::Rotation(Axis::X) => "Xrotation",
            Self::Rotation(Axis::Y) => "Yrotation",
            Self::Rotation(Axis::Z) => "Zrotation",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        const ALL: [Motion; 6] = [
            Motion::Position(Axis::X),
            Motion::Position(Axis::Y),
            Motion::Position(Axis::Z),
            Motion::Rotation(Axis::X),
            Motion::Rotation(Axis::Y),
            Motion::Rotation(Axis::Z),
        ];
        ALL.into_iter().find(|m| m.name() == name)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Joint {
    pub name: String,
    /// Index of the parent joint; `None` only for the root, which is joint 0.
    pub parent: Option<usize>,
    pub offset: [f64; 3],
    /// Channels in the order their values appear in each motion row.
    pub channels: Vec<Motion>,
    /// The End Site's offset, when the joint has one.
    pub end_site: Option<[f64; 3]>,
}

/// A BVH file. `motion` is row-major: value `c` of frame `f` is `motion[f * channel_count() + c]`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Bvh {
    pub joints: Vec<Joint>,
    pub frame_time: f64,
    pub frame_count: usize,
    pub motion: Vec<f64>,
}

impl Bvh {
    /// Values per motion row: the channels of every joint.
    pub fn channel_count(&self) -> usize {
        self.joints.iter().map(|j| j.channels.len()).sum()
    }

    pub fn value(&self, frame: usize, channel: usize) -> Option<f64> {
        let width = self.channel_count();
        if channel >= width || frame >= self.frame_count {
            return None;
        }
        self.motion.get(frame * width + channel).copied()
    }

    /// Checks the invariants that both reading and writing rely on.
    pub fn validate(&self) -> Result<(), BvhError> {
        if self.joints.is_empty() {
            return Err(BvhError::Empty);
        }
        if self.joints.len() > MAX_JOINTS {
            return Err(BvhError::Limit {
                what: "joints",
                limit: MAX_JOINTS,
            });
        }
        let mut depth: Vec<usize> = Vec::with_capacity(self.joints.len());
        let mut roots = 0;
        for (index, joint) in self.joints.iter().enumerate() {
            if joint.name.is_empty() || joint.name.chars().any(char::is_whitespace) {
                return Err(BvhError::BadName { index });
            }
            let finite = joint.offset.iter().all(|v| v.is_finite())
                && joint
                    .end_site
                    .is_none_or(|e| e.iter().all(|v| v.is_finite()));
            if !finite {
                return Err(BvhError::NonFinite);
            }
            let level = match joint.parent {
                None => {
                    roots += 1;
                    0
                }
                Some(p) if p < index => depth.get(p).map_or(0, |d| *d + 1),
                Some(parent) => return Err(BvhError::Parent { index, parent }),
            };
            if level >= MAX_DEPTH {
                return Err(BvhError::Limit {
                    what: "joint depth",
                    limit: MAX_DEPTH,
                });
            }
            depth.push(level);
        }
        if roots != 1 {
            return Err(BvhError::Roots { count: roots });
        }
        if self.frame_count > MAX_FRAMES {
            return Err(BvhError::Limit {
                what: "frames",
                limit: MAX_FRAMES,
            });
        }
        if !self.frame_time.is_finite() || self.frame_time < 0.0 {
            return Err(BvhError::NonFinite);
        }
        let width = self.channel_count();
        if self.frame_count > 0 && width == 0 {
            return Err(BvhError::NoChannels);
        }
        let expected = self
            .frame_count
            .checked_mul(width)
            .filter(|n| *n <= MAX_MOTION_VALUES)
            .ok_or(BvhError::Limit {
                what: "motion values",
                limit: MAX_MOTION_VALUES,
            })?;
        if self.motion.len() != expected {
            return Err(BvhError::MotionLength {
                expected,
                found: self.motion.len(),
            });
        }
        if !self.motion.iter().all(|v| v.is_finite()) {
            return Err(BvhError::NonFinite);
        }
        Ok(())
    }
}

#[derive(Debug, Error, PartialEq)]
pub enum BvhError {
    #[error("line {line}: {message}")]
    Syntax { line: usize, message: String },
    #[error("the BVH text is {size} bytes, over the limit of {limit}")]
    TooLarge { size: usize, limit: usize },
    #[error("the BVH file exceeds the limit of {limit} for {what}")]
    Limit { what: &'static str, limit: usize },
    #[error("the BVH file has no joints")]
    Empty,
    #[error("the hierarchy has {count} root joints; BVH has exactly one")]
    Roots { count: usize },
    #[error("joint {index} names parent {parent}, which is not an earlier joint")]
    Parent { index: usize, parent: usize },
    #[error("joint {index} has an empty name or a name with spaces")]
    BadName { index: usize },
    #[error("a value is not a finite number")]
    NonFinite,
    #[error("frames are listed but no joint has channels")]
    NoChannels,
    #[error("the motion has {found} values where {expected} are needed")]
    MotionLength { expected: usize, found: usize },
}
