//! Building documents and decoding what was published.
#![allow(clippy::expect_used, dead_code)]

use flare_core::{
    DocumentId, DocumentPoint, FrameRate, InstanceId, LayerId, Rgba8, SceneId, ShapeId, SymbolId,
    Transform2D,
};
use flare_document::{
    Content, Document, FrameSpan, Instance, Layer, PathSegment, Scene, Shape, Stage, Stroke,
    Symbol, Timeline, Tween,
};
use flare_publish::{PublishOptions, publish_swf};
use glamx::{DAffine2, DVec2};
use std::collections::HashMap;
use std::num::NonZeroU32;
use swf::{PlaceObjectAction, Tag};

mod geometry;

#[allow(unused_imports)]
use geometry::*;
pub use geometry::{original_contains, rebuild, segments, swf_fill_at, to_affine};

// ---- building documents ---------------------------------------------------------------

pub fn span(start: u32, duration: u32, content: Content) -> FrameSpan {
    FrameSpan::new(start, NonZeroU32::new(duration).expect("nonzero"), content)
}

pub fn layer(name: &str, spans: Vec<FrameSpan>) -> Layer {
    Layer::new(LayerId::new(), name, spans).expect("layer")
}

pub fn pt(x: f64, y: f64) -> DocumentPoint {
    DocumentPoint::new(x, y).expect("finite")
}

pub fn rect(x: f64, y: f64, w: f64, h: f64, fill: Rgba8) -> Shape {
    Shape::rect(ShapeId::new(), x, y, w, h, fill).expect("rect")
}

pub fn instance(symbol: SymbolId, x: f64, y: f64) -> Content {
    let transform = Transform2D::new(DAffine2::from_translation(DVec2::new(x, y))).expect("finite");
    Content::Instance(Instance::new(InstanceId::new(), symbol, transform))
}

pub fn document(width: u32, height: u32, scenes: Vec<Scene>, symbols: Vec<Symbol>) -> Document {
    let rate = FrameRate::new(12, 1).expect("rate");
    let stage = Stage::new(width, height, Rgba8::opaque(16, 32, 48)).expect("stage");
    Document::new(DocumentId::new(), rate, scenes, symbols)
        .expect("doc")
        .with_stage(stage)
}

/// A square ring with a square hole; `flip` winds both contours the other way.
pub fn ring(flip: bool) -> Shape {
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

pub fn one_frame_scene(name: &str, shapes: Vec<Shape>, frames: u32) -> Scene {
    let layers = shapes
        .into_iter()
        .enumerate()
        .map(|(i, s)| layer(&format!("L{i}"), vec![span(0, frames, Content::Shape(s))]))
        .collect();
    Scene::new(SceneId::new(), name, Timeline::new(layers))
}

pub fn animated() -> Document {
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

pub struct Parsed {
    pub version: u8,
    pub stage: (i32, i32),
    pub frame_rate: f64,
    pub declared_frames: u16,
    pub background: Option<(u8, u8, u8)>,
    pub tags: Vec<Tag<'static>>,
}

pub fn parse(bytes: &[u8]) -> Parsed {
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
pub fn owned_tags(tags: Vec<Tag<'_>>) -> Vec<Tag<'static>> {
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
pub struct OnScreen {
    pub character: u16,
    pub matrix: swf::Matrix,
}

/// Plays the display list and returns what is on screen after each ShowFrame, bottom first.
pub fn display_lists(tags: &[Tag<'static>]) -> Vec<Vec<(u16, OnScreen)>> {
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

/// Re-renders frame `index` of the SWF from what the reader decoded.
pub fn render_swf_frame(parsed: &Parsed, index: usize, background: Rgba8) -> flare_render::Image {
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
                clips: Vec::new(),
            });
        }
    }
    let frame = flare_eval::Frame {
        placed: Vec::new(),
        shapes: drawn,
        clips: Vec::new(),
    };
    flare_render::render(
        &frame,
        (parsed.stage.0 / 20) as u32,
        (parsed.stage.1 / 20) as u32,
        background,
    )
    .expect("render")
}

pub fn render_original(document: &Document, scene: usize, frame: u32) -> flare_render::Image {
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

pub fn difference(a: &flare_render::Image, b: &flare_render::Image) -> (u8, f64) {
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
pub fn the_header_stage_frame_rate_background_and_frame_count_are_right() {
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

pub fn document_of_distinct_shapes(count: u32) -> Document {
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
