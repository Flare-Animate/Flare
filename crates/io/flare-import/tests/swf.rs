//! Importing Flash movies, from movies built here tag by tag.

#![allow(clippy::expect_used)]

use flare_document::{Content, SymbolKind};
use flare_import::{ImportError, Severity, import_swf_bytes};
use swf::avm2::types::{AbcFile, ConstantPool};
use swf::{
    CharacterId, Color, DefineBinaryData, DoAbc2, DoAbc2Flag, FillStyle, Fixed8, Header, Matrix,
    PlaceObject, PlaceObjectAction, Point, PointDelta, Rectangle, RemoveObject, Shape, ShapeFlag,
    ShapeRecord, ShapeStyles, StyleChangeData, SwfStr, Tag, Twips,
};

fn px(v: f64) -> Twips {
    Twips::from_pixels(v)
}

fn header(width: f64, height: f64, fps: f32) -> Header {
    Header {
        compression: swf::Compression::None,
        version: 10,
        stage_size: Rectangle {
            x_min: Twips::ZERO,
            x_max: px(width),
            y_min: Twips::ZERO,
            y_max: px(height),
        },
        frame_rate: Fixed8::from_f32(fps),
        num_frames: 1,
    }
}

fn bytes(header: &Header, tags: &[Tag<'_>]) -> Vec<u8> {
    let mut out = Vec::new();
    swf::write_swf(header, tags, &mut out).expect("write");
    out
}

fn red() -> Color {
    Color {
        r: 255,
        g: 0,
        b: 0,
        a: 255,
    }
}

/// A square of `side` pixels filled with `fill`, from the origin.
fn square(id: CharacterId, side: f64, fill: FillStyle) -> Tag<'static> {
    let edge = |dx: f64, dy: f64| ShapeRecord::StraightEdge {
        delta: PointDelta {
            dx: px(dx),
            dy: px(dy),
        },
    };
    let bounds = Rectangle {
        x_min: Twips::ZERO,
        x_max: px(side),
        y_min: Twips::ZERO,
        y_max: px(side),
    };
    Tag::DefineShape(Box::new(Shape {
        version: 1,
        id,
        shape_bounds: bounds,
        edge_bounds: bounds,
        flags: ShapeFlag::empty(),
        styles: ShapeStyles {
            fill_styles: vec![fill],
            line_styles: vec![],
        },
        shape: vec![
            ShapeRecord::StyleChange(Box::new(StyleChangeData {
                move_to: Some(Point {
                    x: Twips::ZERO,
                    y: Twips::ZERO,
                }),
                fill_style_0: None,
                fill_style_1: Some(1),
                line_style: None,
                new_styles: None,
            })),
            edge(side, 0.0),
            edge(0.0, side),
            edge(-side, 0.0),
            edge(0.0, -side),
        ],
    }))
}

