//! What Ruffle draws for a published SWF, against what Flare draws for the same frame.
//!
//! Needs the `render-check` feature (Ruffle's exporter, built from third_party/ruffle) and a
//! GPU adapter wgpu can open; without one the exporter fails and so does this test.
#![cfg(feature = "render-check")]
#![allow(clippy::expect_used)]

use clap::Parser;
use flare_core::{DocumentId, FrameRate, LayerId, Rgba8, SceneId, ShapeId};
use flare_document::{
    Content, Document, FrameSpan, Instance, Layer, PlayMode, Playback, Scene, Shape, Stage, Symbol,
    SymbolKind, Timeline,
};
use flare_publish::{PublishOptions, publish_swf};
use std::num::NonZeroU32;

const WIDTH: u32 = 120;
const HEIGHT: u32 = 80;

fn document() -> Document {
    let span = |shape: Shape| FrameSpan::new(0, NonZeroU32::MIN, Content::Shape(shape));
    let rect = Shape::rect(
        ShapeId::new(),
        10.0,
        10.0,
        50.0,
        40.0,
        Rgba8::opaque(220, 30, 30),
    )
    .expect("rect");
    let disc = Shape::ellipse(
        ShapeId::new(),
        85.0,
        40.0,
        22.0,
        22.0,
        Rgba8::opaque(30, 160, 60),
    )
    .expect("ellipse");
    let layers = vec![
        Layer::new(LayerId::new(), "disc", vec![span(disc)]).expect("layer"),
        Layer::new(LayerId::new(), "rect", vec![span(rect)]).expect("layer"),
    ];
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(layers));
    Document::new(
        DocumentId::new(),
        FrameRate::new(12, 1).expect("rate"),
        vec![scene],
        vec![],
    )
    .expect("document")
    .with_stage(Stage::new(WIDTH, HEIGHT, Rgba8::opaque(250, 250, 250)).expect("stage"))
}

/// Ruffle's picture of frame `skip` of `swf`, as RGBA rows.
fn ruffle_draws(swf: &std::path::Path, png_path: &std::path::Path, skip: u32) -> Vec<u8> {
    let skip = skip.to_string();
    let options = exporter::cli::Opt::parse_from([
        "exporter".as_ref(),
        swf.as_os_str(),
        png_path.as_os_str(),
        "--silent".as_ref(),
        "--skipframes".as_ref(),
        skip.as_ref(),
    ]);
    exporter::run_main(options).expect("Ruffle exports the frame");
    let decoder = png::Decoder::new(std::io::BufReader::new(
        std::fs::File::open(png_path).expect("the exported picture"),
    ));
    let mut reader = decoder.read_info().expect("a PNG");
    let mut pixels = vec![0; reader.output_buffer_size().expect("size")];
    let info = reader.next_frame(&mut pixels).expect("a frame");
    assert_eq!((info.width, info.height), (WIDTH, HEIGHT));
    assert_eq!(info.color_type, png::ColorType::Rgba);
    pixels
}

/// Publishes `doc`, has Ruffle draw frame 0, and returns that with Flare's own drawing of `compare`.
fn both(doc: &Document, compare: &Document) -> (Vec<u8>, flare_render::Image) {
    both_at(doc, compare, 0)
}

/// [`both`] for a frame other than the first.
fn both_at(doc: &Document, compare: &Document, frame: u32) -> (Vec<u8>, flare_render::Image) {
    let published = publish_swf(doc, &PublishOptions::default()).expect("publish");
    let dir = tempfile::tempdir().expect("dir");
    let swf = dir.path().join("movie.swf");
    std::fs::write(&swf, &published.bytes).expect("write");
    let theirs = ruffle_draws(&swf, &dir.path().join("frame.png"), frame);
    let evaluated =
        flare_eval::evaluate_frame(compare, compare.scenes()[0].id(), frame).expect("frame");
    let ours = flare_render::render(&evaluated, WIDTH, HEIGHT, compare.stage().background())
        .expect("draw");
    (theirs, ours)
}

