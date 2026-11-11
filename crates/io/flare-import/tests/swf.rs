//! Importing Flash movies, from movies built here tag by tag.

#![allow(clippy::expect_used)]

use flare_document::{Content, SymbolKind};
use flare_import::{ImportError, Severity, import_swf_bytes};
use swf::{Color, FillStyle, Matrix, PlaceObjectAction, RemoveObject, Tag};

mod swf_support;
use swf_support::{bytes, header, moving_square, placement, red, square};

fn instance_at(content: &Content) -> (f64, f64) {
    let Content::Instance(instance) = content else {
        panic!("not an instance: {content:?}")
    };
    let m = instance.transform().as_affine();
    (m.translation.x, m.translation.y)
}

#[test]
fn a_shape_becomes_a_graphic_symbol_and_its_placements_become_keyframes() {
    let imported = import_swf_bytes(&moving_square()).expect("import");
    let document = &imported.document;
    let stage = document.stage();
    assert_eq!((stage.width(), stage.height()), (200, 100));
    assert_eq!(document.frame_rate().numerator(), 24);
    assert_eq!(document.stage().background().r, 16);

    assert_eq!(document.symbols().len(), 1);
    let symbol = &document.symbols()[0];
    assert_eq!(symbol.kind(), SymbolKind::Graphic);
    assert_eq!(symbol.timeline().layers().len(), 1);

    let layers = document.scenes()[0].timeline().layers();
    assert_eq!(layers.len(), 1, "one layer for the one depth");
    let spans = layers[0].spans();
    let seen: Vec<(u32, u32, (f64, f64))> = spans
        .iter()
        .map(|s| (s.start(), s.duration().get(), instance_at(s.content())))
        .collect();
    assert_eq!(
        seen,
        [(0, 2, (10.0, 20.0)), (2, 1, (30.0, 20.0))],
        "unchanged frames hold, the move is a keyframe"
    );
    assert!(
        imported
            .diagnostics
            .iter()
            .all(|d| d.severity == Severity::Info),
        "{:?}",
        imported.diagnostics
    );
}

#[test]
fn removing_an_object_ends_its_span_and_the_highest_depth_is_on_top() {
    let mut header = header(100.0, 100.0, 12.0);
    header.num_frames = 3;
    let data = bytes(
        &header,
        &[
            square(1, 10.0, FillStyle::Color(red())),
            placement(PlaceObjectAction::Place(1), 1, Some((0.0, 0.0))),
            placement(PlaceObjectAction::Place(1), 5, Some((5.0, 5.0))),
            Tag::ShowFrame,
            Tag::RemoveObject(RemoveObject {
                depth: 1,
                character_id: None,
            }),
            Tag::ShowFrame,
            Tag::ShowFrame,
            Tag::End,
        ],
    );
    let imported = import_swf_bytes(&data).expect("import");
    let layers = imported.document.scenes()[0].timeline().layers();
    assert_eq!(layers.len(), 2);
    assert_eq!(
        layers[0].name(),
        "Depth 5",
        "the top layer is the highest depth"
    );
    assert_eq!(layers[0].spans()[0].duration().get(), 3);
    assert_eq!(
        layers[1].spans()[0].duration().get(),
        1,
        "removed after one frame"
    );
}

#[test]
fn a_sprite_becomes_a_movie_clip_with_a_timeline_of_its_own() {
    let mut header = header(100.0, 100.0, 24.0);
    header.num_frames = 1;
    let data = bytes(
        &header,
        &[
            square(1, 10.0, FillStyle::Color(red())),
            Tag::DefineSprite(swf::Sprite {
                id: 2,
                num_frames: 2,
                tags: vec![
                    placement(PlaceObjectAction::Place(1), 1, Some((0.0, 0.0))),
                    Tag::ShowFrame,
                    placement(PlaceObjectAction::Modify, 1, Some((8.0, 0.0))),
                    Tag::ShowFrame,
                    Tag::End,
                ],
            }),
            placement(PlaceObjectAction::Place(2), 1, Some((40.0, 40.0))),
            Tag::ShowFrame,
            Tag::End,
        ],
    );
    let imported = import_swf_bytes(&data).expect("import");
    let document = &imported.document;
    assert_eq!(document.symbols().len(), 2);
    let clip = document
        .symbols()
        .iter()
        .find(|s| s.kind() == SymbolKind::MovieClip)
        .expect("a movie clip");
    let spans = clip.timeline().layers()[0].spans();
    assert_eq!(spans.len(), 2, "its own two frames");
    assert_eq!(instance_at(spans[1].content()), (8.0, 0.0));
}

#[test]
fn export_names_name_the_symbols() {
    let mut header = header(100.0, 100.0, 24.0);
    header.num_frames = 1;
    let name = swf::SwfStr::from_utf8_str("Logo");
    let data = bytes(
        &header,
        &[
            square(1, 10.0, FillStyle::Color(red())),
            Tag::ExportAssets(vec![swf::ExportedAsset { id: 1, name }]),
            placement(PlaceObjectAction::Place(1), 1, None),
            Tag::ShowFrame,
            Tag::End,
        ],
    );
    let imported = import_swf_bytes(&data).expect("import");
    assert_eq!(imported.document.symbols()[0].name(), "Logo");
}

