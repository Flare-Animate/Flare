//! A layer's blend mode: its picture mixed with the layers under it, in an export and on the
//! stage's own path for transparent layers, each layer mixed on its own.

use flare_document::LayerBlend;

use super::*;

const GREY: Rgba8 = Rgba8::opaque(128, 128, 128);

fn layer(content: Content) -> Layer {
    Layer::new(LayerId::new(), "L", vec![span(content)]).expect("layer")
}

/// A grey square over the left half of a grey backdrop layer, the square's layer mixed by `blend`.
fn grey_over_grey(blend: LayerBlend) -> (Document, SceneId) {
    let top = layer(rect(0.0, 0.0, 10.0, 20.0, GREY)).with_blend(blend);
    let under = layer(rect(0.0, 0.0, 20.0, 20.0, GREY));
    scene_with_layers(vec![top, under])
}

fn rgb(image: &Image, x: u32, y: u32) -> [u8; 3] {
    let p = image.pixel(x, y).expect("pixel");
    [p.r, p.g, p.b]
}

fn near(got: [u8; 3], want: u8) -> bool {
    got.iter().all(|c| c.abs_diff(want) <= 2)
}

#[test]
fn a_layer_is_mixed_with_the_layers_under_it_by_its_mode() {
    for (blend, want) in [
        (LayerBlend::Normal, 128),
        (LayerBlend::Multiply, 64),
        (LayerBlend::Screen, 192),
        (LayerBlend::Add, 255),
        (LayerBlend::Difference, 0),
    ] {
        let (doc, id) = grey_over_grey(blend);
        let image = draw(&doc, id, Rgba8::WHITE);
        assert!(
            near(rgb(&image, 5, 5), want),
            "{blend:?}: {:?}",
            rgb(&image, 5, 5)
        );
        assert!(
            near(rgb(&image, 15, 5), 128),
            "{blend:?}: outside the square"
        );
    }
}

#[test]
fn two_multiplied_layers_each_darken_what_is_under_them() {
    let a = layer(rect(0.0, 0.0, 20.0, 20.0, GREY)).with_blend(LayerBlend::Multiply);
    let b = layer(rect(0.0, 0.0, 20.0, 20.0, GREY)).with_blend(LayerBlend::Multiply);
    let (doc, id) = scene_with_layers(vec![a, b]);
    // White × grey × grey: each layer is mixed in turn.
    let image = draw(&doc, id, Rgba8::WHITE);
    assert!(near(rgb(&image, 5, 5), 64), "{:?}", rgb(&image, 5, 5));
}

#[test]
fn the_stage_mixes_a_blended_layer_when_another_layer_is_transparent() {
    let top = layer(rect(0.0, 0.0, 10.0, 20.0, GREY)).with_blend(LayerBlend::Multiply);
    let under = layer(rect(0.0, 0.0, 20.0, 20.0, GREY));
    let faint = layer(rect(18.0, 18.0, 2.0, 2.0, RED)).with_view_opacity(50);
    let (doc, id) = scene_with_layers(vec![faint, top, under]);
    let frame = flare_eval::evaluate_frame(&doc, id, 0).expect("eval");
    assert!(has_view_alpha(&frame), "the stage takes its own path");
    let viewport = Viewport::full(20, 20);
    let image = render_stage_viewport(&frame, &viewport, 20, 20, Rgba8::WHITE).expect("render");
    assert!(near(rgb(&image, 5, 5), 64), "{:?}", rgb(&image, 5, 5));
    assert!(near(rgb(&image, 15, 5), 128));
}
