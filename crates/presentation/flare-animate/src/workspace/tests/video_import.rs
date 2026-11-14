//! File > Import > Import Video, driven through the workspace with stand-in programs in the
//! place of ffprobe and ffmpeg (shell scripts that answer as they do), so the dialog, the read
//! and what it makes are checked on a computer without ffmpeg.
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::Arc;

use super::*;
use crate::workspace::Tool;
use crate::workspace::video_import::VideoImport;
use flare_runtime::VideoProgress;

fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("write");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    path
}

/// A 64x36 movie at 24 frames a second, two seconds long. ffmpeg writes `pictures` different
/// 64x36 pictures.
fn tools(dir: &Path, pictures: usize) -> flare_ffmpeg::Tools {
    flare_ffmpeg::Tools {
        ffprobe: script(
            dir,
            "ffprobe",
            r#"case "$*" in
  *format=duration*) echo "2.0" ;;
  *stream_side_data*) ;;
  *) echo "64,36,24/1,24/1" ;;
esac"#,
        ),
        ffmpeg: script(
            dir,
            "ffmpeg",
            &format!("head -c {} /dev/urandom", pictures * 64 * 36 * 4),
        ),
    }
}

/// A workspace reading videos with the stand-ins, and the movie's path.
fn with_tools(dir: &Path, pictures: usize) -> (Workspace, PathBuf) {
    let mut ws = workspace();
    ws.background.use_video_tools(tools(dir, pictures));
    (ws, dir.join("walk.mp4"))
}

fn bitmap_transforms(ws: &Workspace, frame: u32) -> Vec<DAffine2> {
    let scene = ws.document().scenes()[0].id();
    flare_eval::evaluate_frame(ws.document(), scene, frame)
        .expect("frame")
        .bitmaps
        .iter()
        .map(|b| b.transform)
        .collect()
}

#[test]
fn import_video_is_on_the_file_menu_and_asks_for_a_movie() {
    let mut ws = workspace();
    assert!(ws.is_enabled(MenuAction::ImportVideo));
    ws.perform(MenuAction::ImportVideo);
    assert_eq!(ws.take_dialog(), Some(DialogRequest::ImportVideo));
    let (mut inside, symbol) = with_symbol();
    inside.enter_symbol(symbol);
    assert!(
        !inside.is_enabled(MenuAction::ImportVideo),
        "a video goes in a scene"
    );
    inside.import_video(Path::new("walk.mp4"));
    assert!(inside.video_form().is_none());
}

#[test]
fn the_dialog_starts_at_the_document_rate_and_enter_imports_one_frame_per_picture() {
    let dir = tempfile::tempdir().expect("dir");
    let (mut ws, movie) = with_tools(dir.path(), 24);
    let ctx = egui::Context::default();
    ws.frame = 2;
    ws.import_video(&movie);
    let form = ws.video_form().expect("the dialog is open");
    assert_eq!(form.name, "walk");
    assert_eq!(form.settings.rate, FrameRate::new(24, 1).expect("rate"));
    assert!(
        (form.settings.max_seconds - 2.0).abs() < 1e-9,
        "a short video is read whole"
    );
    // Twelve pictures a second, for animating on twos: 24 frames.
    form.settings.rate = FrameRate::new(12, 1).expect("rate");
    run(&mut ws, &ctx, 0.0);
    assert!(ws.video_form().is_some(), "drawn and still open");
    pass(
        &mut ws,
        &ctx,
        input(0.1, vec![key(Key::Enter, Modifiers::NONE)]),
    );
    assert!(ws.video_form().is_none(), "Enter imports");
    assert!(matches!(ws.video_import, VideoImport::Idle));

    let layers = ws.document().scenes()[0].timeline().layers();
    assert_eq!(layers.len(), 2);
    let layer = layers.last().expect("the bottom layer");
    assert_eq!(layer.name(), "walk");
    assert_eq!(layer.view_opacity(), flare_tools::video::DEFAULT_OPACITY);
    let span = &layer.spans()[0];
    assert_eq!((span.start(), span.duration().get()), (2, 24));
    let Content::Instance(instance) = span.content() else {
        panic!("an instance of the video's symbol");
    };
    let symbol = ws.document().symbol(instance.symbol()).expect("symbol");
    assert_eq!(
        symbol.timeline().layers()[0].spans().len(),
        24,
        "a picture a frame"
    );
    assert_eq!(ws.document().bitmaps().len(), 24);
    // Selected with Free Transform, ready to line up.
    assert_eq!(ws.tool, Tool::FreeTransform);
    assert_eq!(ws.selected.map(|s| s.layer), Some(layer.id()));
    assert!(ws.status.as_deref().is_some_and(|s| s.contains("24")));

    assert!(ws.session.undo(), "one undo step");
    assert_eq!(layer_count(&ws), 1);
    assert!(ws.document().bitmaps().is_empty());
}

