//! Export a document, import it again with `flare-import`, and compare. This shows the two
//! sides agree with each other, and quantifies what the format cannot hold (curve
//! approximation, gaps). It does not show that Adobe Animate accepts the output.
#![allow(clippy::expect_used)]

use flare_core::{
    DocumentId, DocumentPoint, FrameRate, InstanceId, LayerId, Rgba8, SceneId, ShapeId, SymbolId,
    Transform2D,
};
use flare_document::{
    Content, Document, FrameSpan, Instance, Layer, PathSegment, Scene, Shape, Stage, Stroke,
    Symbol, Timeline, Tween,
};
use flare_export::{Output, Severity, export_xfl, write_directory, write_fla};
use flare_import::{FileSource, import_path, import_xfl};
use glamx::{DAffine2, DVec2};
use std::num::NonZeroU32;

mod reports;
mod structure;

struct Mem(Vec<(String, String)>);
impl FileSource for Mem {
    fn read(&self, name: &str) -> std::io::Result<Option<Vec<u8>>> {
        Ok(self
            .0
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, b)| b.clone().into_bytes()))
    }
}
fn roundtrip(document: &Document) -> (Output, flare_import::Imported) {
    let output = export_xfl(document).expect("export");
    let imported = import_xfl(&Mem(output.files.clone())).expect("the export must re-import");
    (output, imported)
}
fn span(start: u32, duration: u32, content: Content) -> FrameSpan {
    FrameSpan::new(start, NonZeroU32::new(duration).expect("nonzero"), content)
}
fn rect(x: f64, y: f64, w: f64, h: f64, fill: Rgba8) -> Shape {
    Shape::rect(ShapeId::new(), x, y, w, h, fill).expect("rect")
}
fn instance(symbol: SymbolId, x: f64, y: f64) -> Content {
    let transform = Transform2D::new(DAffine2::from_translation(DVec2::new(x, y))).expect("finite");
    Content::Instance(Instance::new(InstanceId::new(), symbol, transform))
}
fn layer(name: &str, spans: Vec<FrameSpan>) -> Layer {
    Layer::new(LayerId::new(), name, spans).expect("layer")
}
fn one_layer(shape: Shape) -> Timeline {
    Timeline::new(vec![layer("L", vec![span(0, 1, Content::Shape(shape))])])
}
fn pt(x: f64, y: f64) -> DocumentPoint {
    DocumentPoint::new(x, y).expect("finite")
}
const RED: Rgba8 = Rgba8::opaque(255, 0, 0);
fn rich() -> Document {
    let (dot, odd) = (SymbolId::new(), SymbolId::new());
    let ellipse = Shape::ellipse(
        ShapeId::new(),
        10.0,
        10.0,
        8.0,
        6.0,
        Rgba8::opaque(0, 0, 255),
    )
    .expect("ellipse");
    let symbols = vec![
        Symbol::new(dot, "Dot", one_layer(rect(0.0, 0.0, 10.0, 10.0, RED))),
        Symbol::new(odd, "A & \"B\" <c>", one_layer(ellipse)),
    ];
    let tweened = span(0, 5, instance(dot, 10.0, 20.0)).with_tween(Tween::Linear);
    let hero = layer(
        "Hero",
        vec![tweened, span(5, 5, instance(dot, 110.0, 20.0))],
    )
    .with_locked(true);
    let disc = Shape::ellipse(
        ShapeId::new(),
        100.0,
        60.0,
        20.0,
        20.0,
        Rgba8::opaque(0, 255, 0),
    )
    .expect("ellipse")
    .with_stroke(Stroke::new(3.0, Rgba8::opaque(0, 0, 0)).expect("stroke"));
    let hidden = layer("Hidden", vec![span(0, 10, Content::Shape(disc))]).with_visible(false);
    let gappy = layer(
        "Gappy",
        vec![
            span(0, 2, Content::Empty),
            span(5, 3, instance(odd, 30.0, 50.0)),
        ],
    );
    let scene_one = Scene::new(
        SceneId::new(),
        "Scene 1",
        Timeline::new(vec![hero, hidden, gappy, layer("Blank", vec![])]),
    );

    let stroke = Stroke::new(2.0, Rgba8::opaque(10, 20, 30)).expect("stroke");
    let line = Shape::line(ShapeId::new(), (5.0, 5.0), (60.0, 40.0), stroke).expect("line");
    let ring = Shape::new(
        ShapeId::new(),
        vec![
            PathSegment::MoveTo(pt(100.0, 10.0)),
            PathSegment::LineTo(pt(140.0, 10.0)),
            PathSegment::LineTo(pt(140.0, 50.0)),
            PathSegment::LineTo(pt(100.0, 50.0)),
            PathSegment::Close,
            PathSegment::MoveTo(pt(110.0, 20.0)),
            PathSegment::LineTo(pt(110.0, 40.0)),
            PathSegment::LineTo(pt(130.0, 40.0)),
            PathSegment::LineTo(pt(130.0, 20.0)),
            PathSegment::Close,
        ],
        Rgba8::opaque(255, 128, 0),
    )
    .expect("ring");
    let scene_two = Scene::new(
        SceneId::new(),
        "Second",
        Timeline::new(vec![
            layer("Lines", vec![span(0, 3, Content::Shape(line))]),
            layer("Ring", vec![span(0, 3, Content::Shape(ring))]),
        ]),
    );
    let rate = FrameRate::new(12, 1).expect("rate");
    let stage = Stage::new(200, 100, Rgba8::opaque(16, 32, 48)).expect("stage");
    Document::new(DocumentId::new(), rate, vec![scene_one, scene_two], symbols)
        .expect("doc")
        .with_stage(stage)
}
fn render(document: &Document, scene: usize, frame: u32) -> flare_render::Image {
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
/// (largest channel difference, mean channel difference) between two renders.
fn difference(a: &flare_render::Image, b: &flare_render::Image) -> (u8, f64) {
    let (mut max, mut sum) = (0_u8, 0_u64);
    for (x, y) in a.rgba().iter().zip(b.rgba()) {
        let d = x.abs_diff(*y);
        max = max.max(d);
        sum += u64::from(d);
    }
    (max, sum as f64 / a.rgba().len() as f64)
}
