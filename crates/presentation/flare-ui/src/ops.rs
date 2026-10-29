//! Selection-based editing operations as pure functions from a document to commands.
//!
//! Nothing here touches egui or the session, so every operation is unit-testable and the
//! UI only decides *which* operation to run. Selections are frame spans, identified by
//! their layer and start frame (spans on a layer never overlap, so this is unique).

use std::num::NonZeroU32;

use egui::{Pos2, Rect};
use flare_core::{DocumentPoint, InstanceId, LayerId, SceneId, SymbolId, Transform2D};
use flare_document::{
    Content, Document, FrameSpan, Instance, Layer, PathSegment, Shape, Symbol, Timeline, Tween,
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

fn translate_shape(shape: &Shape, delta: DVec2) -> Option<Shape> {
    let moved =
        |p: &DocumentPoint| DocumentPoint::new(p.as_vec().x + delta.x, p.as_vec().y + delta.y).ok();
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
    let moved = Shape::new(shape.id(), path, shape.fill()).ok()?;
    // Rebuilding must not drop the stroke.
    Some(match shape.stroke() {
        Some(stroke) => moved.with_stroke(stroke),
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
    let span = find_span(document, scene, at)?;
    let content = match span.content() {
        Content::Empty => return None,
        Content::Shape(shape) => Content::Shape(translate_shape(shape, delta)?),
        Content::Instance(instance) => {
            let moved = DAffine2::from_translation(delta) * instance.transform().as_affine();
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

/// Stage-space bounds of a selected span, for drawing the selection outline.
pub fn span_bounds(document: &Document, scene: SceneId, at: SpanRef) -> Option<Rect> {
    match find_span(document, scene, at)?.content() {
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
#[allow(clippy::expect_used)]
mod tests {
    use flare_core::{DocumentId, FrameRate, Rgba8, ShapeId};
    use flare_document::Scene;
    use flare_edit::EditorSession;

    use super::*;

    const RED: Rgba8 = Rgba8::opaque(255, 0, 0);

    struct Fixture {
        session: EditorSession,
        scene: SceneId,
        layer: LayerId,
    }

    fn fixture() -> Fixture {
        let (scene, layer) = (SceneId::new(), LayerId::new());
        let layer_value = Layer::new(layer, "L", vec![]).expect("layer");
        let scene_value = Scene::new(scene, "S", Timeline::new(vec![layer_value]));
        let rate = FrameRate::new(24, 1).expect("rate");
        let doc = Document::new(DocumentId::new(), rate, vec![scene_value], vec![]).expect("doc");
        Fixture {
            session: EditorSession::new(doc),
            scene,
            layer,
        }
    }

    fn rect_span(start: u32, duration: u32, x: f64, y: f64, w: f64, h: f64) -> FrameSpan {
        let shape = Shape::rect(ShapeId::new(), x, y, w, h, RED).expect("rect");
        FrameSpan::new(
            start,
            NonZeroU32::new(duration).expect("nonzero"),
            Content::Shape(shape),
        )
    }

    fn add(f: &mut Fixture, span: FrameSpan) {
        let command = Command::InsertSpan {
            scene: f.scene,
            layer: f.layer,
            span,
        };
        f.session.apply([command]).expect("insert");
    }

    fn at(f: &Fixture, start: u32) -> SpanRef {
        SpanRef {
            layer: f.layer,
            start,
        }
    }

    fn bounds(f: &Fixture, start: u32) -> Rect {
        span_bounds(f.session.document(), f.scene, at(f, start)).expect("bounds")
    }

    #[test]
    fn translating_a_shape_moves_it_and_keeps_duration_and_identity() {
        let mut f = fixture();
        add(&mut f, rect_span(3, 10, 0.0, 0.0, 20.0, 10.0));
        let before = f.session.document().clone();
        let commands = translate(&before, f.scene, at(&f, 3), DVec2::new(5.0, -2.0)).expect("move");
        f.session.apply(commands).expect("apply");
        let span = find_span(f.session.document(), f.scene, at(&f, 3)).expect("span");
        assert_eq!(span.duration().get(), 10);
        let old = find_span(&before, f.scene, at(&f, 3)).expect("old");
        let (Content::Shape(a), Content::Shape(b)) = (old.content(), span.content()) else {
            panic!("shapes expected");
        };
        assert_eq!(a.id(), b.id());
        assert_eq!(
            bounds(&f, 3),
            Rect::from_min_max(Pos2::new(5.0, -2.0), Pos2::new(25.0, 8.0))
        );
        f.session.undo();
        assert_eq!(f.session.document(), &before);
    }

    #[test]
    fn translating_an_instance_composes_the_translation_and_keeps_tween() {
        let mut f = fixture();
        let symbol = SymbolId::new();
        let rotate =
            Transform2D::new(DAffine2::from_angle(std::f64::consts::FRAC_PI_2)).expect("finite");
        let instance = Instance::new(InstanceId::new(), symbol, rotate);
        let span = FrameSpan::new(
            0,
            NonZeroU32::new(5).expect("nonzero"),
            Content::Instance(instance),
        )
        .with_tween(Tween::Linear);
        f.session
            .apply([Command::AddSymbol(Symbol::new(
                symbol,
                "S",
                Timeline::default(),
            ))])
            .expect("symbol");
        add(&mut f, span);
        let commands = translate(
            f.session.document(),
            f.scene,
            at(&f, 0),
            DVec2::new(10.0, 0.0),
        )
        .expect("move");
        f.session.apply(commands).expect("apply");
        let moved = find_span(f.session.document(), f.scene, at(&f, 0)).expect("span");
        assert_eq!(moved.tween(), Tween::Linear);
        let Content::Instance(instance) = moved.content() else {
            panic!("instance expected")
        };
        let affine = instance.transform().as_affine();
        assert_eq!(affine.translation, DVec2::new(10.0, 0.0));
        assert!(
            (affine.matrix2.x_axis.y - 1.0).abs() < 1e-12,
            "rotation is preserved"
        );
    }

    #[test]
    fn degenerate_translations_do_nothing() {
        let mut f = fixture();
        add(&mut f, rect_span(0, 5, 0.0, 0.0, 5.0, 5.0));
        let doc = f.session.document();
        assert!(translate(doc, f.scene, at(&f, 0), DVec2::ZERO).is_none());
        assert!(translate(doc, f.scene, at(&f, 99), DVec2::ONE).is_none());
        assert!(translate(doc, f.scene, at(&f, 0), DVec2::new(f64::NAN, 0.0)).is_none());
    }

    #[test]
    fn convert_to_symbol_replaces_the_shape_with_an_equivalent_instance() {
        let mut f = fixture();
        add(&mut f, rect_span(2, 8, 30.0, 40.0, 10.0, 10.0));
        let before = flare_eval::evaluate_frame(f.session.document(), f.scene, 4).expect("eval");
        let before_id = before.shapes[0].shape.id();
        let (commands, symbol) =
            convert_to_symbol(f.session.document(), f.scene, at(&f, 2)).expect("convert");
        f.session.apply(commands).expect("apply");

        let doc = f.session.document();
        assert_eq!(doc.symbols().len(), 1);
        assert_eq!(doc.symbols()[0].name(), "Symbol 1");
        let span = find_span(doc, f.scene, at(&f, 2)).expect("span");
        assert_eq!(span.duration().get(), 8);
        assert!(matches!(span.content(), Content::Instance(i) if i.symbol() == symbol));
        // Same artwork on stage: same shape, identity transform, one nesting level.
        let after = flare_eval::evaluate_frame(doc, f.scene, 4).expect("eval");
        assert_eq!(after.shapes.len(), 1);
        assert_eq!(after.shapes[0].shape.id(), before_id);
        assert_eq!(after.shapes[0].transform, DAffine2::IDENTITY);
        assert_eq!(after.shapes[0].path.len(), 1);
        // Undo restores the original in one step.
        f.session.undo();
        assert_eq!(f.session.document().symbols().len(), 0);
    }

    #[test]
    fn only_shapes_can_be_converted() {
        let mut f = fixture();
        let symbol = SymbolId::new();
        f.session
            .apply([Command::AddSymbol(Symbol::new(
                symbol,
                "S",
                Timeline::default(),
            ))])
            .expect("symbol");
        let instance = Instance::new(InstanceId::new(), symbol, Transform2D::IDENTITY);
        let span = FrameSpan::new(0, NonZeroU32::MIN, Content::Instance(instance));
        add(&mut f, span);
        assert!(convert_to_symbol(f.session.document(), f.scene, at(&f, 0)).is_none());
        assert!(convert_to_symbol(f.session.document(), f.scene, at(&f, 5)).is_none());
    }

    #[test]
    fn placing_an_instance_centers_the_symbol_on_the_target_point() {
        let mut f = fixture();
        add(&mut f, rect_span(0, 24, 0.0, 0.0, 10.0, 10.0));
        let (commands, symbol) =
            convert_to_symbol(f.session.document(), f.scene, at(&f, 0)).expect("convert");
        f.session.apply(commands).expect("convert");
        let commands = place_instance(
            f.session.document(),
            f.scene,
            symbol,
            DVec2::new(100.0, 60.0),
        )
        .expect("place");
        f.session.apply(commands).expect("place");
        let scene = &f.session.document().scenes()[0];
        let new_layer = scene.timeline().layers().last().expect("layer").id();
        let placed = span_bounds(
            f.session.document(),
            f.scene,
            SpanRef {
                layer: new_layer,
                start: 0,
            },
        )
        .expect("bounds");
        assert_eq!(placed.center(), Pos2::new(100.0, 60.0));
        assert_eq!(scene.timeline().layers().len(), 2);
    }

    #[test]
    fn hits_resolve_to_the_owning_scene_level_span() {
        let mut f = fixture();
        add(&mut f, rect_span(0, 24, 0.0, 0.0, 10.0, 10.0));
        let (commands, symbol) =
            convert_to_symbol(f.session.document(), f.scene, at(&f, 0)).expect("convert");
        f.session.apply(commands).expect("convert");
        let shape_layer = f.layer;
        let commands = place_instance(
            f.session.document(),
            f.scene,
            symbol,
            DVec2::new(100.0, 100.0),
        )
        .expect("place");
        f.session.apply(commands).expect("place");

        let doc = f.session.document();
        let frame = flare_eval::evaluate_frame(doc, f.scene, 3).expect("eval");
        // Paint order: the new layer (last) is drawn first, so the original instance is on top.
        for drawn in &frame.shapes {
            let found = locate_hit(doc, f.scene, 3, drawn).expect("located");
            assert_eq!(found.start, 0);
            assert!(
                doc.scenes()[0]
                    .timeline()
                    .layers()
                    .iter()
                    .any(|l| l.id() == found.layer)
            );
        }
        assert!(
            frame
                .shapes
                .iter()
                .any(|d| locate_hit(doc, f.scene, 3, d).is_some_and(|r| r.layer == shape_layer))
        );
    }

    #[test]
    fn stroke_width_is_part_of_the_bounds_and_survives_moves() {
        use flare_document::Stroke;
        let mut f = fixture();
        let stroke = Stroke::new(6.0, RED).expect("stroke");
        let shape = Shape::rect(ShapeId::new(), 10.0, 10.0, 20.0, 20.0, RED)
            .expect("rect")
            .with_stroke(stroke);
        let span = FrameSpan::new(0, NonZeroU32::MIN, Content::Shape(shape));
        add(&mut f, span);
        assert_eq!(
            bounds(&f, 0),
            Rect::from_min_max(Pos2::new(7.0, 7.0), Pos2::new(33.0, 33.0))
        );
        let commands = translate(
            f.session.document(),
            f.scene,
            at(&f, 0),
            DVec2::new(1.0, 1.0),
        )
        .expect("move");
        f.session.apply(commands).expect("apply");
        let moved = find_span(f.session.document(), f.scene, at(&f, 0)).expect("span");
        let Content::Shape(shape) = moved.content() else {
            panic!("shape expected")
        };
        assert_eq!(shape.stroke(), Some(stroke), "moving keeps the stroke");
    }

    #[test]
    fn tweens_toggle_on_instances_only() {
        let mut f = fixture();
        let symbol = SymbolId::new();
        f.session
            .apply([Command::AddSymbol(Symbol::new(
                symbol,
                "S",
                Timeline::default(),
            ))])
            .expect("symbol");
        let instance = Instance::new(InstanceId::new(), symbol, Transform2D::IDENTITY);
        add(
            &mut f,
            FrameSpan::new(
                0,
                NonZeroU32::new(4).expect("nonzero"),
                Content::Instance(instance),
            ),
        );
        add(&mut f, rect_span(4, 2, 0.0, 0.0, 5.0, 5.0));
        let doc = f.session.document();
        assert!(
            set_tween(doc, f.scene, at(&f, 4), Tween::Linear).is_none(),
            "shapes do not tween"
        );
        assert!(
            set_tween(doc, f.scene, at(&f, 0), Tween::None).is_none(),
            "already untweened"
        );
        let commands = set_tween(doc, f.scene, at(&f, 0), Tween::Linear).expect("enable");
        f.session.apply(commands).expect("apply");
        let span = find_span(f.session.document(), f.scene, at(&f, 0)).expect("span");
        assert_eq!((span.tween(), span.duration().get()), (Tween::Linear, 4));
        let commands =
            set_tween(f.session.document(), f.scene, at(&f, 0), Tween::None).expect("disable");
        f.session.apply(commands).expect("apply");
        assert_eq!(
            find_span(f.session.document(), f.scene, at(&f, 0))
                .expect("span")
                .tween(),
            Tween::None
        );
    }

    #[test]
    fn deleting_removes_the_span() {
        let mut f = fixture();
        add(&mut f, rect_span(0, 5, 0.0, 0.0, 5.0, 5.0));
        f.session.apply(delete(f.scene, at(&f, 0))).expect("delete");
        assert!(find_span(f.session.document(), f.scene, at(&f, 0)).is_none());
    }

    #[test]
    fn symbols_made_only_of_other_symbols_have_bounds_through_their_transforms() {
        let (inner, outer) = (SymbolId::new(), SymbolId::new());
        let tl = |spans: Vec<FrameSpan>| {
            Timeline::new(vec![Layer::new(LayerId::new(), "L", spans).expect("layer")])
        };
        let shift =
            Transform2D::new(DAffine2::from_translation(DVec2::new(100.0, 20.0))).expect("finite");
        let placed = FrameSpan::new(
            0,
            NonZeroU32::new(1).expect("nonzero"),
            Content::Instance(Instance::new(InstanceId::new(), inner, shift)),
        );
        let symbols = vec![
            Symbol::new(
                inner,
                "Inner",
                tl(vec![rect_span(0, 1, 0.0, 0.0, 10.0, 5.0)]),
            ),
            Symbol::new(outer, "Outer", tl(vec![placed])),
        ];
        let rate = FrameRate::new(24, 1).expect("rate");
        let scene = Scene::new(SceneId::new(), "S", Timeline::default());
        let doc = Document::new(DocumentId::new(), rate, vec![scene], symbols).expect("doc");
        let bounds = symbol_bounds(&doc, outer).expect("nested bounds");
        assert_eq!(
            (bounds.min, bounds.max),
            (Pos2::new(100.0, 20.0), Pos2::new(110.0, 25.0))
        );
        assert!(
            symbol_bounds(&doc, SymbolId::new()).is_none(),
            "unknown symbol"
        );
    }
}
