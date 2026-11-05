#![allow(clippy::expect_used)]
//! Image layers: the picture comes from the caller's source, is added to the bitmap library once
//! per path, and a layer whose picture cannot be had is reported with the reason.

use std::collections::HashMap;

use flare_document::Content;
use flare_moho::{ImageSource, NoImages, Picture, Severity, parse_bytes, to_flare_document_with};

const TWO_LAYERS: &str = r#"{
  "version": 1038, "major_version": 13,
  "project_data": {"width": 320, "height": 240, "fps": 24.0, "end_frame": 2},
  "layers": [
    {"name": "Body", "type": "ImageLayer", "visible": true, "parent_bone": -2, "image_path": "art/body.png",
     "transforms": {"translation": {"when": [0, 2], "val": [{"x": 0, "y": 0}, {"x": 1, "y": 0}],
                    "interp": ["Linear", "Linear"], "type": "Vec2"}}},
    {"name": "Body copy", "type": "ImageLayer", "visible": true, "parent_bone": -2, "image_path": "art/body.png"},
    {"name": "Missing", "type": "ImageLayer", "visible": true, "parent_bone": -2, "image_path": "art/gone.png"},
    {"name": "Blank", "type": "ImageLayer", "visible": true, "parent_bone": -2}
  ]
}"#;

/// Serves the pictures it holds and counts how often each path is asked for.
#[derive(Default)]
struct Folder {
    pictures: HashMap<String, Picture>,
    asked: HashMap<String, usize>,
}

impl ImageSource for Folder {
    fn load(&mut self, path: &str) -> Result<Picture, String> {
        *self.asked.entry(path.to_owned()).or_default() += 1;
        self.pictures
            .get(path)
            .cloned()
            .ok_or_else(|| "not found in the project folder".to_owned())
    }
}

fn two_by_one() -> Picture {
    Picture {
        width: 2,
        height: 1,
        rgba: vec![255, 0, 0, 255, 0, 255, 0, 255],
    }
}

#[test]
fn image_layers_share_one_bitmap_per_path() {
    let project = parse_bytes(TWO_LAYERS.as_bytes(), "img.mohoproj").expect("parse");
    let mut folder = Folder::default();
    folder.pictures.insert("art/body.png".into(), two_by_one());
    let out = to_flare_document_with(&project, &mut folder).expect("convert");

    assert_eq!(
        out.document.bitmaps().len(),
        1,
        "one library entry for two layers"
    );
    assert_eq!(
        folder.asked["art/body.png"], 1,
        "asked once, then remembered"
    );

    let layers = out.document.scenes()[0].timeline().layers();
    let body = layers.iter().find(|l| l.name() == "Body").expect("body");
    let Content::Instance(instance) = body.spans()[0].content() else {
        panic!("the image layer places a symbol")
    };
    let symbol = out.document.symbol(instance.symbol()).expect("symbol");
    let Content::Bitmap(picture) = symbol.timeline().layers()[0].spans()[0].content() else {
        panic!("the symbol holds a bitmap")
    };
    assert_eq!(picture.bitmap(), out.document.bitmaps()[0].id());
    assert_eq!(body.spans().len(), 2, "its translation track is kept");
}

#[test]
fn a_picture_that_cannot_be_read_is_named_with_its_reason() {
    let project = parse_bytes(TWO_LAYERS.as_bytes(), "img.mohoproj").expect("parse");
    let mut folder = Folder::default();
    folder.pictures.insert("art/body.png".into(), two_by_one());
    let out = to_flare_document_with(&project, &mut folder).expect("convert");
    let missing: Vec<_> = out
        .notes
        .iter()
        .filter(|n| n.severity == Severity::Unsupported && n.location.starts_with("Missing"))
        .collect();
    assert_eq!(missing.len(), 1);
    assert!(
        missing[0]
            .message
            .contains("not found in the project folder")
    );
    let blank = out
        .notes
        .iter()
        .any(|n| n.location.starts_with("Blank") && n.message == "image layer has no picture");
    assert!(blank, "a layer with no picture path says so");
}

#[test]
fn without_a_folder_every_image_layer_is_unsupported() {
    let project = parse_bytes(TWO_LAYERS.as_bytes(), "img.mohoproj").expect("parse");
    let out = to_flare_document_with(&project, &mut NoImages).expect("convert");
    assert!(out.document.bitmaps().is_empty());
    // Body, Body copy and Missing each say why their picture is absent; Blank has no path.
    let unsupported = out
        .notes
        .iter()
        .filter(|n| n.severity == Severity::Unsupported)
        .count();
    assert_eq!(unsupported, 4);
}
