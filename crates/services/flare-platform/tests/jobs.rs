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
        gif: false,
        mp4: None,
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
        mp4: None,
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

#[cfg(unix)]
fn fake_ffmpeg(dir: &std::path::Path, body: &str) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let path = dir.join("fake-ffmpeg");
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("write");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    path
}

#[cfg(unix)]
#[test]
fn an_mp4_is_written_beside_the_others_and_a_failing_encoder_does_not_stop_them() {
    let tools = tempfile::tempdir().expect("tools");
    let good = fake_ffmpeg(tools.path(), "for last; do :; done\ncat > \"$last\"");
    let dir = tempfile::tempdir().expect("dir");
    let mut with_video = plan(false, true);
    with_video.mp4 = Some(good);
    jobs::publish_now(&document(), &dir.path().join("Film.MP4"), &with_video).expect("published");
    assert!(dir.path().join("Film.mp4").exists() && dir.path().join("Film.html").exists());

    let bad = fake_ffmpeg(
        tools.path(),
        "cat > /dev/null\necho 'no libx264' >&2\nexit 2",
    );
    let mut failing = plan(false, true);
    failing.mp4 = Some(bad);
    let error = jobs::publish_now(&document(), &dir.path().join("Other"), &failing)
        .expect_err("the video fails");
    assert!(error.contains("no libx264"), "{error}");
    assert!(
        dir.path().join("Other.html").exists(),
        "the page was still written"
    );
    assert!(!dir.path().join("Other.mp4").exists());

    // An MP4 alone is enough to publish.
    let mut alone = plan(false, false);
    alone.mp4 = Some(fake_ffmpeg(
        tools.path(),
        "for last; do :; done\ncat > \"$last\"",
    ));
    jobs::publish_now(&document(), &dir.path().join("Solo"), &alone).expect("a video alone");
    assert!(dir.path().join("Solo.mp4").exists());
}
