//! `flare blur` and `flare smudge`: the wiring of `flare_tools::blend` to the command line. What
//! a stroke does to pixels is tested in the library; here the file, the options and the refusals.

use super::*;
use flare_commands::{Command as Edit, apply_all};
use flare_core::{BitmapId, DocumentId, FrameRate, InstanceId, LayerId, SceneId, Transform2D};
use flare_document::{
    Bitmap, BitmapInstance, Content, Document, FrameSpan, Layer, Scene, Timeline,
};
use std::num::NonZeroU32;

/// A project with a 64x64 bitmap at the stage's corner, black on the left and white on the right.
fn picture_project(file: &str) {
    let (layer, scene) = (LayerId::new(), SceneId::new());
    let layer_value = Layer::new(layer, "L", vec![]).expect("layer");
    let empty = Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![Scene::new(scene, "S", Timeline::new(vec![layer_value]))],
        vec![],
    )
    .expect("document");
    let rgba = (0..64 * 64)
        .flat_map(|i| {
            if i % 64 < 32 {
                [0u8, 0, 0, 255]
            } else {
                [255; 4]
            }
        })
        .collect();
    let bitmap = Bitmap::new(BitmapId::new(), "b", 64, 64, rgba).expect("bitmap");
    let content = Content::Bitmap(BitmapInstance::new(
        InstanceId::new(),
        bitmap.id(),
        Transform2D::IDENTITY,
    ));
    let edits = vec![
        Edit::AddBitmap(bitmap),
        Edit::InsertSpan {
            scene,
            layer,
            span: FrameSpan::new(0, NonZeroU32::new(2).expect("nz"), content),
        },
    ];
    let doc = apply_all(&empty, edits).expect("applied").expect("changed");
    flare_storage::save(&doc, Path::new(file)).expect("save");
}

/// The pixel at `(x, y)` of the bitmap the project shows.
fn pixel(file: &str, x: usize, y: usize) -> [u8; 4] {
    let doc = flare_storage::load(Path::new(file)).expect("load");
    let Content::Bitmap(placed) = doc.scenes()[0].timeline().layers()[0].spans()[0].content()
    else {
        panic!("not a bitmap");
    };
    let bitmap = doc.bitmap(placed.bitmap()).expect("bitmap");
    let i = (y * 64 + x) * 4;
    let p = &bitmap.pixels()[i..i + 4];
    [p[0], p[1], p[2], p[3]]
}

fn ok(output: Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("json")
}

#[test]
fn blur_softens_the_edge_along_the_stroke() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    picture_project(&file);
    let report = ok(run(&[
        "blur", &file, "--along", "32", "4", "--along", "32", "60", "--size", "16",
    ]));
    assert_eq!(report["tool"], "blur");
    assert_eq!(report["points"], 2);
    assert!(report["bitmap"].is_string());
    assert!(pixel(&file, 31, 32)[0] > 20, "the black side got lighter");
    assert_eq!(pixel(&file, 2, 32), [0, 0, 0, 255], "far from the edge");
}

#[test]
fn smudge_drags_the_dark_side_into_the_light_and_takes_a_pressure_per_point() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    picture_project(&file);
    ok(run(&[
        "smudge",
        &file,
        "--along",
        "20",
        "32",
        "--along",
        "50",
        "32",
        "--pressure",
        "1",
        "1",
        "--size",
        "10",
        "--stretch",
        "90",
    ]));
    assert!(
        pixel(&file, 40, 32)[0] < 200,
        "dark color carried into the white"
    );
}

#[test]
fn every_refusal_leaves_the_file_as_it_was() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    picture_project(&file);
    let before = std::fs::read(&file).expect("read");
    let f = file.as_str();
    let stroke = ["--along", "32", "4", "--along", "32", "60"];
    let with = |tool: &'static str, extra: &[&'static str]| -> Vec<String> {
        [tool, f]
            .iter()
            .chain(&stroke)
            .chain(extra)
            .map(|s| s.to_string())
            .collect()
    };
    for (args, says) in [
        (
            vec![
                "blur".into(),
                f.into(),
                "--along".into(),
                "500".into(),
                "500".into(),
            ],
            "no bitmap",
        ),
        (with("blur", &["--stretch", "5"]), "--stretch is for smudge"),
        (with("blur", &["--size", "0"]), "size"),
        (with("blur", &["--strength", "150"]), "strength"),
        (
            with("blur", &["--pressure", "1"]),
            "one --pressure for each",
        ),
        (
            with("blur", &["--pressure", "2", "1"]),
            "pressure is 0 to 1",
        ),
        (with("smudge", &["--frame", "9"]), "no bitmap"),
    ] {
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let output = run(&args);
        assert!(!output.status.success(), "{args:?} was refused");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains(says), "{args:?}: {stderr}");
    }
    assert_eq!(std::fs::read(&file).expect("read"), before, "unchanged");
}
