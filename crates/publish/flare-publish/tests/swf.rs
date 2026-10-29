//! Publish documents and check the SWF by parsing it back with the `swf` crate (the reader
//! the Ruffle project uses), simulating its display list, and re-rendering what it decodes.
//! This shows the file is well-formed and says what we meant. It does not show how Flash
//! Player or Ruffle would play it.
#![allow(clippy::expect_used)]

use std::collections::HashMap;
use std::num::NonZeroU32;

use flare_core::{
    DocumentId, DocumentPoint, FrameRate, InstanceId, LayerId, Rgba8, SceneId, ShapeId, SymbolId,
    Transform2D,
};
use flare_document::{
    Content, Document, FrameSpan, Instance, Layer, PathSegment, Scene, Shape, Stage, Stroke,
    Symbol, Timeline, Tween,
};
use flare_publish::{PublishError, PublishOptions, Severity, publish_swf};
use glamx::{DAffine2, DVec2};
use swf::{FillStyle, PlaceObjectAction, ShapeRecord, Tag};

// ---- building documents ---------------------------------------------------------------

fn span(start: u32, duration: u32, content: Content) -> FrameSpan {
    FrameSpan::new(start, NonZeroU32::new(duration).expect("nonzero"), content)
}

fn layer(name: &str, spans: Vec<FrameSpan>) -> Layer {
    Layer::new(LayerId::new(), name, spans).expect("layer")
}

fn pt(x: f64, y: f64) -> DocumentPoint {
    DocumentPoint::new(x, y).expect("finite")
}

fn rect(x: f64, y: f64, w: f64, h: f64, fill: Rgba8) -> Shape {
    Shape::rect(ShapeId::new(), x, y, w, h, fill).expect("rect")
}

fn instance(symbol: SymbolId, x: f64, y: f64) -> Content {
    let transform = Transform2D::new(DAffine2::from_translation(DVec2::new(x, y))).expect("finite");
    Content::Instance(Instance::new(InstanceId::new(), symbol, transform))
}

fn document(width: u32, height: u32, scenes: Vec<Scene>, symbols: Vec<Symbol>) -> Document {
    let rate = FrameRate::new(12, 1).expect("rate");
    let stage = Stage::new(width, height, Rgba8::opaque(16, 32, 48)).expect("stage");
    Document::new(DocumentId::new(), rate, scenes, symbols)
        .expect("doc")
        .with_stage(stage)
}

/// A square ring with a square hole; `flip` winds both contours the other way.
fn ring(flip: bool) -> Shape {
    let outer = [(10.0, 10.0), (50.0, 10.0), (50.0, 50.0), (10.0, 50.0)];
    let inner = [(20.0, 20.0), (20.0, 40.0), (40.0, 40.0), (40.0, 20.0)];
    let wind = |pts: [(f64, f64); 4]| -> Vec<(f64, f64)> {
        let mut v = pts.to_vec();
        if flip {
            v.reverse();
        }
        v
    };
    let (outer, inner) = (wind(outer), wind(inner));
    let mut path = vec![PathSegment::MoveTo(pt(outer[0].0, outer[0].1))];
    path.extend(outer[1..].iter().map(|p| PathSegment::LineTo(pt(p.0, p.1))));
    path.push(PathSegment::Close);
    path.push(PathSegment::MoveTo(pt(inner[0].0, inner[0].1)));
    path.extend(inner[1..].iter().map(|p| PathSegment::LineTo(pt(p.0, p.1))));
    path.push(PathSegment::Close);
    Shape::new(ShapeId::new(), path, Rgba8::opaque(255, 128, 0)).expect("ring")
}

fn one_frame_scene(name: &str, shapes: Vec<Shape>, frames: u32) -> Scene {
    let layers = shapes
        .into_iter()
        .enumerate()
        .map(|(i, s)| layer(&format!("L{i}"), vec![span(0, frames, Content::Shape(s))]))
        .collect();
    Scene::new(SceneId::new(), name, Timeline::new(layers))
}

