//! Pen-profile import: the legacy `.abr` table, Krita's `.kpp` and bundles, and the formats
//! that are refused by name. Fixtures are built by hand, byte for byte, so a test fails when
//! the reading is wrong rather than when a foreign tool changes.
#![allow(clippy::expect_used)]

use std::io::{Cursor, Write};
use std::path::Path;

use flare_import::brushes::{BrushProfileError, ProfileKind, import_profile_bytes, profile_kind};

/// A `.abr` header: version, count.
fn abr_header(version: u16, count: u16) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&version.to_be_bytes());
    out.extend_from_slice(&count.to_be_bytes());
    out
}

/// A sampled brush record: the six-byte head, the settings, and a minimal one-pixel tip.
fn sampled_record(name: Option<&str>, spacing: i16) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(&0i32.to_be_bytes());
    body.extend_from_slice(&spacing.to_be_bytes());
    if let Some(name) = name {
        body.extend_from_slice(&(name.chars().count() as u32).to_be_bytes());
        for unit in name.encode_utf16() {
            body.extend_from_slice(&unit.to_be_bytes());
        }
    }
    body.push(1);
    for value in [0i16, 0, 10, 10] {
        body.extend_from_slice(&value.to_be_bytes());
    }
    for value in [0i32, 0, 10, 10] {
        body.extend_from_slice(&value.to_be_bytes());
    }
    body.extend_from_slice(&8i16.to_be_bytes());
    body.push(0);
    body.push(255);
    let mut record = Vec::new();
    record.extend_from_slice(&2i16.to_be_bytes());
    record.extend_from_slice(&(body.len() as i32).to_be_bytes());
    record.extend_from_slice(&body);
    record
}

/// A computed brush record: type one, a size, and opaque data GIMP and Krita also skip.
fn computed_record() -> Vec<u8> {
    let mut record = Vec::new();
    record.extend_from_slice(&1i16.to_be_bytes());
    record.extend_from_slice(&6i32.to_be_bytes());
    record.extend_from_slice(&[0u8; 6]);
    record
}

/// An XML a Krita `.kpp` holds inside its zip, with the few things that map over.
fn krita_xml() -> String {
    r#"<?xml version="1.0"?>
<paintoppreset name="Chalk" paintop="paintbrush">
  <option name="Angle"><value a="15"/></option>
  <option name="Ratio"><value ratio="2.0"/></option>
  <option name="Opacity"><value o="0.8"/></option>
  <option name="Size"><curve>0:0;0.5:0.4;1:1</curve></option>
</paintoppreset>
"#
    .to_owned()
}

/// A zip holding `members`, built the way Krita builds its preset files.
fn zip_of(members: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    for (name, contents) in members {
        zip.start_file(*name, options).expect("member");
        zip.write_all(contents).expect("member bytes");
    }
    zip.finish().expect("zip").into_inner()
}

fn kind_of(extension: &str) -> Option<ProfileKind> {
    profile_kind(Path::new(&format!("brush.{extension}")))
}

#[test]
fn a_version_two_abr_keeps_its_names_in_order() {
    let mut bytes = abr_header(2, 2);
    bytes.extend(sampled_record(Some("Round"), 25));
    bytes.extend(computed_record());
    let set = import_profile_bytes(Path::new("brushes.abr"), &bytes).expect("read");
    assert_eq!(
        set.presets
            .iter()
            .map(|p| p.name.clone())
            .collect::<Vec<_>>(),
        vec!["Round".to_owned(), "Brush 2".to_owned()],
        "the named tip keeps its name; the computed one is a placeholder by its place"
    );
    assert!(set.notes.iter().any(|note| note.contains("computed")));
}

#[test]
fn a_version_one_abr_has_no_names_yet() {
    let mut bytes = abr_header(1, 1);
    bytes.extend(sampled_record(None, 40));
    let set = import_profile_bytes(Path::new("brushes.abr"), &bytes).expect("read");
    assert_eq!(set.presets.len(), 1);
    assert_eq!(set.presets[0].name, "Brush 1");
}

#[test]
fn a_resync_anchor_survives_a_record_larger_than_its_own_fields() {
    // The computed record's body is whatever the size says; the next record must start where
    // the size says, not where the fields run out.
    let mut bytes = abr_header(2, 2);
    bytes.extend(computed_record());
    bytes.extend(sampled_record(Some("After"), 10));
    let set = import_profile_bytes(Path::new("brushes.abr"), &bytes).expect("read");
    assert_eq!(set.presets[1].name, "After");
}

