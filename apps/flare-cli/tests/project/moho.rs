//! Moho projects through `flare import`: a Moho 13 project with bones, a filled drawing and an
//! image layer comes in, and a picture outside the project's folder is reported, not read.

use super::*;

const PROJECT: &str = r#"{"version": 1038, "major_version": 13,
  "project_data": {"width": 64, "height": 48, "fps": 24.0, "end_frame": 2},
  "layers": [
    {"name": "Rig", "type": "BoneLayer", "visible": true, "parent_bone": -2,
     "skeleton": {"bones": [{"name": "Arm", "parent": -1, "length": 0.5}]}},
    {"name": "Drawing", "type": "MeshLayer", "visible": true, "parent_bone": -2,
     "mesh": {"points": [{"position": {"x": 0, "y": 0}}, {"position": {"x": 1, "y": 0}}, {"position": {"x": 0, "y": 1}}],
              "curves": [{"closed": true, "points": [{"point": 0}, {"point": 1}, {"point": 2}]}],
              "shapes": [{"has_fill": true, "style": {"fill_color": {"when": [0], "val": [{"r": 0.0, "g": 1.0, "b": 0.0, "a": 1.0}]}}}]}},
    {"name": "Photo", "type": "ImageLayer", "visible": true, "parent_bone": -2, "image_path": "art/photo.png"},
    {"name": "Outside", "type": "ImageLayer", "visible": true, "parent_bone": -2, "image_path": "../outside.png"}
  ]}"#;

fn png_1x1() -> Vec<u8> {
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, 1, 1);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("header");
    writer.write_image_data(&[0, 0, 255, 255]).expect("data");
    writer.finish().expect("finish");
    out
}

#[test]
fn moho_project_imports_with_its_picture_and_names_the_outside_one() {
    let dir = tempfile::tempdir().expect("dir");
    let folder = dir.path().join("show");
    std::fs::create_dir_all(folder.join("art")).expect("folder");
    std::fs::write(folder.join("art/photo.png"), png_1x1()).expect("picture");
    std::fs::write(dir.path().join("outside.png"), png_1x1()).expect("outside");
    let source = folder.join("show.mohoproj");
    std::fs::write(&source, PROJECT).expect("project");
    let out = dir.path().join("show.flare").to_string_lossy().into_owned();

    let report = json(&["import", &source.to_string_lossy(), &out]);
    assert_eq!(report["imported"], out);
    assert_eq!(
        report["unsupported"], 1,
        "only the picture outside the folder"
    );
    let diagnostics = report["diagnostics"].as_array().expect("diagnostics");
    let outside: Vec<&Value> = diagnostics
        .iter()
        .filter(|d| d["severity"] == "unsupported")
        .collect();
    assert!(
        outside[0]["message"]
            .as_str()
            .is_some_and(|m| m.contains("leaves the project folder")),
        "{outside:?}"
    );
    // Rig's bone and Drawing's shape are symbols too, alongside the picture's symbol.
    assert!(report["symbols"].as_u64().unwrap_or(0) >= 3);
    let info = json(&["info", &out]);
    assert!(info["symbols"].as_array().is_some_and(|s| s.len() >= 3));
}

#[test]
fn a_moho_file_that_is_not_json_is_refused() {
    let dir = tempfile::tempdir().expect("dir");
    let source = dir.path().join("bad.moho");
    std::fs::write(&source, b"Anime Studio Project\n").expect("write");
    let out = dir.path().join("bad.flare").to_string_lossy().into_owned();
    assert!(
        !run(&["import", &source.to_string_lossy(), &out])
            .status
            .success()
    );
    assert!(!std::path::Path::new(&out).exists(), "nothing is written");
}