fn animated() -> Document {
    let dot = SymbolId::new();
    let symbol_layer = layer(
        "L",
        vec![span(
            0,
            1,
            Content::Shape(rect(0.0, 0.0, 10.0, 10.0, Rgba8::opaque(255, 0, 0))),
        )],
    );
    let symbols = vec![Symbol::new(dot, "Dot", Timeline::new(vec![symbol_layer]))];
    let mover = layer(
        "Mover",
        vec![
            span(0, 5, instance(dot, 10.0, 20.0)).with_tween(Tween::Linear),
            span(5, 5, instance(dot, 110.0, 20.0)),
        ],
    );
    let twin = layer("Twin", vec![span(0, 10, instance(dot, 60.0, 60.0))]);
    let disc = Shape::ellipse(
        ShapeId::new(),
        40.0,
        40.0,
        15.0,
        15.0,
        Rgba8::opaque(0, 255, 0),
    )
    .expect("ellipse")
    .with_stroke(Stroke::new(3.0, Rgba8::opaque(0, 0, 0)).expect("stroke"));
    let discs = layer("Disc", vec![span(0, 10, Content::Shape(disc))]);
    let hidden = layer(
        "Hidden",
        vec![span(
            0,
            10,
            Content::Shape(rect(0.0, 0.0, 50.0, 50.0, Rgba8::WHITE)),
        )],
    )
    .with_visible(false);
    let scene = Scene::new(
        SceneId::new(),
        "S",
        Timeline::new(vec![mover, twin, discs, hidden]),
    );
    document(160, 100, vec![scene], symbols)
}

// ---- reading the SWF back -------------------------------------------------------------

struct Parsed {
    version: u8,
    stage: (i32, i32),
    frame_rate: f64,
    declared_frames: u16,
    background: Option<(u8, u8, u8)>,
    tags: Vec<Tag<'static>>,
}

fn parse(bytes: &[u8]) -> Parsed {
    let buf = swf::decompress_swf(bytes).expect("decompress");
    let parsed = swf::parse_swf(&buf).expect("the SWF parses");
    let header = &parsed.header;
    let stage = header.stage_size();
    Parsed {
        version: header.version(),
        stage: (stage.x_max.get(), stage.y_max.get()),
        frame_rate: header.frame_rate().to_f64(),
        declared_frames: header.num_frames(),
        background: header.background_color().map(|c| (c.r, c.g, c.b)),
        // Tags borrow from `buf`, so keep only the ones these tests inspect, as owned values.
        tags: owned_tags(parsed.tags),
    }
}

