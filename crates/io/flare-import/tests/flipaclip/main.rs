#![allow(clippy::expect_used)]
//! FlipaClip projects: the reader's behaviour on synthetic archives in the sample layout, its
//! refusals, and the wiring that sends a `.fc` file to it.

mod support;

use std::path::Path;

use flare_document::{Content, Document};
use flare_import::{
    ImportError, Imported, PathKind, Severity, classify, foreign_kind, import_foreign_bytes,
    import_path,
};
use support::{HEIGHT, WIDTH, drawn, meta, project, solid, zip_of};

const RED: [u8; 4] = [255, 0, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];
const GREEN: [u8; 4] = [0, 255, 0, 255];
const WHITE: [u8; 4] = [255, 255, 255, 255];

fn import(bytes: Vec<u8>) -> Imported {
    import_foreign_bytes(Path::new("project.fc"), bytes).expect("imports")
}

/// The refusal's message; a project that imports is a failed test.
fn refusal(bytes: Vec<u8>) -> String {
    match import_foreign_bytes(Path::new("project.fc"), bytes) {
        Err(ImportError::Foreign(message)) => message,
        Err(other) => panic!("expected a foreign refusal, got {other:?}"),
        Ok(_) => panic!("expected a refusal"),
    }
}

fn layer_names(document: &Document) -> Vec<String> {
    document.scenes()[0]
        .timeline()
        .layers()
        .iter()
        .map(|l| l.name().to_owned())
        .collect()
}

fn starts(document: &Document, layer: &str) -> Vec<u32> {
    document.scenes()[0]
        .timeline()
        .layers()
        .iter()
        .find(|l| l.name() == layer)
        .expect("layer")
        .spans()
        .iter()
        .map(|s| s.start())
        .collect()
}

#[test]
fn names_pick_the_flipaclip_reader() {
    assert_eq!(foreign_kind(Path::new("a.fc")), Some("flipaclip"));
    assert_eq!(foreign_kind(Path::new("A.FC")), Some("flipaclip"));
    assert_eq!(classify(Path::new("a.fc"), false), Some(PathKind::Import));
    assert_eq!(foreign_kind(Path::new("a.fla")), None);
}

#[test]
fn a_project_imports_its_layers_frames_and_background() {
    let bytes = project(
        meta(&["Capa 1", "Capa 2"], 3),
        Some(solid(WHITE)),
        &[
            ("0/0", solid(RED)),
            ("0/2", solid(BLUE)),
            ("1/1", solid(GREEN)),
        ],
    );
    let out = import(bytes);
    let document = &out.document;
    assert_eq!(document.stage().width(), WIDTH);
    assert_eq!(document.stage().height(), HEIGHT);
    assert_eq!(document.frame_rate().numerator(), 12);
    // FlipaClip lists the bottom layer first; the timeline lists the top layer first.
    assert_eq!(layer_names(document), ["Capa 2", "Capa 1", "Background"]);
    assert_eq!(starts(document, "Capa 1"), [0, 2], "a gap at frame 1");
    assert_eq!(starts(document, "Capa 2"), [1]);
    assert_eq!(starts(document, "Background"), [0]);
    assert_eq!(
        document.bitmaps().len(),
        4,
        "red, blue, green and the background"
    );
    assert!(
        out.diagnostics
            .iter()
            .any(|d| d.severity == Severity::Info && d.message.contains("format 1")),
        "the import says what it read"
    );
}

#[test]
fn frames_that_draw_nothing_leave_gaps() {
    let blank = drawn(RED, |_, _| false);
    let bytes = project(
        meta(&["Capa 1"], 2),
        None,
        &[("0/0", blank), ("0/1", solid(RED))],
    );
    let document = import(bytes).document;
    assert_eq!(starts(&document, "Capa 1"), [1]);
    assert_eq!(document.bitmaps().len(), 1, "a blank frame keeps no bitmap");
}

#[test]
fn identical_frames_share_one_bitmap_and_drawn_parts_are_cropped() {
    let bytes = project(
        meta(&["Capa 1"], 2),
        None,
        &[("0/0", solid(RED)), ("0/1", solid(RED))],
    );
    let document = import(bytes).document;
    assert_eq!(
        document.bitmaps().len(),
        1,
        "two identical frames store one bitmap"
    );

    // A small region at (2, 1) that is 3 wide and 2 high, premultiplied: 64 of 128 alpha.
    let region = [64_u8, 0, 0, 128];
    let bytes = project(
        meta(&["Capa 1"], 1),
        None,
        &[(
            "0/0",
            drawn(region, |x, y| (2..5).contains(&x) && (1..3).contains(&y)),
        )],
    );
    let document = import(bytes).document;
    let bitmap = &document.bitmaps()[0];
    assert_eq!(
        (bitmap.width(), bitmap.height()),
        (3, 2),
        "cropped to what is drawn"
    );
    assert_eq!(&bitmap.pixels()[..4], &[128, 0, 0, 128], "un-premultiplied");
    let Content::Bitmap(instance) =
        document.scenes()[0].timeline().layers()[0].spans()[0].content()
    else {
        panic!("a bitmap span");
    };
    let offset = instance.transform().as_affine().translation;
    assert_eq!(
        (offset.x, offset.y),
        (2.0, 1.0),
        "placed at the crop's corner"
    );
}

