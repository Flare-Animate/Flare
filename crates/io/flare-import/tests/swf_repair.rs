//! Movies whose tag blocks lack their End records, which Flash Player plays, and movies nested
//! too deep to read without a deep stack.

#![allow(clippy::expect_used)]

use flare_document::SymbolKind;
use flare_import::{ImportError, Severity, import_swf_bytes};
use swf::{
    CharacterId, Color, FillStyle, Fixed8, Header, Matrix, PlaceObject, PlaceObjectAction,
    Rectangle, Tag, Twips,
};

fn header(frames: u16) -> Header {
    Header {
        compression: swf::Compression::None,
        version: 10,
        stage_size: Rectangle {
            x_min: Twips::ZERO,
            x_max: Twips::from_pixels(100.0),
            y_min: Twips::ZERO,
            y_max: Twips::from_pixels(100.0),
        },
        frame_rate: Fixed8::from_f32(24.0),
        num_frames: frames,
    }
}

fn bytes(tags: &[Tag<'_>]) -> Vec<u8> {
    let mut out = Vec::new();
    swf::write_swf(&header(1), tags, &mut out).expect("write");
    out
}

/// The same movie with its last two bytes (the End record) cut off, as some files are.
fn without_movie_end(mut data: Vec<u8>) -> Vec<u8> {
    data.truncate(data.len() - 2);
    let length = u32::try_from(data.len()).expect("small");
    data[4..8].copy_from_slice(&length.to_le_bytes());
    data
}

fn square(id: CharacterId) -> Tag<'static> {
    Tag::DefineShape(Box::new(swf::Shape {
        version: 1,
        id,
        shape_bounds: Rectangle {
            x_min: Twips::ZERO,
            x_max: Twips::from_pixels(10.0),
            y_min: Twips::ZERO,
            y_max: Twips::from_pixels(10.0),
        },
        edge_bounds: Rectangle {
            x_min: Twips::ZERO,
            x_max: Twips::from_pixels(10.0),
            y_min: Twips::ZERO,
            y_max: Twips::from_pixels(10.0),
        },
        flags: swf::ShapeFlag::empty(),
        styles: swf::ShapeStyles {
            fill_styles: vec![FillStyle::Color(Color {
                r: 255,
                g: 0,
                b: 0,
                a: 255,
            })],
            line_styles: vec![],
        },
        shape: vec![
            swf::ShapeRecord::StyleChange(Box::new(swf::StyleChangeData {
                move_to: Some(swf::Point {
                    x: Twips::ZERO,
                    y: Twips::ZERO,
                }),
                fill_style_0: None,
                fill_style_1: Some(1),
                line_style: None,
                new_styles: None,
            })),
            swf::ShapeRecord::StraightEdge {
                delta: swf::PointDelta {
                    dx: Twips::from_pixels(10.0),
                    dy: Twips::ZERO,
                },
            },
        ],
    }))
}

fn place(id: CharacterId, depth: u16) -> Tag<'static> {
    Tag::PlaceObject(Box::new(PlaceObject {
        version: 2,
        action: PlaceObjectAction::Place(id),
        depth,
        matrix: Some(Matrix::translate(Twips::ZERO, Twips::ZERO)),
        color_transform: None,
        ratio: None,
        name: None,
        clip_depth: None,
        class_name: None,
        filters: None,
        background_color: None,
        blend_mode: None,
        clip_actions: None,
        has_image: false,
        is_bitmap_cached: None,
        is_visible: None,
        amf_data: None,
    }))
}

#[test]
fn a_movie_without_its_end_record_imports_and_says_so() {
    // The writer adds the movie's End record itself; cutting it off leaves the block open.
    let data = without_movie_end(bytes(&[square(1), place(1, 1), Tag::ShowFrame]));
    let imported = import_swf_bytes(&data).expect("import");
    assert_eq!(imported.document.symbols().len(), 1);
    assert!(
        imported
            .diagnostics
            .iter()
            .any(|d| d.severity == Severity::Info && d.message.contains("no End record")),
        "{:?}",
        imported.diagnostics
    );
}

#[test]
fn a_sprite_without_its_end_record_imports_with_its_own_frames() {
    let data = bytes(&[
        square(1),
        Tag::DefineSprite(swf::Sprite {
            id: 2,
            num_frames: 2,
            tags: vec![place(1, 1), Tag::ShowFrame, Tag::ShowFrame],
        }),
        place(2, 1),
        Tag::ShowFrame,
        Tag::End,
    ]);
    let imported = import_swf_bytes(&data).expect("import");
    let clip = imported
        .document
        .symbols()
        .iter()
        .find(|s| s.kind() == SymbolKind::MovieClip)
        .expect("a movie clip");
    // The sprite's object does not move, so both of its frames are one span.
    assert_eq!(clip.timeline().layers()[0].spans().len(), 1);
}

#[test]
fn sprites_nested_past_the_limit_are_refused_with_a_reason() {
    let mut inner: Vec<Tag<'static>> = vec![place(1, 1), Tag::ShowFrame];
    for level in 0..40_u16 {
        inner = vec![Tag::DefineSprite(swf::Sprite {
            id: 100 + level,
            num_frames: 1,
            tags: inner,
        })];
    }
    let data = bytes(&[square(1), inner.remove(0), Tag::ShowFrame]);
    match import_swf_bytes(&data) {
        Err(ImportError::NotSwf(message)) => assert!(message.contains("nest"), "{message}"),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn a_swf_name_on_something_else_is_refused_as_a_movie_not_read_as_xfl() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("picture.swf");
    std::fs::write(&path, b"\x89PNG\r\n\x1a\nnot a movie").expect("write");
    match flare_import::import_path(&path) {
        Err(ImportError::NotSwf(message)) => {
            assert!(message.contains("Flash movie"), "{message}");
        }
        other => panic!("expected a refusal as a movie, got {other:?}"),
    }
}
