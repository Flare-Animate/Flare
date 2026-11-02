//! Tweens go out as XFL writes them and come back the same.
#![allow(clippy::expect_used)]

use std::num::NonZeroU32;

use flare_core::{DocumentId, FrameRate, LayerId, Rgba8, SceneId, ShapeId};
use flare_document::{Content, Document, FrameSpan, Layer, Scene, Shape, Timeline, Tween};
use flare_export::{export_xfl, fla_bytes};
use flare_import::import_fla_bytes;

fn shape(x: f64) -> Content {
    Content::Shape(Shape::rect(ShapeId::new(), x, 0.0, 10.0, 10.0, Rgba8::WHITE).expect("rect"))
}

fn span(start: u32, duration: u32, content: Content) -> FrameSpan {
    FrameSpan::new(start, NonZeroU32::new(duration).expect("nonzero"), content)
}

fn document(spans: Vec<FrameSpan>) -> Document {
    let layer = Layer::new(LayerId::new(), "L", spans).expect("layer");
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(vec![layer]));
    Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![scene],
        vec![],
    )
    .expect("document")
}

fn main_xml(document: &Document) -> String {
    let output = export_xfl(document).expect("export");
    output
        .files
        .iter()
        .find(|(name, _)| name.ends_with("DOMDocument.xml"))
        .expect("the document")
        .1
        .clone()
}

#[test]
fn a_shape_tween_is_written_with_its_type_and_ease_and_nothing_extra_otherwise() {
    let tweened = document(vec![
        span(0, 4, shape(0.0))
            .with_tween(Tween::Shape)
            .with_ease(40),
        span(4, 1, shape(50.0)),
    ]);
    let xml = main_xml(&tweened);
    assert!(
        xml.contains(r#"tweenType="shape""#) && xml.contains(r#"acceleration="40""#),
        "{xml}"
    );
    let even = main_xml(&document(vec![
        span(0, 4, shape(0.0)).with_tween(Tween::Shape),
        span(4, 1, shape(50.0)),
    ]));
    assert!(
        even.contains(r#"tweenType="shape""#) && !even.contains("acceleration"),
        "{even}"
    );
    // An ease on a span that is not tweened is not written: there is nothing for it to ease.
    let held = main_xml(&document(vec![span(0, 4, shape(0.0)).with_ease(40)]));
    assert!(
        !held.contains("acceleration") && !held.contains("tweenType"),
        "{held}"
    );
}

#[test]
fn a_shape_tween_and_its_ease_survive_an_exported_fla() {
    let before = document(vec![
        span(0, 4, shape(0.0))
            .with_tween(Tween::Shape)
            .with_ease(-65),
        span(4, 1, shape(50.0)),
    ]);
    let bytes = fla_bytes(&export_xfl(&before).expect("export")).expect("bytes");
    let back = import_fla_bytes(bytes).expect("import");
    let spans = back.document.scenes()[0].timeline().layers()[0]
        .spans()
        .to_vec();
    assert_eq!((spans[0].tween(), spans[0].ease()), (Tween::Shape, -65));
    assert_eq!((spans[1].tween(), spans[1].ease()), (Tween::None, 0));
}