/// The mean absolute difference per color channel.
fn difference(ours: &flare_render::Image, theirs: &[u8]) -> f64 {
    let mut total = 0_u64;
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let mine = ours.pixel(x, y).expect("pixel");
            let at = ((y * WIDTH + x) * 4) as usize;
            total += u64::from(mine.r.abs_diff(theirs[at]))
                + u64::from(mine.g.abs_diff(theirs[at + 1]))
                + u64::from(mine.b.abs_diff(theirs[at + 2]));
        }
    }
    total as f64 / f64::from(WIDTH * HEIGHT * 3)
}

#[test]
fn ruffle_draws_the_published_frame_as_flare_does() {
    let doc = document();
    let (theirs, ours) = both(&doc, &doc);
    // Edges are anti-aliased differently by the two renderers, so the picture as a whole must
    // be close, and the middle of each shape and the open background must match.
    let mean = difference(&ours, &theirs);
    assert!(
        mean < 1.0,
        "the pictures differ by {mean} per channel on average"
    );
    for (x, y, want) in [
        (35, 30, Rgba8::opaque(220, 30, 30)),
        (85, 40, Rgba8::opaque(30, 160, 60)),
        (2, 2, Rgba8::opaque(250, 250, 250)),
    ] {
        let at = ((y * WIDTH + x) * 4) as usize;
        let got = &theirs[at..at + 3];
        for (g, w) in got.iter().zip([want.r, want.g, want.b]) {
            assert!(
                g.abs_diff(w) <= 2,
                "Ruffle drew {got:?} at ({x},{y}), expected {want:?}"
            );
        }
    }
}

/// A gradient across the top half and, under a round mask, a green rectangle across the bottom.
fn gradient_and_mask() -> Document {
    let one = |shape: Shape| FrameSpan::new(0, NonZeroU32::MIN, Content::Shape(shape));
    let gradient = flare_document::Gradient::linear(
        flare_core::DocumentPoint::new(0.0, 0.0).expect("point"),
        flare_core::DocumentPoint::new(120.0, 0.0).expect("point"),
        Rgba8::opaque(220, 30, 30),
        Rgba8::opaque(30, 30, 220),
    )
    .expect("gradient");
    let bar = Shape::rect(ShapeId::new(), 0.0, 0.0, 120.0, 40.0, Rgba8::TRANSPARENT)
        .expect("rect")
        .with_gradient(gradient);
    let hole =
        Shape::ellipse(ShapeId::new(), 60.0, 60.0, 15.0, 15.0, Rgba8::WHITE).expect("ellipse");
    let art = Shape::rect(
        ShapeId::new(),
        0.0,
        40.0,
        120.0,
        40.0,
        Rgba8::opaque(30, 160, 60),
    )
    .expect("rect");
    let layers = vec![
        Layer::new(LayerId::new(), "mask", vec![one(hole)])
            .expect("layer")
            .with_mask(true),
        Layer::new(LayerId::new(), "art", vec![one(art)]).expect("layer"),
        Layer::new(LayerId::new(), "bar", vec![one(bar)]).expect("layer"),
    ];
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(layers));
    Document::new(
        DocumentId::new(),
        FrameRate::new(12, 1).expect("rate"),
        vec![scene],
        vec![],
    )
    .expect("document")
    .with_stage(Stage::new(WIDTH, HEIGHT, Rgba8::opaque(250, 250, 250)).expect("stage"))
}

#[test]
fn ruffle_draws_gradients_and_masks_as_flare_does() {
    let doc = gradient_and_mask();
    let (theirs, ours) = both(&doc, &doc);
    let mean = difference(&ours, &theirs);
    assert!(
        mean < 3.0,
        "the pictures differ by {mean} per channel on average"
    );
    let pixel = |x: u32, y: u32| {
        let at = ((y * WIDTH + x) * 4) as usize;
        [theirs[at], theirs[at + 1], theirs[at + 2]]
    };
    // Red at the left end of the gradient, blue at the right, and between them a mix.
    assert!(
        pixel(2, 20)[0] > 200 && pixel(2, 20)[2] < 50,
        "{:?}",
        pixel(2, 20)
    );
    assert!(
        pixel(117, 20)[2] > 200 && pixel(117, 20)[0] < 50,
        "{:?}",
        pixel(117, 20)
    );
    // Green inside the mask, and the background where the mask hides it.
    assert_eq!(pixel(60, 60), [30, 160, 60]);
    assert_eq!(pixel(10, 70), [250, 250, 250]);
}

