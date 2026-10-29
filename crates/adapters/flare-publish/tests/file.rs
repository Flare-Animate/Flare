//! Publishing to a file: it appears whole, and an existing file is never replaced.
#![allow(clippy::expect_used)]

use flare_core::{DocumentId, FrameRate, LayerId, SceneId};
use flare_document::{Document, Layer, Scene, Timeline};
use flare_publish::{PublishFileError, PublishOptions, publish_to_file};

fn document() -> Document {
    let layer = Layer::new(LayerId::new(), "L", vec![]).expect("layer");
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(vec![layer]));
    Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![scene],
        vec![],
    )
    .expect("document")
}

#[test]
fn the_file_is_written_whole_and_never_overwritten() {
    let dir = tempfile::tempdir().expect("dir");
    let out = dir.path().join("movie.swf");
    let options = PublishOptions { compress: false };
    let published = publish_to_file(&document(), &options, &out).expect("publish");
    let bytes = std::fs::read(&out).expect("read");
    assert_eq!(bytes, published.bytes);
    assert!(bytes.starts_with(b"FWS"), "an uncompressed SWF");
    let error = publish_to_file(&document(), &options, &out).expect_err("exists");
    assert!(matches!(error, PublishFileError::Exists(_)));
    assert_eq!(std::fs::read(&out).expect("read"), bytes, "untouched");
    assert_eq!(
        std::fs::read_dir(dir.path()).expect("ls").count(),
        1,
        "no temp files left"
    );
}

#[test]
fn a_missing_folder_is_an_error_and_leaves_nothing() {
    let out = std::path::Path::new("/definitely/not/here/movie.swf");
    let error = publish_to_file(&document(), &PublishOptions { compress: true }, out)
        .expect_err("no folder");
    assert!(matches!(error, PublishFileError::Io(_)), "{error}");
}
