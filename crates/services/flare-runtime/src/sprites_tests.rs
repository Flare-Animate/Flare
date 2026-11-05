#![allow(clippy::expect_used)]
use super::*;
use flare_core::{DocumentId, FrameRate, LayerId, SceneId, ShapeId};
use flare_document::{Content, FrameSpan, Layer, Scene, Shape, Timeline};
use std::num::NonZeroU32;

fn doc(frames: u32) -> Document {
    let shape =
        Shape::rect(ShapeId::new(), 0.0, 0.0, 4.0, 4.0, Rgba8::opaque(255, 0, 0)).expect("rect");
    let span = FrameSpan::new(
        0,
        NonZeroU32::new(frames).expect("n"),
        Content::Shape(shape),
    );
    let layer = Layer::new(LayerId::new(), "L", vec![span]).expect("layer");
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(vec![layer]));
    Document::new(
        DocumentId::new(),
        FrameRate::new(25, 1).expect("rate"),
        vec![scene],
        vec![],
    )
    .expect("doc")
    .with_stage(flare_document::Stage::new(8, 6, Rgba8::WHITE).expect("stage"))
}

#[test]
fn frames_are_laid_out_in_a_grid_with_an_atlas() {
    let sheet = sprite_sheet(&doc(5), 0, None, Some(3), "a \"b\".png").expect("sheet");
    assert_eq!((sheet.width, sheet.height, sheet.frames), (24, 12, 5));
    let decoder = png::Decoder::new(std::io::Cursor::new(&sheet.png));
    let mut reader = decoder.read_info().expect("png");
    let mut buf = vec![0; reader.output_buffer_size().expect("size")];
    reader.next_frame(&mut buf).expect("decode");
    // Frame 0 holds the red square at its corner; the sixth cell is empty and transparent.
    assert_eq!(&buf[..4], &[255, 0, 0, 255]);
    let empty = ((6 * 24) + 16) * 4;
    assert_eq!(buf[empty + 3], 0);
    assert!(
        sheet
            .json
            .contains("\"frame_00004\": {\"frame\": {\"x\": 8, \"y\": 6")
    );
    assert!(sheet.json.contains("\"duration\": 40"));
    assert!(sheet.json.contains("\"image\": \"a \\\"b\\\".png\""));
}

#[test]
fn a_range_has_both_ends_and_bad_ranges_fail() {
    let d = doc(5);
    assert_eq!(
        sprite_sheet(&d, 1, Some(2), None, "s.png")
            .expect("sheet")
            .frames,
        2
    );
    assert_eq!(
        sprite_sheet(&d, 4, Some(4), None, "s.png")
            .expect("sheet")
            .frames,
        1
    );
    assert!(sprite_sheet(&d, 3, Some(2), None, "s.png").is_err());
    assert!(sprite_sheet(&d, 0, Some(5), None, "s.png").is_err());
    assert!(sprite_sheet(&d, 0, None, Some(0), "s.png").is_err());
    assert!(sprite_sheet(&d, 0, None, Some(9), "s.png").is_err());
}

#[test]
fn a_sheet_over_the_size_limit_is_refused() {
    let wide =
        doc(3).with_stage(flare_document::Stage::new(9000, 10, Rgba8::WHITE).expect("stage"));
    let err = sprite_sheet(&wide, 0, None, Some(2), "s.png").expect_err("too wide");
    assert!(err.contains("limit"));
}
