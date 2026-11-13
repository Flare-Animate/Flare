use super::*;
use proptest::prelude::*;

#[test]
fn without_a_range_the_whole_scene_loops_or_playback_stops_at_the_end() {
    assert_eq!(advance(3, 1, 10, true, None), (4, true));
    assert_eq!(advance(9, 1, 10, true, None), (0, true));
    assert_eq!(advance(8, 25, 10, true, None), (3, true));
    assert_eq!(advance(8, 1, 10, false, None), (9, true));
    assert_eq!(advance(9, 1, 10, false, None), (9, false));
    assert_eq!(advance(4, 99, 10, false, None), (9, false));
}

#[test]
fn a_range_loops_over_its_own_frames() {
    let range = Some(LoopRange::new(2, 5));
    let frames: Vec<u32> = (0..8)
        .scan(2, |f, _| {
            let (next, playing) = advance(*f, 1, 10, true, range);
            assert!(playing);
            *f = next;
            Some(next)
        })
        .collect();
    assert_eq!(frames, vec![3, 4, 5, 2, 3, 4, 5, 2]);
    // Several frames in one step (a slow UI frame) land where single steps would.
    assert_eq!(advance(4, 6, 10, true, range), (2, true));
}

#[test]
fn a_playhead_outside_the_range_is_brought_into_it() {
    let range = Some(LoopRange::new(2, 5));
    // Before it: plays on into it.
    assert_eq!(advance(0, 1, 10, true, range), (1, true));
    assert_eq!(advance(0, 7, 10, true, range), (3, true));
    // After it: the next frame is its start.
    assert_eq!(advance(8, 1, 10, true, range), (2, true));
    assert_eq!(advance(8, 3, 10, true, range), (4, true));
}

#[test]
fn the_range_does_nothing_when_not_looping() {
    let range = Some(LoopRange::new(2, 5));
    assert_eq!(advance(5, 1, 10, false, range), (6, true));
    assert_eq!(advance(9, 1, 10, false, range), (9, false));
}

#[test]
fn a_range_is_cut_to_the_scene_and_one_past_its_end_is_none() {
    assert_eq!(LoopRange::new(4, 20).within(10), Some(LoopRange::new(4, 9)));
    assert_eq!(LoopRange::new(12, 20).within(10), None);
    assert_eq!(LoopRange::new(0, 0).within(0), None);
    // A range past the end of a scene that shrank loops the whole scene.
    assert_eq!(
        advance(9, 1, 10, true, Some(LoopRange::new(12, 20))),
        (0, true)
    );
}

#[test]
fn an_empty_scene_stays_on_frame_zero() {
    assert_eq!(advance(5, 3, 0, true, None), (0, true));
    assert_eq!(advance(5, 3, 0, false, None), (0, false));
}

#[test]
fn dragging_an_end_sets_the_range_and_the_whole_scene_is_no_range() {
    let dragged = LoopRange::dragged(None, LoopEnd::End, 5, 10);
    assert_eq!(dragged, Some(LoopRange::new(0, 5)));
    let dragged = LoopRange::dragged(dragged, LoopEnd::Start, 2, 10);
    assert_eq!(dragged, Some(LoopRange::new(2, 5)));
    // Past the other end: the ends swap.
    assert_eq!(
        LoopRange::dragged(dragged, LoopEnd::Start, 8, 10),
        Some(LoopRange::new(5, 8))
    );
    // Past the scene: held at its last frame.
    assert_eq!(
        LoopRange::dragged(dragged, LoopEnd::End, 400, 10),
        Some(LoopRange::new(2, 9))
    );
    assert_eq!(
        LoopRange::dragged(Some(LoopRange::new(0, 5)), LoopEnd::End, 400, 10),
        None,
        "dragged back to the whole scene"
    );
    assert_eq!(LoopRange::dragged(None, LoopEnd::End, 3, 0), None);
}

proptest! {
    #[test]
    fn looping_always_lands_inside_the_range_and_the_scene(
        frame in 0u32..200, steps in 1u64..10_000, frames in 1u32..150,
        a in 0u32..200, b in 0u32..200,
    ) {
        let range = LoopRange::new(a, b);
        let (next, playing) = advance(frame, steps, frames, true, Some(range));
        prop_assert!(playing);
        prop_assert!(next < frames);
        // A playhead already inside the range stays inside it.
        if let Some(r) = range.within(frames)
            && r.from() <= frame
            && frame <= r.to()
        {
            prop_assert!(r.from() <= next && next <= r.to());
        }
    }

    #[test]
    fn steps_add_up(frame in 0u32..50, x in 1u64..40, y in 1u64..40, a in 0u32..50, b in 0u32..50) {
        // Two steps in a row land where one step of both would, once inside the range.
        let range = Some(LoopRange::new(a, b));
        let (mid, _) = advance(frame, x, 50, true, range);
        let r = LoopRange::new(a, b);
        prop_assume!(r.from() <= mid && mid <= r.to() && r.from() <= frame && frame <= r.to());
        prop_assert_eq!(advance(mid, y, 50, true, range), advance(frame, x + y, 50, true, range));
    }
}