#[test]
fn a_descriptor_abr_is_refused_by_what_it_needs() {
    let bytes = abr_header(6, 31);
    let err = import_profile_bytes(Path::new("brushes.abr"), &bytes).expect_err("refused");
    match err {
        BrushProfileError::Unsupported(message) => {
            assert!(message.contains("6"), "the version is named: {message}");
            assert!(
                message.to_lowercase().contains("pen-profile import"),
                "the feature is named: {message}"
            );
        }
        other => panic!("expected an unsupported-format error, got {other:?}"),
    }
}

#[test]
fn a_truncated_abr_is_refused_not_guessed_at() {
    let mut bytes = abr_header(2, 2);
    bytes.extend(sampled_record(Some("Round"), 25));
    bytes.pop();
    assert!(matches!(
        import_profile_bytes(Path::new("brushes.abr"), &bytes),
        Err(BrushProfileError::Truncated(_))
    ));
}

#[test]
fn a_kpp_zip_gives_its_name_angle_flatness_flow_and_curve() {
    let bytes = zip_of(&[("Chalk.image.xml", krita_xml().into_bytes())]);
    let set = import_profile_bytes(Path::new("chalk.kpp"), &bytes).expect("read");
    assert_eq!(set.presets.len(), 1);
    let preset = &set.presets[0];
    assert_eq!(preset.name, "Chalk");
    assert_eq!(preset.angle, 15.0);
    assert_eq!(preset.flatness, 50.0, "ratio 2 means half the height");
    assert_eq!(preset.flow, 0.8);
    assert_eq!(
        preset.pressure_points,
        // The curve is kept through the width response, which starts at `pressure_min`.
        Some(vec![(0.0, 0.15), (0.5, 0.4), (1.0, 1.0)])
    );
}

#[test]
fn an_unpacked_kpp_is_read_the_same_way() {
    let set = import_profile_bytes(Path::new("chalk.kpp"), krita_xml().as_bytes()).expect("read");
    assert_eq!(set.presets.len(), 1);
    assert_eq!(set.presets[0].name, "Chalk");
}

#[test]
fn a_text_that_is_not_krita_is_not_recognized() {
    assert!(matches!(
        import_profile_bytes(Path::new("chalk.kpp"), b"<html>not a preset</html>"),
        Err(BrushProfileError::NotRecognized(_))
    ));
}

#[test]
fn a_bundle_reads_its_presets_and_names_the_ones_it_leaves() {
    let other = b"<?xml version=\"1.0\"?><not-a-preset/>".to_vec();
    let bytes = zip_of(&[
        ("Chalk.kpp", krita_xml().into_bytes()),
        ("thumbnail.png", vec![0u8; 16]),
        ("Broken.kpp", other),
    ]);
    let set = import_profile_bytes(Path::new("pack.bundle"), &bytes).expect("read");
    assert_eq!(set.presets.len(), 1);
    assert_eq!(set.presets[0].name, "Chalk");
    assert!(
        set.notes.iter().any(|note| note.contains("Broken.kpp")),
        "the broken preset is named: {:?}",
        set.notes
    );
}

#[test]
fn picture_only_formats_are_refused_by_name() {
    for extension in ["gbr", "gih", "brushset", "myb"] {
        let error =
            import_profile_bytes(Path::new(&format!("b.{extension}")), b"").expect_err("refused");
        match error {
            BrushProfileError::Unsupported(message) => {
                assert!(
                    message.contains(extension),
                    "{extension} is named in the refusal: {message}"
                );
            }
            other => panic!("expected an unsupported-format error, got {other:?}"),
        }
    }
}

#[test]
fn an_unknown_extension_is_not_a_pen_profile_at_all() {
    assert!(matches!(
        import_profile_bytes(Path::new("b.txt"), b""),
        Err(BrushProfileError::NotRecognized(_))
    ));
}

#[test]
fn the_kinds_are_named_by_extension() {
    assert_eq!(kind_of("abr"), Some(ProfileKind::Abr));
    assert_eq!(kind_of("KPP"), Some(ProfileKind::Kpp), "case is not theirs");
    assert_eq!(kind_of("bundle"), Some(ProfileKind::Bundle));
    assert_eq!(kind_of("gbr"), None);
    assert_eq!(kind_of("txt"), None);
}