#[test]
fn other_entries_are_named_as_not_imported() {
    let bytes = project(
        meta(&["Capa 1"], 1),
        None,
        &[("0/0", solid(RED)), ("audio/track.m4a", vec![1, 2, 3])],
    );
    let out = import(bytes);
    assert!(
        out.diagnostics
            .iter()
            .any(|d| d.severity == Severity::Unsupported && d.message.contains("audio/track.m4a"))
    );
}

#[test]
fn a_fc_file_imports_through_the_path_entry_point() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("sample.fc");
    std::fs::write(
        &path,
        project(meta(&["Capa 1"], 1), None, &[("0/0", solid(RED))]),
    )
    .expect("writes");
    let out = import_path(&path).expect("imports");
    assert_eq!(out.document.frame_rate().numerator(), 12);
}

#[test]
fn a_corrupt_archive_is_refused() {
    assert!(refusal(b"not a zip archive at all".to_vec()).contains("not a zip archive"));
}

#[test]
fn an_entry_name_that_would_leave_the_project_is_refused() {
    for name in [
        "../evil",
        "a/../../evil",
        "/abs",
        "a\\b",
        "C:evil",
        "a//b",
        "./x",
    ] {
        let bytes = zip_of(&[("meta", meta(&["Capa 1"], 1)), (name, solid(RED))]);
        assert!(
            refusal(bytes).contains("would leave the project"),
            "{name} is refused"
        );
    }
}

#[test]
fn a_repeated_entry_is_refused() {
    // The zip writer will not repeat a name, so the second name is renamed in the bytes: the
    // local header and the central directory both change, and the archive lists `meta` twice.
    let mut bytes = zip_of(&[
        ("meta", meta(&["Capa 1"], 1)),
        ("metb", meta(&["Capa 1"], 1)),
    ]);
    let renamed = replace_all(&mut bytes, b"metb", b"meta");
    assert_eq!(renamed, 2, "the local header and the central directory");
    assert!(refusal(bytes).contains("listed twice"));
}

fn replace_all(bytes: &mut [u8], from: &[u8], to: &[u8]) -> usize {
    let mut count = 0;
    for start in 0..=bytes.len().saturating_sub(from.len()) {
        if &bytes[start..start + from.len()] == from {
            bytes[start..start + to.len()].copy_from_slice(to);
            count += 1;
        }
    }
    count
}

#[test]
fn a_project_without_meta_is_refused() {
    let bytes = zip_of(&[("0/0", solid(RED))]);
    assert!(refusal(bytes).contains("no meta entry"));
}

#[test]
fn an_unknown_format_version_is_refused() {
    let text = String::from_utf8(meta(&["Capa 1"], 1)).expect("utf-8");
    let text = text.replace("\"format\":1", "\"format\":2");
    let bytes = project(text.into_bytes(), None, &[("0/0", solid(RED))]);
    assert!(refusal(bytes).contains("format 2 is not read"));
}

#[test]
fn meta_values_out_of_range_are_refused() {
    let cases = [
        ("\"canvasWidth\":8", "\"canvasWidth\":9000", "canvasWidth"),
        ("\"fps\":12", "\"fps\":0", "fps"),
        ("\"frameCount\":1", "\"frameCount\":0", "frameCount"),
        ("\"frameCount\":1", "\"frameCount\":99999", "frameCount"),
    ];
    for (from, to, field) in cases {
        let text = String::from_utf8(meta(&["Capa 1"], 1))
            .expect("utf-8")
            .replace(from, to);
        let bytes = project(text.into_bytes(), None, &[("0/0", solid(RED))]);
        assert!(refusal(bytes).contains(field), "{field} is refused");
    }
    let not_json = project(b"[1,2]".to_vec(), None, &[]);
    assert!(refusal(not_json).contains("meta is not a JSON object"));
}

#[test]
fn a_layers_list_that_is_not_a_string_is_refused() {
    let text = serde_json::json!({
        "canvasWidth": WIDTH, "canvasHeight": HEIGHT, "format": 1, "fps": 12,
        "frameCount": 1, "layersState": [{"id": 1, "name": "Capa 1"}],
    });
    let bytes = project(text.to_string().into_bytes(), None, &[]);
    assert!(refusal(bytes).contains("layersState"));
}

