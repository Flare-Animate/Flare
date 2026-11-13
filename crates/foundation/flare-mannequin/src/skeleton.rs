//! The joints of the figure, how they hang together, and how far each one turns.

use crate::math::Angles;

/// A joint of the figure. Each one turns the part of the body below it in the chain: the
/// shoulder turns the upper arm, the elbow the forearm, the wrist the hand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Joint {
    /// The root: turning it turns the whole figure.
    Pelvis,
    Spine,
    Chest,
    Neck,
    Head,
    LeftShoulder,
    LeftElbow,
    LeftWrist,
    RightShoulder,
    RightElbow,
    RightWrist,
    LeftHip,
    LeftKnee,
    LeftAnkle,
    RightHip,
    RightKnee,
    RightAnkle,
}

/// How many joints the figure has.
pub const JOINT_COUNT: usize = 17;

/// Which side of the body a joint is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Center,
    Left,
    Right,
}

/// How far a joint turns about each of its axes, in degrees.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Limits {
    pub bend: (f64, f64),
    pub twist: (f64, f64),
    pub side: (f64, f64),
}

impl Limits {
    const fn new(bend: (f64, f64), twist: (f64, f64), side: (f64, f64)) -> Self {
        Self { bend, twist, side }
    }

    /// `angles` held inside the limits.
    pub fn clamp(&self, angles: Angles) -> Angles {
        Angles::new(
            angles.bend.clamp(self.bend.0, self.bend.1),
            angles.twist.clamp(self.twist.0, self.twist.1),
            angles.side.clamp(self.side.0, self.side.1),
        )
    }

    pub fn contains(&self, angles: Angles) -> bool {
        self.clamp(angles) == angles
    }
}

const FULL: (f64, f64) = (-180.0, 180.0);
const NONE: (f64, f64) = (0.0, 0.0);

impl Joint {
    /// Every joint, parents before their children.
    pub const ALL: [Joint; JOINT_COUNT] = [
        Self::Pelvis,
        Self::Spine,
        Self::Chest,
        Self::Neck,
        Self::Head,
        Self::LeftShoulder,
        Self::LeftElbow,
        Self::LeftWrist,
        Self::RightShoulder,
        Self::RightElbow,
        Self::RightWrist,
        Self::LeftHip,
        Self::LeftKnee,
        Self::LeftAnkle,
        Self::RightHip,
        Self::RightKnee,
        Self::RightAnkle,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    /// The joint this one hangs from; the pelvis has none.
    pub fn parent(self) -> Option<Joint> {
        use Joint::*;
        Some(match self {
            Pelvis => return None,
            Spine | LeftHip | RightHip => Pelvis,
            Chest => Spine,
            Neck | LeftShoulder | RightShoulder => Chest,
            Head => Neck,
            LeftElbow => LeftShoulder,
            LeftWrist => LeftElbow,
            RightElbow => RightShoulder,
            RightWrist => RightElbow,
            LeftKnee => LeftHip,
            LeftAnkle => LeftKnee,
            RightKnee => RightHip,
            RightAnkle => RightKnee,
        })
    }

    pub fn side(self) -> Side {
        use Joint::*;
        match self {
            Pelvis | Spine | Chest | Neck | Head => Side::Center,
            LeftShoulder | LeftElbow | LeftWrist | LeftHip | LeftKnee | LeftAnkle => Side::Left,
            _ => Side::Right,
        }
    }

    /// The same joint on the other side of the body; a joint of the middle is its own.
    pub fn opposite(self) -> Joint {
        use Joint::*;
        match self {
            LeftShoulder => RightShoulder,
            LeftElbow => RightElbow,
            LeftWrist => RightWrist,
            RightShoulder => LeftShoulder,
            RightElbow => LeftElbow,
            RightWrist => LeftWrist,
            LeftHip => RightHip,
            LeftKnee => RightKnee,
            LeftAnkle => RightAnkle,
            RightHip => LeftHip,
            RightKnee => LeftKnee,
            RightAnkle => LeftAnkle,
            center => center,
        }
    }

    /// The name used in pose files, the command line and the MCP server.
    pub fn id(self) -> &'static str {
        use Joint::*;
        match self {
            Pelvis => "pelvis",
            Spine => "spine",
            Chest => "chest",
            Neck => "neck",
            Head => "head",
            LeftShoulder => "left_shoulder",
            LeftElbow => "left_elbow",
            LeftWrist => "left_wrist",
            RightShoulder => "right_shoulder",
            RightElbow => "right_elbow",
            RightWrist => "right_wrist",
            LeftHip => "left_hip",
            LeftKnee => "left_knee",
            LeftAnkle => "left_ankle",
            RightHip => "right_hip",
            RightKnee => "right_knee",
            RightAnkle => "right_ankle",
        }
    }

    pub fn from_id(id: &str) -> Option<Joint> {
        Self::ALL.into_iter().find(|j| j.id() == id)
    }

    /// How far the joint turns, the same on both sides (the right's angles are mirrored).
    /// Signs: a positive bend tips the spine, neck and head forward, bends the knee and points
    /// the toes; a negative one swings an arm or leg forward and bends the elbow. A positive side
    /// swing takes an arm or leg out from the body.
    pub fn limits(self) -> Limits {
        use Joint::*;
        match self {
            Pelvis => Limits::new(FULL, FULL, FULL),
            Spine => Limits::new((-30.0, 60.0), (-35.0, 35.0), (-30.0, 30.0)),
            Chest => Limits::new((-25.0, 45.0), (-35.0, 35.0), (-25.0, 25.0)),
            Neck => Limits::new((-40.0, 50.0), (-60.0, 60.0), (-30.0, 30.0)),
            Head => Limits::new((-40.0, 40.0), (-30.0, 30.0), (-25.0, 25.0)),
            LeftShoulder | RightShoulder => {
                Limits::new((-180.0, 60.0), (-90.0, 90.0), (-30.0, 180.0))
            }
            LeftElbow | RightElbow => Limits::new((-150.0, 0.0), (-90.0, 90.0), NONE),
            LeftWrist | RightWrist => Limits::new((-80.0, 80.0), (-20.0, 20.0), (-30.0, 40.0)),
            LeftHip | RightHip => Limits::new((-125.0, 30.0), (-45.0, 45.0), (-30.0, 80.0)),
            LeftKnee | RightKnee => Limits::new((0.0, 150.0), (-10.0, 10.0), NONE),
            LeftAnkle | RightAnkle => Limits::new((-30.0, 50.0), (-20.0, 20.0), (-25.0, 25.0)),
        }
    }

    /// The joints a drag of this one pulls along, nearest first, when it is moved to a point
    /// rather than turned: a hand brings the elbow and shoulder, a foot the knee and hip.
    pub fn reach_chain(self) -> Option<[Joint; 2]> {
        use Joint::*;
        Some(match self {
            LeftWrist => [LeftElbow, LeftShoulder],
            RightWrist => [RightElbow, RightShoulder],
            LeftAnkle => [LeftKnee, LeftHip],
            RightAnkle => [RightKnee, RightHip],
            _ => return None,
        })
    }
}
