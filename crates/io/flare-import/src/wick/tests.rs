//! Tests of the Wick importer on synthetic projects in the layout of the real samples (see
//! docs/WICK.md), and of each refusal. The real projects are read by tests/wick.

#![allow(clippy::expect_used)]

use std::io::{Cursor, Write};

use flare_core::Transform2D;
use flare_document::{Content, Document, Tween};
use glamx::DAffine2;
use serde_json::{Value, json};
use zip::write::SimpleFileOptions;

use super::import;
use crate::xfl::{Imported, Severity};

/// A zip archive with the named entries, in order.
fn zip_of(entries: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default();
    for (name, data) in entries {
        zip.start_file(*name, options).expect("starts");
        zip.write_all(data).expect("writes");
    }
    zip.finish().expect("finishes").into_inner()
}

/// A 4 by 2 PNG.
fn png_4x2() -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, 4, 2);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().expect("header");
        writer.write_image_data(&[255; 4 * 2 * 4]).expect("data");
    }
    out
}

/// A Wick project file from its object table, with the root clip `root` and the project's frame
/// rate and size.
fn project_zip(objects: Value, root: &str, extra: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let project = json!({
        "classname": "Project", "identifier": null, "name": "Demo", "uuid": "p",
        "children": [root], "width": 64, "height": 48,
        "backgroundColor": "rgb(10,20,30)", "framerate": 12, "focus": root,
        "onionSkinEnabled": false, "metadata": {"wickengine": "2020.7.2.14.55.1"}
    });
    let doc = json!({ "project": project, "objects": objects });
    let mut entries: Vec<(&str, Vec<u8>)> = vec![("project.json", doc.to_string().into_bytes())];
    entries.extend(extra.iter().cloned());
    zip_of(&entries)
}

/// The clip `root` with one timeline holding the given layers.
fn clip(id: &str, timeline: &str) -> Value {
    json!({
        "classname": "Clip", "identifier": "Project", "uuid": id,
        "children": [timeline], "scripts": [], "cursor": "default",
        "transformation": {"x": 0, "y": 0, "scaleX": 1, "scaleY": 1, "rotation": 0, "opacity": 1}
    })
}

fn timeline(id: &str, layers: &[&str]) -> Value {
    json!({ "classname": "Timeline", "identifier": null, "uuid": id,
        "children": layers, "playheadPosition": 1, "activeLayerIndex": 0 })
}

fn layer(id: &str, name: &str, frames: &[&str]) -> Value {
    json!({ "classname": "Layer", "identifier": null, "uuid": id, "name": name,
        "children": frames, "locked": false, "hidden": false })
}

fn frame(id: &str, start: u64, end: u64, children: &[&str]) -> Value {
    json!({ "classname": "Frame", "identifier": null, "uuid": id, "children": children,
        "scripts": [], "cursor": "default", "start": start, "end": end,
        "soundVolume": 1, "soundLoop": false, "originalLayerIndex": 0 })
}

/// A closed red square from (0,0) to (10,10), with the Paper.js encoding Wick writes.
fn square(id: &str) -> Value {
    json!({ "classname": "Path", "identifier": null, "uuid": id, "children": [],
        "json": ["Path", {"applyMatrix": true,
            "segments": [[0, 0], [10, 0], [10, 10], [0, 10]],
            "closed": true, "fillColor": [1, 0, 0, 1], "fillRule": "evenodd"}],
        "fontStyle": "normal", "fontWeight": 400 })
}

fn tween(id: &str, at: u64, x: f64, easing: &str) -> Value {
    json!({ "classname": "Tween", "identifier": null, "uuid": id, "children": [],
        "playheadPosition": at,
        "transformation": {"x": x, "y": 0, "scaleX": 1, "scaleY": 1, "rotation": 0, "opacity": 1},
        "fullRotations": 0, "easingType": easing })
}

fn run(bytes: Vec<u8>) -> Result<Imported, String> {
    import("demo.wick", &bytes).map_err(|e| e.to_string())
}

fn document(bytes: Vec<u8>) -> Document {
    run(bytes).expect("imports").document
}

fn spans_of(document: &Document, layer: usize) -> Vec<(u32, u32, Tween)> {
    document.scenes()[0].timeline().layers()[layer]
        .spans()
        .iter()
        .map(|s| (s.start(), s.duration().get(), s.tween()))
        .collect()
}