#[test]
fn what_is_not_carried_over_is_said_not_silently_dropped() {
    let mut header = header(100.0, 100.0, 24.0);
    header.num_frames = 1;
    let gradient = FillStyle::LinearGradient(swf::Gradient {
        matrix: Matrix::IDENTITY,
        spread: swf::GradientSpread::Pad,
        interpolation: swf::GradientInterpolation::Rgb,
        records: vec![
            swf::GradientRecord {
                ratio: 0,
                color: Color {
                    r: 0,
                    g: 0,
                    b: 255,
                    a: 255,
                },
            },
            swf::GradientRecord {
                ratio: 255,
                color: red(),
            },
        ],
    });
    let data = bytes(
        &header,
        &[
            square(1, 10.0, gradient),
            Tag::DoAction(&[0][..]),
            // Character 9 is never defined (text and buttons are not imported).
            placement(PlaceObjectAction::Place(9), 2, None),
            placement(PlaceObjectAction::Place(1), 1, None),
            Tag::ShowFrame,
            Tag::End,
        ],
    );
    let imported = import_swf_bytes(&data).expect("import");
    let said: String = imported
        .diagnostics
        .iter()
        .map(|d| d.message.clone())
        .collect::<Vec<_>>()
        .join("\n");
    for wanted in ["gradient", "scripts", "not imported"] {
        assert!(said.contains(wanted), "{wanted}: {said}");
    }
    // The gradient shape is still drawn, in its first color.
    let layers = imported.document.symbols()[0].timeline().layers();
    assert_eq!(layers.len(), 1);
    assert_eq!(imported.document.scenes()[0].timeline().layers().len(), 1);
}

#[test]
fn files_that_are_not_movies_or_claim_too_much_are_refused_with_a_reason() {
    assert!(matches!(
        import_swf_bytes(b"PK\x03\x04 not a movie"),
        Err(ImportError::NotSwf(_))
    ));
    assert!(matches!(import_swf_bytes(&[]), Err(ImportError::NotSwf(_))));
    // A header that says the movie is gigabytes long is refused before anything is allocated.
    let mut huge = moving_square();
    huge[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(
        import_swf_bytes(&huge),
        Err(ImportError::TooLarge(_))
    ));
}

#[test]
fn every_truncation_and_many_mutations_of_a_movie_end_cleanly() {
    let valid = moving_square();
    for end in 0..valid.len() {
        let _ = import_swf_bytes(&valid[..end]);
    }
    let mut state = 0x9e37_79b9_7f4a_7c15_u64;
    for _ in 0..400 {
        let mut copy = valid.clone();
        for _ in 0..3 {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let at = 8 + (state >> 33) as usize % (copy.len() - 8);
            copy[at] ^= (state >> 20) as u8;
        }
        let _ = import_swf_bytes(&copy);
    }
}

fn pixel(document: &flare_document::Document, frame: u32, x: u32, y: u32) -> flare_core::Rgba8 {
    let scene = document.scenes()[0].id();
    let evaluated = flare_eval::evaluate_frame(document, scene, frame).expect("eval");
    let stage = document.stage();
    let image = flare_render::render(
        &evaluated,
        stage.width(),
        stage.height(),
        stage.background(),
    )
    .expect("render");
    image.pixel(x, y).expect("pixel")
}

#[test]
fn the_imported_shape_draws_where_the_movie_put_it_on_each_frame() {
    let imported = import_swf_bytes(&moving_square()).expect("import");
    let document = &imported.document;
    let red = flare_core::Rgba8::opaque(255, 0, 0);
    let background = flare_core::Rgba8::opaque(16, 32, 48);
    // Frames 0 and 1: the square is at (10, 20); frame 2: at (30, 20).
    assert_eq!(pixel(document, 0, 15, 25), red);
    assert_eq!(pixel(document, 1, 15, 25), red);
    assert_eq!(pixel(document, 2, 15, 25), background);
    assert_eq!(pixel(document, 2, 35, 25), red);
    assert_eq!(pixel(document, 0, 35, 25), background);
}

#[test]
fn a_movie_is_imported_from_a_path_or_from_bytes_by_what_it_starts_with() {
    let dir = tempfile::tempdir().expect("dir");
    // The name does not matter: the start of the file says what it is.
    let path = dir.path().join("movie.dat");
    std::fs::write(&path, moving_square()).expect("write");
    let from_path = flare_import::import_path(&path).expect("path");
    let from_bytes = flare_import::import_bytes(moving_square()).expect("bytes");
    assert_eq!(from_path.document.symbols().len(), 1);
    assert_eq!(from_bytes.document.symbols().len(), 1);
    // Anything else still goes the way of a .fla.
    assert!(matches!(
        flare_import::import_bytes(b"not a movie or a zip".to_vec()),
        Err(ImportError::NotXfl(_))
    ));
}
