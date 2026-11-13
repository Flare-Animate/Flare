//! Create New Symbol and Convert to Symbol as Animate asks them: a name, a type and, for a
//! conversion, where in the artwork the symbol's own origin sits. The window's dialog, the
//! command line and the MCP server all come here, so a name one refuses the others refuse too.

use flare_commands::Command;
use flare_core::{InstanceId, LayerId, SceneId, SymbolId, Transform2D};
use flare_document::{Content, Document, FrameSpan, Instance, Layer, Symbol, SymbolKind, Timeline};
use glamx::{DAffine2, DVec2};
use std::num::NonZeroU32;
use thiserror::Error;

use crate::common::*;

/// The longest name a symbol may have, in characters, as long as a scene's.
pub const MAX_SYMBOL_NAME_CHARS: usize = 256;

/// The most numbers [`free_symbol_name`] tries before it gives up; the library would have to
/// hold this many symbols of the same pattern first.
const MAX_NAME_TRIES: u32 = 100_000;

/// Why a symbol could not be made.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum SymbolError {
    #[error("a symbol needs a name")]
    NameEmpty,
    #[error("a symbol's name must be at most {MAX_SYMBOL_NAME_CHARS} characters")]
    NameTooLong,
    #[error("the library already has a symbol of that name")]
    NameTaken,
    #[error("only a drawn shape can be converted to a symbol")]
    NotAShape,
    #[error("the artwork's position cannot be kept")]
    NotFinite,
    #[error("the timeline would not accept the change")]
    TimelineRefused,
}

/// The point of the artwork's box that becomes the symbol's own origin, the nine squares of
/// Animate's registration grid, read left to right and top to bottom.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Registration {
    #[default]
    TopLeft,
    Top,
    TopRight,
    Left,
    Center,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
}

impl Registration {
    /// In the grid's order: the top row first.
    pub const ALL: [Self; 9] = [
        Self::TopLeft,
        Self::Top,
        Self::TopRight,
        Self::Left,
        Self::Center,
        Self::Right,
        Self::BottomLeft,
        Self::Bottom,
        Self::BottomRight,
    ];

    /// The spelling the command line and the MCP server take and print.
    pub fn name(self) -> &'static str {
        match self {
            Self::TopLeft => "top-left",
            Self::Top => "top",
            Self::TopRight => "top-right",
            Self::Left => "left",
            Self::Center => "center",
            Self::Right => "right",
            Self::BottomLeft => "bottom-left",
            Self::Bottom => "bottom",
            Self::BottomRight => "bottom-right",
        }
    }

    /// Reads [`Registration::name`], ignoring case and taking a space or an underscore for the
    /// hyphen, and "centre" for "center".
    pub fn parse(text: &str) -> Option<Self> {
        let wanted = text.trim().to_lowercase().replace([' ', '_'], "-");
        let wanted = if wanted == "centre" {
            "center".to_owned()
        } else {
            wanted
        };
        Self::ALL.into_iter().find(|r| r.name() == wanted)
    }

    /// Where this point is on a box from `min` to `max`.
    fn point(self, min: DVec2, max: DVec2) -> DVec2 {
        let index = Self::ALL.iter().position(|r| *r == self).unwrap_or(0);
        let (column, row) = (index % 3, index / 3);
        let along = |low: f64, high: f64, step: usize| low + (high - low) * step as f64 / 2.0;
        DVec2::new(along(min.x, max.x, column), along(min.y, max.y, row))
    }
}

/// `name` trimmed, if it may name a new symbol in `document`: not blank, not too long, and not
/// the name of a symbol the library already holds.
pub fn check_symbol_name(document: &Document, name: &str) -> Result<String, SymbolError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(SymbolError::NameEmpty);
    }
    if name.chars().count() > MAX_SYMBOL_NAME_CHARS {
        return Err(SymbolError::NameTooLong);
    }
    if document.symbols().iter().any(|s| s.name() == name) {
        return Err(SymbolError::NameTaken);
    }
    Ok(name.to_owned())
}

/// The first of `pattern(1)`, `pattern(2)` and so on that no symbol in `document` is called:
/// "Symbol 1", "Symbol 2", in whatever language the pattern speaks.
pub fn free_symbol_name(document: &Document, pattern: impl Fn(u32) -> String) -> String {
    (1..=MAX_NAME_TRIES)
        .map(&pattern)
        .find(|name| check_symbol_name(document, name).is_ok())
        .unwrap_or_else(|| pattern(MAX_NAME_TRIES))
}

/// Create New Symbol: an empty symbol of `kind` called `name` joins the library.
pub fn create_symbol(
    document: &Document,
    name: &str,
    kind: SymbolKind,
) -> Result<(Vec<Command>, SymbolId), SymbolError> {
    create_symbol_with(document, name, kind, Timeline::default())
}

/// [`create_symbol`] with the symbol already holding `timeline`.
pub fn create_symbol_with(
    document: &Document,
    name: &str,
    kind: SymbolKind,
    timeline: Timeline,
) -> Result<(Vec<Command>, SymbolId), SymbolError> {
    let name = check_symbol_name(document, name)?;
    let id = SymbolId::new();
    let symbol = Symbol::new(id, name, timeline).with_kind(kind);
    Ok((vec![Command::AddSymbol(symbol)], id))
}

/// Convert to Symbol: the shape at `at` becomes a symbol of `kind` called `name`, and an
/// instance of it takes the shape's place. The artwork does not move on the stage: the symbol's
/// origin is put at `registration` on the shape's box, and the instance stands there.
pub fn convert_to_symbol(
    document: &Document,
    scene: SceneId,
    at: SpanRef,
    name: &str,
    kind: SymbolKind,
    registration: Registration,
) -> Result<(Vec<Command>, SymbolId), SymbolError> {
    let name = check_symbol_name(document, name)?;
    let span = find_span(document, scene, at).ok_or(SymbolError::NotAShape)?;
    let Some(Content::Shape(shape)) = span.element(at.element) else {
        return Err(SymbolError::NotAShape);
    };
    let origin = shape_bounds(shape).map_or(DVec2::ZERO, |b| {
        registration.point(
            DVec2::new(f64::from(b.min.x), f64::from(b.min.y)),
            DVec2::new(f64::from(b.max.x), f64::from(b.max.y)),
        )
    });
    let inside = transform_shape(shape, DAffine2::from_translation(-origin))
        .ok_or(SymbolError::NotFinite)?;
    let place =
        Transform2D::new(DAffine2::from_translation(origin)).map_err(|_| SymbolError::NotFinite)?;
    let first = FrameSpan::new(0, NonZeroU32::MIN, Content::Shape(inside));
    let layer = Layer::new(LayerId::new(), "Layer 1", vec![first])
        .map_err(|_| SymbolError::TimelineRefused)?;
    let id = SymbolId::new();
    let symbol = Symbol::new(id, name, Timeline::new(vec![layer])).with_kind(kind);
    let instance = Content::Instance(Instance::new(InstanceId::new(), id, place));
    // A keyframe of several elements keeps the others, as Convert to Symbol always has.
    let outer = if span.element_count() > 1 {
        span.with_element(at.element, instance)
            .ok_or(SymbolError::TimelineRefused)?
    } else {
        FrameSpan::new(span.start(), span.duration(), instance)
    };
    Ok((
        vec![
            Command::RemoveSpan {
                scene,
                layer: at.layer,
                start: at.start,
            },
            Command::AddSymbol(symbol),
            Command::InsertSpan {
                scene,
                layer: at.layer,
                span: outer,
            },
        ],
        id,
    ))
}
