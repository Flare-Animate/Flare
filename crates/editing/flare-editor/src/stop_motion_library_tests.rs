#![allow(clippy::expect_used)]

use flare_core::{BitmapId, DocumentId, FrameRate};
use flare_document::{Scene, Timeline};
use flare_history::EditorSession;

use super::*;

fn session() -> (EditorSession, SceneId) {
    let scene = SceneId::new();
    let rate = FrameRate::new(24, 1).expect("rate");
    let document = Document::new(
        DocumentId::new(),
        rate,
        vec![Scene::new(scene, "S", Timeline::new(vec![]))],
        vec![],
    )
    .expect("document");
    (EditorSession::new(document), scene)
}

fn picture(shade: u8) -> Bitmap {
    Bitmap::new(BitmapId::new(), "frame", 2, 2, vec![shade; 16]).expect("bitmap")
}

fn started() -> (EditorSession, SceneId) {
    let (mut s, scene) = session();
    let command = start(s.document(), 12, 2).expect("start");
    s.apply([command]).expect("apply");
    (s, scene)
}

#[test]
fn a_session_collects_frames_shares_equal_pictures_and_every_change_is_one_undo_step() {
    let (mut s, _) = started();
    let one = add_frame(s.document(), picture(10), None).expect("add");
    s.apply(one).expect("apply");
    let again = add_frame(s.document(), picture(10), Some(5)).expect("same pixels");
    assert_eq!(again.len(), 1, "no second bitmap for the same pixels");
    s.apply(again).expect("apply");
    let other = add_frame(s.document(), picture(99), None).expect("add");
    s.apply(other).expect("apply");
    let session_now = s.document().stop_motion().expect("session");
    assert_eq!(s.document().bitmaps().len(), 2);
    assert_eq!(
        session_now
            .frames()
            .iter()
            .map(StopFrame::hold)
            .collect::<Vec<_>>(),
        [2, 5, 2]
    );
    let hold = set_hold(s.document(), 0, 7).expect("hold");
    s.apply([hold]).expect("apply");
    assert_eq!(s.document().stop_motion().expect("session").duration(), 14);
    s.undo();
    assert_eq!(s.document().stop_motion().expect("session").duration(), 9);
    let gone = remove_frame(s.document(), 1).expect("remove");
    s.apply([gone]).expect("apply");
    assert_eq!(
        s.document().stop_motion().expect("session").frames().len(),
        2
    );
    let rate = set_rate(s.document(), 24, 3).expect("rate");
    s.apply([rate]).expect("apply");
    assert_eq!(s.document().stop_motion().expect("session").fps(), 24);
}

#[test]
fn it_lays_the_frames_on_a_new_layer_end_to_end() {
    let (mut s, scene) = started();
    for (shade, hold) in [(1, 2), (2, 3), (3, 1)] {
        let commands = add_frame(s.document(), picture(shade), Some(hold)).expect("add");
        s.apply(commands).expect("apply");
    }
    let (layer, commands) = lay_on_timeline(s.document(), scene, "Stop motion").expect("lay");
    s.apply(commands).expect("apply");
    let placed = &s.document().scenes()[0].timeline().layers()[0];
    assert_eq!(placed.id(), layer);
    let spans: Vec<(u32, u32)> = placed
        .spans()
        .iter()
        .map(|sp| (sp.start(), sp.duration().get()))
        .collect();
    assert_eq!(spans, [(0, 2), (2, 3), (5, 1)]);
    s.undo();
    assert!(s.document().scenes()[0].timeline().layers().is_empty());
}

#[test]
fn refusals_name_the_reason_and_change_nothing() {
    let (mut s, scene) = session();
    assert_eq!(
        add_frame(s.document(), picture(1), None).map(|_| ()),
        Err(StopMotionLibraryError::NotStarted)
    );
    assert_eq!(end(s.document()), Err(StopMotionLibraryError::NotStarted));
    assert!(matches!(
        start(s.document(), 0, 1),
        Err(StopMotionLibraryError::Session(_))
    ));
    assert!(matches!(
        start(s.document(), 24, 61),
        Err(StopMotionLibraryError::Session(_))
    ));
    let command = start(s.document(), 24, 1).expect("start");
    s.apply([command]).expect("apply");
    assert_eq!(
        start(s.document(), 24, 1),
        Err(StopMotionLibraryError::AlreadyStarted)
    );
    assert!(matches!(
        lay_on_timeline(s.document(), scene, "L"),
        Err(StopMotionLibraryError::NoFrames)
    ));
    assert!(matches!(
        set_hold(s.document(), 3, 2),
        Err(StopMotionLibraryError::NoSuchFrame { index: 3, len: 0 })
    ));
    let commands = add_frame(s.document(), picture(5), None).expect("add");
    s.apply(commands).expect("apply");
    assert!(matches!(
        set_hold(s.document(), 0, 0),
        Err(StopMotionLibraryError::Session(_))
    ));
    assert!(matches!(
        lay_on_timeline(s.document(), SceneId::new(), "L"),
        Err(StopMotionLibraryError::NoSuchScene)
    ));
    // A picture in use cannot be taken out of the library.
    let in_use = s.document().bitmaps()[0].id();
    assert!(s.apply([Command::RemoveBitmap(in_use)]).is_err());
    let finished = end(s.document()).expect("end");
    s.apply([finished]).expect("apply");
    assert!(s.document().stop_motion().is_none());
    s.undo();
    assert!(s.document().stop_motion().is_some());
}
