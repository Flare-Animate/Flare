#![allow(clippy::expect_used)]

use flare_core::{DocumentId, DocumentPoint, FrameRate, LayerId, Rgba8, SceneId, ShapeId};
use flare_document::{Scene, Shape, Stage, Timeline};
use flare_history::EditorSession;

use super::*;

fn camera(x: f64, zoom: f64) -> Camera {
    Camera::new(DocumentPoint::new(x, 50.0).expect("finite"), zoom, 0.0).expect("camera")
}

fn session(frames: u32) -> (EditorSession, SceneId) {
    let shape = Shape::rect(ShapeId::new(), 0.0, 0.0, 4.0, 4.0, Rgba8::WHITE).expect("rect");
    let art = Layer::new(
        LayerId::new(),
        "art",
        vec![FrameSpan::new(
            0,
            NonZeroU32::new(frames).expect("nonzero"),
            Content::Shape(shape),
        )],
    )
    .expect("layer");
    let id = SceneId::new();
    let document = Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![Scene::new(id, "S", Timeline::new(vec![art]))],
        vec![],
    )
    .expect("document")
    .with_stage(Stage::new(200, 100, Rgba8::WHITE).expect("stage"));
    (EditorSession::new(document), id)
}

fn spans(session: &EditorSession) -> Vec<(u32, u32, Tween)> {
    session.document().scenes()[0]
        .timeline()
        .layers()
        .iter()
        .find(|l| l.is_camera())
        .expect("a camera layer")
        .spans()
        .iter()
        .map(|s| (s.start(), s.duration().get(), s.tween()))
        .collect()
}

fn set(
    session: &mut EditorSession,
    id: SceneId,
    frame: u32,
    camera: Camera,
    motion: Option<(Tween, i8)>,
) {
    let commands = set_camera_key(session.document(), id, frame, camera, motion).expect("sets");
    session.apply(commands).expect("valid");
}

#[test]
fn the_first_keyframe_makes_a_camera_layer_on_top_that_runs_to_the_end_of_the_scene() {
    let (mut s, id) = session(24);
    set(&mut s, id, 0, camera(80.0, 2.0), None);
    let layers = s.document().scenes()[0].timeline().layers();
    assert!(layers[0].is_camera(), "above the art");
    assert_eq!(layers.len(), 2);
    assert_eq!(spans(&s), vec![(0, 24, Tween::None)]);
    // Keyed later than the scene is long, it still holds the picture from there.
    let (mut late, late_id) = session(10);
    set(&mut late, late_id, 30, camera(80.0, 2.0), None);
    assert_eq!(spans(&late), vec![(30, 1, Tween::None)]);
}

#[test]
fn a_keyframe_inside_another_splits_it_and_both_halves_keep_the_motion() {
    let (mut s, id) = session(24);
    set(&mut s, id, 0, camera(80.0, 2.0), Some((Tween::Linear, 20)));
    set(&mut s, id, 10, camera(120.0, 3.0), None);
    assert_eq!(
        spans(&s),
        vec![(0, 10, Tween::Linear), (10, 14, Tween::Linear)]
    );
    let layer = &s.document().scenes()[0].timeline().layers()[0];
    assert_eq!(layer.spans()[1].ease(), 20);
    assert_eq!(camera_now(s.document(), id, 0), Some(camera(80.0, 2.0)));
    // Partway, on the tween between the two: strictly between them.
    let partway = camera_now(s.document(), id, 5).expect("camera");
    assert!(partway.center().as_vec().x > 80.0 && partway.center().as_vec().x < 120.0);
    assert!(partway.zoom() > 2.0 && partway.zoom() < 3.0);
    assert_eq!(camera_now(s.document(), id, 10), Some(camera(120.0, 3.0)));
    // Keying the start of a keyframe replaces its camera and keeps its length.
    set(&mut s, id, 10, camera(150.0, 4.0), Some((Tween::None, 0)));
    assert_eq!(
        spans(&s),
        vec![(0, 10, Tween::Linear), (10, 14, Tween::None)]
    );
    assert_eq!(camera_now(s.document(), id, 12), Some(camera(150.0, 4.0)));
}

#[test]
fn a_keyframe_in_a_gap_runs_up_to_the_next_and_one_after_the_last_to_the_end() {
    let (mut s, id) = session(30);
    set(&mut s, id, 10, camera(80.0, 2.0), None);
    set(&mut s, id, 4, camera(60.0, 1.5), None);
    assert_eq!(spans(&s), vec![(4, 6, Tween::None), (10, 20, Tween::None)]);
    set(&mut s, id, 40, camera(90.0, 1.0), None);
    assert_eq!(
        spans(&s),
        vec![
            (4, 6, Tween::None),
            (10, 20, Tween::None),
            (40, 1, Tween::None)
        ]
    );
}

#[test]
fn without_a_camera_the_one_that_shows_the_stage_as_it_is_is_current() {
    let (s, id) = session(10);
    assert_eq!(
        camera_now(s.document(), id, 3),
        Some(Camera::centered(&s.document().stage()))
    );
    assert_eq!(camera_now(s.document(), SceneId::new(), 3), None);
}

#[test]
fn an_unknown_scene_or_a_frame_past_the_limit_changes_nothing() {
    let (s, id) = session(10);
    assert!(set_camera_key(s.document(), SceneId::new(), 0, camera(1.0, 1.0), None).is_none());
    assert!(
        set_camera_key(
            s.document(),
            id,
            MAX_CAMERA_FRAME + 1,
            camera(1.0, 1.0),
            None
        )
        .is_none()
    );
    assert!(set_camera_key(s.document(), id, MAX_CAMERA_FRAME, camera(1.0, 1.0), None).is_some());
}
