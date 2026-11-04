#![allow(clippy::expect_used)]
use flare_stopmotion::{MAX_FPS, MAX_FRAMES, MAX_HOLD, StopMotionError, StopMotionSession};

#[test]
fn new_session_keeps_rate_and_hold() {
    let session = StopMotionSession::new(12, 2).expect("valid session");
    assert_eq!(session.fps(), 12);
    assert_eq!(session.default_hold(), 2);
    assert!(session.frames().is_empty());
    assert_eq!(session.duration(), 0);
}

#[test]
fn rate_and_hold_bounds_are_inclusive_and_refuse_outside() {
    assert!(StopMotionSession::new(1, 1).is_ok());
    assert!(StopMotionSession::new(MAX_FPS, MAX_HOLD).is_ok());
    assert_eq!(
        StopMotionSession::new(0, 1),
        Err(StopMotionError::FrameRate {
            got: 0,
            max: MAX_FPS
        })
    );
    assert_eq!(
        StopMotionSession::new(MAX_FPS + 1, 1),
        Err(StopMotionError::FrameRate {
            got: MAX_FPS + 1,
            max: MAX_FPS
        })
    );
    assert_eq!(
        StopMotionSession::new(24, 0),
        Err(StopMotionError::Hold {
            got: 0,
            max: MAX_HOLD
        })
    );
    assert_eq!(
        StopMotionSession::new(24, MAX_HOLD + 1),
        Err(StopMotionError::Hold {
            got: MAX_HOLD + 1,
            max: MAX_HOLD
        })
    );
}

#[test]
fn added_frames_take_the_default_hold_and_ids_are_sequential() {
    let mut session = StopMotionSession::new(24, 3).expect("valid session");
    let first = session.add_frame().expect("room");
    let second = session.add_frame().expect("room");
    assert_eq!(first.get(), 0);
    assert_eq!(second.get(), 1);
    assert!(session.frames().iter().all(|frame| frame.hold() == 3));
    assert_eq!(session.duration(), 6);
}

#[test]
fn changing_the_default_hold_leaves_captured_frames_alone() {
    let mut session = StopMotionSession::new(24, 1).expect("valid session");
    session.add_frame().expect("room");
    session.set_default_hold(4).expect("valid hold");
    session.add_frame().expect("room");
    let holds: Vec<u32> = session.frames().iter().map(|f| f.hold()).collect();
    assert_eq!(holds, vec![1, 4]);
    assert_eq!(
        session.set_default_hold(MAX_HOLD + 1),
        Err(StopMotionError::Hold {
            got: MAX_HOLD + 1,
            max: MAX_HOLD
        })
    );
    assert_eq!(session.default_hold(), 4, "a refused hold changes nothing");
}

#[test]
fn set_hold_changes_one_frame_and_refuses_bad_input_without_change() {
    let mut session = StopMotionSession::new(24, 1).expect("valid session");
    let a = session.add_frame().expect("room");
    let b = session.add_frame().expect("room");
    session.set_hold(b, 5).expect("valid");
    assert_eq!(session.frames()[0].hold(), 1);
    assert_eq!(session.frames()[1].hold(), 5);

    assert_eq!(
        session.set_hold(b, 0),
        Err(StopMotionError::Hold {
            got: 0,
            max: MAX_HOLD
        })
    );
    assert_eq!(
        session.frames()[1].hold(),
        5,
        "refused hold keeps the old one"
    );

    session.remove_frame(a).expect("present");
    assert_eq!(
        session.set_hold(a, 2),
        Err(StopMotionError::UnknownFrame(a))
    );
}

#[test]
fn remove_frame_keeps_other_ids_and_refuses_unknown() {
    let mut session = StopMotionSession::new(24, 1).expect("valid session");
    let a = session.add_frame().expect("room");
    let b = session.add_frame().expect("room");
    let c = session.add_frame().expect("room");
    let removed = session.remove_frame(b).expect("present");
    assert_eq!(removed.id(), b);
    let ids: Vec<u32> = session.frames().iter().map(|f| f.id().get()).collect();
    assert_eq!(ids, vec![a.get(), c.get()]);

    assert_eq!(
        session.remove_frame(b),
        Err(StopMotionError::UnknownFrame(b)),
        "a removed id is not reused"
    );
    let d = session.add_frame().expect("room");
    assert_ne!(d, b, "ids are never reused after removal");
}

#[test]
fn session_refuses_frames_past_the_limit() {
    let mut session = StopMotionSession::new(24, 1).expect("valid session");
    for _ in 0..MAX_FRAMES {
        session.add_frame().expect("within the limit");
    }
    assert_eq!(
        session.add_frame(),
        Err(StopMotionError::TooManyFrames { max: MAX_FRAMES })
    );
    assert_eq!(session.frames().len(), MAX_FRAMES);
}