/// The tags this test inspects, detached from the parse buffer. Only the variants used here
/// are kept; the rest are irrelevant to the checks.
fn owned_tags(tags: Vec<Tag<'_>>) -> Vec<Tag<'static>> {
    tags.into_iter()
        .filter_map(|tag| match tag {
            Tag::DefineShape(shape) => Some(Tag::DefineShape(shape)),
            Tag::RemoveObject(r) => Some(Tag::RemoveObject(r)),
            Tag::ShowFrame => Some(Tag::ShowFrame),
            Tag::PlaceObject(p) => {
                let p = *p;
                Some(Tag::PlaceObject(Box::new(swf::PlaceObject {
                    version: p.version,
                    action: p.action,
                    depth: p.depth,
                    matrix: p.matrix,
                    color_transform: None,
                    ratio: None,
                    name: None,
                    clip_depth: None,
                    class_name: None,
                    filters: None,
                    background_color: None,
                    blend_mode: None,
                    clip_actions: None,
                    has_image: false,
                    is_bitmap_cached: None,
                    is_visible: None,
                    amf_data: None,
                })))
            }
            _ => None,
        })
        .collect()
}

#[derive(Clone)]
struct OnScreen {
    character: u16,
    matrix: swf::Matrix,
}

/// Plays the display list and returns what is on screen after each ShowFrame, bottom first.
fn display_lists(tags: &[Tag<'static>]) -> Vec<Vec<(u16, OnScreen)>> {
    let mut depths: std::collections::BTreeMap<u16, OnScreen> = std::collections::BTreeMap::new();
    let mut frames = Vec::new();
    for tag in tags {
        match tag {
            Tag::PlaceObject(p) => match p.action {
                PlaceObjectAction::Place(c) | PlaceObjectAction::Replace(c) => {
                    let matrix = p.matrix.unwrap_or(swf::Matrix::IDENTITY);
                    depths.insert(
                        p.depth,
                        OnScreen {
                            character: c,
                            matrix,
                        },
                    );
                }
                PlaceObjectAction::Modify => {
                    let entry = depths.get_mut(&p.depth).expect("Modify of an empty depth");
                    if let Some(m) = p.matrix {
                        entry.matrix = m;
                    }
                }
            },
            Tag::RemoveObject(r) => {
                depths
                    .remove(&r.depth)
                    .expect("RemoveObject of an empty depth");
            }
            Tag::ShowFrame => frames.push(depths.iter().map(|(d, o)| (*d, o.clone())).collect()),
            _ => {}
        }
    }
    frames
}

fn to_affine(m: &swf::Matrix) -> DAffine2 {
    DAffine2::from_cols_array(&[
        m.a.to_f64(),
        m.b.to_f64(),
        m.c.to_f64(),
        m.d.to_f64(),
        m.tx.to_pixels(),
        m.ty.to_pixels(),
    ])
}

// ---- rebuilding shapes from SWF records ------------------------------------------------

type P = (i32, i32);

#[derive(Clone, Copy)]
struct Seg {
    from: P,
    ctrl: Option<P>,
    to: P,
    fill0: u32,
    fill1: u32,
    line: u32,
}

fn segments(shape: &swf::Shape) -> Vec<Seg> {
    let (mut at, mut f0, mut f1, mut line) = ((0, 0), 0, 0, 0);
    let mut out = Vec::new();
    for record in &shape.shape {
        match record {
            ShapeRecord::StyleChange(data) => {
                if let Some(m) = &data.move_to {
                    at = (m.x.get(), m.y.get());
                }
                f0 = data.fill_style_0.unwrap_or(f0);
                f1 = data.fill_style_1.unwrap_or(f1);
                line = data.line_style.unwrap_or(line);
            }
            ShapeRecord::StraightEdge { delta } => {
                let to = (at.0 + delta.dx.get(), at.1 + delta.dy.get());
                out.push(Seg {
                    from: at,
                    ctrl: None,
                    to,
                    fill0: f0,
                    fill1: f1,
                    line,
                });
                at = to;
            }
            ShapeRecord::CurvedEdge {
                control_delta,
                anchor_delta,
            } => {
                let ctrl = (at.0 + control_delta.dx.get(), at.1 + control_delta.dy.get());
                let to = (
                    ctrl.0 + anchor_delta.dx.get(),
                    ctrl.1 + anchor_delta.dy.get(),
                );
                out.push(Seg {
                    from: at,
                    ctrl: Some(ctrl),
                    to,
                    fill0: f0,
                    fill1: f1,
                    line,
                });
                at = to;
            }
        }
    }
    out
}

fn reversed(s: &Seg) -> Seg {
    Seg {
        from: s.to,
        to: s.from,
        ..*s
    }
}

/// Links segments end to start into contours (each segment used once).
fn chain(segs: &[Seg]) -> Vec<(Vec<Seg>, bool)> {
    let mut used = vec![false; segs.len()];
    let mut out = Vec::new();
    for first in 0..segs.len() {
        if used[first] {
            continue;
        }
        used[first] = true;
        let mut run = vec![segs[first]];
        let origin = segs[first].from;
        let mut closed = false;
        loop {
            let end = run[run.len() - 1].to;
            if end == origin {
                closed = true;
                break;
            }
            match (0..segs.len()).find(|i| !used[*i] && segs[*i].from == end) {
                Some(i) => {
                    used[i] = true;
                    run.push(segs[i]);
                }
                None => break,
            }
        }
        out.push((run, closed));
    }
    out
}

fn px(p: P) -> (f64, f64) {
    (f64::from(p.0) / 20.0, f64::from(p.1) / 20.0)
}

fn path_of(contours: &[(Vec<Seg>, bool)], close_always: bool) -> Vec<PathSegment> {
    let mut path = Vec::new();
    for (run, closed) in contours {
        let (x, y) = px(run[0].from);
        path.push(PathSegment::MoveTo(pt(x, y)));
        for s in run {
            let to = px(s.to);
            match s.ctrl {
                None => path.push(PathSegment::LineTo(pt(to.0, to.1))),
                Some(c) => {
                    let (p0, c, p1) = (px(s.from), px(c), to);
                    let third = |a: (f64, f64)| {
                        (a.0 + 2.0 / 3.0 * (c.0 - a.0), a.1 + 2.0 / 3.0 * (c.1 - a.1))
                    };
                    let (c1, c2) = (third(p0), third(p1));
                    path.push(PathSegment::CubicTo(
                        pt(c1.0, c1.1),
                        pt(c2.0, c2.1),
                        pt(to.0, to.1),
                    ));
                }
            }
        }
        if *closed || close_always {
            path.push(PathSegment::Close);
        }
    }
    path
}

/// Flare shapes equivalent to one SWF shape: one per fill style, then one for the strokes.
fn rebuild(shape: &swf::Shape) -> Vec<Shape> {
    let segs = segments(shape);
    let mut out = Vec::new();
    for (index, style) in shape.styles.fill_styles.iter().enumerate() {
        let id = u32::try_from(index + 1).expect("small");
        let FillStyle::Color(c) = style else {
            panic!("only solid fills are published")
        };
        let mut edges: Vec<Seg> = segs.iter().filter(|s| s.fill1 == id).copied().collect();
        edges.extend(segs.iter().filter(|s| s.fill0 == id).map(reversed));
        let path = path_of(&chain(&edges), true);
        if !path.is_empty() {
            out.push(
                Shape::new(ShapeId::new(), path, Rgba8::new(c.r, c.g, c.b, c.a)).expect("fill"),
            );
        }
    }
    for (index, style) in shape.styles.line_styles.iter().enumerate() {
        let id = u32::try_from(index + 1).expect("small");
        let FillStyle::Color(c) = style.fill_style() else {
            panic!("only solid strokes are published")
        };
        let edges: Vec<Seg> = segs.iter().filter(|s| s.line == id).copied().collect();
        let path = path_of(&chain(&edges), false);
        if !path.is_empty() {
            let stroke = Stroke::new(style.width().to_pixels(), Rgba8::new(c.r, c.g, c.b, c.a))
                .expect("stroke");
            out.push(
                Shape::new(ShapeId::new(), path, Rgba8::TRANSPARENT)
                    .expect("outline")
                    .with_stroke(stroke),
            );
        }
    }
    out
}

/// Re-renders frame `index` of the SWF from what the reader decoded.
fn render_swf_frame(parsed: &Parsed, index: usize, background: Rgba8) -> flare_render::Image {
    let mut by_character: HashMap<u16, Vec<Shape>> = HashMap::new();
    for tag in &parsed.tags {
        if let Tag::DefineShape(shape) = tag {
            by_character.insert(shape.id, rebuild(shape));
        }
    }
    let list = &display_lists(&parsed.tags)[index];
    let mut drawn = Vec::new();
    for (_, placed) in list {
        for shape in &by_character[&placed.character] {
            drawn.push(flare_eval::DrawnShape {
                path: Vec::new(),
                shape,
                transform: to_affine(&placed.matrix),
            });
        }
    }
    let frame = flare_eval::Frame {
        placed: Vec::new(),
        shapes: drawn,
    };
    flare_render::render(
        &frame,
        (parsed.stage.0 / 20) as u32,
        (parsed.stage.1 / 20) as u32,
        background,
    )
    .expect("render")
}

fn render_original(document: &Document, scene: usize, frame: u32) -> flare_render::Image {
    let id = document.scenes()[scene].id();
    let evaluated = flare_eval::evaluate_frame(document, id, frame).expect("eval");
    let stage = document.stage();
    flare_render::render(
        &evaluated,
        stage.width(),
        stage.height(),
        stage.background(),
    )
    .expect("render")
}

fn difference(a: &flare_render::Image, b: &flare_render::Image) -> (u8, f64) {
    let (mut max, mut sum) = (0_u8, 0_u64);
    for (x, y) in a.rgba().iter().zip(b.rgba()) {
        let d = x.abs_diff(*y);
        max = max.max(d);
        sum += u64::from(d);
    }
    (max, sum as f64 / a.rgba().len() as f64)
}

// ---- tests ----------------------------------------------------------------------------

#[test]
fn the_header_stage_frame_rate_background_and_frame_count_are_right() {
    let doc = animated();
    let published = publish_swf(&doc, &PublishOptions::default()).expect("publish");
    let parsed = parse(&published.bytes);
    assert_eq!(parsed.version, 10);
    assert_eq!(parsed.stage, (160 * 20, 100 * 20));
    assert!((parsed.frame_rate - 12.0).abs() < 1e-3);
    assert_eq!(parsed.background, Some((16, 32, 48)));
    assert_eq!(published.frames, 10);
    assert_eq!(parsed.declared_frames, 10);
    let shows = parsed
        .tags
        .iter()
        .filter(|t| matches!(t, Tag::ShowFrame))
        .count();
    assert_eq!(
        shows, 10,
        "the header's frame count matches the frames actually written"
    );
}

#[test]
fn the_display_list_matches_the_evaluator_frame_by_frame() {
    let doc = animated();
    let parsed = parse(
        &publish_swf(&doc, &PublishOptions::default())
            .expect("publish")
            .bytes,
    );
    let lists = display_lists(&parsed.tags);
    let scene = doc.scenes()[0].id();
    for (frame, list) in lists.iter().enumerate() {
        let evaluated = flare_eval::evaluate_frame(&doc, scene, frame as u32).expect("eval");
        assert_eq!(
            list.len(),
            evaluated.shapes.len(),
            "frame {frame}: same number of objects"
        );
        for ((depth, placed), drawn) in list.iter().zip(&evaluated.shapes) {
            assert_eq!(
                usize::from(*depth),
                list.iter().position(|(d, _)| d == depth).expect("pos") + 1
            );
            let (got, want) = (
                to_affine(&placed.matrix).to_cols_array(),
                drawn.transform.to_cols_array(),
            );
            for (g, w) in got.iter().zip(want) {
                assert!(
                    (g - w).abs() < 0.06,
                    "frame {frame}: matrix {got:?} vs {want:?}"
                );
            }
        }
    }
}

#[test]
fn shared_shapes_are_defined_once_and_hidden_layers_are_not_published() {
    let doc = animated();
    let parsed = parse(
        &publish_swf(&doc, &PublishOptions::default())
            .expect("publish")
            .bytes,
    );
    let defined: Vec<u16> = parsed
        .tags
        .iter()
        .filter_map(|t| {
            if let Tag::DefineShape(s) = t {
                Some(s.id)
            } else {
                None
            }
        })
        .collect();
    // The red square (used by two instances over ten frames) and the disc: two definitions.
    assert_eq!(defined.len(), 2, "{defined:?}");
    let mut unique = defined.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), defined.len(), "character ids are unique");
}

