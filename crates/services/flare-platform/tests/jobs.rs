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
    let plan = jobs::PublishPlan {
        swf: Some(jobs::PublishOptions { compress: true }),
        html5: None,
    };
    jobs::publish(document(), dir.path().join("a.swf"), plan, move |r| {
        let _ = tx.send(r);
    })
    .expect("started");
    wait(&rx).expect("swf");
    assert!(dir.path().join("a.png").exists() && dir.path().join("a.swf").exists());
}

fn plan(swf: bool, html5: bool) -> jobs::PublishPlan {
    jobs::PublishPlan {
        swf: swf.then_some(jobs::PublishOptions { compress: false }),
        html5: html5.then(jobs::Html5Options::default),
    }
}

#[test]
fn publishing_writes_each_chosen_format_beside_the_others_under_one_name() {
    let dir = tempfile::tempdir().expect("dir");
    // A person may have typed an ending; the formats bring their own.
    let typed = dir.path().join("My.Movie.HTML");
    jobs::publish_now(&document(), &typed, &plan(true, true)).expect("published");
    let mut names: Vec<String> = std::fs::read_dir(dir.path())
        .expect("ls")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, ["My.Movie.html", "My.Movie.swf"]);
    let page = std::fs::read_to_string(dir.path().join("My.Movie.html")).expect("page");
    assert!(
        page.contains("<title>My.Movie</title>"),
        "the page is named for the file"
    );
}

#[test]
fn only_the_ticked_formats_are_written_and_none_is_an_error_that_writes_nothing() {
    let dir = tempfile::tempdir().expect("dir");
    jobs::publish_now(&document(), &dir.path().join("a"), &plan(false, true)).expect("html");
    assert!(dir.path().join("a.html").exists() && !dir.path().join("a.swf").exists());
    let error = jobs::publish_now(&document(), &dir.path().join("b"), &plan(false, false))
        .expect_err("nothing chosen");
    assert!(error.contains("Choose a format"), "{error}");
    assert!(!dir.path().join("b.html").exists() && !dir.path().join("b.swf").exists());
}

#[test]
fn an_existing_file_is_never_replaced_and_the_other_format_is_still_written() {
    let dir = tempfile::tempdir().expect("dir");
    std::fs::write(dir.path().join("a.swf"), b"mine").expect("write");
    let error = jobs::publish_now(&document(), &dir.path().join("a"), &plan(true, true))
        .expect_err("the swf exists");
    assert!(error.contains("a.swf"), "{error}");
    assert_eq!(
        std::fs::read(dir.path().join("a.swf")).expect("read"),
        b"mine"
    );
    assert!(
        dir.path().join("a.html").exists(),
        "the page was still written"
    );
}