#[test]
fn a_frame_that_is_not_a_picture_is_refused() {
    let bytes = project(
        meta(&["Capa 1"], 1),
        None,
        &[("0/0", b"\x89PNG\r\n\x1a\n........".to_vec())],
    );
    assert!(refusal(bytes).contains("not a FlipaClip picture"));
}

#[test]
fn pictures_with_a_wrong_header_are_refused() {
    let mut version = solid(RED);
    version[8] = 2;
    let bytes = project(meta(&["Capa 1"], 1), None, &[("0/0", version)]);
    assert!(refusal(bytes).contains("picture version is not 1"));

    let mut length = solid(RED);
    length[14] ^= 1;
    let bytes = project(meta(&["Capa 1"], 1), None, &[("0/0", length)]);
    assert!(refusal(bytes).contains("length field"));

    let mut size = solid(RED);
    size[17] = 9;
    let bytes = project(meta(&["Capa 1"], 1), None, &[("0/0", size)]);
    assert!(refusal(bytes).contains("size differs from the canvas"));
}

#[test]
fn pictures_of_another_canvas_size_are_refused() {
    let small = support::picture(4, 4, &[0; 64]);
    let bytes = project(meta(&["Capa 1"], 1), None, &[("0/0", small)]);
    assert!(refusal(bytes).contains("size differs from the canvas"));
}

#[test]
fn a_picture_whose_stream_is_corrupt_is_refused() {
    let mut bad = solid(RED)[..24].to_vec();
    bad.extend_from_slice(b"definitely not zstd");
    let length = bad.len() as u32;
    bad[12..15].copy_from_slice(&length.to_be_bytes()[1..]);
    let bytes = project(meta(&["Capa 1"], 1), None, &[("0/0", bad)]);
    assert!(refusal(bytes).contains("the zstd stream is corrupt"));
}

#[test]
fn a_picture_that_inflates_past_its_canvas_is_refused() {
    let too_much = support::picture(WIDTH, HEIGHT, &vec![0; (WIDTH * HEIGHT * 4 + 8) as usize]);
    let bytes = project(meta(&["Capa 1"], 1), None, &[("0/0", too_much)]);
    assert!(refusal(bytes).contains("decode to the wrong size"));
}

#[test]
fn frames_and_layers_outside_meta_are_refused() {
    let bytes = project(meta(&["Capa 1"], 2), None, &[("0/2", solid(RED))]);
    assert!(refusal(bytes).contains("past frameCount"));

    let bytes = project(meta(&["Capa 1"], 2), None, &[("1/0", solid(RED))]);
    assert!(refusal(bytes).contains("the layer is not in meta"));

    let bytes = project(
        meta(&["Capa 1"], 2),
        None,
        &[("0/1", solid(RED)), ("00/1", solid(BLUE))],
    );
    assert!(refusal(bytes).contains("listed twice"));
}

#[test]
fn too_many_entries_are_refused_before_any_is_read() {
    let names: Vec<String> = (0..4_097).map(|i| format!("9/{i}")).collect();
    let mut files: Vec<(&str, Vec<u8>)> = vec![("meta", meta(&["Capa 1"], 1))];
    files.extend(names.iter().map(|n| (n.as_str(), vec![1])));
    assert!(refusal(zip_of(&files)).contains("entry count"));
}

#[test]
fn an_entry_over_the_size_limit_is_refused() {
    let big = vec![0_u8; 16 * 1024 * 1024 + 1];
    let bytes = project(meta(&["Capa 1"], 1), None, &[("0/0", big)]);
    assert!(refusal(bytes).contains("over the limit"));
}

/// Imports a real FlipaClip project named by `FLIPACLIP_SAMPLE`. Run by hand with
/// `cargo test -p flare-import --test flipaclip -- --ignored --nocapture`; the sample is never
/// committed.
#[test]
#[ignore = "needs a real .fc file in FLIPACLIP_SAMPLE"]
fn a_real_sample_imports() {
    let path = std::env::var("FLIPACLIP_SAMPLE").expect("FLIPACLIP_SAMPLE is set");
    let started = std::time::Instant::now();
    let out = flare_import::import_foreign_path(Path::new(&path)).expect("imports");
    eprintln!("imported in {:?}", started.elapsed());
    let document = out.document;
    assert_eq!(
        (document.stage().width(), document.stage().height()),
        (1920, 1080)
    );
    assert_eq!(document.frame_rate().numerator(), 24);
    for diagnostic in &out.diagnostics {
        eprintln!("{:?}: {}", diagnostic.severity, diagnostic.message);
    }
}
