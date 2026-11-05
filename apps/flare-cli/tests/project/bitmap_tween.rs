//! `flare tween bitmap-inbetweens`: the wiring of the library to the command line. The pictures
//! themselves are tested in `flare_tools`; here the files and the refusals are checked.

use super::*;
use flare_commands::{Command as Edit, apply_all};
use flare_core::{BitmapId, DocumentId, FrameRate, InstanceId, LayerId, SceneId, Transform2D};
use flare_document::{
    Bitmap, BitmapInstance, Content, Document, FrameSpan, Layer, Scene, Timeline,
};
use std::num::NonZeroU32;

/// A project with a solid 4x4 bitmap keyframe at frame 0 and at frame 4, saved to `file`.
fn bitmap_project(file: &str) {
    let (layer, scene) = (LayerId::new(), SceneId::new());
    let layer_value = Layer::new(layer, "L", vec![]).expect("layer");
    let empty = Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![Scene::new(scene, "S", Timeline::new(vec![layer_value]))],
        vec![],
    )
    .expect("document");
    let bitmap =
        Bitmap::new(BitmapId::new(), "b", 4, 4, [40, 90, 200, 255].repeat(16)).expect("bitmap");
    let mut edits = vec![Edit::AddBitmap(bitmap.clone())];
    for start in [0, 4] {
        let content = Content::Bitmap(BitmapInstance::new(
            InstanceId::new(),
            bitmap.id(),
            Transform2D::IDENTITY,
        ));
        edits.push(Edit::InsertSpan {
            scene,
            layer,
            span: FrameSpan::new(start, NonZeroU32::new(2).expect("nz"), content),
        });
    }
    let doc = apply_all(&empty, edits).expect("applied").expect("changed");
    flare_storage::save(&doc, Path::new(file)).expect("save");
}

fn bitmap_tween(file: &str, out: &str, count: &str) -> Output {
    run(&[
        "tween",
        "bitmap-inbetweens",
        file,
        "--from",
        "0",
        "--to",
        "4",
        "--count",
        count,
        "--out",
        out,
    ])
}

#[test]
fn writes_one_png_per_inbetween_and_refuses_to_overwrite() {
    let dir = tempfile::tempdir().expect("dir");
    let file = dir.path().join("b.flare").to_string_lossy().into_owned();
    bitmap_project(&file);
    let out = dir.path().join("frames").to_string_lossy().into_owned();
    let made = json(&[
        "tween",
        "bitmap-inbetweens",
        &file,
        "--from",
        "0",
        "--to",
        "4",
        "--count",
        "2",
        "--out",
        &out,
    ]);
    let frames = made["frames"].as_array().expect("frames");
    assert_eq!(frames.len(), 2);
    let first = read_png(frames[0].as_str().expect("path"));
    assert_eq!((first.1, first.2), (4, 4));
    assert_eq!(
        pixel(&first, 1, 1),
        [40, 90, 200, 255],
        "identical keyframes give that picture"
    );
    let again = bitmap_tween(&file, &out, "2");
    assert!(
        !again.status.success(),
        "existing files are never overwritten"
    );
}

#[test]
fn refuses_a_count_out_of_bounds_and_a_frame_without_a_bitmap() {
    let dir = tempfile::tempdir().expect("dir");
    let file = dir.path().join("b.flare").to_string_lossy().into_owned();
    bitmap_project(&file);
    let out = dir.path().join("frames").to_string_lossy().into_owned();
    assert!(!bitmap_tween(&file, &out, "0").status.success());
    let missing = run(&[
        "tween",
        "bitmap-inbetweens",
        &file,
        "--from",
        "0",
        "--to",
        "2",
        "--count",
        "1",
        "--out",
        &out,
    ]);
    assert!(!missing.status.success());
    assert!(
        !Path::new(&out).join("inbetween-001.png").exists(),
        "a refusal writes nothing"
    );
}
