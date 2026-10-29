//! The threads for slow desktop work report back through their callback.
#![allow(clippy::expect_used)]

use std::sync::mpsc;
use std::time::Duration;

use flare_core::{DocumentId, FrameRate, LayerId, SceneId};
use flare_document::{Document, Layer, Scene, Timeline};
use flare_platform::jobs;

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

fn wait(rx: &mpsc::Receiver<Result<(), String>>) -> Result<(), String> {
    rx.recv_timeout(Duration::from_secs(20))
        .expect("the job reports back")
}

#[test]
fn a_movie_is_written_whole_once_and_never_over_an_existing_file() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("movie.gif");
    let (tx, rx) = mpsc::channel();
    let report = tx.clone();
    jobs::export_gif(document(), path.clone(), move |r| {
        let _ = report.send(r);
    })
    .expect("started");
    wait(&rx).expect("exported");
    assert!(std::fs::read(&path).expect("read").starts_with(b"GIF89a"));
    jobs::export_gif(document(), path.clone(), move |r| {
        let _ = tx.send(r);
    })
    .expect("started");
    let error = wait(&rx).expect_err("exists");
    assert!(error.contains("already exists"), "{error}");
    assert_eq!(
        std::fs::read_dir(dir.path()).expect("ls").count(),
        1,
        "no temp files left"
    );
}

#[test]
fn an_image_and_a_swf_report_through_the_same_callback() {
    let dir = tempfile::tempdir().expect("dir");
    let (tx, rx) = mpsc::channel();
    let report = tx.clone();
    jobs::export_png(document(), 0, dir.path().join("a.png"), move |r| {
        let _ = report.send(r);
    })
    .expect("started");
    wait(&rx).expect("png");
    jobs::publish_swf(document(), dir.path().join("a.swf"), move |r| {
        let _ = tx.send(r);
    })
    .expect("started");
    wait(&rx).expect("swf");
    assert!(dir.path().join("a.png").exists() && dir.path().join("a.swf").exists());
}
