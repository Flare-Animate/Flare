//! `flare magic-wand`: the wiring of `flare_tools::magic_wand` to the command line. The picking
//! is tested in the library; here the file, the options and the refusals are checked.

use super::*;
use flare_commands::{Command as Edit, apply_all};
use flare_core::{BitmapId, DocumentId, FrameRate, InstanceId, LayerId, SceneId, Transform2D};
use flare_document::{
    Bitmap, BitmapInstance, Content, Document, FrameSpan, Layer, Scene, Timeline,
};
use std::num::NonZeroU32;

const WHITE: [u8; 4] = [255, 255, 255, 255];
const BLACK: [u8; 4] = [0, 0, 0, 255];

/// A project with a 4x2 bitmap, white on the left and black on the right, at the stage's corner.
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
    let rgba = [WHITE, WHITE, BLACK, BLACK].repeat(2).concat();
    let bitmap = Bitmap::new(BitmapId::new(), "b", 4, 2, rgba).expect("bitmap");
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

/// The alpha of each pixel of the bitmap the project shows, row by row.
fn alphas(file: &str) -> Vec<u8> {
    let doc = flare_storage::load(Path::new(file)).expect("load");
    let Content::Bitmap(placed) = doc.scenes()[0].timeline().layers()[0].spans()[0].content()
    else {
        panic!("not a bitmap");
    };
    let bitmap = doc.bitmap(placed.bitmap()).expect("bitmap");
    bitmap
        .pixels()
        .as_chunks::<4>()
        .0
        .iter()
        .map(|p| p[3])
        .collect()
}

fn wand(file: &str, extra: &[&str]) -> Output {
    let mut args = vec!["magic-wand", file, "--smoothing", "pixels"];
    args.extend_from_slice(extra);
    run(&args)
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
fn clears_the_pixels_picked_at_a_point_and_keeps_the_rest() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    picture_project(&file);
    let report = ok(wand(&file, &["--at", "0.5", "0.5"]));
    assert_eq!(
        (report["picked"].as_u64(), report["cleared"].as_bool()),
        (Some(4), Some(true))
    );
    assert!(report["bitmap"].is_string());
    assert_eq!(alphas(&file), [0, 0, 255, 255, 0, 0, 255, 255]);
}

#[test]
fn keep_clears_everything_else_and_more_points_add_to_the_pick() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    picture_project(&file);
    ok(wand(&file, &["--at", "3.5", "1.5", "--keep"]));
    assert_eq!(alphas(&file), [0, 0, 255, 255, 0, 0, 255, 255]);
    picture_project(&file);
    let both = ok(wand(
        &file,
        &["--at", "0.5", "0.5", "--at", "3.5", "0.5", "--dry-run"],
    ));
    assert_eq!(both["picked"].as_u64(), Some(8));
}

#[test]
fn a_dry_run_and_every_refusal_leave_the_file_as_it_was() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    picture_project(&file);
    let before = std::fs::read(&file).expect("read");
    let report = ok(wand(&file, &["--at", "0.5", "0.5", "--dry-run"]));
    assert_eq!(
        (report["picked"].as_u64(), report["cleared"].as_bool()),
        (Some(4), Some(false))
    );
    for (args, says) in [
        (&["--at", "50", "50"][..], "no bitmap"),
        (
            &["--at", "0.5", "0.5", "--smoothing", "blurry"][..],
            "smoothing",
        ),
        (&["--at", "0.5", "0.5", "--threshold", "201"][..], "201"),
        (&["--at", "0.5", "0.5", "--frame", "9"][..], "no bitmap"),
    ] {
        let output = wand(&file, args);
        assert!(!output.status.success(), "{args:?} was refused");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains(says), "{args:?}: {stderr}");
    }
    assert_eq!(std::fs::read(&file).expect("read"), before, "unchanged");
}
