//! The component data. Components hold values only; the systems in `system/` do the work.

use flare_core::SoundId;

use crate::EntityId;

/// Where an entity is, how it is turned and scaled. Every other component needs one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    pub x: f64,
    pub y: f64,
    /// Radians. Carried for export; nothing in the port turns anything yet.
    pub rotation: f64,
    pub scale_x: f64,
    pub scale_y: f64,
    /// The pivot as a fraction of the size, 0.5 being the middle.
    pub pivot_x: f64,
    pub pivot_y: f64,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            rotation: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            pivot_x: 0.5,
            pivot_y: 0.5,
        }
    }
}

/// A picture that changes over time: `frames` frames at `fps` frames per second, starting on
/// `frame`. An `fps` of zero holds the frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sprite {
    pub frames: u32,
    pub fps: f64,
    pub frame: u32,
    pub looping: bool,
    pub visible: bool,
    pub alpha: f64,
    /// Draw order: lower first.
    pub order: i32,
}

impl Default for Sprite {
    fn default() -> Self {
        Self {
            frames: 1,
            fps: 0.0,
            frame: 0,
            looping: true,
            visible: true,
            alpha: 1.0,
            order: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyKind {
    /// Never moves.
    Static,
    /// Moves by its velocity, which a controller or the script sets; gravity does not act on it.
    Kinematic,
    /// Moves by its velocity and is pulled by the world's gravity.
    Dynamic,
}

/// An axis-aligned box whose top-left corner is the entity's position. Two bodies whose
/// categories and masks let them meet are reported as colliding while they overlap; nothing
/// pushes them apart.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Body {
    pub kind: BodyKind,
    pub width: f64,
    pub height: f64,
    pub vx: f64,
    pub vy: f64,
    pub category: u16,
    pub mask: u16,
}

impl Default for Body {
    fn default() -> Self {
        Self {
            kind: BodyKind::Dynamic,
            width: 10.0,
            height: 10.0,
            vx: 0.0,
            vy: 0.0,
            category: 1,
            mask: u16::MAX,
        }
    }
}

/// The keys a game reads. A small fixed set: the surfaces send these names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Key {
    Left,
    Right,
    Up,
    Down,
    Space,
    Enter,
}

impl Key {
    pub const ALL: [Key; 6] = [
        Key::Left,
        Key::Right,
        Key::Up,
        Key::Down,
        Key::Space,
        Key::Enter,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Key::Left => "left",
            Key::Right => "right",
            Key::Up => "up",
            Key::Down => "down",
            Key::Space => "space",
            Key::Enter => "enter",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|key| key.name() == text)
    }
}

/// Moves a body with the arrow keys (or whichever keys are bound): the velocity is the held
/// direction times `speed`. Needs a body.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Control {
    pub left: Key,
    pub right: Key,
    pub up: Key,
    pub down: Key,
    pub speed: f64,
}

impl Default for Control {
    fn default() -> Self {
        Self {
            left: Key::Left,
            right: Key::Right,
            up: Key::Up,
            down: Key::Down,
            speed: 100.0,
        }
    }
}

/// A rectangle in world pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// The view onto the world. Its own entity's transform is where the camera looks.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    pub width: f64,
    pub height: f64,
    pub zoom: f64,
    /// An entity with a transform to follow, if any.
    pub follow: Option<EntityId>,
    /// How quickly the camera closes on its target, per second (the C++ `lerpSpeed`).
    pub lerp: f64,
    /// Where the camera's centre may go. `None` is unbounded.
    pub bounds: Option<Bounds>,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            width: 800.0,
            height: 600.0,
            zoom: 1.0,
            follow: None,
            lerp: 5.0,
            bounds: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiKind {
    Button,
    Text,
    Image,
    ProgressBar,
    Slider,
    InputField,
    Panel,
}

impl UiKind {
    pub fn name(self) -> &'static str {
        match self {
            UiKind::Button => "button",
            UiKind::Text => "text",
            UiKind::Image => "image",
            UiKind::ProgressBar => "progress",
            UiKind::Slider => "slider",
            UiKind::InputField => "input",
            UiKind::Panel => "panel",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        [
            UiKind::Button,
            UiKind::Text,
            UiKind::Image,
            UiKind::ProgressBar,
            UiKind::Slider,
            UiKind::InputField,
            UiKind::Panel,
        ]
        .into_iter()
        .find(|kind| kind.name() == text)
    }
}

/// A rectangle at the entity's position that reports clicks inside it while `interactive`.
#[derive(Debug, Clone, PartialEq)]
pub struct Ui {
    pub kind: UiKind,
    pub width: f64,
    pub height: f64,
    pub text: String,
    pub interactive: bool,
}

impl Default for Ui {
    fn default() -> Self {
        Self {
            kind: UiKind::Button,
            width: 100.0,
            height: 30.0,
            text: String::new(),
            interactive: true,
        }
    }
}

/// A sound to play. The step reports the cue; playing it is the surface's job.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Audio {
    pub sound: Option<SoundId>,
    pub looping: bool,
    pub play_on_start: bool,
    pub volume: f64,
}

impl Default for Audio {
    fn default() -> Self {
        Self {
            sound: None,
            looping: false,
            play_on_start: false,
            volume: 1.0,
        }
    }
}

/// A particle emitter: how many particles it produces per second.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Particle {
    pub rate: f64,
    /// Seconds a particle lives. Carried for the surfaces; the port does not simulate particles.
    pub life: f64,
    /// Whether the emitter runs from the first step. One that does not emits nothing: the port
    /// has no start command yet.
    pub auto_play: bool,
}

impl Default for Particle {
    fn default() -> Self {
        Self {
            rate: 10.0,
            life: 1.0,
            auto_play: true,
        }
    }
}

/// A script's name. Scripts are preserved as data and never run (see `docs/PARITY.md`).
#[derive(Debug, Clone, PartialEq)]
pub struct Script {
    pub name: String,
}
