//! Selection-based editing operations as pure functions from a document to commands.
//!
//! Nothing here touches egui or the session, so every operation is unit-testable and the
//! UI only decides *which* operation to run. Selections are frame spans, identified by
//! their layer and start frame (spans on a layer never overlap, so this is unique).

use std::num::NonZeroU32;

use egui::{Pos2, Rect};
use flare_core::{
    DocumentPoint, InstanceId, LayerId, Rgba8, SceneId, ShapeId, SymbolId, Transform2D,
};
use flare_document::{
    Content, Document, FrameSpan, Gradient, Instance, Layer, PathSegment, Shape, Stroke, Symbol,
    Timeline, Tween,
};
use flare_edit::Command;
use flare_eval::DrawnShape;
use glamx::{DAffine2, DVec2};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpanRef {
    pub layer: LayerId,
    pub start: u32,
}

fn scene_timeline(document: &Document, scene: SceneId) -> Option<&Timeline> {
    document
        .scenes()
        .iter()
        .find(|s| s.id() == scene)
        .map(|s| s.timeline())
}

pub fn find_span(document: &Document, scene: SceneId, at: SpanRef) -> Option<&FrameSpan> {
    scene_timeline(document, scene)?
        .layers()
        .iter()
        .find(|layer| layer.id() == at.layer)?
        .spans()
        .iter()
        .find(|span| span.start() == at.start)
}

/// The scene-level span that produced a hit shape at `frame`: the shape's own span, or
/// the top-level instance that contains it.
pub fn locate_hit(
    document: &Document,
    scene: SceneId,
    frame: u32,
    drawn: &DrawnShape<'_>,
) -> Option<SpanRef> {
    for layer in scene_timeline(document, scene)?.layers() {
        let Some(span) = layer.span_at(frame) else {
            continue;
        };
        let owns = match (span.content(), drawn.path.first()) {
            (Content::Shape(shape), None) => shape.id() == drawn.shape.id(),
            (Content::Instance(instance), Some(top)) => instance.id() == *top,
            _ => false,
        };
        if owns {
            return Some(SpanRef {
                layer: layer.id(),
                start: span.start(),
            });
        }
    }
    None
}

pub fn delete(scene: SceneId, at: SpanRef) -> Vec<Command> {
    vec![Command::RemoveSpan {
        scene,
        layer: at.layer,
        start: at.start,
    }]
}

/// The span's shape repainted: `fill` replaces the solid fill (dropping a gradient) and `stroke`
/// replaces the outline (`Some(None)` removes it). `None` when the span holds no shape or the
/// paint would not change.
pub fn restyle(
    document: &Document,
    scene: SceneId,
    at: SpanRef,
    fill: Option<Rgba8>,
    stroke: Option<Option<Stroke>>,
) -> Option<Vec<Command>> {
    let span = find_span(document, scene, at)?;
    let Content::Shape(shape) = span.content() else {
        return None;
    };
    let mut painted = shape.clone();
    if let Some(fill) = fill {
        painted = painted.with_fill(fill);
    }
    if let Some(stroke) = stroke {
        painted = painted.restroked(stroke);
    }
    if painted == *shape {
        return None;
    }
    let replacement = FrameSpan::new(span.start(), span.duration(), Content::Shape(painted))
        .with_tween(span.tween());
    Some(vec![
        Command::RemoveSpan {
            scene,
            layer: at.layer,
            start: at.start,
        },
        Command::InsertSpan {
            scene,
            layer: at.layer,
            span: replacement,
        },
    ])
}

/// `shape` with every point mapped through `matrix`; the stroke width is kept.
fn transform_shape(shape: &Shape, matrix: DAffine2) -> Option<Shape> {
    transform_shape_as(shape, shape.id(), matrix)
}

