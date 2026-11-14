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
    jobs::export_movie(document(), path.clone(), move |r| {
        let _ = report.send(r);
    })
    .expect("started");
    wait(&rx).expect("exported");
    assert!(std::fs::read(&path).expect("read").starts_with(b"GIF89a"));
    jobs::export_movie(document(), path.clone(), move |r| {
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
        gif: false,
        mp4: false,
        mov: false,
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
        gif: false,
        mp4: false,
        mov: false,
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

#[test]
fn a_gif_is_written_with_the_others_and_alone_it_is_enough_to_publish() {
    let dir = tempfile::tempdir().expect("dir");
    let mut with_gif = plan(true, true);
    with_gif.gif = true;
    jobs::publish_now(&document(), &dir.path().join("m.GIF"), &with_gif).expect("published");
    let mut names: Vec<String> = std::fs::read_dir(dir.path())
        .expect("ls")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, ["m.gif", "m.html", "m.swf"]);
    assert!(
        std::fs::read(dir.path().join("m.gif"))
            .expect("read")
            .starts_with(b"GIF89a")
    );

    let mut alone = plan(false, false);
    alone.gif = true;
    jobs::publish_now(&document(), &dir.path().join("solo"), &alone).expect("a gif alone");
    assert!(dir.path().join("solo.gif").exists());
    // And an existing one is never replaced.
    let error =
        jobs::publish_now(&document(), &dir.path().join("solo"), &alone).expect_err("it exists");
    assert!(error.contains("solo.gif"), "{error}");
}

#[test]
fn an_mp4_is_written_beside_the_others_and_one_alone_is_enough() {
    let dir = tempfile::tempdir().expect("dir");
    let mut with_video = plan(false, true);
    with_video.mp4 = true;
    jobs::publish_now(&document(), &dir.path().join("Film.MP4"), &with_video).expect("published");
    let bytes = std::fs::read(dir.path().join("Film.mp4")).expect("the video");
    assert_eq!(&bytes[4..8], b"ftyp", "a real MP4");
    assert!(dir.path().join("Film.html").exists());

    let mut alone = plan(false, false);
    alone.mp4 = true;
    jobs::publish_now(&document(), &dir.path().join("Solo"), &alone).expect("a video alone");
    assert!(dir.path().join("Solo.mp4").exists());
}

#[test]
fn an_existing_mp4_is_never_replaced_and_the_other_formats_are_still_written() {
    let dir = tempfile::tempdir().expect("dir");
    std::fs::write(dir.path().join("Film.mp4"), b"mine").expect("write");
    let mut both = plan(false, true);
    both.mp4 = true;
    let error = jobs::publish_now(&document(), &dir.path().join("Film"), &both)
        .expect_err("the video exists");
    assert!(
        error.contains("Film.mp4") && error.contains("already exists"),
        "{error}"
    );
    assert_eq!(
        std::fs::read(dir.path().join("Film.mp4")).expect("read"),
        b"mine"
    );
    assert!(
        dir.path().join("Film.html").exists(),
        "the page was still written"
    );
}

#[test]
fn a_project_is_exported_as_a_fla_that_imports_back_and_is_never_replaced() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("project.fla");
    let (tx, rx) = mpsc::channel();
    let report = tx.clone();
    jobs::export_fla(document(), path.clone(), move |r| {
        let _ = report.send(r);
    })
    .expect("started");
    wait(&rx).expect("exported");
    let bytes = std::fs::read(&path).expect("the file");
    let imported =
        flare_import::import_fla_bytes(bytes.clone()).expect("Animate's format reads back");
    assert_eq!(imported.document.scenes().len(), document().scenes().len());
    // A second export to the same place is refused and leaves the first alone.
    jobs::export_fla(document(), path.clone(), move |r| {
        let _ = tx.send(r);
    })
    .expect("started");
    let error = wait(&rx).expect_err("it exists");
    assert!(error.contains("already exists"), "{error}");
    assert_eq!(std::fs::read(&path).expect("read"), bytes);
    assert_eq!(
        std::fs::read_dir(dir.path()).expect("ls").count(),
        1,
        "no staging file left"
    );
}

#[test]
fn a_movie_for_the_in_app_player_is_published_on_a_thread_and_handed_back_as_bytes() {
    let (tx, rx) = mpsc::channel();
    jobs::publish_movie(document(), move |r| {
        let _ = tx.send(r);
    })
    .expect("started");
    let bytes = rx
        .recv_timeout(Duration::from_secs(20))
        .expect("the job reports back")
        .expect("published");
    assert!(bytes.starts_with(b"CWS") || bytes.starts_with(b"FWS"));
}

#[test]
fn export_movie_writes_the_kind_the_name_ends_in() {
    let dir = tempfile::tempdir().expect("dir");
    for name in ["a.gif", "b.MP4", "c.mov"] {
        jobs::write_movie(&document(), &dir.path().join(name)).expect(name);
    }
    let read = |name: &str| std::fs::read(dir.path().join(name)).expect(name);
    assert!(read("a.gif").starts_with(b"GIF89a"));
    assert_eq!(&read("b.MP4")[4..12], b"ftypisom");
    assert_eq!(&read("c.mov")[4..12], b"ftypqt  ");
    // No ending is a GIF, as before there was a choice.
    jobs::write_movie(&document(), &dir.path().join("d")).expect("d");
    assert!(read("d.gif").starts_with(b"GIF89a"));
    let error = jobs::write_movie(&document(), &dir.path().join("e.avi")).expect_err("avi");
    assert!(error.contains(".mov"), "{error}");
    assert!(!dir.path().join("e.avi").exists());
}

#[test]
fn a_quicktime_movie_is_published_beside_the_others() {
    let dir = tempfile::tempdir().expect("dir");
    let mut with_mov = plan(true, false);
    with_mov.mov = true;
    jobs::publish_now(&document(), &dir.path().join("m.mov"), &with_mov).expect("published");
    let mut names: Vec<String> = std::fs::read_dir(dir.path())
        .expect("ls")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, ["m.mov", "m.swf"]);
    // Alone it is enough to publish.
    let mut only = plan(false, false);
    only.mov = true;
    jobs::publish_now(&document(), &dir.path().join("n"), &only).expect("mov alone");
    assert!(dir.path().join("n.mov").exists());
}
