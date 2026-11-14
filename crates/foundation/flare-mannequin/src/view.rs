//! Looking at the figure: the camera that turns round it, and where its points land in a picture.

use crate::math::{Rot3, Vec3, wrap_degrees};

/// Widest field of view, in degrees; 0 is a flat (orthographic) view.
pub const MAX_FOV: f64 = 90.0;
/// Steepest look from above or below, in degrees.
pub const MAX_PITCH: f64 = 89.0;

/// Which way the figure is seen: turned about its own vertical, looked at from above or below,
/// and how strong the perspective is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct View {
    yaw: f64,
    pitch: f64,
    fov: f64,
}

impl Default for View {
    /// A three-quarter view from just above, with a little perspective.
    fn default() -> Self {
        Self {
            yaw: 25.0,
            pitch: 8.0,
            fov: 30.0,
        }
    }
}

impl View {
    /// The view, brought into range: the turn wraps, the tilt and field of view are held.
    pub fn new(yaw: f64, pitch: f64, fov: f64) -> Self {
        let mut view = Self::default();
        view.set_yaw(yaw);
        view.set_pitch(pitch);
        view.set_fov(fov);
        view
    }

    /// Degrees the figure is turned to its left, in (-180, 180].
    pub fn yaw(&self) -> f64 {
        self.yaw
    }

    /// Degrees the view looks down on the figure (negative looks up at it).
    pub fn pitch(&self) -> f64 {
        self.pitch
    }

    pub fn fov(&self) -> f64 {
        self.fov
    }

    pub fn set_yaw(&mut self, yaw: f64) {
        if yaw.is_finite() {
            self.yaw = wrap_degrees(yaw);
        }
    }

    pub fn set_pitch(&mut self, pitch: f64) {
        if pitch.is_finite() {
            self.pitch = pitch.clamp(-MAX_PITCH, MAX_PITCH);
        }
    }

    pub fn set_fov(&mut self, fov: f64) {
        if fov.is_finite() {
            self.fov = fov.clamp(0.0, MAX_FOV);
        }
    }

    /// The figure's space turned to the camera's: x right, y up, z toward the viewer.
    pub(crate) fn rotation(&self) -> Rot3 {
        Rot3::about_x(self.pitch) * Rot3::about_y(self.yaw)
    }

    /// The direction toward the viewer, in the figure's space.
    pub(crate) fn toward_viewer(&self) -> Vec3 {
        self.rotation().transpose().apply(Vec3::new(0.0, 0.0, 1.0))
    }

    /// The figure seen square from one side, with the same perspective.
    #[must_use]
    pub fn facing(self, facing: Facing) -> Self {
        let (yaw, pitch) = facing.angles();
        Self::new(yaw, pitch, self.fov)
    }
}

/// A side the figure can be seen square from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Facing {
    Front,
    Back,
    /// Its left side toward the viewer.
    Left,
    Right,
    /// Looking down on it.
    Top,
    /// Looking up at it.
    Bottom,
}

impl Facing {
    pub const ALL: [Facing; 6] = [
        Self::Front,
        Self::Back,
        Self::Left,
        Self::Right,
        Self::Top,
        Self::Bottom,
    ];

    /// The name used in files, the command line and the MCP server.
    pub fn id(self) -> &'static str {
        match self {
            Self::Front => "front",
            Self::Back => "back",
            Self::Left => "left",
            Self::Right => "right",
            Self::Top => "top",
            Self::Bottom => "bottom",
        }
    }

    pub fn from_id(id: &str) -> Option<Facing> {
        Self::ALL.into_iter().find(|f| f.id() == id)
    }

    /// The turn and the tilt that show it. Turning the figure to its left brings its right side
    /// round toward the viewer.
    fn angles(self) -> (f64, f64) {
        match self {
            Self::Front => (0.0, 0.0),
            Self::Back => (180.0, 0.0),
            Self::Left => (-90.0, 0.0),
            Self::Right => (90.0, 0.0),
            Self::Top => (0.0, MAX_PITCH),
            Self::Bottom => (0.0, -MAX_PITCH),
        }
    }
}