#[test]
fn rendering_the_decoded_swf_matches_rendering_the_document() {
    let doc = animated();
    let parsed = parse(
        &publish_swf(&doc, &PublishOptions::default())
            .expect("publish")
            .bytes,
    );
    let background = doc.stage().background();
    for frame in 0..10 {
        let (max, mean) = difference(
            &render_original(&doc, 0, frame),
            &render_swf_frame(&parsed, frame as usize, background),
        );
        // The disc is made of curves, so allow a little edge difference; it must stay tiny.
        assert!(
            max <= 60 && mean < 0.15,
            "frame {frame}: max {max}, mean {mean}"
        );
    }
    // A frame with only straight edges and whole-pixel positions would be exact; the tween
    // frames move the red square by whole pixels here, so check one precisely: the square.
    let img = render_swf_frame(&parsed, 0, background);
    assert_eq!(img.pixel(15, 25), Some(Rgba8::opaque(255, 0, 0)));
    assert_eq!(img.pixel(150, 90), Some(background));
}

#[test]
fn holes_stay_holes_whichever_way_the_contours_are_wound() {
    for flip in [false, true] {
        let doc = document(
            80,
            60,
            vec![one_frame_scene("S", vec![ring(flip)], 1)],
            vec![],
        );
        let parsed = parse(
            &publish_swf(&doc, &PublishOptions::default())
                .expect("publish")
                .bytes,
        );
        let image = render_swf_frame(&parsed, 0, doc.stage().background());
        assert_eq!(
            image.pixel(14, 30),
            Some(Rgba8::opaque(255, 128, 0)),
            "flip={flip}: the band is filled"
        );
        assert_eq!(
            image.pixel(30, 30),
            Some(doc.stage().background()),
            "flip={flip}: the middle is a hole"
        );
        let (max, mean) = difference(&render_original(&doc, 0, 0), &image);
        assert_eq!(
            (max, mean),
            (0, 0.0),
            "flip={flip}: identical to the original"
        );
    }
}

