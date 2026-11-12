#![allow(clippy::expect_used)]

use std::sync::atomic::{AtomicUsize, Ordering};

use flare_core::{DocumentId, FrameRate, LayerId, Rgba8, SceneId};
use flare_document::{Document, Layer, Scene, Timeline};

use super::*;

fn document() -> Arc<Document> {
    let layer = Layer::new(LayerId::new(), "Layer 1", vec![]).expect("layer");
    let scene = Scene::new(SceneId::new(), "Scene", Timeline::new(vec![layer]));
    Arc::new(
        Document::new(
            DocumentId::new(),
            FrameRate::new(24, 1).expect("rate"),
            vec![scene],
            vec![],
        )
        .expect("document"),
    )
}

fn render_job(runtime: &mut Runtime, document: &Arc<Document>) -> RenderJob {
    RenderJob {
        id: runtime.next_id(),
        document: Arc::clone(document),
        scene: document.scenes()[0].id(),
        frame: 0,
        ghosts: Vec::new(),
        outlines: false,
        tints: None,
        outline_mode: false,
        through_camera: false,
        viewport: flare_render::Viewport::full(16, 8),
        size: [16, 8],
        background: Rgba8::WHITE,
        text_preview: None,
    }
}

#[test]
fn inline_jobs_are_queued_like_a_workers_results() {
    let mut runtime = Runtime::inline();
    assert_eq!(runtime.mode(), Mode::Inline);
    let document = document();
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("p.flare");
    runtime.submit_io(IoJob::Save {
        path: path.clone(),
        document: Arc::clone(&document),
    });
    assert!(runtime.io_pending(), "reported back only through poll");
    let done = runtime.poll();
    assert!(matches!(done[0], Done::Saved { result: Ok(_), .. }));
    assert!(!runtime.io_pending());
    let id = runtime.next_id();
    runtime.submit_io(IoJob::Open {
        id,
        path,
        recovery: false,
    });
    match runtime.poll().remove(0) {
        Done::Opened {
            id: got,
            result: Ok(opened),
            ..
        } => {
            assert_eq!(got, id);
            assert_eq!(opened.document.scenes().len(), 1);
        }
        _ => panic!("expected an opened document"),
    }
}

#[test]
fn a_render_job_gives_a_frame_of_the_asked_size() {
    let mut runtime = Runtime::inline();
    let document = document();
    let job = render_job(&mut runtime, &document);
    let id = job.id;
    runtime.submit_render(job);
    match runtime.poll().remove(0) {
        Done::Rendered { id: got, result } => {
            assert_eq!(got, id);
            assert_eq!(result.expect("rendered").size(), [16, 8]);
        }
        _ => panic!("expected a rendered frame"),
    }
}

#[test]
fn failures_come_back_as_text_not_panics() {
    let mut runtime = Runtime::inline();
    let id = runtime.next_id();
    runtime.submit_audio(AudioJob::Import {
        id,
        path: "/no/such/sound.wav".into(),
    });
    match runtime.poll().remove(0) {
        Done::SoundImported {
            result: Err(message),
            ..
        } => assert!(!message.is_empty()),
        _ => panic!("expected an import error"),
    }
    runtime.submit_io(IoJob::Open {
        id,
        path: "/no/such/project.flare".into(),
        recovery: false,
    });
    assert!(matches!(
        runtime.poll().remove(0),
        Done::Opened { result: Err(_), .. }
    ));
}

#[test]
fn importing_audio_bytes_rejects_non_audio_files() {
    let mut runtime = Runtime::inline();
    let id = runtime.next_id();
    runtime.submit_audio(AudioJob::ImportBytes {
        id,
        name: "notes.wav".to_owned(),
        bytes: b"not audio".to_vec(),
    });
    match runtime.poll().remove(0) {
        Done::SoundImported {
            id: got,
            path,
            result: Err(message),
        } => {
            assert_eq!(got, id);
            assert_eq!(path, std::path::PathBuf::from("notes.wav"));
            assert!(message.contains("sound"));
        }
        _ => panic!("expected an import error"),
    }
}

#[test]
fn workers_do_the_jobs_wake_the_front_end_and_finish_saves_before_exit() {
    let woken = Arc::new(AtomicUsize::new(0));
    let mut runtime = Runtime::inline();
    let counter = Arc::clone(&woken);
    runtime.start_threads(Arc::new(move || {
        counter.fetch_add(1, Ordering::SeqCst);
    }));
    assert_eq!(runtime.mode(), Mode::Threads);
    let document = document();
    let dir = tempfile::tempdir().expect("dir");
    for n in 0..3 {
        runtime.submit_io(IoJob::Save {
            path: dir.path().join(format!("{n}.flare")),
            document: Arc::clone(&document),
        });
    }
    let results = runtime.finish_io();
    assert_eq!(results.len(), 3, "shutdown waits for every save");
    assert!(!runtime.io_pending());
    for n in 0..3 {
        assert!(dir.path().join(format!("{n}.flare")).exists());
    }
    // A worker sends its result and then wakes the front end, so the last wake can still be on
    // its way when the result is read. Dropping the runtime waits for every worker to finish.
    drop(runtime);
    assert!(woken.load(Ordering::SeqCst) >= 3);
}