#[test]
fn moving_the_video_once_moves_every_picture() {
    let dir = tempfile::tempdir().expect("dir");
    let (mut ws, movie) = with_tools(dir.path(), 48);
    ws.import_video(&movie);
    ws.confirm_video_import();
    let (first, later) = (bitmap_transforms(&ws, 0), bitmap_transforms(&ws, 30));
    assert_eq!((first.len(), later.len()), (1, 1));
    assert_eq!(first, later, "every picture sits in one place");
    ws.perform(MenuAction::FlipHorizontal);
    let (first, later) = (bitmap_transforms(&ws, 0), bitmap_transforms(&ws, 30));
    assert!(first[0].matrix2.x_axis.x < 0.0, "flipped");
    assert_eq!(first, later, "and the later pictures followed");
}

#[test]
fn a_video_too_large_keeps_the_dialog_open_and_reads_nothing() {
    let dir = tempfile::tempdir().expect("dir");
    let (mut ws, movie) = with_tools(dir.path(), 1);
    ws.import_video(&movie);
    let form = ws.video_form().expect("open");
    form.source.width = 3840;
    form.source.height = 2160;
    form.source.seconds = Some(600.0);
    form.settings.max_seconds = 600.0;
    form.settings.max_side = 3840;
    let ctx = egui::Context::default();
    run(&mut ws, &ctx, 0.0);
    ws.confirm_video_import();
    assert!(ws.video_form().is_some(), "still asking");
    assert_eq!(layer_count(&ws), 1, "nothing was read");
}

#[test]
fn a_movie_ffprobe_cannot_read_is_reported_and_no_dialog_opens() {
    let dir = tempfile::tempdir().expect("dir");
    let mut ws = workspace();
    ws.background.use_video_tools(flare_ffmpeg::Tools {
        ffprobe: script(
            dir.path(),
            "ffprobe",
            "echo 'Invalid data found' >&2\nexit 1",
        ),
        ffmpeg: script(dir.path(), "ffmpeg", "exit 1"),
    });
    ws.import_video(&dir.path().join("notes.mp4"));
    assert!(ws.video_form().is_none());
    assert!(matches!(ws.video_import, VideoImport::Idle));
    let status = ws.status.clone().unwrap_or_default();
    assert!(status.contains("Invalid data found"), "{status}");
    assert!(ws.is_enabled(MenuAction::ImportVideo), "ready for another");
}

#[test]
fn escape_on_the_progress_stops_the_reading_and_a_stopped_import_changes_nothing() {
    let mut ws = workspace();
    let progress = Arc::new(VideoProgress::default());
    ws.video_import = VideoImport::Reading {
        id: 7,
        name: "walk".to_owned(),
        total: 48,
        progress: Arc::clone(&progress),
    };
    assert!(!ws.is_enabled(MenuAction::ImportVideo), "one at a time");
    assert_eq!(ws.video_reading(), Some((0, 48)));
    let ctx = egui::Context::default();
    run(&mut ws, &ctx, 0.0);
    pass(
        &mut ws,
        &ctx,
        input(0.1, vec![key(Key::Escape, Modifiers::NONE)]),
    );
    assert!(progress.is_stopped());
    ws.video_read(7, flare_runtime::VideoOutcome::Stopped);
    assert!(matches!(ws.video_import, VideoImport::Idle));
    assert_eq!(layer_count(&ws), 1);
    assert!(ws.status.is_some(), "it says it stopped");
}

#[test]
fn a_click_on_the_video_picks_it_unless_its_layer_is_locked() {
    let dir = tempfile::tempdir().expect("dir");
    let (mut ws, movie) = with_tools(dir.path(), 48);
    ws.import_video(&movie);
    ws.confirm_video_import();
    let at = ws.selected.expect("selected after the import");
    ws.selected = None;
    ws.frame = 20;
    // The 64x36 pictures are fitted to the 1920x1080 stage, so the middle is on them.
    assert_eq!(ws.pick(Pos2::new(960.0, 540.0)), Some(at));
    ws.toggle_layer_locked(at.layer);
    assert_eq!(ws.pick(Pos2::new(960.0, 540.0)), None, "locked");
}
