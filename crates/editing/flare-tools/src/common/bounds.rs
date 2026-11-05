//! Where things are: bounds of shapes, spans and symbols.

use emath::{Pos2, Rect};
use flare_core::{DocumentPoint, LayerId, SceneId, SymbolId};
use flare_document::{Content, Document, FrameSpan, Layer, PathSegment, Shape};
use glamx::{DAffine2, DVec2};

use super::span::{SpanRef, find_span, scene_timeline};

/// Control-polygon bounds of the shapes in a symbol's first frame (one level deep).
pub fn symbol_bounds(document: &Document, symbol: SymbolId) -> Option<Rect> {
    symbol_bounds_nested(document, symbol, 0)
}

/// How many levels of symbols inside symbols are followed. Documents reject symbol cycles,
/// so this only bounds work on deep but legal nesting.
pub(crate) const MAX_BOUNDS_DEPTH: usize = 32;

/// Bounds of the artwork at frame 0, following nested instances through their transforms.
pub(crate) fn symbol_bounds_nested(
    document: &Document,
    symbol: SymbolId,
    depth: usize,
) -> Option<Rect> {
    if depth > MAX_BOUNDS_DEPTH {
        return None;
    }
    let timeline = document.symbol(symbol)?.timeline();
    let mut bounds: Option<Rect> = None;
    for layer in timeline.layers() {
        for element in layer.span_at(0).into_iter().flat_map(FrameSpan::elements) {
            let rect = match element {
                Content::Shape(shape) => shape_bounds(shape),
                Content::Instance(instance) => {
                    symbol_bounds_nested(document, instance.symbol(), depth + 1)
                        .map(|inner| transformed(inner, instance.transform().as_affine()))
                }
                _ => None,
            };
            if let Some(rect) = rect {
                bounds = Some(bounds.map_or(rect, |b| b.union(rect)));
            }
        }
    }
    bounds
}

/// The axis-aligned box around `rect` after `matrix`.
pub(crate) fn transformed(rect: Rect, matrix: DAffine2) -> Rect {
    let corners = [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
    ]
    .map(|c| matrix.transform_point2(DVec2::new(f64::from(c.x), f64::from(c.y))));
    let first = Pos2::new(corners[0].x as f32, corners[0].y as f32);
    let mut out = Rect::from_min_max(first, first);
    for c in &corners[1..] {
        out.extend_with(Pos2::new(c.x as f32, c.y as f32));
    }
    out
}

pub fn shape_bounds(shape: &Shape) -> Option<Rect> {
    let mut bounds: Option<Rect> = None;
    let mut add = |p: &DocumentPoint| {
        let point = Pos2::new(p.as_vec().x as f32, p.as_vec().y as f32);
        bounds = Some(bounds.map_or(Rect::from_min_max(point, point), |b| {
            b.union(Rect::from_min_max(point, point))
        }));
    };
    for segment in shape.path() {
        match segment {
            PathSegment::MoveTo(p) | PathSegment::LineTo(p) => add(p),
            PathSegment::CubicTo(a, b, c) => {
                add(a);
                add(b);
                add(c);
            }
            PathSegment::Close => {}
        }
    }
    // Strokes extend half their width beyond the path.
    let half = shape
        .stroke()
        .map_or(0.0, |stroke| stroke.width() as f32 / 2.0);
    bounds.map(|b| b.expand(half))
}

/// Stage-space bounds of a selected span, for drawing the selection outline.
pub fn span_bounds(document: &Document, scene: SceneId, at: SpanRef) -> Option<Rect> {
    content_bounds(
        document,
        find_span(document, scene, at)?.element(at.element)?,
    )
}

/// Stage-space bounds of each of `spans` that still exists and has something to show. One pass
/// over the layers serves all of them, so a big selection costs `layers + spans`, not their
/// product.
pub fn spans_bounds(document: &Document, scene: SceneId, spans: &[SpanRef]) -> Vec<Rect> {
    let Some(timeline) = scene_timeline(document, scene) else {
        return Vec::new();
    };
    let by_layer: std::collections::HashMap<LayerId, &Layer> =
        timeline.layers().iter().map(|l| (l.id(), l)).collect();
    spans
        .iter()
        .filter_map(|at| {
            let span = by_layer
                .get(&at.layer)?
                .spans()
                .iter()
                .find(|s| s.start() == at.start)?;
            content_bounds(document, span.element(at.element)?)
        })
        .collect()
}

/// Like [`spans_bounds`], but where a motion tween has carried an instance by `frame`: inside a
/// tweened span the box follows the in-between placement, not the keyframe's stored one.
pub fn spans_bounds_at(
    document: &Document,
    scene: SceneId,
    spans: &[SpanRef],
    frame: u32,
) -> Vec<Rect> {
    let Some(timeline) = scene_timeline(document, scene) else {
        return Vec::new();
    };
    // Evaluated at most once, and only when a selected span is really mid-tween.
    let mut evaluated = None;
    spans
        .iter()
        .filter_map(|at| {
            let span = timeline
                .layers()
                .iter()
                .find(|l| l.id() == at.layer)?
                .spans()
                .iter()
                .find(|s| s.start() == at.start)?;
            let element = span.element(at.element)?;
            let inside = frame > span.start()
                && u64::from(frame) < u64::from(span.start()) + u64::from(span.duration().get());
            if let (Content::Instance(instance), true, true) =
                (element, inside, span.tween() != flare_document::Tween::None)
            {
                let frame_state = evaluated.get_or_insert_with(|| {
                    flare_eval::evaluate_frame_without_camera(document, scene, frame).ok()
                });
                let placed = frame_state
                    .as_ref()?
                    .placed
                    .iter()
                    .find(|p| p.path == [instance.id()])?;
                let local = symbol_bounds(document, instance.symbol())?;
                return Some(transformed(local, placed.transform));
            }
            content_bounds(document, element)
        })
        .collect()
}

pub(crate) fn content_bounds(document: &Document, content: &Content) -> Option<Rect> {
    match content {
        // The camera is not art: it has no outline to select or move. An armature is posed by its
        // bones (see `bones`), not selected and moved as one thing.
        Content::Empty | Content::Camera(_) | Content::Armature(_) => None,
        Content::Shape(shape) => shape_bounds(shape),
        Content::Bitmap(placed) => {
            let bitmap = document.bitmap(placed.bitmap())?;
            let local = Rect::from_min_max(
                Pos2::ZERO,
                Pos2::new(bitmap.width() as f32, bitmap.height() as f32),
            );
            Some(transformed(local, placed.transform().as_affine()))
        }
        Content::Instance(instance) => {
            let local = symbol_bounds(document, instance.symbol())?;
            Some(transformed(local, instance.transform().as_affine()))
        }
    }
}