#[test]
fn job_numbers_are_never_reused() {
    let mut runtime = Runtime::inline();
    let a = runtime.next_id();
    let b = runtime.next_id();
    assert_ne!(a, b);
}

/// A document with one red 8 by 8 square at the corner, and that square's identity.
fn with_square() -> (Arc<Document>, flare_core::ShapeId) {
    use flare_document::{Content, FrameSpan, Shape};
    let id = flare_core::ShapeId::new();
    let square = Shape::rect(id, 0.0, 0.0, 8.0, 8.0, Rgba8::opaque(255, 0, 0)).expect("shape");
    let span = FrameSpan::new(0, std::num::NonZeroU32::MIN, Content::Shape(square));
    let layer = Layer::new(LayerId::new(), "Layer 1", vec![span]).expect("layer");
    let scene = Scene::new(SceneId::new(), "Scene", Timeline::new(vec![layer]));
    let document = Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![scene],
        vec![],
    )
    .expect("document");
    (Arc::new(document), id)
}

fn pixel_of(done: Done, x: usize, y: usize) -> [u8; 4] {
    let Done::Rendered {
        result: Ok(rendered),
        ..
    } = done
    else {
        panic!("a rendered frame");
    };
    let (width, bytes) = match &rendered {
        Rendered::Premultiplied(image) => (image.width() as usize, image.rgba()),
        Rendered::Straight(image) => (image.width() as usize, image.rgba()),
    };
    let at = (y * width + x) * 4;
    [bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]
}

#[test]
fn text_being_typed_takes_the_place_of_the_text_it_edits_or_goes_on_top_if_it_is_new() {
    use flare_document::Shape;
    let mut runtime = Runtime::inline();
    let (document, square) = with_square();
    let blue = |id| Shape::rect(id, 0.0, 0.0, 8.0, 8.0, Rgba8::opaque(0, 0, 255)).expect("shape");
    let draw = |runtime: &mut Runtime, preview: Option<TextPreviewRender>| {
        let mut job = render_job(runtime, &document);
        job.text_preview = preview;
        pixel_of(run_render(&job), 2, 2)
    };
    assert_eq!(
        draw(&mut runtime, None),
        [255, 0, 0, 255],
        "the document as it is"
    );
    // Replaced: only the new letters show, where the old ones were.
    let replaced = TextPreviewRender {
        replaces: Some(square),
        shape: Some(blue(flare_core::ShapeId::new())),
    };
    assert_eq!(draw(&mut runtime, Some(replaced)), [0, 0, 255, 255]);
    // Replaced by nothing (all the letters were deleted): the old ones are gone too.
    let emptied = TextPreviewRender {
        replaces: Some(square),
        shape: None,
    };
    assert_eq!(draw(&mut runtime, Some(emptied)), [255, 255, 255, 255]);
    // New text is drawn over the scene.
    let fresh = TextPreviewRender {
        replaces: None,
        shape: Some(blue(flare_core::ShapeId::new())),
    };
    assert_eq!(draw(&mut runtime, Some(fresh)), [0, 0, 255, 255]);
    // Something that is no longer there to replace changes nothing.
    let gone = TextPreviewRender {
        replaces: Some(flare_core::ShapeId::new()),
        shape: None,
    };
    assert_eq!(draw(&mut runtime, Some(gone)), [255, 0, 0, 255]);
}

#[test]
fn tinted_ghosts_take_the_before_color_behind_the_frame_and_the_after_color_ahead() {
    use flare_document::{Content, FrameSpan, Shape};
    // A red square on frame 2 only; frames 0 and 4 are empty, so only the ghost shows there.
    let square = Shape::rect(
        flare_core::ShapeId::new(),
        0.0,
        0.0,
        8.0,
        8.0,
        Rgba8::opaque(255, 0, 0),
    )
    .expect("square");
    let span = FrameSpan::new(2, std::num::NonZeroU32::MIN, Content::Shape(square));
    let layer = Layer::new(LayerId::new(), "L", vec![span]).expect("layer");
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(vec![layer]));
    let document = Arc::new(
        Document::new(
            DocumentId::new(),
            FrameRate::new(24, 1).expect("rate"),
            vec![scene],
            vec![],
        )
        .expect("document"),
    );
    let (before, after) = (Rgba8::opaque(0, 0, 255), Rgba8::opaque(0, 160, 0));
    let mut runtime = Runtime::inline();
    let mut draw = |frame: u32, tints: Option<[Rgba8; 2]>| {
        let mut job = render_job(&mut runtime, &document);
        job.frame = frame;
        job.ghosts = vec![(2, 1.0)];
        job.tints = tints;
        pixel_of(run_render(&job), 2, 2)
    };
    // Red is about a fifth as light as white, so the tint is lightened by that much
    // (`Image::tinted`): mostly the tint, with the drawing's light and dark still showing.
    assert_eq!(
        draw(4, Some([before, after])),
        [38, 38, 255, 255],
        "behind: before"
    );
    assert_eq!(
        draw(0, Some([before, after])),
        [38, 174, 38, 255],
        "ahead: after"
    );
    assert_eq!(draw(0, None), [255, 0, 0, 255], "untinted: its own red");
}
