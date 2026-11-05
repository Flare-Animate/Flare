#![allow(clippy::expect_used)]
//! Moho 13 layout: skeletons inside bone layers, parents that follow their children, loops of
//! parents, and shape fill colours. The documents are written by hand to match the layout of a
//! Moho 13 `Project.mohoproj` (checked against the MIT-licensed godot_moho_importer sample, not
//! copied); no sample file is in the repository.

use flare_moho::limits::MAX_BONES;
use flare_moho::{MohoError, Severity, parse_bytes, to_flare_document};

/// A bone layer holding three bones. `Body` names `CG` (the third bone) as its parent, so a
/// parent comes after its child. `Head` is a child of `Body`.
const RIG: &str = r#"{
  "version": 1038, "major_version": 13,
  "project_data": {"width": 800, "height": 600, "fps": 24.0, "end_frame": 4},
  "layers": [
    {"name": "CharRig", "type": "BoneLayer", "visible": true, "parent_bone": -2,
     "skeleton": {"type": "Skeleton", "binding_mode": 1, "bones": [
       {"name": "Body", "parent": 2, "length": 0.5,
        "anim_pos": {"type": "Vec2", "when": [0, 4], "val": [{"x": 0, "y": 0}, {"x": 1, "y": 0}], "interp": ["Linear", "Linear"]}},
       {"name": "Head", "parent": 0, "length": 0.3},
       {"name": "CG", "parent": -1, "length": 0.2}
     ]}}
  ]
}"#;

fn convert(text: &str) -> flare_moho::Converted {
    let project = parse_bytes(text.as_bytes(), "rig.mohoproj").expect("parse");
    to_flare_document(&project).expect("convert")
}

#[test]
fn bone_layer_skeleton_is_read_with_its_track_paths() {
    let project = parse_bytes(RIG.as_bytes(), "rig.mohoproj").expect("parse");
    let names: Vec<&str> = project.bones.iter().map(|b| b.name.as_str()).collect();
    assert_eq!(names, ["Body", "Head", "CG"]);
    assert_eq!(project.bones[0].parent, Some(2), "a forward parent is kept");
    assert_eq!(project.bones[1].parent, Some(0));
    assert_eq!(project.bones[2].parent, None);
    assert_eq!(project.bones[0].path, "layers[0].skeleton.bones[0]");
    assert!(
        project
            .tracks
            .iter()
            .any(|t| t.path == "layers[0].skeleton.bones[0].anim_pos"),
        "the bone's position track is found at its path"
    );
}

#[test]
fn forward_parents_become_folders_from_the_root_down() {
    let out = convert(RIG);
    let layers = out.document.scenes()[0].timeline().layers();
    let find = |name: &str| {
        layers
            .iter()
            .find(|l| l.name() == name)
            .unwrap_or_else(|| panic!("layer {name}"))
    };
    assert_eq!(find("Body").parent(), Some(find("CG").id()));
    assert_eq!(find("Head").parent(), Some(find("Body").id()));
    assert_eq!(find("CG").parent(), Some(find("Skeleton").id()));
    // The pose of Body is its own track: two keys, so a moving span and a second key span.
    assert_eq!(find("Body pose").spans().len(), 2);
}

#[test]
fn a_loop_of_parents_is_broken_not_followed() {
    let doc = r#"{"skeleton": {"bones": [
        {"name": "a", "parent": 1}, {"name": "b", "parent": 0}, {"name": "c", "parent": 1}]}}"#;
    let project = parse_bytes(doc.as_bytes(), "loop.mohoproj").expect("a loop parses");
    // a and b form a loop: a is cut loose. c leads into the loop and keeps its parent.
    assert_eq!(project.bones[0].parent, None);
    assert_eq!(project.bones[1].parent, Some(0));
    assert_eq!(project.bones[2].parent, Some(1));
    to_flare_document(&project).expect("converts");
}

#[test]
fn a_parent_naming_no_bone_makes_a_root() {
    let doc =
        r#"{"skeleton": {"bones": [{"name": "a", "parent": 7}, {"name": "b", "parent": -1}]}}"#;
    let project = parse_bytes(doc.as_bytes(), "p.mohoproj").expect("parse");
    assert_eq!(project.bones[0].parent, None);
}

