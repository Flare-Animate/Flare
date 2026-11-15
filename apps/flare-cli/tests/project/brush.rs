//! `flare brush-import`: a pen-profile file read and printed, and Flare's portable
//! brush-profile file written. The `.abr` fixture is built by hand, byte for byte.

use super::*;

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

fn write_profile(dir: &Path, name: &str, bytes: &[u8]) -> String {
    let path = dir.join(name).to_string_lossy().into_owned();
    std::fs::write(&path, bytes).expect("write");
    path
}

#[test]
fn a_profile_is_printed_with_its_name_and_nib() {
    let dir = tempfile::tempdir().expect("dir");
    let profile = write_profile(dir.path(), "brushes.abr", &one_brush_abr());
    let out = json(&["brush-import", &profile]);
    let presets = out["presets"].as_array().expect("presets");
    assert_eq!(presets.len(), 1);
    assert_eq!(presets[0]["name"], "Round");
    assert_eq!(out["notes"], serde_json::json!([]));
}

#[test]
fn the_presets_can_be_written_as_a_portable_file() {
    let dir = tempfile::tempdir().expect("dir");
    let profile = write_profile(dir.path(), "brushes.abr", &one_brush_abr());
    let out_file = dir.path().join("out.json").to_string_lossy().into_owned();
    let out = json(&["brush-import", &profile, "--out", &out_file]);
    assert_eq!(out["file"], out_file);
    let written = std::fs::read_to_string(&out_file).expect("read back");
    assert!(written.contains("\"version\": 1"), "{written}");
    assert!(written.contains("\"Round\""), "{written}");
}

#[test]
fn a_file_that_is_not_a_profile_is_refused_and_nothing_is_written() {
    let dir = tempfile::tempdir().expect("dir");
    let profile = write_profile(dir.path(), "junk.abr", b"not a photoshop brush file");
    let out_file = dir.path().join("out.json");
    let output = run(&[
        "brush-import",
        &profile,
        "--out",
        &out_file.to_string_lossy(),
    ]);
    assert!(!output.status.success(), "junk is refused");
    assert!(!out_file.exists(), "nothing was written");
    let wrong_kind = write_profile(dir.path(), "brushes.gbr", &one_brush_abr());
    let output = run(&["brush-import", &wrong_kind]);
    assert!(
        !output.status.success() && String::from_utf8_lossy(&output.stderr).contains("GIMP"),
        "a picture brush is refused by its name: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