#[test]
fn stroked_closed_shapes_keep_all_four_sides_and_open_filled_shapes_are_closed() {
    let boxed = rect(10.0, 10.0, 40.0, 30.0, Rgba8::opaque(250, 250, 0))
        .with_stroke(Stroke::new(4.0, Rgba8::opaque(0, 0, 0)).expect("stroke"));
    // An open triangle with a fill but no Close: the fill must still be a solid triangle.
    let open = Shape::new(
        ShapeId::new(),
        vec![
            PathSegment::MoveTo(pt(60.0, 10.0)),
            PathSegment::LineTo(pt(90.0, 10.0)),
            PathSegment::LineTo(pt(90.0, 40.0)),
        ],
        Rgba8::opaque(0, 200, 200),
    )
    .expect("open");
    let doc = document(
        100,
        60,
        vec![one_frame_scene("S", vec![boxed, open], 1)],
        vec![],
    );
    let parsed = parse(
        &publish_swf(&doc, &PublishOptions::default())
            .expect("publish")
            .bytes,
    );
    let image = render_swf_frame(&parsed, 0, doc.stage().background());
    assert_eq!(
        image.pixel(10, 25),
        Some(Rgba8::opaque(0, 0, 0)),
        "left side (the closing edge) is stroked"
    );
    assert_eq!(
        image.pixel(30, 10),
        Some(Rgba8::opaque(0, 0, 0)),
        "top side"
    );
    assert_eq!(
        image.pixel(30, 25),
        Some(Rgba8::opaque(250, 250, 0)),
        "the inside"
    );
    let (max, mean) = difference(&render_original(&doc, 0, 0), &image);
    assert_eq!((max, mean), (0, 0.0));
}

