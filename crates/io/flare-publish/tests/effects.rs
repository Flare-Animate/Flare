//! SWF and HTML5 are vector formats with no place for a layer's effects: each says which layers
//! were drawn without them, and says nothing when no layer has any.
#![allow(clippy::expect_used)]

use std::num::NonZeroU32;

use flare_core::{DocumentId, FrameRate, LayerId, Rgba8, SceneId, ShapeId};
use flare_document::{Content, Document, FrameSpan, Layer, Scene, Shape, Timeline};
use flare_fx::{EffectGraph, EffectKind};
use flare_publish::{Diagnostic, Html5Options, PublishOptions, publish_html5, publish_swf};

fn layer(name: &str, effects: Option<EffectGraph>) -> Layer {
    let shape = Shape::rect(ShapeId::new(), 0.0, 0.0, 10.0, 10.0, Rgba8::WHITE).expect("rect");
    let span = FrameSpan::new(0, NonZeroU32::new(1).expect("one"), Content::Shape(shape));
    Layer::new(LayerId::new(), name, vec![span])
        .expect("layer")
        .with_effects(effects)
}

fn document(layers: Vec<Layer>) -> Document {
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(layers));
    Document::new(
        DocumentId::new(),
        FrameRate::new(12, 1).expect("rate"),
        vec![scene],
        vec![],
    )
    .expect("document")
}

fn shadow() -> Option<EffectGraph> {
    Some(
        EffectGraph::new()
            .append(EffectKind::DropShadow)
            .expect("add")
            .0,
    )
}

fn note(diagnostics: &[Diagnostic]) -> Option<&str> {
    diagnostics
        .iter()
        .map(|d| d.message.as_str())
        .find(|m| m.contains("layer effects"))
}

fn notes(doc: &Document) -> (Option<String>, Option<String>) {
    let swf = publish_swf(doc, &PublishOptions::default()).expect("swf");
    let html = publish_html5(doc, &Html5Options::default()).expect("html");
    (
        note(&swf.diagnostics).map(str::to_owned),
        note(&html.diagnostics).map(str::to_owned),
    )
}

#[test]
fn both_formats_name_the_layers_drawn_without_their_effects() {
    let doc = document(vec![layer("Hero", shadow()), layer("Ground", None)]);
    let (swf, html) = notes(&doc);
    let swf = swf.expect("the SWF says so");
    assert!(swf.contains("\"Hero\"") && !swf.contains("Ground"), "{swf}");
    let html = html.expect("the page says so");
    assert!(
        html.starts_with("an HTML5 page") && html.contains("\"Hero\""),
        "{html}"
    );
}

#[test]
fn no_layer_with_effects_means_no_note() {
    let (unconnected, _) = EffectGraph::new()
        .add(EffectKind::Glow, [0.0, 0.0])
        .expect("add");
    let doc = document(vec![layer("A", None), layer("B", Some(unconnected))]);
    assert_eq!(
        notes(&doc),
        (None, None),
        "a graph that changes nothing is not lost"
    );
}

#[test]
fn a_long_list_names_the_first_few_and_counts_the_rest() {
    let layers = (0..8).map(|i| layer(&format!("L{i}"), shadow())).collect();
    let (swf, _) = notes(&document(layers));
    let swf = swf.expect("note");
    assert!(swf.contains("\"L4\"") && !swf.contains("\"L5\""), "{swf}");
    assert!(swf.contains("and 3 more"), "{swf}");
}

#[test]
fn both_formats_name_the_layers_drawn_as_normal_instead_of_their_blend_mode() {
    let shade = layer("Shade", None).with_blend(flare_document::LayerBlend::Multiply);
    let doc = document(vec![shade, layer("Ground", None)]);
    let blend = |diagnostics: &[Diagnostic]| {
        diagnostics
            .iter()
            .map(|d| d.message.clone())
            .find(|m| m.contains("blend mode"))
    };
    let swf = publish_swf(&doc, &PublishOptions::default()).expect("swf");
    let swf = blend(&swf.diagnostics).expect("the SWF says so");
    assert!(
        swf.contains("\"Shade\"") && !swf.contains("Ground"),
        "{swf}"
    );
    let html = publish_html5(&doc, &Html5Options::default()).expect("html");
    assert!(blend(&html.diagnostics).is_some(), "the page says so");

    let plain = document(vec![layer("A", None)]);
    let swf = publish_swf(&plain, &PublishOptions::default()).expect("swf");
    assert!(blend(&swf.diagnostics).is_none(), "nothing to say");
}