#[test]
fn bones_across_bone_layers_are_bounded_together() {
    let bones = |n: usize| {
        (0..n)
            .map(|i| format!(r#"{{"name": "b{i}", "parent": -1}}"#))
            .collect::<Vec<_>>()
            .join(",")
    };
    let half = MAX_BONES / 2 + 1;
    let doc = format!(
        r#"{{"layers": [
          {{"type": "BoneLayer", "name": "x", "skeleton": {{"bones": [{}]}}}},
          {{"type": "BoneLayer", "name": "y", "skeleton": {{"bones": [{}]}}}}]}}"#,
        bones(half),
        bones(half)
    );
    assert!(matches!(
        parse_bytes(doc.as_bytes(), "many.mohoproj"),
        Err(MohoError::Limit { what: "bones", .. })
    ));
}

const FILLED: &str = r#"{
  "version": 1038, "major_version": 13,
  "project_data": {"width": 800, "height": 600, "fps": 24.0, "end_frame": 2},
  "layers": [
    {"name": "Shape", "type": "MeshLayer", "visible": true, "parent_bone": -2,
     "mesh": {"points": [{"position": {"x": 0, "y": 0}}, {"position": {"x": 1, "y": 0}}, {"position": {"x": 0, "y": 1}}],
              "curves": [{"closed": true, "points": [{"point": 0}, {"point": 1}, {"point": 2}]}],
              "shapes": [
                {"type": "Shape", "has_fill": true, "style": {"fill_color": {"type": "Color", "when": [0],
                   "val": [{"r": 1.0, "g": 0.5, "b": 0.0, "a": 1.0}]}}},
                {"type": "Shape", "has_fill": false, "style": {"fill_color": {"type": "Color", "when": [0],
                   "val": [{"r": 0.0, "g": 0.0, "b": 1.0, "a": 1.0}]}}}
              ]}}
  ]
}"#;

#[test]
fn filled_shape_takes_its_fill_colour() {
    let project = parse_bytes(FILLED.as_bytes(), "f.mohoproj").expect("parse");
    let mesh = project.layers[0].mesh.as_ref().expect("mesh");
    assert_eq!(mesh.fill, Some([1.0, 0.5, 0.0, 1.0]));
    assert_eq!(mesh.fills, 1, "an unfilled shape does not count");

    let out = to_flare_document(&project).expect("convert");
    let layers = out.document.scenes()[0].timeline().layers();
    let symbol_id = match layers[0].spans()[0].content() {
        flare_document::Content::Instance(i) => i.symbol(),
        _ => panic!("instance"),
    };
    let symbol = out.document.symbol(symbol_id).expect("symbol");
    let flare_document::Content::Shape(shape) = symbol.timeline().layers()[0].spans()[0].content()
    else {
        panic!("shape")
    };
    assert_eq!(shape.fill().r, 255);
    assert_eq!(shape.fill().g, 128);
    assert_eq!(shape.fill().b, 0);
    assert!(
        !out.notes
            .iter()
            .any(|n| n.message.contains("fill colour not read")),
        "a read fill is not reported as missing"
    );
}

#[test]
fn several_fill_colours_are_reported_as_a_warning() {
    let text = FILLED.replace(r#""has_fill": false"#, r#""has_fill": true"#);
    let project = parse_bytes(text.as_bytes(), "f.mohoproj").expect("parse");
    let mesh = project.layers[0].mesh.as_ref().expect("mesh");
    assert_eq!(mesh.fills, 2);
    let out = to_flare_document(&project).expect("convert");
    assert!(out.notes.iter().any(|n| n.severity == Severity::Warning
        && n.message == "several fill colours; the first is used"));
}

#[test]
fn a_mesh_without_shapes_has_no_fill_and_says_so() {
    let text = FILLED.replace(r#""shapes""#, r#""unused""#);
    let project = parse_bytes(text.as_bytes(), "f.mohoproj").expect("parse");
    assert_eq!(project.layers[0].mesh.as_ref().expect("mesh").fill, None);
    let out = to_flare_document(&project).expect("convert");
    assert!(
        out.notes
            .iter()
            .any(|n| n.message.contains("fill colour not read"))
    );
}
