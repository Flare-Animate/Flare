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
        viewport: flare_render::Viewport::full(16, 8),
        size: [16, 8],
        background: Rgba8::WHITE,
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
    assert!(matches!(done[0], Done::Saved { result: Ok(()), .. }));
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
            assert_eq!(opened.scenes().len(), 1);
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
    assert!(woken.load(Ordering::SeqCst) >= 3);
    assert!(!runtime.io_pending());
    for n in 0..3 {
        assert!(dir.path().join(format!("{n}.flare")).exists());
    }
}

#[test]
fn job_numbers_are_never_reused() {
    let mut runtime = Runtime::inline();
    let a = runtime.next_id();
    let b = runtime.next_id();
    assert_ne!(a, b);
}
