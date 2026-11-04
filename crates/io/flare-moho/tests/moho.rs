#![allow(clippy::expect_used)]
//! Golden and failure-path tests for the Moho reader, on a small synthetic project written by
//! hand for these tests (tests/fixtures/tiny.mohoproj.json). No downloaded samples.

use std::io::{Cursor, Write};

use flare_moho::limits::{MAX_DOCUMENT_BYTES, MAX_FILE_BYTES, MAX_LAYER_DEPTH};
use flare_moho::{
    Binding, Census, Container, KeyValue, LayerKind, MohoError, detect, is_moho_path, parse_bytes,
    to_flare_document,
};

const TINY: &str = include_str!("fixtures/tiny.mohoproj.json");

fn zipped(name: &str, document: &[u8]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    writer.start_file(name, options).expect("start file");
    writer.write_all(document).expect("write document");
    writer.finish().expect("finish").into_inner()
}

#[test]
fn raw_document_reads_header_layers_bones_and_tracks() {
    let project = parse_bytes(TINY.as_bytes(), "tiny.mohoproj").expect("parse");
    assert_eq!(project.entry, "tiny.mohoproj");
    assert_eq!(project.version, 1045);
    assert_eq!(project.major_version, 13);
    assert_eq!((project.width, project.height), (640, 480));
    assert_eq!(project.fps, 24.0);
    assert_eq!(project.end_frame, 48);

    assert_eq!(project.bones.len(), 2);
    assert_eq!(project.bones[0].parent, None);
    assert_eq!(project.bones[1].parent, Some(0));
    assert_eq!(project.bones[1].length, 2.5);
    assert!(project.bones[1].dynamic);

    let kinds: Vec<&LayerKind> = project.layers.iter().map(|l| &l.kind).collect();
    assert_eq!(
        kinds,
        [
            &LayerKind::Group,
            &LayerKind::Image,
            &LayerKind::Switch,
            &LayerKind::Other("AudioLayer".to_owned())
        ]
    );
    let shape = &project.layers[0].children[0];
    assert_eq!(shape.kind, LayerKind::Vector);
    assert_eq!(shape.depth, 1);
    assert_eq!(shape.binding, Binding::Bone(1));
    assert_eq!(shape.texture_path.as_deref(), Some("images/shape.png"));
    assert!(!project.layers[1].visible);
    assert_eq!(project.layers[1].binding, Binding::Region);
}

#[test]
fn switch_layer_reports_rest_and_end_cells() {
    let project = parse_bytes(TINY.as_bytes(), "tiny.mohoproj").expect("parse");
    let eyes = project.layers[2].switch.as_ref().expect("switch cells");
    assert_eq!(eyes.alternatives, ["open", "closed"]);
    assert_eq!(eyes.at_rest.as_deref(), Some("open"));
    assert_eq!(eyes.at_end.as_deref(), Some("closed"));
    assert_eq!(eyes.keys.keys.len(), 2);
}

#[test]
fn tracks_keep_their_path_keys_and_interpolation() {
    let project = parse_bytes(TINY.as_bytes(), "tiny.mohoproj").expect("parse");
    assert_eq!(project.tracks.len(), 2);
    let translation = project
        .tracks
        .iter()
        .find(|t| t.path == "layers[0].layers[0].transforms.translation")
        .expect("translation track");
    assert_eq!(translation.channel.keys.len(), 2);
    assert_eq!(translation.channel.keys[1].frame, 24);
    assert_eq!(
        translation.channel.keys[1].value,
        KeyValue::Vector(vec![10.0, 5.0])
    );
    assert_eq!(translation.channel.interp, ["Smooth", "Linear"]);
}

#[test]
fn references_are_relative_sorted_and_deduplicated() {
    let project = parse_bytes(TINY.as_bytes(), "tiny.mohoproj").expect("parse");
    assert_eq!(
        project.images,
        ["images/open.png", "images/photo.jpg", "images/shape.png"]
    );
    assert_eq!(project.audio, ["audio/line.wav"]);
}

#[test]
fn census_counts_what_the_project_contains() {
    let project = parse_bytes(TINY.as_bytes(), "tiny.mohoproj").expect("parse");
    let census = project.census();
    let mut by_kind = std::collections::BTreeMap::new();
    by_kind.insert("group".to_owned(), 1);
    by_kind.insert("vector".to_owned(), 1);
    by_kind.insert("image".to_owned(), 3);
    by_kind.insert("switch".to_owned(), 1);
    by_kind.insert("AudioLayer".to_owned(), 1);
    assert_eq!(
        census,
        Census {
            layers: 7,
            layers_by_kind: by_kind,
            bones: 2,
            switches: 1,
            tracks: 2,
            keyframes: 4,
            images: 3,
            audio: 1,
        }
    );
}

