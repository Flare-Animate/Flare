//! The Vector Eraser, after OpenToonz's: it takes lines away by the piece or whole rather than
//! rubbing out the area it passes over. Segment takes the stretch of each line its round tip
//! touches out to where other lines cross it (see `segment.rs`); Whole Line takes every shape the
//! tip touches; Rectangle and Freehand take every shape that lies wholly inside the box or loop
//! dragged, so what only crosses it stays. What is touched or inside is judged by the same cut
//! the Eraser rubs out with, so the two agree on where a shape is. The layers, the frame and the
//! undo step are the Eraser's.

use flare_commands::Command;
use flare_core::{LayerId, SceneId};
use flare_document::Document;
use glamx::DVec2;

use super::segment::erase_segments;
use super::{Cut, MAX_ERASER_SIZE, cut, eraser_outline, rework};
use flare_input::InputSample;

/// The most corners a Freehand loop keeps; past this the rest of the loop is left off.
pub const MAX_LOOP_POINTS: usize = 8_000;

/// How the Vector Eraser picks what it takes, as the Eraser's list names it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum VectorEraserType {
    /// Drag the round tip: each line it touches loses the stretch out to the crossings either
    /// side, or all of it when nothing crosses it.
    #[default]
    Segment,
    /// Drag the round tip: every shape it touches goes, whole.
    Whole,
    /// Drag a box: every shape wholly inside it goes.
    Rectangular,
    /// Drag a loop: every shape wholly inside it goes.
    Freehand,
}

/// What the Vector Eraser does to the shapes under its area.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Take {
    /// The stretches touched, out to the crossings.
    Segments,
    /// Every shape touched, whole.
    Touched,
    /// Every shape wholly inside.
    Inside,
}

impl VectorEraserType {
    /// Every type, in the order the Eraser's list shows them.
    pub const ALL: [Self; 4] = [
        Self::Segment,
        Self::Whole,
        Self::Rectangular,
        Self::Freehand,
    ];

    /// The name the command line and the MCP server take.
    pub fn name(self) -> &'static str {
        match self {
            Self::Segment => "segment",
            Self::Whole => "whole",
            Self::Rectangular => "rectangular",
            Self::Freehand => "freehand",
        }
    }

    /// The type `name` names, ignoring case.
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|kind| kind.name().eq_ignore_ascii_case(name.trim()))
    }

    /// Whether this type works with the round tip along a drag, rather than a box or loop.
    pub fn uses_tip(self) -> bool {
        matches!(self, Self::Segment | Self::Whole)
    }

    /// What this type takes of the shapes under its area.
    pub fn take(self) -> Take {
        match self {
            Self::Segment => Take::Segments,
            Self::Whole => Take::Touched,
            Self::Rectangular | Self::Freehand => Take::Inside,
        }
    }
}

/// The box with corners `a` and `b`, as a closed outline; empty when it has no area or a corner
/// is not a finite point.
pub fn rectangle(a: DVec2, b: DVec2) -> Vec<DVec2> {
    if !(a.is_finite() && b.is_finite()) || a.x == b.x || a.y == b.y {
        return Vec::new();
    }
    vec![
        DVec2::new(a.x, a.y),
        DVec2::new(b.x, a.y),
        DVec2::new(b.x, b.y),
        DVec2::new(a.x, b.y),
    ]
}

/// The loop through `points`, closed back to the first: at most [`MAX_LOOP_POINTS`] of them,
/// without points that are not finite. Empty when fewer than three are left.
pub fn freehand_loop(points: &[DVec2]) -> Vec<DVec2> {
    let corners: Vec<DVec2> = points
        .iter()
        .copied()
        .filter(|p| p.is_finite())
        .take(MAX_LOOP_POINTS)
        .collect();
    if corners.len() < 3 {
        Vec::new()
    } else {
        corners
    }
}

/// Why the points given for an area make none.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum VectorEraseError {
    #[error("the points must be numbers")]
    NotFinite,
    #[error("the size must be a number from 1 to {MAX_ERASER_SIZE}")]
    BadSize,
    #[error("{kind} needs {needs}, not {given} points")]
    WrongPoints {
        kind: &'static str,
        needs: &'static str,
        given: usize,
    },
    #[error("no shape is taken there")]
    NothingTaken,
}

/// The area a Vector Eraser of `kind` covers through `points`, as a front end with no pointer
/// gives them: for Segment and Whole Line the path of a round tip `size` across (a dot for one
/// point), for Rectangle the box between two corners, for Freehand the loop through at least
/// three.
pub fn area(
    kind: VectorEraserType,
    points: &[DVec2],
    size: f64,
) -> Result<Vec<DVec2>, VectorEraseError> {
    if !points.iter().all(|p| p.is_finite()) {
        return Err(VectorEraseError::NotFinite);
    }
    let wrong = |needs| VectorEraseError::WrongPoints {
        kind: kind.name(),
        needs,
        given: points.len(),
    };
    let outline = match kind {
        VectorEraserType::Segment | VectorEraserType::Whole => {
            if !(1.0..=MAX_ERASER_SIZE).contains(&size) {
                return Err(VectorEraseError::BadSize);
            }
            let samples: Vec<InputSample> = points
                .iter()
                .take(MAX_LOOP_POINTS)
                .filter_map(|p| InputSample::mouse(p.x, p.y))
                .collect();
            if samples.is_empty() {
                return Err(wrong("at least 1"));
            }
            eraser_outline(&samples, size, false)
        }
        VectorEraserType::Rectangular => match points {
            [a, b] => rectangle(*a, *b),
            _ => return Err(wrong("2")),
        },
        VectorEraserType::Freehand => freehand_loop(points),
    };
    if outline.len() < 3 {
        return Err(match kind {
            VectorEraserType::Freehand => wrong("at least 3"),
            _ => VectorEraseError::WrongPoints {
                kind: kind.name(),
                needs: "points that make an area",
                given: points.len(),
            },
        });
    }
    Ok(outline)
}

/// The commands that take from every shape at `frame` what `take` says of the closed outlines
/// `area`: the stretches of lines they touch out to the crossings, every shape they touch whole,
/// or every shape wholly inside them. It works on the layers the Eraser reaches and on that frame
/// alone, as [`erase`](super::erase) says. Symbols, bitmaps and text are left, as the Eraser
/// leaves them. Empty when nothing is taken, so no empty undo step is made.
pub fn erase_with(
    document: &Document,
    scene: SceneId,
    frame: u32,
    area: &[Vec<DVec2>],
    take: Take,
    only: Option<LayerId>,
) -> Vec<Command> {
    if take == Take::Segments {
        let eraser: Vec<Vec<[f64; 2]>> = area
            .iter()
            .filter(|outline| outline.len() >= 3)
            .map(|outline| outline.iter().map(|p| [p.x, p.y]).collect())
            .collect();
        return rework(document, scene, frame, only, |span, index, shape| {
            erase_segments(span, index, shape, &eraser)
        });
    }
    rework(document, scene, frame, only, |_, _, shape| {
        match cut(shape, area) {
            Cut::Untouched => Cut::Untouched,
            // Nothing of it is left outside the area: it was inside.
            Cut::Left(pieces) if take == Take::Touched || pieces.is_empty() => {
                Cut::Left(Vec::new())
            }
            Cut::Left(_) => Cut::Untouched,
        }
    })
}
