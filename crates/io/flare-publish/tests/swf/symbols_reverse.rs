//! Animate's two reverse modes in a published movie: Loop Reverse and Play Once Reverse count
//! down from the first frame, and the SWF shows the same frames the document does.

use flare_core::SymbolId;
use flare_document::{PlayMode, Symbol};

use super::symbols::{playback, playing, sprites, steps};
use super::*;

/// The three-frame symbol played backwards three ways, each on a row of its own.
fn reversed() -> Document {
    let graphic = SymbolId::new();
    let row = |y: f64, mode: PlayMode, first: u32| {
        layer(
            "row",
            vec![span(0, 7, playing(graphic, 0.0, y, playback(mode, first)))],
        )
    };
    let scene = Scene::new(
        SceneId::new(),
        "S",
        Timeline::new(vec![
            row(0.0, PlayMode::LoopReverse, 1),
            row(14.0, PlayMode::PlayOnceReverse, 2),
            row(28.0, PlayMode::PlayOnceReverse, 0),
        ]),
    );
    document(
        40,
        40,
        vec![scene],
        vec![Symbol::new(graphic, "Graphic", steps())],
    )
}

#[test]
fn reverse_modes_play_in_the_swf_as_they_do_in_the_document() {
    let doc = reversed();
    let published = publish_swf(&doc, &PublishOptions::default()).expect("publish");
    let parsed = parse(&published.bytes);
    let background = doc.stage().background();
    for frame in 0..7 {
        let (max, mean) = difference(
            &render_original(&doc, 0, frame),
            &render_swf_frame(&parsed, frame as usize, background),
        );
        assert!(
            max <= 8 && mean < 0.05,
            "frame {frame}: max {max}, mean {mean}"
        );
    }
    // Counting down is not counting up: frames 1 and 2 of the loop differ from frame 0.
    assert_ne!(
        render_swf_frame(&parsed, 0, background).rgba(),
        render_swf_frame(&parsed, 1, background).rgba()
    );
}

#[test]
fn a_reverse_loop_has_every_frame_and_playing_once_in_reverse_stops() {
    let parsed = parse(
        &publish_swf(&reversed(), &PublishOptions::default())
            .expect("publish")
            .bytes,
    );
    let mut found: Vec<(u16, usize)> = sprites(&parsed).iter().map(|s| (s.1, s.2)).collect();
    found.sort_unstable();
    // Play once in reverse from frame 0 is one frame and needs no stop; from frame 2 it is three
    // frames ending on a stop; the reverse loop is all three frames with no stop.
    assert_eq!(found, vec![(1, 0), (3, 0), (3, 1)], "{found:?}");
}