fn document_of_distinct_shapes(count: u32) -> Document {
    let layers: Vec<Layer> = (0..count)
        .map(|i| {
            layer(
                &format!("L{i}"),
                vec![span(
                    0,
                    1,
                    Content::Shape(rect(0.0, 0.0, 1.0, 1.0, Rgba8::WHITE)),
                )],
            )
        })
        .collect();
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(layers));
    document(10, 10, vec![scene], vec![])
}

#[test]
fn running_out_of_shape_ids_is_an_error_not_a_corrupt_file() {
    // Ids run 1..=65 534; one more shape than that cannot be numbered.
    let result = publish_swf(
        &document_of_distinct_shapes(65_535),
        &PublishOptions::default(),
    );
    assert!(
        matches!(result, Err(PublishError::TooManyShapes)),
        "{:?}",
        result.map(|p| p.frames)
    );
    let at_the_limit = publish_swf(
        &document_of_distinct_shapes(65_534),
        &PublishOptions::default(),
    );
    assert!(at_the_limit.is_ok(), "exactly the limit still publishes");
}

#[test]
fn gradients_are_flattened_to_their_average_color_with_a_warning() {
    let gradient = flare_document::Gradient::linear(
        pt(0.0, 0.0),
        pt(20.0, 0.0),
        Rgba8::opaque(200, 0, 0),
        Rgba8::opaque(0, 100, 0),
    )
    .expect("gradient");
    let shape = rect(0.0, 0.0, 20.0, 20.0, Rgba8::TRANSPARENT).with_gradient(gradient);
    let doc = document(20, 20, vec![one_frame_scene("A", vec![shape], 1)], vec![]);
    let published = publish_swf(&doc, &PublishOptions::default()).expect("publish");
    let parsed = parse(&published.bytes);
    assert_eq!(
        render_swf_frame(&parsed, 0, doc.stage().background()).pixel(10, 10),
        Some(Rgba8::opaque(100, 50, 0))
    );
    assert!(
        published
            .diagnostics
            .iter()
            .any(|d| d.severity == Severity::Warning && d.message.contains("gradients")),
        "{:?}",
        published.diagnostics
    );
}

#[test]
fn scenes_play_in_sequence_with_a_warning() {
    let a = one_frame_scene(
        "A",
        vec![rect(0.0, 0.0, 10.0, 10.0, Rgba8::opaque(255, 0, 0))],
        3,
    );
    let b = one_frame_scene(
        "B",
        vec![rect(20.0, 0.0, 10.0, 10.0, Rgba8::opaque(0, 0, 255))],
        2,
    );
    let doc = document(40, 20, vec![a, b], vec![]);
    let published = publish_swf(&doc, &PublishOptions::default()).expect("publish");
    assert_eq!(published.frames, 5);
    let parsed = parse(&published.bytes);
    let lists = display_lists(&parsed.tags);
    assert_eq!(lists.len(), 5);
    let background = doc.stage().background();
    assert_eq!(
        render_swf_frame(&parsed, 1, background).pixel(5, 5),
        Some(Rgba8::opaque(255, 0, 0))
    );
    assert_eq!(
        render_swf_frame(&parsed, 3, background).pixel(25, 5),
        Some(Rgba8::opaque(0, 0, 255))
    );
    assert_eq!(
        render_swf_frame(&parsed, 3, background).pixel(5, 5),
        Some(background),
        "scene A is gone"
    );
    assert!(
        published
            .diagnostics
            .iter()
            .any(|d| d.severity == Severity::Warning && d.message.contains("one after another"))
    );
}

