//! How wide a gap in the lines the Paint Bucket still fills across, as Animate's Gap Size choice.
//! A loop drawn by hand seldom meets itself exactly, so the bucket treats lines whose ends come
//! close enough as closed. The size is measured on the screen, as a person sees the gap, so it
//! closes the same-looking gap at any zoom.

/// How large a gap the Paint Bucket closes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GapSize {
    /// Only areas the lines close exactly are filled.
    DontClose,
    /// Gaps a hand-drawn loop leaves where it nearly meets itself.
    #[default]
    Small,
    Medium,
    /// Gaps a quick sketch leaves between strokes.
    Large,
}

/// The most a gap can stand for on the stage, in stage units, however far out the view is zoomed:
/// past it, lines that far apart are not one outline but separate drawings.
pub const MAX_GAP: f64 = 200.0;

impl GapSize {
    /// Every size, smallest first, as the choice lists them.
    pub const ALL: [Self; 4] = [Self::DontClose, Self::Small, Self::Medium, Self::Large];

    /// How far apart, in screen points, the ends of a line may be for it still to close an area.
    pub fn points(self) -> f64 {
        match self {
            Self::DontClose => 0.0,
            Self::Small => 10.0,
            Self::Medium => 24.0,
            Self::Large => 48.0,
        }
    }

    /// The gap in stage units when the stage is shown at `scale` screen points a stage unit, at
    /// most [`MAX_GAP`]. A scale that is not a positive number counts as one.
    pub fn on_stage(self, scale: f64) -> f64 {
        let scale = if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        };
        (self.points() / scale).min(MAX_GAP)
    }
}
