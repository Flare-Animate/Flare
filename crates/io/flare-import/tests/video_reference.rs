//! A video brought into a document, read through stand-in programs in the place of ffprobe and
//! ffmpeg (small shell scripts answering as they do), so the probe, the plan, the reading, the
//! stop and the refusals are checked on a computer without ffmpeg. The real ffmpeg's part is in
//! `flare-ffmpeg`'s own tests.
#![cfg(unix)]
#![allow(clippy::expect_used)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use flare_core::{DocumentId, FrameRate, LayerId, SceneId};
use flare_document::{Content, Document, Layer, Scene, Timeline};
use flare_ffmpeg::Tools;
use flare_import::{VideoImportError, import_video_with, probe_video_with, read_video_with};
use flare_tools::video::{Settings, plan};

fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("write");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    path
}

/// A 64x36 movie at 25 frames a second, two seconds long; ffmpeg writes `pictures` of the
/// 32x18 the plan below asks for, and leaves a note that it ran.
fn tools(dir: &Path, pictures: usize) -> Tools {
    let ran = dir.join("ran");
    Tools {
        ffprobe: script(
            dir,
            "ffprobe",
            r#"case "$*" in
  *format=duration*) echo "2.0" ;;
  *stream_side_data*) ;;
  *) echo "64,36,25/1,25/1" ;;
esac"#,
        ),
        ffmpeg: script(
            dir,
            "ffmpeg",
            &format!(
                "touch '{}'\nhead -c {} /dev/zero",
                ran.display(),
                pictures * 32 * 18 * 4
            ),
        ),
    }
}

fn document() -> (Document, SceneId) {
    let scene = SceneId::new();
    let layer = Layer::new(LayerId::new(), "Layer 1", vec![]).expect("layer");
    let rate = FrameRate::new(24, 1).expect("rate");
    let doc = Document::new(
        DocumentId::new(),
        rate,
        vec![Scene::new(scene, "Scene 1", Timeline::new(vec![layer]))],
        vec![],
    )
    .expect("doc");
    (doc, scene)
}

fn settings(fps: u32, seconds: f64, side: u32) -> Settings {
    Settings {
        rate: FrameRate::new(fps, 1).expect("rate"),
        max_seconds: seconds,
        max_side: side,
        opacity: 50,
    }
}

#[test]
fn a_probed_video_planned_and_read_becomes_a_layer_of_one_symbol() {
    let dir = tempfile::tempdir().expect("dir");
    let tools = tools(dir.path(), 24);
    let movie = dir.path().join("walk.mp4");
    let source = probe_video_with(&tools, &movie).expect("probe");
    assert_eq!(
        (source.width, source.height, source.seconds),
        (64, 36, Some(2.0))
    );
    let (document, scene) = document();
    let plan = plan(&document, &source, &settings(12, 60.0, 32)).expect("plan");
    assert_eq!((plan.frames, plan.size), (24, (32, 18)));
    let placed =
        import_video_with(&tools, &document, scene, 3, "walk", &movie, &plan).expect("import");
    assert_eq!(placed.frames, 24);
    assert_eq!(placed.distinct, 1, "the stand-in's pictures are all alike");
    let after = flare_commands::apply_all(&document, placed.commands)
        .expect("applies")
        .expect("changed");
    let layer = after.scenes()[0]
        .timeline()
        .layers()
        .last()
        .expect("bottom layer");
    assert_eq!(layer.id(), placed.layer);
    let span = &layer.spans()[0];
    assert_eq!((span.start(), span.duration().get()), (3, 24));
    assert!(matches!(span.content(), Content::Instance(_)));
}

#[test]
fn a_progress_that_says_stop_stops_the_reading() {
    let dir = tempfile::tempdir().expect("dir");
    let tools = tools(dir.path(), 24);
    let movie = dir.path().join("walk.mp4");
    let source = probe_video_with(&tools, &movie).expect("probe");
    let (document, _) = document();
    let plan = plan(&document, &source, &settings(12, 60.0, 32)).expect("plan");
    let mut heard = Vec::new();
    let result = read_video_with(&tools, &movie, &plan, |n| {
        heard.push(n);
        n < 3
    });
    assert!(
        matches!(result, Err(VideoImportError::Stopped)),
        "{result:?}"
    );
    assert_eq!(heard, vec![1, 2, 3], "how many have come, after each");
}

#[test]
fn a_plan_too_large_for_the_document_is_refused_before_ffmpeg_runs() {
    let dir = tempfile::tempdir().expect("dir");
    let tools = tools(dir.path(), 1);
    let movie = dir.path().join("walk.mp4");
    let mut source = probe_video_with(&tools, &movie).expect("probe");
    source.width = 3840;
    source.height = 2160;
    source.seconds = Some(600.0);
    let (document, _) = document();
    let plan = plan(&document, &source, &settings(24, 600.0, 3840)).expect("plan");
    assert!(!plan.fits());
    let result = read_video_with(&tools, &movie, &plan, |_| true);
    assert!(
        matches!(result, Err(VideoImportError::Refused(_))),
        "{result:?}"
    );
    assert!(!dir.path().join("ran").exists(), "nothing was read");
}

#[test]
fn a_file_ffmpeg_cannot_read_is_reported_with_what_it_said() {
    let dir = tempfile::tempdir().expect("dir");
    let tools = Tools {
        ffprobe: script(
            dir.path(),
            "ffprobe",
            "echo 'walk.mp4: Invalid data found when processing input' >&2\nexit 1",
        ),
        ffmpeg: script(dir.path(), "ffmpeg", "exit 1"),
    };
    let error = probe_video_with(&tools, &dir.path().join("walk.mp4")).expect_err("fails");
    assert!(
        matches!(error, VideoImportError::Unreadable(_)),
        "{error:?}"
    );
    assert!(error.to_string().contains("Invalid data"), "{error}");
}
