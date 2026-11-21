#![allow(clippy::expect_used)]

use std::num::NonZeroU32;

use flare_core::{LayerId, ShapeId};
use flare_document::{Content, FrameSpan, Layer, Shape, Timeline};

use super::*;

fn span(start: u32, duration: u32) -> FrameSpan {
    let shape =
        Shape::rect(ShapeId::new(), 0.0, 0.0, 5.0, 5.0, flare_core::Rgba8::WHITE).expect("rect");
    FrameSpan::new(
        start,
        NonZeroU32::new(duration).expect("nonzero"),
        Content::Shape(shape),
    )
}

/// Keyframes at 0, 4 and 9 (the last two frames long) on one layer; 11 frames.
fn timeline() -> Timeline {
    let layer = Layer::new(
        LayerId::new(),
        "L",
        vec![span(0, 4), span(4, 5), span(9, 2)],
    )
    .expect("layer");
    Timeline::new(vec![layer])
}

#[test]
fn cells_mark_keyframes_and_stop_at_the_end() {
    let cells = cells(&timeline(), 3, 100);
    assert_eq!(cells.len(), 8, "frames 3 to 10");
    assert_eq!(cells[0].frame, 3);
    assert_eq!(cells[0].keyframes, 0);
    assert_eq!(cells[1].keyframes, 1, "frame 4 starts a keyframe");
    assert!(cells.iter().all(|c| c.shows_art));
    assert_eq!(
        super::cells(&timeline(), 50, 5),
        vec![],
        "nothing past the end"
    );
}

#[test]
fn a_request_for_cells_is_bounded() {
    let long = Layer::new(LayerId::new(), "L", vec![span(0, 100_000)]).expect("layer");
    let cells = cells(&Timeline::new(vec![long]), 0, u32::MAX);
    assert_eq!(cells.len(), MAX_CELLS as usize);
}

#[test]
fn goto_words_and_numbers_are_parsed_and_the_rest_refused() {
    assert_eq!("12".parse::<Goto>(), Ok(Goto::Frame(12)));
    assert_eq!(" Next-Keyframe ".parse::<Goto>(), Ok(Goto::NextKeyframe));
    assert_eq!("last".parse::<Goto>(), Ok(Goto::Last));
    assert!(matches!("-3".parse::<Goto>(), Err(GotoError::Unknown(_))));
    assert!(matches!("soon".parse::<Goto>(), Err(GotoError::Unknown(_))));
}

#[test]
fn goto_lands_on_frames_and_keyframes_and_refuses_to_leave_the_scene() {
    let t = timeline();
    assert_eq!(resolve(&t, 2, Goto::Frame(7)), Ok(7));
    assert_eq!(resolve(&t, 2, Goto::NextKeyframe), Ok(4));
    assert_eq!(resolve(&t, 4, Goto::NextKeyframe), Ok(9));
    assert_eq!(resolve(&t, 7, Goto::PreviousKeyframe), Ok(4));
    assert_eq!(resolve(&t, 0, Goto::First), Ok(0));
    assert_eq!(resolve(&t, 0, Goto::Last), Ok(10));
    assert_eq!(
        resolve(&t, 0, Goto::Frame(11)),
        Err(GotoError::PastTheEnd {
            frame: 11,
            frames: 11
        })
    );
    assert_eq!(
        resolve(&t, 9, Goto::NextKeyframe),
        Err(GotoError::NothingThere)
    );
    assert_eq!(resolve(&t, 0, Goto::Previous), Err(GotoError::NothingThere));
    assert_eq!(resolve(&t, 10, Goto::Next), Err(GotoError::NothingThere));
    assert_eq!(
        resolve(&Timeline::default(), 0, Goto::First),
        Err(GotoError::NoFrames)
    );
}
