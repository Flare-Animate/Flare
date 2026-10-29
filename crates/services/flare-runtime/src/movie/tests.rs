#![allow(clippy::expect_used)]

use std::num::NonZeroU32;

use flare_core::{DocumentId, FrameRate, LayerId, Rgba8, SceneId, ShapeId};
use flare_document::{Content, Layer, Scene, Shape, Stage, Timeline};

use super::*;

fn animation(frames: u32, stage: (u32, u32), rate: (u32, u32)) -> Document {
    let shape = Shape::rect(
        ShapeId::new(),
        0.0,
        0.0,
        20.0,
        20.0,
        Rgba8::opaque(255, 0, 0),
    )
    .expect("rect");
    let span = flare_document::FrameSpan::new(
        0,
        NonZeroU32::new(frames).expect("nonzero"),
        Content::Shape(shape),
    );
    let layer = Layer::new(LayerId::new(), "L", vec![span]).expect("layer");
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(vec![layer]));
    Document::new(
        DocumentId::new(),
        FrameRate::new(rate.0, rate.1).expect("rate"),
        vec![scene],
        vec![],
    )
    .expect("document")
    .with_stage(Stage::new(stage.0, stage.1, Rgba8::WHITE).expect("stage"))
}

fn decode(bytes: &[u8]) -> (gif::Decoder<&[u8]>, Vec<gif::Frame<'static>>) {
    let mut options = gif::DecodeOptions::new();
    options.set_color_output(gif::ColorOutput::RGBA);
    let mut decoder = options.read_info(bytes).expect("a GIF");
    let mut frames = Vec::new();
    while let Some(frame) = decoder.read_next_frame().expect("frame") {
        frames.push(frame.clone());
    }
    (decoder, frames)
}

#[test]
fn every_frame_becomes_a_gif_frame_at_the_documents_rate_and_it_loops() {
    let bytes = gif_bytes(&animation(6, (40, 20), (25, 1))).expect("gif");
    assert!(bytes.starts_with(b"GIF89a"));
    let (decoder, frames) = decode(&bytes);
    assert_eq!((decoder.width(), decoder.height()), (40, 20));
    assert_eq!(frames.len(), 6);
    assert!(
        frames.iter().all(|f| f.delay == 4),
        "25 frames a second is 4 hundredths each"
    );
    let first = &frames[0];
    // The red square at the top left, white elsewhere.
    assert_eq!(&first.buffer[..4], &[255, 0, 0, 255]);
    let last_pixel = first.buffer.len() - 4;
    assert_eq!(&first.buffer[last_pixel..], &[255, 255, 255, 255]);
}

#[test]
fn big_stages_are_scaled_down_and_absurd_requests_are_refused_in_words() {
    let bytes = gif_bytes(&animation(1, (1280, 640), (24, 1))).expect("gif");
    let (decoder, _) = decode(&bytes);
    assert_eq!(
        (decoder.width(), decoder.height()),
        (640, 320),
        "the long side fits the limit"
    );
    // A very fast rate still gets a delay a viewer will honour.
    let fast = gif_bytes(&animation(2, (20, 20), (240, 1))).expect("gif");
    assert!(decode(&fast).1.iter().all(|f| f.delay >= 2));
    let long = animation(MAX_MOVIE_FRAMES as u32 + 1, (10, 10), (24, 1));
    let error = gif_bytes(&long).expect_err("too many frames");
    assert!(error.contains("limited to"), "{error}");
}

proptest::proptest! {
    #![proptest_config(proptest::prelude::ProptestConfig::with_cases(24))]

    /// Any small animation encodes to a GIF with one picture per frame, a delay a viewer will
    /// honour, and a size that fits the limit.
    #[test]
    fn any_small_animation_becomes_a_gif_with_a_picture_per_frame(
        frames in 1_u32..10,
        width in 1_u32..120,
        height in 1_u32..120,
        numerator in 1_u32..240,
        denominator in 1_u32..4,
    ) {
        let bytes = gif_bytes(&animation(frames, (width, height), (numerator, denominator)))
            .expect("gif");
        let (decoder, pictures) = decode(&bytes);
        proptest::prop_assert_eq!(pictures.len(), frames as usize);
        proptest::prop_assert_eq!((u32::from(decoder.width()), u32::from(decoder.height())), (width, height));
        proptest::prop_assert!(pictures.iter().all(|f| f.delay >= 2));
    }
}
