//! Taking a range of frames out, and reversing it.

use super::*;

fn spans(f: &Fixture) -> Vec<(u32, u32)> {
    layer(f)
        .spans()
        .iter()
        .map(|s| (s.start(), s.duration().get()))
        .collect()
}

#[test]
fn taking_frames_out_shortens_what_they_cut_drops_what_they_cover_and_pulls_the_rest_left() {
    // A: 0..3, B: 3..6, C: 6..8.
    let mut f = fixture(vec![shape_span(0, 3), shape_span(3, 3), shape_span(6, 2)]);
    let ids = first_ids(layer(&f));
    // Frames 2..=4: the last frame of A and the first two of B.
    let commands = remove_frames(f.scene, layer(&f), 2, 4);
    run(&mut f, commands);
    assert_eq!(spans(&f), vec![(0, 2), (2, 1), (3, 2)]);
    assert_eq!(first_ids(layer(&f)), ids, "the same keyframes, shorter");
    assert!(f.session.undo());
    assert_eq!(spans(&f), vec![(0, 3), (3, 3), (6, 2)], "one undo step");

    // Frames 3..=5 are all of B: it goes, and C comes up behind A.
    let commands = remove_frames(f.scene, layer(&f), 3, 5);
    run(&mut f, commands);
    assert_eq!(spans(&f), vec![(0, 3), (3, 2)]);
    assert_eq!(first_ids(layer(&f)), vec![ids[0], ids[2]]);
}

#[test]
fn taking_out_nothing_is_refused_and_cues_in_the_range_go_and_those_after_it_move() {
    let mut f = fixture(vec![shape_span(0, 4), shape_span(4, 4)]);
    assert!(
        remove_frames(f.scene, layer(&f), 3, 1).is_none(),
        "backwards"
    );
    assert!(
        remove_frames(f.scene, layer(&f), 20, 30).is_none(),
        "past the layer"
    );
    let sound = flare_core::SoundId::new();
    let data = flare_document::Sound::new(sound, "s", vec![1], 8000, 1, 8).expect("sound");
    f.session.apply([Command::AddSound(data)]).expect("sound");
    let cues = [0, 2, 6]
        .into_iter()
        .map(|start| SoundCue { start, sound })
        .collect();
    f.session
        .apply([Command::SetCues {
            scene: f.scene,
            layer: layer(&f).id(),
            cues,
        }])
        .expect("cues");
    let commands = remove_frames(f.scene, layer(&f), 1, 3);
    run(&mut f, commands);
    let starts: Vec<u32> = layer(&f).cues().iter().map(|c| c.start).collect();
    assert_eq!(
        starts,
        vec![0, 3],
        "the cue inside went, the one after moved left by three"
    );
}

#[test]
fn reversing_puts_the_last_keyframe_first_with_the_gaps_mirrored() {
    // A: 0..1, B: 1..3, gap at 3, C: 4..5.
    let mut f = fixture(vec![shape_span(0, 1), shape_span(1, 2), shape_span(4, 1)]);
    let ids = first_ids(layer(&f));
    let commands = reverse_frames(f.scene, layer(&f), 0, 4).expect("reverses");
    f.session.apply(commands).expect("applies");
    assert_eq!(spans(&f), vec![(0, 1), (2, 2), (4, 1)], "C, gap, B, A");
    assert_eq!(first_ids(layer(&f)), vec![ids[2], ids[1], ids[0]]);
    assert!(f.session.undo());
    assert_eq!(first_ids(layer(&f)), ids, "one undo step");
}

#[test]
fn reversing_leaves_what_is_outside_the_range_and_refuses_a_range_that_cuts_a_keyframe() {
    let mut f = fixture(vec![
        shape_span(0, 2),
        shape_span(2, 1),
        shape_span(3, 1),
        shape_span(4, 2),
    ]);
    let ids = first_ids(layer(&f));
    let commands = reverse_frames(f.scene, layer(&f), 2, 3).expect("reverses");
    f.session.apply(commands).expect("applies");
    assert_eq!(first_ids(layer(&f)), vec![ids[0], ids[2], ids[1], ids[3]]);
    assert_eq!(spans(&f), vec![(0, 2), (2, 1), (3, 1), (4, 2)]);
    // Frames 1..=2 cut the first keyframe (0..2) in two.
    assert_eq!(
        reverse_frames(f.scene, layer(&f), 1, 2),
        Err(ReverseRefusal::CutsAKeyframe)
    );
    assert_eq!(
        reverse_frames(f.scene, layer(&f), 3, 2),
        Err(ReverseRefusal::NothingThere)
    );
    assert_eq!(
        reverse_frames(f.scene, layer(&f), 40, 50),
        Err(ReverseRefusal::NothingThere)
    );
    // Every refusal says why.
    assert!(!ReverseRefusal::CutsAKeyframe.to_string().is_empty());
}

#[test]
fn a_tween_goes_with_the_pair_it_joined_with_its_easing_reversed() {
    // A tweens into B with ease 30; B tweens into C with ease -50; C has a tween out of the range.
    let a = shape_span(0, 2).with_tween(Tween::Shape).with_ease(30);
    let b = shape_span(2, 2).with_tween(Tween::Shape).with_ease(-50);
    let c = shape_span(4, 2).with_tween(Tween::Shape).with_ease(80);
    let mut f = fixture(vec![a, b, c]);
    let commands = reverse_frames(f.scene, layer(&f), 0, 5).expect("reverses");
    f.session.apply(commands).expect("applies");
    // The order is now C, B, A. C leads into B as B led into C, so it takes B's tween with the
    // easing the other way round; B leads into A the way A led into B; A, last, leads nowhere.
    let motion: Vec<(Tween, i8)> = layer(&f)
        .spans()
        .iter()
        .map(|s| (s.tween(), s.ease()))
        .collect();
    assert_eq!(
        motion,
        vec![(Tween::Shape, 50), (Tween::Shape, -30), (Tween::None, 0),]
    );
}

#[test]
fn a_tween_keeps_its_curve_mirrored_when_reversed() {
    use flare_document::SpanInterp;
    use flare_tween::{Easing, HandleKind, Interpolation, Params};
    let curve = SpanInterp::new(
        Interpolation::Quad,
        Easing::In,
        HandleKind::AutoClamped,
        [0.0; 4],
        Params::default(),
    )
    .expect("valid");
    let a = shape_span(0, 2)
        .with_tween(Tween::Shape)
        .with_interp(Some(curve));
    let b = shape_span(2, 2);
    let mut f = fixture(vec![a, b]);
    let commands = reverse_frames(f.scene, layer(&f), 0, 3).expect("reverses");
    f.session.apply(commands).expect("applies");
    let spans = layer(&f).spans();
    assert_eq!(spans[0].interp(), Some(curve.mirrored()));
    assert_eq!(spans[0].interp().map(|i| i.easing()), Some(Easing::Out));
    assert_eq!(spans[1].interp(), None);
}
