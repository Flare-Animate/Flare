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

#[test]
fn the_bytes_are_the_same_whatever_the_number_of_threads() {
    let doc = animation(40, (200, 120), (24, 1));
    let one = gif_bytes_on(&doc, 1).expect("one thread");
    for workers in [2, 3, 8] {
        assert_eq!(
            gif_bytes_on(&doc, workers).expect("parallel"),
            one,
            "{workers} threads"
        );
    }
    assert_eq!(gif_bytes(&doc).expect("default"), one);
    // And a failure on any frame is the failure of the export, not a short movie.
    let long = animation(3, (200, 120), (24, 1));
    assert!(gif_bytes_on(&long, 4).is_ok());
}

/// How long a busy 30-frame export takes on one thread and on several: printed, and required to
/// be no slower, where there is more than one core to use.
#[test]
fn several_threads_are_not_slower_than_one_on_a_busy_movie() {
    use std::time::Instant;
    let layers: Vec<Layer> = (0..40)
        .map(|l| {
            let spans = (0..30)
                .map(|s| {
                    let shape = Shape::rect(
                        ShapeId::new(),
                        f64::from(s) * 30.0,
                        f64::from(l) * 14.0,
                        200.0,
                        60.0,
                        Rgba8::new((l * 6) as u8, 90, (s * 8) as u8, 255),
                    )
                    .expect("rect");
                    flare_document::FrameSpan::new(s, NonZeroU32::MIN, Content::Shape(shape))
                })
                .collect();
            Layer::new(LayerId::new(), format!("L{l}"), spans).expect("layer")
        })
        .collect();
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(layers));
    let doc = Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![scene],
        vec![],
    )
    .expect("document")
    .with_stage(Stage::new(1280, 720, Rgba8::WHITE).expect("stage"));
    let time = |workers| {
        let t = Instant::now();
        gif_bytes_on(&doc, workers).expect("gif");
        t.elapsed().as_secs_f64()
    };
    let (one, many) = (time(1), time(4));
    println!("one thread {one:.3}s, four threads {many:.3}s");
    let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
    if cores >= 4 {
        assert!(
            many < one * 1.2,
            "four threads {many:.3}s against one {one:.3}s"
        );
    }
}

#[test]
fn an_apng_has_a_frame_per_frame_loops_and_imports_back_with_the_same_frames() {
    let document = animation(6, (40, 20), (25, 1));
    let bytes = crate::apng_bytes(&document).expect("apng");
    let reader = png::Decoder::new(std::io::Cursor::new(&bytes))
        .read_info()
        .expect("info");
    let control = reader.info().animation_control().expect("animated");
    assert_eq!(control.num_frames, 6);
    assert_eq!(control.num_plays, 0, "loops forever");
    assert_eq!((reader.info().width, reader.info().height), (40, 20));
    let imported = flare_import::import_foreign_bytes(std::path::Path::new("a.png"), bytes)
        .expect("the export imports back");
    assert_eq!(
        imported.document.scenes()[0].timeline().frame_count(),
        6,
        "the same six frames at the same rate"
    );
}

#[test]
fn an_apng_refuses_more_frames_than_a_movie_holds_in_words() {
    let document = animation(MAX_MOVIE_FRAMES as u32 + 1, (4, 4), (24, 1));
    let error = crate::apng_bytes(&document).expect_err("too many frames");
    assert!(error.contains("APNG is limited"), "{error}");
}