/// Where the figure goes in a picture: the point the camera looks at (its focus, the middle of
/// where the figure stands unless it is moved), where that lands, and how many units of the
/// picture one figure unit (the default figure's height) takes. The picture's y runs down.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    pub center: [f64; 2],
    pub height: f64,
    focus: Vec3,
}

/// The largest figure, in picture units.
pub const MAX_PLACEMENT_HEIGHT: f64 = 100_000.0;
/// How far from the middle of the ground the camera may look, in figure units each way: past
/// anywhere the figure can be moved to.
pub const MAX_FOCUS: f64 = 4.0;

impl Placement {
    pub fn new(center: [f64; 2], height: f64) -> Option<Self> {
        (center[0].is_finite()
            && center[1].is_finite()
            && center[0].abs() <= 1e9
            && center[1].abs() <= 1e9
            && height.is_finite()
            && height > 0.0
            && height <= MAX_PLACEMENT_HEIGHT)
            .then_some(Self {
                center,
                height,
                focus: Vec3::new(0.0, MIDDLE, 0.0),
            })
    }

    /// The same placement with the camera looking at `focus`, a point of the figure's space,
    /// which then lands on `center`; none for a point that is not a number or is past
    /// [`MAX_FOCUS`].
    pub fn with_focus(self, focus: Vec3) -> Option<Self> {
        let inside = |v: f64| v.is_finite() && v.abs() <= MAX_FOCUS;
        (inside(focus.x) && inside(focus.y) && inside(focus.z)).then_some(Self { focus, ..self })
    }

    /// The point of the figure's space the camera looks at.
    pub fn focus(&self) -> Vec3 {
        self.focus
    }
}

/// A point of the figure in the picture.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Projected {
    pub at: [f64; 2],
    /// Larger is nearer the viewer.
    pub depth: f64,
    /// How much the perspective enlarges a thickness here.
    pub scale: f64,
}

/// Turns points of the figure's space into points of the picture.
#[derive(Debug, Clone, Copy)]
pub struct Projector {
    rotation: Rot3,
    target: Vec3,
    placement: Placement,
    /// The camera's distance from the target, with perspective.
    distance: Option<f64>,
}

/// The middle of the figure, which the camera looks at unless it is moved.
pub(crate) const MIDDLE: f64 = 0.5;

impl Projector {
    /// The camera looks at the placement's focus, by default the middle of where the figure
    /// stands, and stays there when the figure is moved or lifted, so the floor holds still and
    /// a body lowered over planted feet is seen to crouch.
    pub(crate) fn new(view: &View, placement: Placement) -> Self {
        let distance = (view.fov > 0.5).then(|| 0.5 / (view.fov.to_radians() / 2.0).tan() + 0.5);
        Self {
            rotation: view.rotation(),
            target: placement.focus,
            placement,
            distance,
        }
    }

    pub fn project(&self, p: Vec3) -> Projected {
        let c = self.rotation.apply(p - self.target);
        let scale = self.distance.map_or(1.0, |d| d / (d - c.z).max(d * 0.05));
        let k = self.placement.height * scale;
        Projected {
            at: [
                self.placement.center[0] + c.x * k,
                self.placement.center[1] - c.y * k,
            ],
            depth: c.z,
            scale,
        }
    }

    /// The point of the figure's space that lands on `at` at `depth` (as [`project`] gives it).
    ///
    /// [`project`]: Self::project
    pub fn unproject(&self, at: [f64; 2], depth: f64) -> Vec3 {
        let scale = self.distance.map_or(1.0, |d| d / (d - depth).max(d * 0.05));
        let k = self.placement.height * scale;
        let c = Vec3::new(
            (at[0] - self.placement.center[0]) / k,
            -(at[1] - self.placement.center[1]) / k,
            depth,
        );
        self.rotation.transpose().apply(c) + self.target
    }

    pub fn placement(&self) -> Placement {
        self.placement
    }

    /// Whether a point at `depth` (as [`project`](Self::project) gives it) is in front of the
    /// camera; with no perspective everything is.
    pub(crate) fn in_front(&self, depth: f64) -> bool {
        self.distance.is_none_or(|d| depth < d)
    }
}
