//! The frame mapping of an instance: bounded, deterministic, defined for every input.
#![allow(clippy::expect_used)]

use flare_document::{PlayMode, Playback, SymbolKind};
use flare_eval::symbol_frame;
use proptest::prelude::*;

fn pb(mode: PlayMode, first_frame: u32) -> Playback {
    Playback { mode, first_frame }
}

#[test]
fn every_mode_before_at_and_beyond_the_end() {
    let g = SymbolKind::Graphic;
    assert_eq!(symbol_frame(g, pb(PlayMode::Loop, 2), 0, 5), 2);
    assert_eq!(symbol_frame(g, pb(PlayMode::Loop, 2), 3, 5), 0);
    assert_eq!(symbol_frame(g, pb(PlayMode::PlayOnce, 2), 1, 5), 3);
    assert_eq!(symbol_frame(g, pb(PlayMode::PlayOnce, 2), 99, 5), 4);
    assert_eq!(symbol_frame(g, pb(PlayMode::SingleFrame, 2), 99, 5), 2);
    // A first frame beyond the end is clamped to the last frame.
    assert_eq!(symbol_frame(g, pb(PlayMode::SingleFrame, 50), 0, 5), 4);
    assert_eq!(symbol_frame(g, pb(PlayMode::Loop, 50), 0, 5), 4);
}

#[test]
fn the_reverse_modes_count_down_from_the_first_frame() {
    let g = SymbolKind::Graphic;
    let reverse_loop: Vec<u32> = (0..7)
        .map(|e| symbol_frame(g, pb(PlayMode::LoopReverse, 2), e, 5))
        .collect();
    assert_eq!(reverse_loop, vec![2, 1, 0, 4, 3, 2, 1]);
    let reverse_once: Vec<u32> = (0..5)
        .map(|e| symbol_frame(g, pb(PlayMode::PlayOnceReverse, 2), e, 5))
        .collect();
    assert_eq!(reverse_once, vec![2, 1, 0, 0, 0]);
    // A first frame beyond the end starts from the last frame, as the forward modes do.
    assert_eq!(symbol_frame(g, pb(PlayMode::LoopReverse, 50), 1, 5), 3);
    assert_eq!(symbol_frame(g, pb(PlayMode::PlayOnceReverse, 50), 99, 5), 0);
    // The far end of the clock does not overflow.
    assert_eq!(
        symbol_frame(g, pb(PlayMode::LoopReverse, 0), u64::MAX, 5),
        0
    );
}

#[test]
fn a_movie_clip_runs_its_own_clock_and_a_button_holds_up() {
    let p = pb(PlayMode::SingleFrame, 3);
    assert_eq!(symbol_frame(SymbolKind::MovieClip, p, 7, 5), 2);
    assert_eq!(symbol_frame(SymbolKind::Button, p, 7, 5), 0);
}

#[test]
fn an_empty_symbol_shows_frame_zero_without_dividing_by_zero() {
    for kind in SymbolKind::ALL {
        for mode in PlayMode::ALL {
            assert_eq!(symbol_frame(kind, pb(mode, 4), 10, 0), 0);
        }
    }
}

proptest! {
    #[test]
    fn the_frame_is_always_inside_the_symbol(
        kind in 0usize..3, mode in 0usize..PlayMode::ALL.len(),
        first in any::<u32>(), elapsed in any::<u64>(), length in 1u64..100_000,
    ) {
        let frame = symbol_frame(
            SymbolKind::ALL[kind], pb(PlayMode::ALL[mode], first), elapsed, length);
        prop_assert!(u64::from(frame) < length);
    }

    #[test]
    fn play_once_never_goes_back(first in 0u32..50, length in 1u64..50, elapsed in 0u64..200) {
        let g = SymbolKind::Graphic;
        let a = symbol_frame(g, pb(PlayMode::PlayOnce, first), elapsed, length);
        let b = symbol_frame(g, pb(PlayMode::PlayOnce, first), elapsed + 1, length);
        prop_assert!(b >= a);
    }

    #[test]
    fn play_once_reverse_never_goes_forward(first in 0u32..50, length in 1u64..50, elapsed in 0u64..200) {
        let g = SymbolKind::Graphic;
        let a = symbol_frame(g, pb(PlayMode::PlayOnceReverse, first), elapsed, length);
        let b = symbol_frame(g, pb(PlayMode::PlayOnceReverse, first), elapsed + 1, length);
        prop_assert!(b <= a);
    }

    #[test]
    fn loop_reverse_retraces_loop(first in 0u32..50, length in 1u64..50, elapsed in 0u64..200) {
        // Going back `elapsed` frames and then forward `elapsed` frames lands on the start.
        let g = SymbolKind::Graphic;
        let back = symbol_frame(g, pb(PlayMode::LoopReverse, first), elapsed, length);
        let start = symbol_frame(g, pb(PlayMode::Loop, first), 0, length);
        prop_assert_eq!(symbol_frame(g, pb(PlayMode::Loop, back), elapsed, length), start);
    }

    #[test]
    fn loop_repeats_with_the_symbol_length(first in 0u32..50, length in 1u64..50, elapsed in 0u64..200) {
        let g = SymbolKind::Graphic;
        prop_assert_eq!(
            symbol_frame(g, pb(PlayMode::Loop, first), elapsed, length),
            symbol_frame(g, pb(PlayMode::Loop, first), elapsed + length, length));
    }
}
