//! Refusals and diagnostics of the Wick importer: each file that is named as wrong, and each thing
//! that is reported rather than imported, on synthetic projects (see `tests.rs` for the layout).

#![allow(clippy::expect_used)]

use serde_json::json;

use super::tests::{
    clip, frame, ink_project, ink_project_objects, layer, project_zip, run, timeline, zip_of,
};

#[test]
fn a_gzip_file_and_a_plain_json_project_are_named_and_refused() {
    let gzip = run(vec![0x1f, 0x8b, 8, 0, 0]).expect_err("refused");
    assert!(gzip.contains("gzip"), "{gzip}");
    let legacy = run(br#"  {"children": []}"#.to_vec()).expect_err("refused");
    assert!(legacy.contains("plain JSON"), "{legacy}");
}

#[test]
fn a_file_that_is_not_a_zip_or_has_no_project_is_refused() {
    assert!(
        run(b"not a zip archive at all".to_vec())
            .expect_err("refused")
            .contains("not a zip")
    );
    let empty = zip_of(&[("readme.txt", b"x".to_vec())]);
    assert!(run(empty).expect_err("refused").contains("no project.json"));
    let not_json = zip_of(&[("project.json", b"{oops".to_vec())]);
    assert!(
        run(not_json)
            .expect_err("refused")
            .contains("not valid JSON")
    );
    let no_project = zip_of(&[("project.json", br#"{"objects": {}}"#.to_vec())]);
    assert!(
        run(no_project)
            .expect_err("refused")
            .contains("no project object")
    );
}

#[test]
fn an_entry_name_that_would_leave_the_project_is_refused() {
    for name in ["../evil.png", "assets/../../x", "/abs", "a\\b", "assets//x"] {
        let bytes = zip_of(&[("project.json", b"{}".to_vec()), (name, b"x".to_vec())]);
        let error = run(bytes).expect_err("refused");
        assert!(error.contains("would leave the project"), "{name}: {error}");
    }
}

#[test]
fn a_repeated_entry_is_refused() {
    // The zip writer will not repeat a name, so a second 12-character name is renamed in the bytes.
    let mut bytes = zip_of(&[
        ("project.json", b"{}".to_vec()),
        ("abcdefghijkl", b"x".to_vec()),
    ]);
    assert!(replace_all(&mut bytes, b"abcdefghijkl", b"project.json") == 2);
    let error = run(bytes).expect_err("refused");
    assert!(error.contains("listed twice"), "{error}");
}

fn replace_all(bytes: &mut [u8], from: &[u8], to: &[u8]) -> usize {
    let mut count = 0;
    let mut i = 0;
    while i + from.len() <= bytes.len() {
        if &bytes[i..i + from.len()] == from {
            bytes[i..i + to.len()].copy_from_slice(to);
            count += 1;
            i += from.len();
        } else {
            i += 1;
        }
    }
    count
}

#[test]
fn a_bad_frame_range_is_refused_by_field() {
    let mut objects = ink_project_objects();
    objects["f1"]["end"] = json!(0);
    let error = run(project_zip(objects, "root", &[])).expect_err("refused");
    assert!(error.contains("start") || error.contains("end"), "{error}");
}

#[test]
fn overlapping_frames_in_a_layer_are_refused() {
    let mut objects = ink_project_objects();
    objects["f2"]["start"] = json!(2);
    objects["f2"]["end"] = json!(2);
    let error = run(project_zip(objects, "root", &[])).expect_err("refused");
    assert!(error.contains("overlap"), "{error}");
}

#[test]
fn an_unknown_animation_type_is_refused() {
    let mut objects = ink_project_objects();
    objects["f1"]["children"] = json!(["box"]);
    objects["box"] = clip("box", "btl");
    objects["box"]["animationType"] = json!("bounce");
    objects["btl"] = timeline("btl", &[]);
    let error = run(project_zip(objects, "root", &[])).expect_err("refused");
    assert!(error.contains("animationType"), "{error}");
}

#[test]
fn a_clip_that_contains_itself_is_refused() {
    let objects = json!({
        "p": {"classname": "Project"},
        "root": clip("root", "tl"),
        "tl": timeline("tl", &["main"]),
        "main": layer("main", "Main", &["f1"]),
        "f1": frame("f1", 1, 1, &["box"]),
        "box": clip("box", "btl"),
        "btl": timeline("btl", &["bl"]),
        "bl": layer("bl", "Inner", &["bf"]),
        "bf": frame("bf", 1, 1, &["box"]),
    });
    let error = run(project_zip(objects, "root", &[])).expect_err("refused");
    assert!(error.contains("contains itself"), "{error}");
}

#[test]
fn a_path_segment_that_is_not_a_point_is_refused() {
    let mut objects = ink_project_objects();
    objects["sq"]["json"] = json!(["Path", {"applyMatrix": true, "closed": false,
        "segments": [[1, 2, 3]], "fillColor": [1, 1, 1, 1]}]);
    let error = run(project_zip(objects, "root", &[])).expect_err("refused");
    assert!(error.contains("point"), "{error}");
}

#[test]
fn a_project_without_a_clip_to_start_from_is_refused() {
    let mut objects = ink_project_objects();
    objects["root"]["classname"] = json!("Path");
    let error = run(project_zip(objects, "root", &[])).expect_err("refused");
    assert!(
        error.contains("root clip") || error.contains("Path"),
        "{error}"
    );
}

#[test]
fn a_zero_width_stage_is_refused_by_field() {
    let doc = json!({"project": {"classname": "Project", "name": "x", "children": ["root"],
        "width": 0, "height": 4, "framerate": 12, "focus": "root"},
        "objects": {"root": clip("root", "tl")}});
    let error =
        run(zip_of(&[("project.json", doc.to_string().into_bytes())])).expect_err("refused");
    assert!(error.contains("width"), "{error}");
}

#[test]
fn a_frame_rate_outside_the_bounds_is_refused_by_field() {
    let mut objects = ink_project_objects();
    objects["p"] = json!({"classname": "Project"});
    let doc = json!({"project": {"classname": "Project", "name": "x", "children": ["root"],
        "width": 8, "height": 8, "framerate": 500, "focus": "root"}, "objects": objects});
    let error =
        run(zip_of(&[("project.json", doc.to_string().into_bytes())])).expect_err("refused");
    assert!(error.contains("framerate"), "{error}");
}

#[test]
fn names_and_magic_pick_the_wick_reader() {
    use std::path::Path;
    assert_eq!(crate::foreign_kind(Path::new("a.wick")), Some("wick"));
    assert_eq!(crate::foreign_kind(Path::new("A.WICK")), Some("wick"));
    assert_eq!(
        crate::classify(Path::new("a.wick"), false),
        Some(crate::PathKind::Import)
    );
    assert!(crate::looks_like_wick(&ink_project()));
    assert!(!crate::looks_like_wick(&zip_of(&[(
        "DOMDocument.xml",
        b"<x/>".to_vec()
    )])));
    assert!(!crate::looks_like_wick(b"plain"));
    let through_bytes = crate::import_bytes(ink_project()).expect("imports by magic");
    assert_eq!(through_bytes.document.scenes()[0].name(), "Demo");
}

#[test]
fn opacity_and_fill_rules_are_reported_not_dropped() {
    let mut objects = ink_project_objects();
    objects["root"]["children"] = json!(["tl"]);
    objects["f1"]["children"] = json!(["sq", "box"]);
    objects["box"] = clip("box", "btl");
    objects["box"]["transformation"]["opacity"] = json!(0.5);
    objects["btl"] = timeline("btl", &[]);
    let imported = run(project_zip(objects, "root", &[])).expect("imports");
    let messages: Vec<&str> = imported
        .diagnostics
        .iter()
        .map(|d| d.message.as_str())
        .collect();
    assert!(
        messages.iter().any(|m| m.starts_with("opacity of clips")),
        "{messages:?}"
    );
    assert!(
        messages
            .iter()
            .any(|m| m.starts_with("fill rules are not carried")),
        "{messages:?}"
    );
}
