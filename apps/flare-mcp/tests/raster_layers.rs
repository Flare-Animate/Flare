//! Raster layers, blend modes, the Brush's presets and the pixel eraser through the MCP server:
//! the wiring of `flare_commands::layers` and `flare_tools::blend`.
#![allow(clippy::expect_used)]

mod common;

use common::fixture_of;
use flare_core::{DocumentId, FrameRate, LayerId, Rgba8, SceneId};
use flare_document::{Content, Document, Layer, Scene, Stage, Timeline};
use serde_json::{Value, json};

fn empty() -> Document {
    let layer = Layer::new(LayerId::new(), "Layer 1", vec![]).expect("layer");
    Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![Scene::new(SceneId::new(), "S", Timeline::new(vec![layer]))],
        vec![],
    )
    .expect("document")
    .with_stage(Stage::new(100, 60, Rgba8::WHITE).expect("stage"))
}

fn layer_of(info: &Value, id: &Value) -> Value {
    info["scenes"][0]["layers"]
        .as_array()
        .expect("layers")
        .iter()
        .find(|l| &l["id"] == id)
        .cloned()
        .expect("the layer")
}

#[tokio::test]
async fn a_raster_layer_takes_a_preset_stroke_and_the_eraser_and_reports_its_kind_and_blend() {
    let f = fixture_of(&empty(), true, false, vec![]).await;
    let made = f
        .ok("add_layer", json!({ "name": "Paint", "raster": true }))
        .await;
    let id = made["layer"].clone();
    let info = f.ok("get_info", json!({})).await;
    assert_eq!(layer_of(&info, &id)["raster"], true);
    assert_eq!(layer_of(&info, &id)["blend"], "normal");

    f.ok(
        "raster_brush",
        json!({ "points": [[5.0, 30.0], [95.0, 30.0]], "preset": "marker",
                "color": "0000ff", "layer": id }),
    )
    .await;
    f.ok(
        "raster_erase",
        json!({ "points": [[50.0, 10.0], [50.0, 50.0]], "size": 10, "layer": id }),
    )
    .await;
    f.ok("save", json!({})).await;
    let doc = flare_storage::load(&f.path).expect("load");
    let layer = doc.scenes()[0]
        .timeline()
        .layers()
        .iter()
        .find(|l| Some(l.id().as_uuid().to_string().as_str()) == id.as_str())
        .expect("layer");
    let span = layer.span_at(0).expect("painted");
    let Some(Content::Bitmap(placed)) = span.element(0) else {
        panic!("a painting");
    };
    let bitmap = doc.bitmap(placed.bitmap()).expect("bitmap");
    let d = bitmap.width() as usize / 100;
    let alpha = |x: usize| bitmap.pixels()[((30 * d) * bitmap.width() as usize + x * d) * 4 + 3];
    assert!(
        (170..=185).contains(&alpha(20)),
        "the marker's 70%: {}",
        alpha(20)
    );
    assert_eq!(alpha(50), 0, "rubbed out");

    let blended = f
        .ok(
            "set_layer_blend",
            json!({ "layer": id, "mode": "multiply" }),
        )
        .await;
    assert_eq!(blended["changed"], true);
    let again = f
        .ok(
            "set_layer_blend",
            json!({ "layer": id, "mode": "Multiply" }),
        )
        .await;
    assert_eq!(again["changed"], false, "the same mode changes nothing");
    let info = f.ok("get_info", json!({})).await;
    assert_eq!(layer_of(&info, &id)["blend"], "multiply");
    let vector = f
        .ok("set_layer_kind", json!({ "layer": id, "raster": false }))
        .await;
    assert_eq!(vector["changed"], true);
}

#[tokio::test]
async fn an_unknown_mode_or_preset_is_refused_and_changes_nothing() {
    let f = fixture_of(&empty(), true, false, vec![]).await;
    let before = f.revision().await;
    for (tool, arguments, says) in [
        (
            "set_layer_blend",
            json!({ "mode": "dissolve" }),
            "unknown blend mode",
        ),
        (
            "raster_brush",
            json!({ "points": [[5.0, 5.0]], "preset": "crayon" }),
            "unknown preset",
        ),
        (
            "raster_erase",
            json!({ "points": [[5.0, 5.0]] }),
            "no painting",
        ),
    ] {
        let (refused, message) = f.tool(tool, arguments).await;
        assert!(refused, "{tool} was refused");
        assert!(message.to_string().contains(says), "{tool}: {message}");
    }
    assert_eq!(f.revision().await, before);
}

#[tokio::test]
async fn set_layer_kind_converts_what_the_layer_shows() {
    let f = fixture_of(&empty(), true, false, vec![]).await;
    let info = f.ok("get_info", json!({})).await;
    let id = info["scenes"][0]["layers"][0]["id"].clone();
    f.ok(
        "draw_rect",
        json!({ "x": 10.0, "y": 10.0, "width": 40.0, "height": 20.0, "color": "ff0000", "layer": id }),
    )
    .await;
    let raster = f
        .ok("set_layer_kind", json!({ "layer": id, "raster": true }))
        .await;
    assert_eq!(raster["changed"], true);
    let kinds = |doc: &Document| {
        let span = doc.scenes()[0].timeline().layers()[0]
            .span_at(0)
            .expect("keyframe");
        (
            span.elements()
                .filter(|c| matches!(c, Content::Bitmap(_)))
                .count(),
            span.elements()
                .filter(|c| matches!(c, Content::Shape(_)))
                .count(),
        )
    };
    f.ok("save", json!({})).await;
    assert_eq!(kinds(&flare_storage::load(&f.path).expect("load")), (1, 0));
    f.ok("set_layer_kind", json!({ "layer": id, "raster": false }))
        .await;
    f.ok("save", json!({})).await;
    let (pictures, shapes) = kinds(&flare_storage::load(&f.path).expect("load"));
    assert_eq!(pictures, 0);
    assert!(shapes > 0, "traced back into shapes");
}