#[test]
fn zipped_container_gives_the_same_project() {
    let bytes = zipped("Project.mohoproj", TINY.as_bytes());
    assert!(matches!(detect(&bytes), Ok(Container::Zip)));
    let zipped_project = parse_bytes(&bytes, "x.moho").expect("parse zip");
    let raw_project = parse_bytes(TINY.as_bytes(), "x.mohoproj").expect("parse raw");
    assert_eq!(zipped_project.entry, "Project.mohoproj");
    assert_eq!(zipped_project.layers, raw_project.layers);
    assert_eq!(zipped_project.tracks, raw_project.tracks);
}

#[test]
fn animeproj_member_and_byte_order_mark_are_accepted() {
    let bytes = zipped("Project.animeproj", TINY.as_bytes());
    assert_eq!(
        parse_bytes(&bytes, "x.anime").expect("parse").entry,
        "Project.animeproj"
    );
    let mut with_bom = vec![0xEF, 0xBB, 0xBF];
    with_bom.extend_from_slice(TINY.as_bytes());
    assert!(matches!(detect(&with_bom), Ok(Container::RawJson)));
    assert!(parse_bytes(&with_bom, "x.mohoproj").is_ok());
}

#[test]
fn zip_without_a_document_is_refused() {
    let bytes = zipped("readme.txt", b"no project here");
    assert!(matches!(
        parse_bytes(&bytes, "x.moho"),
        Err(MohoError::MissingDocument)
    ));
}

#[test]
fn legacy_anme_is_named_not_misread() {
    let legacy = b"Anime Studio Project 10\n{\"layers\": []}";
    assert!(matches!(
        parse_bytes(legacy, "old.anme"),
        Err(MohoError::LegacyFormat)
    ));
}

#[test]
fn binary_files_are_not_moho() {
    let swf = b"FWS\x09\x00\x00\x00\x00binary body";
    assert!(matches!(parse_bytes(swf, "a.swf"), Err(MohoError::NotMoho)));
    assert!(matches!(parse_bytes(b"", "empty"), Err(MohoError::NotMoho)));
}

#[test]
fn invalid_json_and_non_object_are_refused() {
    assert!(matches!(
        parse_bytes(b"{\"layers\": [", "bad.mohoproj"),
        Err(MohoError::Json(_))
    ));
    assert!(matches!(
        parse_bytes(b"[1, 2]", "list.mohoproj"),
        Err(MohoError::NotMoho)
    ));
}

#[test]
fn bone_parent_must_be_an_earlier_bone() {
    let doc = r#"{"skeleton": {"bones": [
        {"name": "a", "parent": 1}, {"name": "b", "parent": -1}]}}"#;
    assert!(matches!(
        parse_bytes(doc.as_bytes(), "p.mohoproj"),
        Err(MohoError::BadBoneParent {
            index: 0,
            parent: 1
        })
    ));
}

#[test]
fn layer_nesting_is_bounded() {
    let mut doc = String::from("{\"layers\": [");
    for _ in 0..=MAX_LAYER_DEPTH {
        doc.push_str("{\"name\": \"g\", \"type\": \"GroupLayer\", \"layers\": [");
    }
    doc.push_str(&"]}".repeat(MAX_LAYER_DEPTH + 1));
    doc.push_str("]}");
    assert!(matches!(
        parse_bytes(doc.as_bytes(), "deep.mohoproj"),
        Err(MohoError::Limit {
            what: "layer nesting",
            ..
        })
    ));
}

#[test]
fn oversize_input_is_refused_before_parsing() {
    let big = vec![b' '; usize::try_from(MAX_FILE_BYTES).unwrap_or(usize::MAX) + 1];
    assert!(matches!(
        parse_bytes(&big, "big.mohoproj"),
        Err(MohoError::TooLarge { .. })
    ));
    let doc_size = usize::try_from(MAX_DOCUMENT_BYTES).unwrap_or(usize::MAX) + 1;
    let mut padded = b"{\"layers\": []}".to_vec();
    padded.resize(
        doc_size.min(usize::try_from(MAX_FILE_BYTES).unwrap_or(0)),
        b' ',
    );
    if padded.len() as u64 > MAX_DOCUMENT_BYTES {
        assert!(matches!(
            parse_bytes(&padded, "x.mohoproj"),
            Err(MohoError::TooLarge { .. })
        ));
    }
}

#[test]
fn every_truncation_of_the_fixture_fails_cleanly_or_parses() {
    let bytes = TINY.as_bytes();
    for end in 0..bytes.len() {
        let _ = parse_bytes(&bytes[..end], "cut.mohoproj");
    }
}

#[test]
fn conversion_to_a_flare_document_is_a_named_gap() {
    let project = parse_bytes(TINY.as_bytes(), "tiny.mohoproj").expect("parse");
    assert!(matches!(
        to_flare_document(&project),
        Err(MohoError::NotYetImplemented { .. })
    ));
}

#[test]
fn moho_extensions_are_recognised_case_insensitively() {
    use std::path::Path;
    assert!(is_moho_path(Path::new("a/Scene.MOHO")));
    assert!(is_moho_path(Path::new("x.mohoproj")));
    assert!(!is_moho_path(Path::new("x.fla")));
}