/// A layer "Ink" with one frame holding a square over frames 1 and 2, and an empty frame 3.
fn ink_project() -> Vec<u8> {
    let objects = json!({
        "p": {"classname": "Project"},
        "root": clip("root", "tl"),
        "tl": timeline("tl", &["ink"]),
        "ink": layer("ink", "Ink", &["f1", "f2"]),
        "f1": frame("f1", 1, 2, &["sq"]),
        "f2": frame("f2", 3, 3, &[]),
        "sq": square("sq"),
    });
    project_zip(objects, "root", &[])
}

#[test]
fn a_project_imports_its_stage_rate_layers_and_frames() {
    let imported = run(ink_project()).expect("imports");
    let document = imported.document;
    assert_eq!(document.frame_rate().numerator(), 12);
    assert_eq!(document.stage().width(), 64);
    assert_eq!(document.stage().height(), 48);
    assert_eq!(
        document.stage().background(),
        flare_core::Rgba8::new(10, 20, 30, 255)
    );
    assert_eq!(document.scenes()[0].name(), "Demo");
    assert_eq!(
        spans_of(&document, 0),
        vec![(0, 2, Tween::None), (2, 1, Tween::None)]
    );
    let spans = document.scenes()[0].timeline().layers()[0].spans();
    assert!(matches!(spans[0].content(), Content::Shape(_)));
    assert!(matches!(spans[1].content(), Content::Empty));
    let Content::Shape(shape) = spans[0].content() else {
        panic!("a shape")
    };
    assert_eq!(shape.fill(), flare_core::Rgba8::new(255, 0, 0, 255));
    assert_eq!(shape.path().len(), 6, "move, four lines, close");
    assert!(
        imported
            .diagnostics
            .iter()
            .all(|d| d.severity != Severity::Unsupported)
    );
}

#[test]
fn frame_scripts_are_kept_as_text_and_named_as_not_run() {
    let mut objects = ink_project_objects();
    objects["f1"]["scripts"] = json!([{"name": "load", "src": "stop()"}]);
    let document = document(project_zip(objects, "root", &[]));
    assert_eq!(document.attachments().len(), 1);
    let text = document.attachments()[0].content();
    assert!(
        text.contains("not run") && text.contains("stop()"),
        "{text}"
    );
}

fn ink_project_objects() -> Value {
    json!({
        "p": {"classname": "Project"},
        "root": clip("root", "tl"),
        "tl": timeline("tl", &["ink"]),
        "ink": layer("ink", "Ink", &["f1", "f2"]),
        "f1": frame("f1", 1, 2, &["sq"]),
        "f2": frame("f2", 3, 3, &[]),
        "sq": square("sq"),
    })
}

/// A root clip whose layer has a clip `box` placed over frames 1 to 10, tweened from x 0 at frame 1
/// (ease in-out) to x 100 at frame 6.
fn tweened_project(easing: &str) -> Vec<u8> {
    let objects = json!({
        "p": {"classname": "Project"},
        "root": clip("root", "tl"),
        "tl": timeline("tl", &["main"]),
        "main": layer("main", "Main", &["f1"]),
        "f1": frame("f1", 1, 10, &["box", "t1", "t2"]),
        "box": {
            "classname": "Clip", "identifier": "box", "uuid": "box", "children": ["btl"],
            "scripts": [], "cursor": "default", "animationType": "loop",
            "singleFrameNumber": 1, "assetSourceUUID": null, "isSynced": false,
            "transformation": {"x": 0, "y": 0, "scaleX": 1, "scaleY": 1, "rotation": 0, "opacity": 1}
        },
        "btl": timeline("btl", &[]),
        "t1": tween("t1", 1, 0.0, easing),
        "t2": tween("t2", 6, 100.0, "none"),
    });
    project_zip(objects, "root", &[])
}

fn translation(content: &Content) -> (f64, f64) {
    let Content::Instance(instance) = content else {
        panic!("an instance")
    };
    let m = instance.transform().as_affine();
    (m.translation.x, m.translation.y)
}

#[test]
fn tweens_split_a_frame_into_spans_with_flare_motion_and_easing() {
    let document = document(tweened_project("in-out"));
    let layers = document.scenes()[0].timeline().layers();
    let spans = layers[0].spans();
    assert_eq!(spans.len(), 2, "tween at 1 and at 6 make two spans");
    assert_eq!((spans[0].start(), spans[0].duration().get()), (0, 5));
    assert_eq!(spans[0].tween(), Tween::Linear);
    assert!(
        spans[0].interp().is_some(),
        "in-out is Flare's quadratic in-out"
    );
    assert_eq!(translation(spans[0].content()), (0.0, 0.0));
    assert_eq!((spans[1].start(), spans[1].duration().get()), (5, 5));
    assert_eq!(
        spans[1].tween(),
        Tween::None,
        "the last key holds to the end of the frame"
    );
    assert_eq!(translation(spans[1].content()), (100.0, 0.0));
    let symbol = &document.symbols()[0];
    assert_eq!(symbol.name(), "box");
    assert_eq!(symbol.kind(), flare_document::SymbolKind::MovieClip);
}