fn placement(action: PlaceObjectAction, depth: u16, at: Option<(f64, f64)>) -> Tag<'static> {
    Tag::PlaceObject(Box::new(PlaceObject {
        version: 2,
        action,
        depth,
        matrix: at.map(|(x, y)| Matrix::translate(px(x), px(y))),
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

fn instance_at(content: &Content) -> (f64, f64) {
    let Content::Instance(instance) = content else {
        panic!("not an instance: {content:?}")
    };
    let m = instance.transform().as_affine();
    (m.translation.x, m.translation.y)
}

fn moving_square() -> Vec<u8> {
    let mut header = header(200.0, 100.0, 24.0);
    header.num_frames = 3;
    bytes(
        &header,
        &[
            Tag::SetBackgroundColor(Color {
                r: 16,
                g: 32,
                b: 48,
                a: 255,
            }),
            square(1, 10.0, FillStyle::Color(red())),
            placement(PlaceObjectAction::Place(1), 1, Some((10.0, 20.0))),
            Tag::ShowFrame,
            Tag::ShowFrame,
            placement(PlaceObjectAction::Modify, 1, Some((30.0, 20.0))),
            Tag::ShowFrame,
            Tag::End,
        ],
    )
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

// ---- Flash 3D engine recognition -----------------------------------------------------------

/// An ABC payload whose constant pool carries `strings` and nothing else, serialised by the
/// swf crate's own writer so the reader is exercised on well-formed data.
fn abc(strings: &[&str]) -> Vec<u8> {
    let file = AbcFile {
        major_version: 46,
        minor_version: 16,
        constant_pool: ConstantPool {
            ints: vec![],
            uints: vec![],
            doubles: vec![],
            strings: strings.iter().map(|s| s.as_bytes().to_vec()).collect(),
            namespaces: vec![],
            namespace_sets: vec![],
            multinames: vec![],
        },
        methods: vec![],
        metadata: vec![],
        instances: vec![],
        classes: vec![],
        scripts: vec![],
        method_bodies: vec![],
    };
    let mut out = Vec::new();
    swf::avm2::write::Writer::new(&mut out)
        .write(file)
        .expect("write abc");
    out
}

fn do_abc2<'a>(data: &'a [u8]) -> Tag<'a> {
    Tag::DoAbc2(DoAbc2 {
        flags: DoAbc2Flag::empty(),
        name: SwfStr::from_utf8_str("main"),
        data,
    })
}

fn script_reports(imported: &flare_import::Imported) -> Vec<&str> {
    imported
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Info && d.location == "scripts")
        .map(|d| d.message.as_str())
        .collect()
}

#[test]
fn a_movie_names_the_3d_engine_its_scripts_are_written_against() {
    let payload = abc(&["flare.basic.Scene3D", "flare.core.Mesh3D"]);
    let data = bytes(
        &header(100.0, 100.0, 12.0),
        &[do_abc2(&payload), Tag::ShowFrame, Tag::End],
    );
    let imported = import_swf_bytes(&data).expect("import");
    let reports = script_reports(&imported);
    assert_eq!(reports.len(), 1, "{:?}", imported.diagnostics);
    assert!(
        reports[0].contains("Flare3D engine") && reports[0].contains("flare.basic.Scene3D"),
        "{reports:?}"
    );
}

#[test]
fn an_alternativa3d_movie_is_reported_by_its_name() {
    let payload = abc(&["alternativa.engine3d.core.Camera3D", "alternativa.utils"]);
    let data = bytes(
        &header(100.0, 100.0, 12.0),
        &[do_abc2(&payload), Tag::ShowFrame, Tag::End],
    );
    let imported = import_swf_bytes(&data).expect("import");
    let reports = script_reports(&imported);
    assert_eq!(reports.len(), 1, "{:?}", imported.diagnostics);
    assert!(reports[0].contains("Alternativa3D engine"), "{reports:?}");
}

#[test]
fn stage3d_is_reported_only_when_no_engine_names_it() {
    let alone = abc(&["flash.display3D.Context3D"]);
    let data = bytes(
        &header(100.0, 100.0, 12.0),
        &[do_abc2(&alone), Tag::ShowFrame, Tag::End],
    );
    let imported = import_swf_bytes(&data).expect("import");
    let reports = script_reports(&imported);
    assert_eq!(reports.len(), 1, "{:?}", imported.diagnostics);
    assert!(reports[0].contains("Stage 3D"), "{reports:?}");

    let with_engine = abc(&["flare.basic.Scene3D", "flash.display3D.Context3D"]);
    let data = bytes(
        &header(100.0, 100.0, 12.0),
        &[do_abc2(&with_engine), Tag::ShowFrame, Tag::End],
    );
    let imported = import_swf_bytes(&data).expect("import");
    let reports = script_reports(&imported);
    assert_eq!(reports.len(), 1, "{:?}", imported.diagnostics);
    assert!(reports[0].contains("Flare3D engine"), "{reports:?}");
}

#[test]
fn embedded_binary_data_is_counted_when_an_engine_is_present() {
    let payload = abc(&["flare.core.Pivot3D"]);
    let binary = Tag::DefineBinaryData(DefineBinaryData {
        id: 9,
        data: &[0, 1, 2],
    });
    let data = bytes(
        &header(100.0, 100.0, 12.0),
        &[do_abc2(&payload), binary, Tag::ShowFrame, Tag::End],
    );
    let imported = import_swf_bytes(&data).expect("import");
    let reports = script_reports(&imported);
    assert!(
        reports.iter().any(|m| m.contains("embedded binary data")),
        "{reports:?}"
    );
}

#[test]
fn an_unreadable_script_block_is_reported_and_does_not_stop_the_import() {
    let data = bytes(
        &header(100.0, 100.0, 12.0),
        &[Tag::DoAbc(&[0xff; 16]), Tag::ShowFrame, Tag::End],
    );
    let imported = import_swf_bytes(&data).expect("import");
    let reports = script_reports(&imported);
    assert_eq!(reports.len(), 1, "{:?}", imported.diagnostics);
    assert!(reports[0].contains("could not be read"), "{reports:?}");
    // The import itself still carried the movie over.
    assert_eq!(imported.document.scenes().len(), 1);
}

#[test]
fn only_the_first_script_blocks_are_scanned_and_the_rest_are_counted() {
    let payload = abc(&[]);
    let mut tags: Vec<Tag<'_>> = (0..40).map(|_| do_abc2(&payload)).collect();
    tags.push(Tag::ShowFrame);
    tags.push(Tag::End);
    let data = bytes(&header(100.0, 100.0, 12.0), &tags);
    let imported = import_swf_bytes(&data).expect("import");
    let reports = script_reports(&imported);
    assert_eq!(reports.len(), 1, "{:?}", imported.diagnostics);
    assert!(
        reports[0].contains("8 further script blocks were not scanned"),
        "{reports:?}"
    );
}

#[test]
fn a_movie_without_3d_scripts_gains_no_engine_report() {
    let imported = import_swf_bytes(&moving_square()).expect("import");
    assert!(
        imported
            .diagnostics
            .iter()
            .all(|d| !d.message.contains("engine")),
        "{:?}",
        imported.diagnostics
    );
}