/// Like `transform_shape`, but the result has the identity `id` (for copies).
fn transform_shape_as(shape: &Shape, id: ShapeId, matrix: DAffine2) -> Option<Shape> {
    let moved = |p: &DocumentPoint| {
        let q = matrix.transform_point2(p.as_vec());
        DocumentPoint::new(q.x, q.y).ok()
    };
    let path = shape
        .path()
        .iter()
        .map(|segment| {
            Some(match segment {
                PathSegment::MoveTo(p) => PathSegment::MoveTo(moved(p)?),
                PathSegment::LineTo(p) => PathSegment::LineTo(moved(p)?),
                PathSegment::CubicTo(a, b, c) => {
                    PathSegment::CubicTo(moved(a)?, moved(b)?, moved(c)?)
                }
                PathSegment::Close => PathSegment::Close,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    let moved = Shape::new(id, path, shape.fill()).ok()?;
    // Rebuilding must not drop the stroke or the gradient, and the gradient moves with the
    // artwork.
    let moved = moved.painted_like(shape);
    Some(match shape.gradient() {
        Some(g) => {
            let map = |p: DocumentPoint| {
                let q = matrix.transform_point2(p.as_vec());
                DocumentPoint::new(q.x, q.y).ok()
            };
            let mapped = Gradient::linear(map(g.from())?, map(g.to())?, g.start(), g.end()).ok()?;
            moved.with_gradient(mapped)
        }
        None => moved,
    })
}

/// Moves a span's content by `delta`, keeping identity, duration and tween.
/// `None` when the span is missing, the delta is zero or the result is not finite.
pub fn translate(
    document: &Document,
    scene: SceneId,
    at: SpanRef,
    delta: DVec2,
) -> Option<Vec<Command>> {
    if delta == DVec2::ZERO {
        return None;
    }
    transform(document, scene, at, DAffine2::from_translation(delta))
}

/// Whether an instance span may take the transform `moved`. A motion tween interpolates between
/// two exposures, which only works for a scale, rotation and translation, so shearing or
/// mirroring an exposure that starts or ends a tween would make the frame impossible to draw.
fn tween_allows(
    document: &Document,
    scene: SceneId,
    at: SpanRef,
    span: &FrameSpan,
    moved: DAffine2,
) -> bool {
    if flare_eval::can_tween(moved) {
        return true;
    }
    if span.tween() == Tween::Linear {
        return false;
    }
    // An earlier exposure that tweens into this one needs it to be tweenable too.
    let tweened_into = scene_timeline(document, scene)
        .and_then(|t| t.layers().iter().find(|l| l.id() == at.layer))
        .is_some_and(|layer| {
            layer
                .spans()
                .iter()
                .any(|p| p.end() == u64::from(span.start()) && p.tween() == Tween::Linear)
        });
    !tweened_into
}

/// Maps a span's content through `matrix` (in stage coordinates), keeping identity, duration
/// and tween. `None` when the span is missing or empty, the matrix is the identity or cannot
/// be inverted (a zero scale would destroy the content), or the result is not finite.
pub fn transform(
    document: &Document,
    scene: SceneId,
    at: SpanRef,
    matrix: DAffine2,
) -> Option<Vec<Command>> {
    if matrix == DAffine2::IDENTITY
        || !matrix.is_finite()
        || matrix.matrix2.determinant().abs() < 1e-9
    {
        return None;
    }
    let span = find_span(document, scene, at)?;
    let content = match span.content() {
        Content::Empty => return None,
        Content::Shape(shape) => Content::Shape(transform_shape(shape, matrix)?),
        Content::Instance(instance) => {
            let moved = matrix * instance.transform().as_affine();
            if !tween_allows(document, scene, at, span, moved) {
                return None;
            }
            let transform = Transform2D::new(moved).ok()?;
            Content::Instance(Instance::new(instance.id(), instance.symbol(), transform))
        }
    };
    let replacement =
        FrameSpan::new(span.start(), span.duration(), content).with_tween(span.tween());
    Some(vec![
        Command::RemoveSpan {
            scene,
            layer: at.layer,
            start: at.start,
        },
        Command::InsertSpan {
            scene,
            layer: at.layer,
            span: replacement,
        },
    ])
}

/// `content` moved through `matrix` with fresh shape and instance identities, so the result
/// is independent of the original. `None` for empty content or a non-finite result.
fn fresh_content(content: &Content, matrix: DAffine2) -> Option<Content> {
    Some(match content {
        Content::Empty => return None,
        Content::Shape(shape) => Content::Shape(transform_shape_as(shape, ShapeId::new(), matrix)?),
        Content::Instance(instance) => {
            let moved = matrix * instance.transform().as_affine();
            let transform = Transform2D::new(moved).ok()?;
            Content::Instance(Instance::new(
                InstanceId::new(),
                instance.symbol(),
                transform,
            ))
        }
    })
}

/// A new layer holding one span, as the commands that add it.
fn new_layer_with(scene: SceneId, name: &str, span: FrameSpan) -> Option<(Vec<Command>, SpanRef)> {
    let layer_id = LayerId::new();
    let layer = Layer::new(layer_id, name, vec![]).ok()?;
    let at = SpanRef {
        layer: layer_id,
        start: span.start(),
    };
    Some((
        vec![
            Command::AddLayer { scene, layer },
            Command::InsertSpan {
                scene,
                layer: layer_id,
                span,
            },
        ],
        at,
    ))
}

/// Copies a span onto a new layer above the others, moved by `offset`, with fresh shape and
/// instance identities so the copy is independent. Returns the commands and where the copy
/// lands. `None` when the span is missing, empty, or the result is not finite.
pub fn duplicate(
    document: &Document,
    scene: SceneId,
    at: SpanRef,
    offset: DVec2,
) -> Option<(Vec<Command>, SpanRef)> {
    let timeline = scene_timeline(document, scene)?;
    let source = timeline.layers().iter().find(|l| l.id() == at.layer)?;
    let span = find_span(document, scene, at)?;
    let content = fresh_content(span.content(), DAffine2::from_translation(offset))?;
    let copy = FrameSpan::new(span.start(), span.duration(), content).with_tween(span.tween());
    new_layer_with(scene, &format!("{} copy", source.name()), copy)
}

/// What Copy remembers about a span, so it can be pasted later or into another document.
#[derive(Clone, Debug, PartialEq)]
pub struct ClipItem {
    name: String,
    duration: NonZeroU32,
    tween: Tween,
    content: Content,
}

/// Remembers a span for pasting. `None` when it is missing or empty.
pub fn copy_span(document: &Document, scene: SceneId, at: SpanRef) -> Option<ClipItem> {
    let timeline = scene_timeline(document, scene)?;
    let source = timeline.layers().iter().find(|l| l.id() == at.layer)?;
    let span = find_span(document, scene, at)?;
    if matches!(span.content(), Content::Empty) {
        return None;
    }
    Some(ClipItem {
        name: source.name().to_owned(),
        duration: span.duration(),
        tween: span.tween(),
        content: span.content().clone(),
    })
}

/// Pastes a remembered span in place on a new top layer, starting at `frame`, with fresh
/// identities. `None` when an instance's symbol is not in this document (pasting into a
/// document that lacks it would be invalid) or the frame range does not fit.
pub fn paste(
    document: &Document,
    scene: SceneId,
    item: &ClipItem,
    frame: u32,
) -> Option<(Vec<Command>, SpanRef)> {
    if let Content::Instance(instance) = &item.content
        && document.symbol(instance.symbol()).is_none()
    {
        return None;
    }
    frame.checked_add(item.duration.get())?;
    let content = fresh_content(&item.content, DAffine2::IDENTITY)?;
    let span = FrameSpan::new(frame, item.duration, content).with_tween(item.tween);
    new_layer_with(scene, &item.name, span)
}

/// Sets or clears the motion tween on an instance span. `None` when the span is missing,
/// is not an instance (only transforms tween), or already has that setting.
pub fn set_tween(
    document: &Document,
    scene: SceneId,
    at: SpanRef,
    tween: Tween,
) -> Option<Vec<Command>> {
    let span = find_span(document, scene, at)?;
    if !matches!(span.content(), Content::Instance(_)) || span.tween() == tween {
        return None;
    }
    let replacement =
        FrameSpan::new(span.start(), span.duration(), span.content().clone()).with_tween(tween);
    Some(vec![
        Command::RemoveSpan {
            scene,
            layer: at.layer,
            start: at.start,
        },
        Command::InsertSpan {
            scene,
            layer: at.layer,
            span: replacement,
        },
    ])
}

/// Turns a shape span into a Graphic symbol plus an identity instance in its place.
/// Returns the commands (one atomic batch) and the new symbol's ID.
pub fn convert_to_symbol(
    document: &Document,
    scene: SceneId,
    at: SpanRef,
) -> Option<(Vec<Command>, SymbolId)> {
    let span = find_span(document, scene, at)?;
    let Content::Shape(shape) = span.content() else {
        return None;
    };
    let symbol_id = SymbolId::new();
    let inner = FrameSpan::new(0, NonZeroU32::MIN, Content::Shape(shape.clone()));
    let layer = Layer::new(LayerId::new(), "Layer 1", vec![inner]).ok()?;
    let name = format!("Symbol {}", document.symbols().len() + 1);
    let symbol = Symbol::new(symbol_id, name, Timeline::new(vec![layer]));
    let instance = Instance::new(InstanceId::new(), symbol_id, Transform2D::IDENTITY);
    let outer = FrameSpan::new(span.start(), span.duration(), Content::Instance(instance));
    Some((
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
        symbol_id,
    ))
}

/// Control-polygon bounds of the shapes in a symbol's first frame (one level deep).
pub fn symbol_bounds(document: &Document, symbol: SymbolId) -> Option<Rect> {
    symbol_bounds_nested(document, symbol, 0)
}

/// How many levels of symbols inside symbols are followed. Documents reject symbol cycles,
/// so this only bounds work on deep but legal nesting.
const MAX_BOUNDS_DEPTH: usize = 32;

/// Bounds of the artwork at frame 0, following nested instances through their transforms.
fn symbol_bounds_nested(document: &Document, symbol: SymbolId, depth: usize) -> Option<Rect> {
    if depth > MAX_BOUNDS_DEPTH {
        return None;
    }
    let timeline = document.symbol(symbol)?.timeline();
    let mut bounds: Option<Rect> = None;
    for layer in timeline.layers() {
        let rect = match layer.span_at(0).map(FrameSpan::content) {
            Some(Content::Shape(shape)) => shape_bounds(shape),
            Some(Content::Instance(instance)) => {
                symbol_bounds_nested(document, instance.symbol(), depth + 1)
                    .map(|inner| transformed(inner, instance.transform().as_affine()))
            }
            _ => None,
        };
        if let Some(rect) = rect {
            bounds = Some(bounds.map_or(rect, |b| b.union(rect)));
        }
    }
    bounds
}

/// The axis-aligned box around `rect` after `matrix`.
fn transformed(rect: Rect, matrix: DAffine2) -> Rect {
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

/// Whether `point` is inside the closed polygon `polygon` (even-odd rule).
pub fn polygon_contains(polygon: &[Pos2], point: Pos2) -> bool {
    let mut inside = false;
    let mut previous = match polygon.last() {
        Some(p) => *p,
        None => return false,
    };
    for &current in polygon {
        if (current.y > point.y) != (previous.y > point.y)
            && point.x
                < (previous.x - current.x) * (point.y - current.y) / (previous.y - current.y)
                    + current.x
        {
            inside = !inside;
        }
        previous = current;
    }
    inside
}

/// The spans on unlocked layers at `frame` whose bounds are centered inside `polygon`: what a
/// lasso drawn around them selects, topmost layer first.
pub fn spans_in_polygon(
    document: &Document,
    scene: SceneId,
    frame: u32,
    polygon: &[Pos2],
) -> Vec<SpanRef> {
    if polygon.len() < 3 {
        return Vec::new();
    }
    let Some(timeline) = scene_timeline(document, scene) else {
        return Vec::new();
    };
    timeline
        .layers()
        .iter()
        .filter(|layer| !layer.is_locked())
        .filter_map(|layer| {
            let span = layer.span_at(frame)?;
            let center = content_bounds(document, span.content())?.center();
            polygon_contains(polygon, center).then_some(SpanRef {
                layer: layer.id(),
                start: span.start(),
            })
        })
        .collect()
}

/// Every span with something to show on unlocked layers at `frame`: what Select All selects.
pub fn spans_at_frame(document: &Document, scene: SceneId, frame: u32) -> Vec<SpanRef> {
    let Some(timeline) = scene_timeline(document, scene) else {
        return Vec::new();
    };
    timeline
        .layers()
        .iter()
        .filter(|layer| !layer.is_locked())
        .filter_map(|layer| {
            let span = layer.span_at(frame)?;
            (!matches!(span.content(), Content::Empty)).then_some(SpanRef {
                layer: layer.id(),
                start: span.start(),
            })
        })
        .collect()
}

/// Which edge or center the Align panel lines shapes up on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    CenterH,
    Right,
    Top,
    CenterV,
    Bottom,
}

/// How far each of `rects` moves to line up on `target`'s edge or center.
pub fn align_deltas(rects: &[Rect], how: Align, target: Rect) -> Vec<egui::Vec2> {
    rects
        .iter()
        .map(|r| match how {
            Align::Left => egui::vec2(target.left() - r.left(), 0.0),
            Align::CenterH => egui::vec2(target.center().x - r.center().x, 0.0),
            Align::Right => egui::vec2(target.right() - r.right(), 0.0),
            Align::Top => egui::vec2(0.0, target.top() - r.top()),
            Align::CenterV => egui::vec2(0.0, target.center().y - r.center().y),
            Align::Bottom => egui::vec2(0.0, target.bottom() - r.bottom()),
        })
        .collect()
}

/// How far each of `rects` moves to leave equal gaps between neighbours along the axis, the two
/// outermost staying put. Fewer than three rectangles have nothing to distribute.
pub fn distribute_deltas(rects: &[Rect], horizontal: bool) -> Vec<egui::Vec2> {
    let mut deltas = vec![egui::Vec2::ZERO; rects.len()];
    if rects.len() < 3 {
        return deltas;
    }
    let span = |r: &Rect| {
        if horizontal {
            (r.left(), r.right())
        } else {
            (r.top(), r.bottom())
        }
    };
    let lo = |r: &Rect| span(r).0;
    let hi = |r: &Rect| span(r).1;
    let size = |r: &Rect| hi(r) - lo(r);
    let mut order: Vec<usize> = (0..rects.len()).collect();
    order.sort_by(|a, b| lo(&rects[*a]).total_cmp(&lo(&rects[*b])));
    let first = lo(&rects[order[0]]);
    let last = order
        .iter()
        .map(|i| hi(&rects[*i]))
        .fold(f32::MIN, f32::max);
    let total: f32 = rects.iter().map(size).sum();
    let gap = (last - first - total) / (rects.len() - 1) as f32;
    let mut at = first;
    for index in order {
        let shift = at - lo(&rects[index]);
        deltas[index] = if horizontal {
            egui::vec2(shift, 0.0)
        } else {
            egui::vec2(0.0, shift)
        };
        at += size(&rects[index]) + gap;
    }
    deltas
}

/// Stage-space bounds of a selected span, for drawing the selection outline.
pub fn span_bounds(document: &Document, scene: SceneId, at: SpanRef) -> Option<Rect> {
    content_bounds(document, find_span(document, scene, at)?.content())
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
            content_bounds(document, span.content())
        })
        .collect()
}

fn content_bounds(document: &Document, content: &Content) -> Option<Rect> {
    match content {
        Content::Empty => None,
        Content::Shape(shape) => shape_bounds(shape),
        Content::Instance(instance) => {
            let local = symbol_bounds(document, instance.symbol())?;
            Some(transformed(local, instance.transform().as_affine()))
        }
    }
}

/// Places a symbol on a new layer, centered on `at`, for the whole scene length.
pub fn place_instance(
    document: &Document,
    scene: SceneId,
    symbol: SymbolId,
    at: DVec2,
) -> Option<Vec<Command>> {
    let timeline = scene_timeline(document, scene)?;
    let center = symbol_bounds(document, symbol).map_or(DVec2::ZERO, |b| {
        DVec2::new(f64::from(b.center().x), f64::from(b.center().y))
    });
    let transform = Transform2D::new(DAffine2::from_translation(at - center)).ok()?;
    let duration = u32::try_from(timeline.frame_count())
        .unwrap_or(u32::MAX)
        .max(24);
    let instance = Instance::new(InstanceId::new(), symbol, transform);
    let span = FrameSpan::new(0, NonZeroU32::new(duration)?, Content::Instance(instance));
    let layer_id = LayerId::new();
    let layer = Layer::new(layer_id, "Instance", vec![]).ok()?;
    Some(vec![
        Command::AddLayer { scene, layer },
        Command::InsertSpan {
            scene,
            layer: layer_id,
            span,
        },
    ])
}

#[cfg(test)]
mod tests;
