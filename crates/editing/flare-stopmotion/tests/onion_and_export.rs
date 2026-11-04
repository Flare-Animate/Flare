#![allow(clippy::expect_used)]
use flare_stopmotion::{
    MAX_ONION_FRAMES, MAX_ONION_OPACITY, MAX_TIMELINE_FRAMES, OnionSkin, StopMotionError,
    StopMotionSession, ghost_frames, timeline_slots,
};

#[test]
fn onion_skin_is_off_by_default() {
    let onion = OnionSkin::default();
    assert!(!onion.is_enabled());
    assert_eq!(onion.before(), 0);
    assert_eq!(onion.after(), 0);
    assert_eq!(onion.opacity(), 50);
}

#[test]
fn onion_skin_refuses_out_of_range_values() {
    assert!(OnionSkin::new(MAX_ONION_FRAMES, MAX_ONION_FRAMES, MAX_ONION_OPACITY).is_ok());
    assert_eq!(
        OnionSkin::new(MAX_ONION_FRAMES + 1, 0, 50),
        Err(StopMotionError::OnionFrames {
            got: MAX_ONION_FRAMES + 1,
            max: MAX_ONION_FRAMES
        })
    );
    assert_eq!(
        OnionSkin::new(0, MAX_ONION_FRAMES + 1, 50),
        Err(StopMotionError::OnionFrames {
            got: MAX_ONION_FRAMES + 1,
            max: MAX_ONION_FRAMES
        })
    );
    assert_eq!(
        OnionSkin::new(1, 1, MAX_ONION_OPACITY + 1),
        Err(StopMotionError::OnionOpacity {
            got: MAX_ONION_OPACITY + 1,
            max: MAX_ONION_OPACITY
        })
    );
}

#[test]
fn zero_opacity_or_zero_frames_is_not_enabled() {
    assert!(!OnionSkin::new(2, 2, 0).expect("valid").is_enabled());
    assert!(!OnionSkin::new(0, 0, 80).expect("valid").is_enabled());
    assert!(OnionSkin::new(0, 1, 80).expect("valid").is_enabled());
}

#[test]
fn ghosts_are_nearest_first_and_stop_at_the_ends() {
    let onion = OnionSkin::new(2, 2, 40).expect("valid");
    assert_eq!(
        ghost_frames(&onion, 10, 5).expect("in range"),
        vec![4, 3, 6, 7]
    );
    // At the start there is nothing before, and the later frames are cut to the session.
    assert_eq!(ghost_frames(&onion, 10, 0).expect("in range"), vec![1, 2]);
    // At the end there is nothing after.
    assert_eq!(ghost_frames(&onion, 10, 9).expect("in range"), vec![8, 7]);
    // A short session never returns the same frame twice.
    assert_eq!(ghost_frames(&onion, 2, 0).expect("in range"), vec![1]);
}

#[test]
fn ghosts_are_empty_when_the_skin_is_off() {
    let onion = OnionSkin::new(3, 3, 0).expect("valid");
    assert!(ghost_frames(&onion, 10, 5).expect("in range").is_empty());
}

#[test]
fn ghosts_refuse_a_position_outside_the_session() {
    let onion = OnionSkin::new(1, 1, 50).expect("valid");
    assert_eq!(
        ghost_frames(&onion, 3, 3),
        Err(StopMotionError::IndexOutOfRange { index: 3, len: 3 })
    );
    assert_eq!(
        ghost_frames(&onion, 0, 0),
        Err(StopMotionError::IndexOutOfRange { index: 0, len: 0 })
    );
}

#[test]
fn timeline_slots_lay_frames_end_to_end_with_their_holds() {
    let mut session = StopMotionSession::new(12, 2).expect("valid");
    let a = session.add_frame().expect("room");
    let b = session.add_frame().expect("room");
    session.set_hold(b, 3).expect("valid");
    let slots = timeline_slots(&session).expect("frames to export");
    assert_eq!(slots.len(), 2);
    assert_eq!(
        (slots[0].frame(), slots[0].start(), slots[0].length()),
        (a, 0, 2)
    );
    assert_eq!(
        (slots[1].frame(), slots[1].start(), slots[1].length()),
        (b, 2, 3)
    );
}

#[test]
fn export_refuses_an_empty_session() {
    let session = StopMotionSession::new(12, 1).expect("valid");
    assert_eq!(timeline_slots(&session), Err(StopMotionError::NoFrames));
}

#[test]
fn export_refuses_a_timeline_over_the_limit() {
    // 60 frames at the largest hold is 3600 timeline frames, well under the limit, so the
    // limit is reached with many long holds: 2000 frames at 60 is 120000.
    let mut session = StopMotionSession::new(12, 60).expect("valid");
    for _ in 0..2_000 {
        session.add_frame().expect("room");
    }
    assert_eq!(session.duration(), 120_000);
    assert_eq!(
        timeline_slots(&session),
        Err(StopMotionError::TimelineTooLong {
            frames: 120_000,
            max: MAX_TIMELINE_FRAMES
        })
    );
}