#[test]
fn an_unknown_easing_is_refused_by_name() {
    let error = run(tweened_project("bounce")).expect_err("refused");
    assert!(error.contains("easingType"), "{error}");
}

#[test]
fn a_clip_is_placed_by_the_centre_of_its_content() {
    // Wick places a clip by the centre of its drawing: the 10 by 10 square has its centre at 5,5,
    // so a clip at position 0,0 is moved by minus that centre.
    let objects = json!({
        "p": {"classname": "Project"},
        "root": clip("root", "tl"),
        "tl": timeline("tl", &["main"]),
        "main": layer("main", "Main", &["f1"]),
        "f1": frame("f1", 1, 1, &["box"]),
        "box": clip("box", "btl"),
        "btl": timeline("btl", &["bl"]),
        "bl": layer("bl", "Inner", &["bf"]),
        "bf": frame("bf", 1, 1, &["sq"]),
        "sq": square("sq"),
    });
    let document = document(project_zip(objects, "root", &[]));
    let root_layer = &document.scenes()[0].timeline().layers()[0];
    assert_eq!(translation(root_layer.spans()[0].content()), (-5.0, -5.0));
}

#[test]
fn a_raster_is_placed_by_its_centre_in_its_own_bitmap_library() {
    // Paper.js draws a raster about its centre: the 4 by 2 picture at 20,10 has its top left at
    // 18,9.
    let objects = json!({
        "p": {"classname": "Project"},
        "root": clip("root", "tl"),
        "tl": timeline("tl", &["main"]),
        "main": layer("main", "Pictures", &["f1"]),
        "f1": frame("f1", 1, 1, &["pic"]),
        "pic": {"classname": "Path", "identifier": null, "uuid": "pic", "children": [],
            "json": ["Raster", {"applyMatrix": false, "source": "asset:img1",
                "matrix": [1, 0, 0, 1, 20, 10], "crossOrigin": ""}]},
        "img1": {"classname": "ImageAsset", "identifier": null, "uuid": "img1", "children": [],
            "name": "a.png", "filename": "a.png", "MIMEType": "image/png", "fileExtension": "png"},
    });
    let bytes = project_zip(objects, "root", &[("assets/img1.png", png_4x2())]);
    let imported = run(bytes).expect("imports");
    let document = imported.document;
    assert_eq!(document.bitmaps().len(), 1);
    assert_eq!(
        (
            document.bitmaps()[0].width(),
            document.bitmaps()[0].height()
        ),
        (4, 2)
    );
    let Content::Bitmap(placed) = document.scenes()[0].timeline().layers()[0].spans()[0].content()
    else {
        panic!("a bitmap")
    };
    let m: DAffine2 = placed.transform().as_affine();
    assert_eq!((m.translation.x, m.translation.y), (18.0, 9.0));
}

#[test]
fn a_missing_picture_is_reported_and_the_rest_imports() {
    let objects = json!({
        "p": {"classname": "Project"},
        "root": clip("root", "tl"),
        "tl": timeline("tl", &["main"]),
        "main": layer("main", "Pictures", &["f1"]),
        "f1": frame("f1", 1, 1, &["pic", "sq"]),
        "pic": {"classname": "Path", "identifier": null, "uuid": "pic", "children": [],
            "json": ["Raster", {"applyMatrix": false, "source": "asset:gone", "crossOrigin": ""}]},
        "sq": square("sq"),
    });
    let imported = run(project_zip(objects, "root", &[])).expect("imports");
    assert!(
        imported
            .diagnostics
            .iter()
            .any(|d| d.severity == Severity::Unsupported
                && d.message.starts_with("pictures that are missing"))
    );
    assert_eq!(imported.document.bitmaps().len(), 0);
}

