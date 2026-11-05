#![allow(clippy::expect_used)]
//! Conversion of synthetic Moho projects and BVH motion into Flare documents.

use flare_document::{Content, Tween};
use flare_moho::bvh;
use flare_moho::{MohoError, Severity, parse_bytes, to_flare_document};

const TINY: &str = include_str!("fixtures/tiny.mohoproj.json");

const ART: &str = r#"{
  "version": 1045, "major_version": 13,
  "project_data": {"width": 800, "height": 600, "fps": 30.0, "end_frame": 10},
  "skeleton": {"bones": [
    {"name": "root", "parent": -1, "length": 1.0},
    {"name": "arm", "parent": 0, "length": 0.5}
  ]},
  "layers": [
    {"name": "Tri", "type": "MeshLayer", "visible": true,
     "mesh": {
       "points": [{"position": {"x": 0, "y": 0}}, {"position": {"x": 1, "y": 0}}, {"position": {"x": 0, "y": 1}}],
       "curves": [{"closed": true, "num_points": 3, "points": [{"point": 0}, {"point": 1}, {"point": 2}]}]},
     "transforms": {"translation": {"when": [0, 4], "val": [{"x": 0, "y": 0}, {"x": 1, "y": 1}],
                    "interp": ["Linear", "Linear"], "type": "Vec2"}}}
  ]
}"#;

fn convert(text: &str) -> flare_moho::Converted {
    let project = parse_bytes(text.as_bytes(), "t.mohoproj").expect("parse");
    to_flare_document(&project).expect("convert")
}

#[test]
fn vector_layer_becomes_a_shape_symbol_with_a_moving_instance() {
    let out = convert(ART);
    let doc = &out.document;
    assert_eq!((doc.stage().width(), doc.stage().height()), (800, 600));
    assert_eq!(doc.frame_rate().numerator(), 30);
    let layers = doc.scenes()[0].timeline().layers();
    let tri = layers.iter().find(|l| l.name() == "Tri").expect("layer");
    assert_eq!(tri.spans().len(), 2);
    assert_eq!(tri.spans()[0].tween(), Tween::Linear);
    let Content::Instance(i) = tri.spans()[0].content() else {
        panic!("instance")
    };
    let symbol = doc.symbol(i.symbol()).expect("symbol");
    let Content::Shape(shape) = symbol.timeline().layers()[0].spans()[0].content() else {
        panic!("shape")
    };
    assert_eq!(shape.path().len(), 4); // move, two lines, close
}

#[test]
fn bones_become_nested_folders_with_pose_layers() {
    let doc = convert(ART).document;
    let layers = doc.scenes()[0].timeline().layers();
    let folders: Vec<&str> = layers
        .iter()
        .filter(|l| l.is_folder())
        .map(|l| l.name())
        .collect();
    assert_eq!(folders, ["Skeleton", "root", "arm"]);
    let arm = layers.iter().find(|l| l.name() == "arm").expect("arm");
    let root = layers.iter().find(|l| l.name() == "root").expect("root");
    assert_eq!(arm.parent(), Some(root.id()));
}

#[test]
fn switch_cells_become_spans_naming_alternatives() {
    let out = convert(TINY);
    let layers = out.document.scenes()[0].timeline().layers();
    let eyes = layers.iter().find(|l| l.name() == "Eyes").expect("eyes");
    assert_eq!(eyes.spans().len(), 2);
    assert_eq!(eyes.spans()[0].start(), 0);
    assert_eq!(eyes.spans()[1].start(), 24);
}

#[test]
fn what_cannot_be_carried_over_is_named() {
    let out = convert(TINY);
    let unsupported: Vec<&str> = out
        .notes
        .iter()
        .filter(|n| n.severity == Severity::Unsupported)
        .map(|n| n.location.as_str())
        .collect();
    assert!(
        unsupported.iter().any(|l| l.contains("Photo")),
        "{unsupported:?}"
    );
    assert!(unsupported.contains(&"audio/line.wav"));
}

#[test]
fn a_bad_size_and_rate_fall_back_with_warnings() {
    let out = convert(r#"{"project_data": {"width": 0, "height": 0, "fps": -1}}"#);
    assert_eq!(out.document.stage().width(), 1920);
    assert_eq!(out.document.frame_rate().numerator(), 24);
    assert!(out.notes.iter().any(|n| n.severity == Severity::Warning));
}

#[test]
fn nan_free_nonsense_does_not_panic() {
    let out = convert(
        r#"{"layers": [{"type": "MeshLayer", "mesh": {"points": [], "curves": [{"points": [{"point": 9}]}]}}]}"#,
    );
    assert!(out.notes.iter().any(|n| n.message.contains("no drawable")));
}

const BVH: &str = "HIERARCHY\nROOT Hips\n{\n OFFSET 0 0 0\n CHANNELS 3 Xposition Yposition Zrotation\n JOINT Arm\n {\n  OFFSET 5 0 0\n  CHANNELS 1 Zrotation\n  End Site\n  {\n   OFFSET 5 0 0\n  }\n }\n}\nMOTION\nFrames: 3\nFrame Time: 0.0416667\n0 0 0 0\n1 1 10 20\n2 2 20 40\n";

#[test]
fn bvh_makes_a_pose_per_frame_for_each_joint() {
    let parsed = bvh::parse(BVH).expect("bvh");
    let out = bvh::to_document(&parsed).expect("document");
    assert_eq!(out.document.frame_rate().numerator(), 24);
    let layers = out.document.scenes()[0].timeline().layers();
    let hips = layers
        .iter()
        .find(|l| l.name() == "Hips pose")
        .expect("hips");
    assert_eq!(hips.spans().len(), 3);
    assert_eq!(hips.spans()[0].tween(), Tween::Linear);
}

#[test]
fn bvh_over_the_pose_limit_is_refused() {
    let mut parsed = bvh::parse(BVH).expect("bvh");
    parsed.frame_count = bvh::MAX_POSES;
    assert!(matches!(
        bvh::to_document(&parsed),
        Err(MohoError::Limit { .. })
    ));
}