#[test]
fn empty_documents_publish_one_blank_frame_and_extremes_are_refused() {
    let layer_only = Scene::new(SceneId::new(), "S", Timeline::new(vec![layer("L", vec![])]));
    let blank = document(20, 20, vec![layer_only], vec![]);
    let published = publish_swf(&blank, &PublishOptions::default()).expect("publish");
    assert_eq!(published.frames, 1);
    assert_eq!(parse(&published.bytes).declared_frames, 1);

    let long = one_frame_scene("S", vec![rect(0.0, 0.0, 5.0, 5.0, Rgba8::WHITE)], 70_000);
    assert!(matches!(
        publish_swf(
            &document(10, 10, vec![long], vec![]),
            &PublishOptions::default()
        ),
        Err(PublishError::TooManyFrames(_))
    ));
    let huge = one_frame_scene("S", vec![rect(0.0, 0.0, 5.0, 5.0, Rgba8::WHITE)], 1);
    assert!(matches!(
        publish_swf(
            &document(9_000, 100, vec![huge], vec![]),
            &PublishOptions::default()
        ),
        Err(PublishError::StageTooLarge)
    ));
}

#[test]
fn compressed_and_plain_files_agree_and_publishing_is_deterministic() {
    let doc = animated();
    let plain = publish_swf(&doc, &PublishOptions { compress: false }).expect("plain");
    let packed = publish_swf(&doc, &PublishOptions { compress: true }).expect("packed");
    assert_eq!(&plain.bytes[..3], b"FWS");
    assert_eq!(&packed.bytes[..3], b"CWS");
    assert!(packed.bytes.len() < plain.bytes.len());
    assert_eq!(
        parse(&plain.bytes).tags.len(),
        parse(&packed.bytes).tags.len()
    );
    let again = publish_swf(&doc, &PublishOptions { compress: true }).expect("again");
    assert_eq!(again.bytes, packed.bytes, "same document, same bytes");
}

#[test]
fn lossy_choices_are_reported() {
    let clear_bg = Stage::new(10, 10, Rgba8::new(1, 2, 3, 100)).expect("stage");
    let fast = FrameRate::new(1000, 1).expect("rate");
    let scene = one_frame_scene("S", vec![rect(0.0, 0.0, 5.0, 5.0, Rgba8::WHITE)], 1);
    let doc = Document::new(DocumentId::new(), fast, vec![scene], vec![])
        .expect("doc")
        .with_stage(clear_bg);
    let published = publish_swf(&doc, &PublishOptions::default()).expect("publish");
    let text: String = published
        .diagnostics
        .iter()
        .map(|d| d.message.clone())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.contains("transparency"), "{text}");
    assert!(text.contains("capped"), "{text}");
    assert!(text.contains("baked"), "{text}");
    let parsed = parse(&published.bytes);
    assert!(parsed.frame_rate <= 255.99, "{}", parsed.frame_rate);
}

// ---- SWF's own fill semantics ---------------------------------------------------------
//
// Rendering the rebuilt contours with a winding rule treats a global swap of "left" and
// "right" as harmless, but a SWF player does not: a fill style applies to one *side of each
// edge*. These checks use that definition directly, so they catch inverted fills.

fn flatten(seg: &Seg) -> Vec<((f64, f64), (f64, f64))> {
    let (a, b) = (px(seg.from), px(seg.to));
    match seg.ctrl {
        None => vec![(a, b)],
        Some(c) => {
            let c = px(c);
            let at = |t: f64| {
                let u = 1.0 - t;
                (
                    u * u * a.0 + 2.0 * u * t * c.0 + t * t * b.0,
                    u * u * a.1 + 2.0 * u * t * c.1 + t * t * b.1,
                )
            };
            (0..16)
                .map(|i| (at(f64::from(i) / 16.0), at(f64::from(i + 1) / 16.0)))
                .collect()
        }
    }
}

