//! Tinted onion-skin ghosts.
#![allow(clippy::expect_used)]

use super::*;
use crate::{Viewport, render_onion_viewport};

const BLUE: Rgba8 = Rgba8::opaque(40, 110, 255);

#[test]
fn a_tint_colors_dark_pixels_fully_and_light_ones_palely_and_keeps_alpha() {
    // A black pixel, a white one and a half-transparent black one.
    let image = Image::from_rgba(3, 1, vec![0, 0, 0, 255, 255, 255, 255, 255, 0, 0, 0, 128])
        .expect("image");
    let tinted = image.tinted(BLUE);
    assert_eq!(tinted.pixel(0, 0), Some(BLUE), "a line takes the tint");
    let pale = tinted.pixel(1, 0).expect("pixel");
    assert!(
        pale.r > 150 && pale.g > 180 && pale.b == 255,
        "a fill is a pale shade of it: {pale:?}"
    );
    assert_eq!(tinted.pixel(2, 0).map(|p| p.a), Some(128));
}

#[test]
fn a_tinted_ghost_is_drawn_in_its_tint_and_an_untinted_one_in_its_own_colors() {
    use flare_core::{DocumentId, FrameRate, LayerId, SceneId, ShapeId};
    use flare_document::{Content, Document, FrameSpan, Layer, Scene, Shape, Timeline};
    // Frame 0 has a red square; frame 1 is empty, so all that shows at frame 1 is the ghost.
    let red = Rgba8::opaque(255, 0, 0);
    let square = Shape::rect(ShapeId::new(), 0.0, 0.0, 10.0, 10.0, red).expect("square");
    let span = FrameSpan::new(0, std::num::NonZeroU32::MIN, Content::Shape(square));
    let layer = Layer::new(LayerId::new(), "L", vec![span]).expect("layer");
    let id = SceneId::new();
    let scene = Scene::new(id, "S", Timeline::new(vec![layer]));
    let rate = FrameRate::new(24, 1).expect("rate");
    let doc = Document::new(DocumentId::new(), rate, vec![scene], vec![]).expect("doc");
    let ghost = flare_eval::evaluate_frame(&doc, id, 0).expect("ghost");
    let current = flare_eval::Frame::default();
    let draw = |tint: Option<Rgba8>| {
        render_onion_viewport(
            &[(&ghost, 1.0, tint)],
            &current,
            &Viewport::full(20, 20),
            20,
            20,
            Rgba8::WHITE,
        )
        .expect("onion")
    };
    let tinted = draw(Some(BLUE)).pixel(5, 5).expect("pixel");
    assert!(
        tinted.b > tinted.r && tinted.b > 200,
        "the red square shows in the blue tint: {tinted:?}"
    );
    assert_eq!(draw(None).pixel(5, 5), Some(red), "untinted, its own red");
    assert_eq!(
        draw(Some(BLUE)).pixel(15, 15),
        Some(Rgba8::WHITE),
        "nothing else"
    );
}