#[test]
fn the_comparison_fails_for_a_different_picture() {
    // Ruffle draws a black stage; Flare is asked for the light one.
    let dark =
        document().with_stage(Stage::new(WIDTH, HEIGHT, Rgba8::opaque(0, 0, 0)).expect("stage"));
    let light = document();
    let published = publish_swf(&dark, &PublishOptions::default()).expect("publish");
    let dir = tempfile::tempdir().expect("dir");
    let swf = dir.path().join("movie.swf");
    std::fs::write(&swf, &published.bytes).expect("write");
    let theirs = ruffle_draws(&swf, &dir.path().join("frame.png"), 0);
    let frame = flare_eval::evaluate_frame(&light, light.scenes()[0].id(), 0).expect("frame");
    let ours =
        flare_render::render(&frame, WIDTH, HEIGHT, light.stage().background()).expect("draw");
    assert!(difference(&ours, &theirs) > 50.0);
}

/// Three rows of one symbol of three colored frames, each playing it a different way, and each
/// starting again after a gap.
fn symbols() -> Document {
    let steps = |kind| {
        let square = |x: f64, c: Rgba8| {
            Content::Shape(Shape::rect(ShapeId::new(), x, 0.0, 20.0, 20.0, c).expect("rect"))
        };
        let frame = |at: u32, content| FrameSpan::new(at, NonZeroU32::MIN, content);
        let layer = Layer::new(
            LayerId::new(),
            "L",
            vec![
                frame(0, square(0.0, Rgba8::opaque(220, 30, 30))),
                frame(1, square(30.0, Rgba8::opaque(30, 160, 60))),
                frame(2, square(60.0, Rgba8::opaque(30, 60, 220))),
            ],
        )
        .expect("layer");
        Symbol::new(
            flare_core::SymbolId::new(),
            "Steps",
            Timeline::new(vec![layer]),
        )
        .with_kind(kind)
    };
    let (graphic, clip) = (steps(SymbolKind::Graphic), steps(SymbolKind::MovieClip));
    let row = |symbol: &Symbol, y: f64, playback: Playback| {
        let place = |start: u32| {
            let at = flare_core::Transform2D::new(glamx::DAffine2::from_translation(
                glamx::DVec2::new(0.0, y),
            ))
            .expect("finite");
            FrameSpan::new(
                start,
                NonZeroU32::new(5).expect("nonzero"),
                Content::Instance(
                    Instance::new(flare_core::InstanceId::new(), symbol.id(), at)
                        .with_playback(playback),
                ),
            )
        };
        Layer::new(LayerId::new(), "row", vec![place(0), place(6)]).expect("layer")
    };
    let rows = vec![
        row(&graphic, 0.0, Playback::default()),
        row(
            &graphic,
            25.0,
            Playback {
                mode: PlayMode::PlayOnce,
                first_frame: 1,
            },
        ),
        row(&clip, 50.0, Playback::default()),
    ];
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(rows));
    Document::new(
        DocumentId::new(),
        FrameRate::new(12, 1).expect("rate"),
        vec![scene],
        vec![graphic, clip],
    )
    .expect("document")
    .with_stage(Stage::new(WIDTH, HEIGHT, Rgba8::opaque(250, 250, 250)).expect("stage"))
}

#[test]
fn ruffle_plays_the_published_symbols_on_the_clocks_flare_gives_them() {
    let doc = symbols();
    let mut seen = Vec::new();
    for frame in [0, 1, 2, 3, 4, 6, 7, 8, 9, 10] {
        let (theirs, ours) = both_at(&doc, &doc, frame);
        let mean = difference(&ours, &theirs);
        assert!(
            mean < 1.0,
            "frame {frame}: the pictures differ by {mean} per channel on average"
        );
        seen.push(ours.rgba().to_vec());
    }
    // The test could not tell a stopped clock from a running one if every frame were alike.
    assert_ne!(seen[0], seen[1], "the symbols move from frame to frame");
    assert_ne!(seen[1], seen[2]);
}