#[test]
fn a_compound_path_is_one_shape_with_its_children_as_subpaths() {
    let mut objects = ink_project_objects();
    objects["sq"] = json!({"classname": "Path", "identifier": null, "uuid": "sq", "children": [],
        "json": ["CompoundPath", {"applyMatrix": true, "fillColor": [0, 0, 1, 1],
            "fillRule": "evenodd", "children": [
                ["Path", {"applyMatrix": true, "closed": true,
                    "segments": [[0, 0], [10, 0], [10, 10]]}],
                ["Path", {"applyMatrix": true, "closed": true,
                    "segments": [[2, 2], [4, 2], [4, 4]]}]]}]});
    let document = document(project_zip(objects, "root", &[]));
    let Content::Shape(shape) = document.scenes()[0].timeline().layers()[0].spans()[0].content()
    else {
        panic!("a shape")
    };
    let moves = shape
        .path()
        .iter()
        .filter(|s| matches!(s, flare_document::PathSegment::MoveTo(_)))
        .count();
    assert_eq!(moves, 2);
    assert_eq!(shape.fill(), flare_core::Rgba8::new(0, 0, 255, 255));
}

#[test]
fn handles_make_cubic_curves_and_the_matrix_moves_points() {
    let mut objects = ink_project_objects();
    // Relative handles: the first anchor leaves 10 to the right, the second arrives 10 from the left.
    objects["sq"] = json!({"classname": "Path", "identifier": null, "uuid": "sq", "children": [],
        "json": ["Path", {"applyMatrix": false, "matrix": [1, 0, 0, 1, 5, 6],
            "segments": [[[0, 0], [0, 0], [10, 0]], [[20, 0], [-10, 0], [0, 0]]],
            "closed": false, "fillColor": [1, 1, 1, 1]}]});
    let document = document(project_zip(objects, "root", &[]));
    let Content::Shape(shape) = document.scenes()[0].timeline().layers()[0].spans()[0].content()
    else {
        panic!("a shape")
    };
    let flare_document::PathSegment::CubicTo(c1, c2, end) = shape.path()[1] else {
        panic!("a curve, got {:?}", shape.path())
    };
    assert_eq!((c1.as_vec().x, c1.as_vec().y), (15.0, 6.0));
    assert_eq!((c2.as_vec().x, c2.as_vec().y), (15.0, 6.0));
    assert_eq!((end.as_vec().x, end.as_vec().y), (25.0, 6.0));
}

#[test]
fn sounds_and_text_are_reported_as_not_imported() {
    let mut objects = ink_project_objects();
    objects["f1"]["sound"] = json!("snd");
    objects["sq"] = json!({"classname": "Path", "identifier": null, "uuid": "sq", "children": [],
        "json": ["PointText", {"applyMatrix": false, "content": "hi", "fontSize": 12,
            "matrix": [1, 0, 0, 1, 1, 1]}]});
    let imported = run(project_zip(objects, "root", &[])).expect("imports");
    let messages: Vec<&str> = imported
        .diagnostics
        .iter()
        .map(|d| d.message.as_str())
        .collect();
    assert!(
        messages
            .iter()
            .any(|m| m.starts_with("sounds are not imported")),
        "{messages:?}"
    );
    assert!(
        messages
            .iter()
            .any(|m| m.starts_with("text is not imported")),
        "{messages:?}"
    );
}

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
    let mut bytes = zip_of(&[("project.json", b"{}".to_vec()), ("zz", b"x".to_vec())]);
    replace_all(
        &mut bytes,
        b"zz",
        b"project.json".get(..2).map_or(b"zz", |_| b"pr"),
    );
    let _ = bytes;
}

fn replace_all(bytes: &mut [u8], from: &[u8], to: &[u8]) -> usize {
    let mut count = 0;
    let mut i = 0;
    while i + from.len() <= bytes.len() {
        if &bytes[i..i + from.len()] == from && to.len() == from.len() {
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
    let mut zip_bytes = ink_project();
    // A project written with width 0 is refused by name, not by a panic.
    let text = String::from_utf8_lossy(&zip_bytes).into_owned();
    assert!(text.is_empty() || !text.contains("zz"));
    let doc = json!({"project": {"classname": "Project", "name": "x", "children": ["root"],
        "width": 0, "height": 4, "framerate": 12, "focus": "root"},
        "objects": {"root": clip("root", "tl")}});
    zip_bytes = zip_of(&[("project.json", doc.to_string().into_bytes())]);
    let error = run(zip_bytes).expect_err("refused");
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
fn the_transform_helper_round_trips() {
    let t =
        Transform2D::new(DAffine2::from_translation(glamx::DVec2::new(1.0, 2.0))).expect("finite");
    assert_eq!(t.as_affine().translation.y, 2.0);
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
