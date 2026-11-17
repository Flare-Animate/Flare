//! Animations exported over a transparent backdrop: what is not drawn stays see-through in every
//! frame, and what is drawn is whole.
#![allow(clippy::expect_used)]

use std::num::NonZeroU32;

use flare_core::{DocumentId, FrameRate, LayerId, Rgba8, SceneId, ShapeId};
use flare_document::{Content, FrameSpan, Layer, Scene, Shape, Stage, Timeline};

use super::*;

/// A 40 by 20 stage, white, with a red square in the top-left corner on the first frame and in
/// the top-right corner on the second: the corner the square leaves must not keep it.
fn moving_square() -> Document {
    let square = |x: f64| {
        Content::Shape(
            Shape::rect(ShapeId::new(), x, 0.0, 10.0, 10.0, Rgba8::opaque(255, 0, 0))
                .expect("rect"),
        )
    };
    let one = NonZeroU32::new(1).expect("nonzero");
    let spans = vec![
        FrameSpan::new(0, one, square(0.0)),
        FrameSpan::new(1, one, square(30.0)),
    ];
    let layer = Layer::new(LayerId::new(), "L", spans).expect("layer");
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(vec![layer]));
    Document::new(
        DocumentId::new(),
        FrameRate::new(12, 1).expect("rate"),
        vec![scene],
        vec![],
    )
    .expect("document")
    .with_stage(Stage::new(40, 20, Rgba8::WHITE).expect("stage"))
}

fn alpha_at(rgba: &[u8], width: u32, x: u32, y: u32) -> u8 {
    rgba[((y * width + x) * 4 + 3) as usize]
}

/// Every frame of a GIF as the player shows it, after the previous frame's disposal.
fn gif_screens(bytes: &[u8]) -> Vec<Vec<u8>> {
    let mut options = gif::DecodeOptions::new();
    options.set_color_output(gif::ColorOutput::RGBA);
    let mut decoder = options.read_info(bytes).expect("a GIF");
    let (w, h) = (usize::from(decoder.width()), usize::from(decoder.height()));
    let mut screen = vec![0u8; w * h * 4];
    let mut screens = Vec::new();
    let mut dispose = gif::DisposalMethod::Keep;
    while let Some(frame) = decoder.read_next_frame().expect("frame") {
        if dispose == gif::DisposalMethod::Background {
            screen.fill(0);
        }
        for (to, from) in screen
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(frame.buffer.as_chunks::<4>().0)
        {
            if from[3] != 0 {
                *to = *from;
            }
        }
        dispose = frame.dispose;
        screens.push(screen.clone());
    }
    screens
}

#[test]
fn a_transparent_gif_shows_only_the_drawing_and_each_frame_clears_the_last() {
    let screens = gif_screens(&gif_bytes(&moving_square(), Backdrop::Transparent).expect("gif"));
    assert_eq!(screens.len(), 2);
    assert_eq!(alpha_at(&screens[0], 40, 5, 5), 255, "the square, frame 1");
    assert_eq!(
        alpha_at(&screens[0], 40, 20, 15),
        0,
        "the empty stage, frame 1"
    );
    assert_eq!(alpha_at(&screens[1], 40, 35, 5), 255, "the square, frame 2");
    assert_eq!(
        alpha_at(&screens[1], 40, 5, 5),
        0,
        "the corner the square left is clear again"
    );

    let opaque = gif_screens(&gif_bytes(&moving_square(), Backdrop::Stage).expect("gif"));
    assert_eq!(
        alpha_at(&opaque[0], 40, 20, 15),
        255,
        "over the stage, nothing is clear"
    );
}

#[test]
fn a_gif_pixel_is_shown_or_not_cut_at_half_coverage() {
    let mut pixels = vec![9, 9, 9, 0, 9, 9, 9, 127, 9, 9, 9, 128, 9, 9, 9, 255];
    one_bit_alpha(&mut pixels);
    let alphas: Vec<u8> = pixels.as_chunks::<4>().0.iter().map(|p| p[3]).collect();
    assert_eq!(alphas, [0, 0, 255, 255]);
}

#[test]
fn a_transparent_apng_keeps_the_alpha_of_every_frame() {
    let bytes = apng_bytes(&moving_square(), Backdrop::Transparent).expect("apng");
    let mut reader = png::Decoder::new(std::io::Cursor::new(&bytes))
        .read_info()
        .expect("info");
    let mut frame = vec![0; reader.output_buffer_size().expect("size")];
    let mut alphas = Vec::new();
    while let Ok(info) = reader.next_frame(&mut frame) {
        assert_eq!(info.color_type, png::ColorType::Rgba);
        let control = reader
            .info()
            .frame_control()
            .expect("a frame of the animation");
        assert_eq!(
            control.blend_op,
            png::BlendOp::Source,
            "replaces, never blends"
        );
        alphas.push((alpha_at(&frame, 40, 5, 5), alpha_at(&frame, 40, 20, 15)));
    }
    assert_eq!(alphas, [(255, 0), (0, 0)]);
}

#[test]
fn a_transparent_animated_webp_imports_back_see_through() {
    let bytes = animated_webp_bytes(&moving_square(), Backdrop::Transparent).expect("webp");
    let imported = flare_import::import_foreign_bytes(std::path::Path::new("a.webp"), bytes)
        .expect("the export imports back");
    let document = imported.document;
    let scene = document.scenes()[0].id();
    let drawn = flare_eval::evaluate_frame(&document, scene, 0).expect("frame 1");
    let image = flare_render::render(&drawn, 40, 20, Rgba8::TRANSPARENT).expect("render");
    assert_eq!(alpha_at(image.rgba(), 40, 5, 5), 255, "the square");
    assert_eq!(alpha_at(image.rgba(), 40, 20, 15), 0, "the empty stage");
}