/// The fill style SWF assigns to `p`: that of the nearest edge, on the side `p` lies.
/// `None` when `p` is within `margin` px of an edge, or when its nearest feature is an
/// edge's end point (a corner), where "which side" is not defined by one edge alone.
fn swf_fill_at(segs: &[Seg], p: (f64, f64), margin: f64) -> Option<u32> {
    let mut best: Option<(f64, u32, bool)> = None;
    for seg in segs {
        for (a, b) in flatten(seg) {
            let (dx, dy) = (b.0 - a.0, b.1 - a.1);
            let length2 = dx * dx + dy * dy;
            if length2 < 1e-12 {
                continue;
            }
            let along = ((p.0 - a.0) * dx + (p.1 - a.1) * dy) / length2;
            let t = along.clamp(0.0, 1.0);
            let nearest = (a.0 + t * dx, a.1 + t * dy);
            let distance = (p.0 - nearest.0).hypot(p.1 - nearest.1);
            // With y pointing down, a positive cross product puts `p` on the right of travel.
            let right = dx * (p.1 - a.1) - dy * (p.0 - a.0) > 0.0;
            let style = if right { seg.fill1 } else { seg.fill0 };
            let interior = along > 0.0 && along < 1.0;
            if best.is_none_or(|(d, _, _)| distance < d) {
                best = Some((distance, style, interior));
            }
        }
    }
    best.filter(|(d, _, interior)| *d > margin && *interior)
        .map(|(_, style, _)| style)
}

fn original_contains(shape: &Shape, p: (f64, f64)) -> bool {
    // Non-zero winding over each contour of the original path (straight edges only here).
    let mut polys: Vec<Vec<(f64, f64)>> = Vec::new();
    for segment in shape.path() {
        match segment {
            PathSegment::MoveTo(q) => polys.push(vec![(q.as_vec().x, q.as_vec().y)]),
            PathSegment::LineTo(q) => polys
                .last_mut()
                .expect("moveto")
                .push((q.as_vec().x, q.as_vec().y)),
            PathSegment::CubicTo(..) => panic!("straight edges only"),
            PathSegment::Close => {}
        }
    }
    let mut winding = 0;
    for poly in &polys {
        for i in 0..poly.len() {
            let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
            let cross = (b.0 - a.0) * (p.1 - a.1) - (p.0 - a.0) * (b.1 - a.1);
            if a.1 <= p.1 && b.1 > p.1 && cross > 0.0 {
                winding += 1;
            } else if a.1 > p.1 && b.1 <= p.1 && cross < 0.0 {
                winding -= 1;
            }
        }
    }
    winding != 0
}

#[test]
fn fill_styles_sit_on_the_correct_side_of_every_edge() {
    let cases: Vec<(&str, Shape)> = vec![
        ("ring", ring(false)),
        ("ring wound the other way", ring(true)),
        (
            "square",
            rect(10.0, 10.0, 30.0, 20.0, Rgba8::opaque(1, 2, 3)),
        ),
        (
            "triangle wound the other way",
            Shape::new(
                ShapeId::new(),
                vec![
                    PathSegment::MoveTo(pt(10.0, 10.0)),
                    PathSegment::LineTo(pt(10.0, 50.0)),
                    PathSegment::LineTo(pt(50.0, 10.0)),
                    PathSegment::Close,
                ],
                Rgba8::opaque(9, 9, 9),
            )
            .expect("triangle"),
        ),
    ];
    for (name, shape) in cases {
        let doc = document(
            80,
            60,
            vec![one_frame_scene("S", vec![shape.clone()], 1)],
            vec![],
        );
        let parsed = parse(
            &publish_swf(&doc, &PublishOptions::default())
                .expect("publish")
                .bytes,
        );
        let Some(Tag::DefineShape(defined)) = parsed
            .tags
            .iter()
            .find(|t| matches!(t, Tag::DefineShape(_)))
        else {
            panic!("no shape was published");
        };
        let segs = segments(defined);
        let (mut checked, mut filled) = (0, 0);
        for gy in 0..40 {
            for gx in 0..55 {
                let p = (f64::from(gx) * 1.5 + 0.37, f64::from(gy) * 1.5 + 0.41);
                let Some(style) = swf_fill_at(&segs, p, 0.75) else {
                    continue;
                };
                let expected = original_contains(&shape, p);
                // Style 1 is the fill; style 0 means "no fill on that side".
                assert_eq!(
                    style == 1,
                    expected,
                    "{name}: at {p:?} SWF says style {style}, the shape says {expected}"
                );
                checked += 1;
                filled += usize::from(expected);
            }
        }
        assert!(
            checked > 300 && filled > 20 && filled < checked,
            "{name}: a meaningful sample ({filled}/{checked})"
        );
    }
}
