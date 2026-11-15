//! Pen-profile import through the MCP server: a Photoshop `.abr` in the export directory read
//! into Flare's brush presets. The fixture is built by hand, byte for byte.
#![allow(clippy::expect_used)]

mod common;

use common::fixture;
use serde_json::json;

/// A `.abr` header: version, count.
fn abr_header(version: u16, count: u16) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&version.to_be_bytes());
    out.extend_from_slice(&count.to_be_bytes());
    out
}

/// A sampled brush record: the six-byte head, its name and settings, and a minimal tip.
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

fn one_brush_abr() -> Vec<u8> {
    let mut bytes = abr_header(2, 1);
    bytes.extend(sampled_record(Some("Round"), 25));
    bytes
}

#[tokio::test]
async fn a_profile_in_the_export_directory_is_read_into_presets() {
    let f = fixture(true, true).await;
    std::fs::create_dir_all(f.dir.path().join("exports")).expect("exports");
    std::fs::write(
        f.dir.path().join("exports").join("brushes.abr"),
        one_brush_abr(),
    )
    .expect("write");
    let out = f
        .ok("import_brush_profiles", json!({ "source": "brushes.abr" }))
        .await;
    let presets = out["presets"].as_array().expect("presets");
    assert_eq!(presets.len(), 1);
    assert_eq!(presets[0]["name"], "Round");
    assert_eq!(out["notes"], json!([]));
}

#[tokio::test]
async fn a_file_that_is_not_a_profile_is_refused_and_one_off_the_directory_too() {
    let f = fixture(true, true).await;
    std::fs::create_dir_all(f.dir.path().join("exports")).expect("exports");
    std::fs::write(
        f.dir.path().join("exports").join("junk.abr"),
        b"not a photoshop brush file",
    )
    .expect("write");
    let (bad, _) = f
        .tool("import_brush_profiles", json!({ "source": "junk.abr" }))
        .await;
    assert!(bad);

    let (bad, _) = f
        .tool("import_brush_profiles", json!({ "source": "gone.abr" }))
        .await;
    assert!(bad);
}

#[tokio::test]
async fn a_profile_is_refused_when_exports_are_off() {
    let f = fixture(false, false).await;
    let (bad, _) = f
        .tool("import_brush_profiles", json!({ "source": "brushes.abr" }))
        .await;
    assert!(bad);
}

#[tokio::test]
async fn the_presets_are_flare_preset_objects_that_round_trip() {
    let f = fixture(true, true).await;
    std::fs::create_dir_all(f.dir.path().join("exports")).expect("exports");
    std::fs::write(
        f.dir.path().join("exports").join("brushes.abr"),
        one_brush_abr(),
    )
    .expect("write");
    let out = f
        .ok("import_brush_profiles", json!({ "source": "brushes.abr" }))
        .await;
    let preset = &out["presets"][0];
    // Every preset the library makes is a full brush, so a caller can keep what comes back.
    assert!(
        ["name", "square", "angle", "flatness"]
            .iter()
            .all(|key| preset.get(*key).is_some()),
        "{preset}"
    );
    assert_eq!(preset["name"], "Round");
}
