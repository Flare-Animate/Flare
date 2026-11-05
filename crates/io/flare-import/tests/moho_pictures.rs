#![allow(clippy::expect_used)]
//! A Moho project's pictures are read from its own folder: inside it, as PNG, within limits. A
//! path that leaves the folder, a link that points out, or a file that is not a PNG is reported
//! on its layer and nothing else is opened.

use std::fs;
use std::path::Path;

use flare_import::{Severity, import_path};

fn png(width: u32, height: u32) -> Vec<u8> {
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("header");
    writer
        .write_image_data(&vec![255; (width * height * 4) as usize])
        .expect("data");
    writer.finish().expect("finish");
    out
}

fn image_layer(name: &str, path: &str) -> String {
    format!(
        r#"{{"name": "{name}", "type": "ImageLayer", "visible": true, "parent_bone": -2, "image_path": "{path}"}}"#
    )
}

/// `<root>/secret.png` lies outside the project folder `<root>/proj`.
fn project(root: &Path, layers: &[String]) -> std::path::PathBuf {
    let dir = root.join("proj");
    fs::create_dir_all(dir.join("art")).expect("art folder");
    fs::write(root.join("secret.png"), png(3, 3)).expect("outside picture");
    fs::write(dir.join("art/body.png"), png(4, 2)).expect("picture");
    fs::write(dir.join("art/junk.jpg"), b"\xFF\xD8\xFF not a png").expect("junk");
    fs::write(dir.join("art/broken.png"), b"\x89PNG\r\n\x1a\nnonsense").expect("broken");
    let doc = format!(
        r#"{{"version": 1038, "major_version": 13,
          "project_data": {{"width": 64, "height": 64, "fps": 24.0, "end_frame": 1}},
          "layers": [{}]}}"#,
        layers.join(",")
    );
    let file = dir.join("rig.mohoproj");
    fs::write(&file, doc).expect("project");
    file
}

fn notes_for<'a>(
    diagnostics: &'a [flare_import::Diagnostic],
    layer: &str,
) -> Vec<&'a flare_import::Diagnostic> {
    diagnostics
        .iter()
        .filter(|d| d.location.starts_with(layer) && d.severity == Severity::Unsupported)
        .collect()
}

#[test]
fn a_picture_inside_the_folder_is_placed_and_the_rest_named() {
    let root = tempfile::tempdir().expect("tempdir");
    let file = project(
        root.path(),
        &[
            image_layer("Body", "art/body.png"),
            image_layer("Escape", "../secret.png"),
            image_layer("Absolute", "/etc/hosts"),
            image_layer("Junk", "art/junk.jpg"),
            image_layer("Broken", "art/broken.png"),
            image_layer("Gone", "art/none.png"),
        ],
    );
    let imported = import_path(&file).expect("the project imports");

    assert_eq!(
        imported.document.bitmaps().len(),
        1,
        "only the picture inside the folder is in the library"
    );
    assert!(notes_for(&imported.diagnostics, "Body").is_empty());
    let reason = |layer: &str| -> String {
        let found = notes_for(&imported.diagnostics, layer);
        assert_eq!(found.len(), 1, "one note for {layer}");
        found[0].message.clone()
    };
    assert!(reason("Escape").contains("leaves the project folder"));
    assert!(reason("Absolute").contains("not relative"));
    assert!(reason("Junk").contains("only PNG"));
    assert!(
        reason("Broken").contains("image layer"),
        "a bad PNG is an error on its layer"
    );
    assert!(reason("Gone").contains("not found"));
}

#[cfg(unix)]
#[test]
fn a_link_that_points_out_of_the_folder_is_not_followed() {
    let root = tempfile::tempdir().expect("tempdir");
    let file = project(root.path(), &[image_layer("Link", "art/link.png")]);
    std::os::unix::fs::symlink(
        root.path().join("secret.png"),
        file.parent().expect("dir").join("art/link.png"),
    )
    .expect("symlink");
    let imported = import_path(&file).expect("the project imports");
    assert_eq!(imported.document.bitmaps().len(), 0);
    let found = notes_for(&imported.diagnostics, "Link");
    assert_eq!(found.len(), 1);
    assert!(found[0].message.contains("outside the project folder"));
}
