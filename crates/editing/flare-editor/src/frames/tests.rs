#![allow(clippy::expect_used)]

use flare_core::{DocumentId, FrameRate, LayerId, Rgba8};
use flare_document::{Document, Scene, Timeline, Tween};

use super::*;
use flare_history::EditorSession;

struct Fixture {
    session: EditorSession,
    scene: SceneId,
}

fn shape_span(start: u32, duration: u32) -> FrameSpan {
    let shape = Shape::rect(ShapeId::new(), 0.0, 0.0, 5.0, 5.0, Rgba8::WHITE).expect("rect");
    FrameSpan::new(
        start,
        NonZeroU32::new(duration).expect("nonzero"),
        Content::Shape(shape),
    )
}

fn fixture(spans: Vec<FrameSpan>) -> Fixture {
    let (scene, layer) = (SceneId::new(), LayerId::new());
    let layer_value = Layer::new(layer, "L", spans).expect("layer");
    let scene_value = Scene::new(scene, "S", Timeline::new(vec![layer_value]));
    let rate = FrameRate::new(24, 1).expect("rate");
    let doc = Document::new(DocumentId::new(), rate, vec![scene_value], vec![]).expect("doc");
    Fixture {
        session: EditorSession::new(doc),
        scene,
    }
}

fn layer(f: &Fixture) -> &Layer {
    &f.session.document().scenes()[0].timeline().layers()[0]
}

/// (start, duration, kind) where kind is 'S' shape, 'E' empty.
fn shape_of(f: &Fixture) -> Vec<(u32, u32, char)> {
    layer(f)
        .spans()
        .iter()
        .map(|s| {
            let kind = match s.content() {
                Content::Empty => 'E',
                Content::Shape(_) => 'S',
                Content::Instance(_) => 'I',
            };
            (s.start(), s.duration().get(), kind)
        })
        .collect()
}

fn run(f: &mut Fixture, commands: Option<Vec<Command>>) {
    f.session
        .apply(commands.expect("operation applies"))
        .expect("valid result");
}

#[test]
fn insert_frame_lengthens_the_span_and_shifts_later_spans() {
    let mut f = fixture(vec![shape_span(0, 5), shape_span(5, 3)]);
    let cmds = insert_frame(f.scene, layer(&f), 2);
    run(&mut f, cmds);
    assert_eq!(shape_of(&f), vec![(0, 6, 'S'), (6, 3, 'S')]);
    f.session.undo();
    assert_eq!(
        shape_of(&f),
        vec![(0, 5, 'S'), (5, 3, 'S')],
        "one undo step"
    );
}

#[test]
fn insert_frame_past_the_end_extends_and_gaps_are_left_alone() {
    let mut f = fixture(vec![shape_span(0, 3), shape_span(6, 2)]);
    assert!(
        insert_frame(f.scene, layer(&f), 4).is_none(),
        "inside a gap"
    );
    let cmds = insert_frame(f.scene, layer(&f), 9);
    run(&mut f, cmds);
    assert_eq!(shape_of(&f), vec![(0, 3, 'S'), (6, 4, 'S')]);
    let empty = fixture(vec![]);
    assert!(insert_frame(empty.scene, layer(&empty), 0).is_none());
}

#[test]
fn remove_frame_shortens_shifts_and_drops_single_frame_spans() {
    let mut f = fixture(vec![shape_span(0, 5), shape_span(5, 1), shape_span(6, 4)]);
    let cmds = remove_frame(f.scene, layer(&f), 1);
    run(&mut f, cmds);
    assert_eq!(shape_of(&f), vec![(0, 4, 'S'), (4, 1, 'S'), (5, 4, 'S')]);
    let cmds = remove_frame(f.scene, layer(&f), 4);
    run(&mut f, cmds);
    assert_eq!(shape_of(&f), vec![(0, 4, 'S'), (4, 4, 'S')]);
    assert!(remove_frame(f.scene, layer(&f), 50).is_none());
}

#[test]
fn insert_keyframe_splits_with_a_copy_that_has_fresh_ids() {
    let mut f = fixture(vec![shape_span(0, 10)]);
    let original = match layer(&f).spans()[0].content() {
        Content::Shape(s) => s.id(),
        _ => panic!("shape"),
    };
    assert!(
        insert_keyframe(f.scene, layer(&f), 0).is_none(),
        "already a keyframe"
    );
    let cmds = insert_keyframe(f.scene, layer(&f), 4);
    run(&mut f, cmds);
    assert_eq!(shape_of(&f), vec![(0, 4, 'S'), (4, 6, 'S')]);
    let ids: Vec<ShapeId> = layer(&f)
        .spans()
        .iter()
        .map(|s| match s.content() {
            Content::Shape(shape) => shape.id(),
            _ => panic!("shape"),
        })
        .collect();
    assert_eq!(ids[0], original);
    assert_ne!(ids[0], ids[1], "the copy has its own identity");
}

#[test]
fn convert_to_keyframes_makes_one_copy_per_frame_with_fresh_identities() {
    let mut f = fixture(vec![shape_span(0, 4)]);
    assert!(
        convert_to_keyframes(f.scene, layer(&f), 9).is_none(),
        "no span there"
    );
    let cmds = convert_to_keyframes(f.scene, layer(&f), 2);
    run(&mut f, cmds);
    assert_eq!(
        shape_of(&f),
        vec![(0, 1, 'S'), (1, 1, 'S'), (2, 1, 'S'), (3, 1, 'S')]
    );
    let ids: std::collections::HashSet<_> = layer(&f)
        .spans()
        .iter()
        .map(|s| match s.content() {
            Content::Shape(shape) => shape.id(),
            _ => panic!("shape"),
        })
        .collect();
    assert_eq!(ids.len(), 4, "every copy has its own identity");
    assert!(
        convert_to_keyframes(f.scene, layer(&f), 1).is_none(),
        "a one-frame keyframe has nothing to convert"
    );
}

#[test]
fn keyframes_past_the_end_hold_then_add_a_one_frame_keyframe() {
    let mut f = fixture(vec![shape_span(0, 3)]);
    let cmds = insert_keyframe(f.scene, layer(&f), 6);
    run(&mut f, cmds);
    assert_eq!(shape_of(&f), vec![(0, 6, 'S'), (6, 1, 'S')]);
    let cmds = insert_blank_keyframe(f.scene, layer(&f), 9);
    run(&mut f, cmds);
    assert_eq!(shape_of(&f), vec![(0, 6, 'S'), (6, 3, 'S'), (9, 1, 'E')]);
    let gap = fixture(vec![shape_span(0, 2), shape_span(5, 2)]);
    assert!(insert_keyframe(gap.scene, layer(&gap), 3).is_none());
}

#[test]
fn blank_keyframes_empty_the_new_part_only() {
    let mut f = fixture(vec![shape_span(0, 10)]);
    let cmds = insert_blank_keyframe(f.scene, layer(&f), 3);
    run(&mut f, cmds);
    assert_eq!(shape_of(&f), vec![(0, 3, 'S'), (3, 7, 'E')]);
}

#[test]
fn clear_keyframe_merges_into_the_previous_span_or_empties_the_first() {
    let mut f = fixture(vec![shape_span(0, 3), shape_span(3, 4)]);
    let cmds = clear_keyframe(f.scene, layer(&f), 3);
    run(&mut f, cmds);
    assert_eq!(shape_of(&f), vec![(0, 7, 'S')]);
    let cmds = clear_keyframe(f.scene, layer(&f), 0);
    run(&mut f, cmds);
    assert_eq!(
        shape_of(&f),
        vec![(0, 7, 'E')],
        "no previous span: it becomes empty"
    );
    assert!(
        clear_keyframe(f.scene, layer(&f), 2).is_none(),
        "not a keyframe start"
    );
    // A gap before the keyframe means there is nothing to absorb it.
    let mut gap = fixture(vec![shape_span(0, 2), shape_span(5, 2)]);
    let cmds = clear_keyframe(gap.scene, layer(&gap), 5);
    run(&mut gap, cmds);
    assert_eq!(shape_of(&gap), vec![(0, 2, 'S'), (5, 2, 'E')]);
}

#[test]
fn tweens_survive_splits_and_shifts() {
    let first = shape_span(0, 6).with_tween(Tween::Linear);
    let mut f = fixture(vec![first, shape_span(6, 2)]);
    let cmds = insert_keyframe(f.scene, layer(&f), 3);
    run(&mut f, cmds);
    let tweens: Vec<Tween> = layer(&f).spans().iter().map(FrameSpan::tween).collect();
    assert_eq!(tweens, vec![Tween::Linear, Tween::Linear, Tween::None]);
    let cmds = insert_frame(f.scene, layer(&f), 0);
    run(&mut f, cmds);
    assert_eq!(layer(&f).spans()[0].tween(), Tween::Linear);
}

#[test]
fn overflowing_frame_numbers_do_not_panic() {
    let f = fixture(vec![shape_span(0, 3)]);
    assert!(insert_frame(f.scene, layer(&f), u32::MAX).is_none());
    // A keyframe at the far end is representable; the layer simply becomes very long.
    assert!(insert_keyframe(f.scene, layer(&f), u32::MAX).is_some());
}

#[test]
fn move_span_shifts_one_span_and_refuses_overlaps() {
    let mut f = fixture(vec![shape_span(0, 3), shape_span(6, 2)]);
    let cmds = move_span(f.scene, layer(&f), 0, 2);
    run(&mut f, cmds);
    assert_eq!(shape_of(&f), vec![(2, 3, 'S'), (6, 2, 'S')]);
    assert!(
        move_span(f.scene, layer(&f), 2, 4).is_none(),
        "4..7 meets 6..8"
    );
    assert!(
        move_span(f.scene, layer(&f), 2, 3).is_some(),
        "3..6 just touches"
    );
    assert!(move_span(f.scene, layer(&f), 2, 2).is_none(), "no movement");
    assert!(
        move_span(f.scene, layer(&f), 1, 9).is_none(),
        "no span starts at 1"
    );
    assert!(
        move_span(f.scene, layer(&f), 6, u32::MAX).is_none(),
        "past the last frame"
    );
    // Moving past a neighbour is allowed when the destination is free, and the list
    // stays sorted.
    let cmds = move_span(f.scene, layer(&f), 2, 10);
    run(&mut f, cmds);
    assert_eq!(shape_of(&f), vec![(6, 2, 'S'), (10, 3, 'S')]);
    f.session.undo();
    assert_eq!(
        shape_of(&f),
        vec![(2, 3, 'S'), (6, 2, 'S')],
        "one undo step"
    );
}

#[test]
fn resize_span_changes_only_the_length_and_stops_at_the_next_span() {
    let mut f = fixture(vec![shape_span(0, 3), shape_span(6, 2)]);
    let cmds = resize_span(f.scene, layer(&f), 0, 6);
    run(&mut f, cmds);
    assert_eq!(shape_of(&f), vec![(0, 6, 'S'), (6, 2, 'S')]);
    assert!(
        resize_span(f.scene, layer(&f), 0, 7).is_none(),
        "would overlap"
    );
    assert!(
        resize_span(f.scene, layer(&f), 0, 0).is_none(),
        "zero length"
    );
    assert!(resize_span(f.scene, layer(&f), 0, 6).is_none(), "unchanged");
    let cmds = resize_span(f.scene, layer(&f), 6, 1);
    run(&mut f, cmds);
    assert_eq!(
        shape_of(&f),
        vec![(0, 6, 'S'), (6, 1, 'S')],
        "shrinking is fine"
    );
}

#[test]
fn a_new_keyframe_copies_the_stroke_and_gradient_of_the_artwork() {
    use flare_core::DocumentPoint;
    use flare_document::{Gradient, Stroke};
    let point = |x| DocumentPoint::new(x, 0.0).expect("finite");
    let gradient = Gradient::linear(point(0.0), point(5.0), Rgba8::WHITE, Rgba8::TRANSPARENT)
        .expect("gradient");
    let stroke = Stroke::new(2.0, Rgba8::WHITE).expect("stroke");
    let shape = Shape::rect(ShapeId::new(), 0.0, 0.0, 5.0, 5.0, Rgba8::WHITE)
        .expect("rect")
        .with_stroke(stroke)
        .with_gradient(gradient);
    let span = FrameSpan::new(
        0,
        NonZeroU32::new(6).expect("nonzero"),
        Content::Shape(shape),
    );
    let mut f = fixture(vec![span]);
    let cmds = insert_keyframe(f.scene, layer(&f), 3);
    run(&mut f, cmds);
    let spans = layer(&f).spans();
    assert_eq!(spans.len(), 2);
    let Content::Shape(copy) = spans[1].content() else {
        panic!("a shape expected")
    };
    assert_eq!(copy.gradient(), Some(gradient));
    assert_eq!(copy.stroke(), Some(stroke));
}

fn first_ids(layer: &Layer) -> Vec<flare_core::ShapeId> {
    layer
        .spans()
        .iter()
        .filter_map(|s| match s.content() {
            Content::Shape(shape) => Some(shape.id()),
            _ => None,
        })
        .collect()
}

#[test]
fn copying_cuts_spans_to_the_range_and_makes_the_start_frame_zero() {
    let f = fixture(vec![shape_span(0, 5), shape_span(5, 3), shape_span(10, 2)]);
    let clip = copy_frames(layer(&f), 3, 6).expect("clip");
    assert_eq!(clip.length(), 4);
    let shape: Vec<(u32, u32)> = clip
        .spans()
        .iter()
        .map(|s| (s.start(), s.duration().get()))
        .collect();
    assert_eq!(
        shape,
        vec![(0, 2), (2, 2)],
        "frames 3..5 of the first, 5..7 of the second"
    );
    // Only a gap: nothing to copy. Backwards: nothing either.
    assert!(copy_frames(layer(&f), 8, 9).is_none());
    assert!(copy_frames(layer(&f), 6, 3).is_none());
    // A range running past the end still copies what is there.
    let tail = copy_frames(layer(&f), 10, 40).expect("clip");
    assert_eq!(tail.length(), 31);
    assert_eq!(tail.spans().len(), 1);
}

#[test]
fn pasting_replaces_the_covered_frames_and_keeps_the_rest_in_place() {
    let mut f = fixture(vec![shape_span(0, 4), shape_span(10, 4)]);
    let clip = copy_frames(layer(&f), 10, 13).expect("clip"); // one 4-frame span
    let cmds = paste_frames(f.scene, layer(&f), 2, &clip);
    run(&mut f, cmds);
    // 0..2 stays, the paste covers 2..6 (cutting the first span's tail), 10..14 stays.
    assert_eq!(shape_of(&f), vec![(0, 2, 'S'), (2, 4, 'S'), (10, 4, 'S')]);
    f.session.undo();
    assert_eq!(
        shape_of(&f),
        vec![(0, 4, 'S'), (10, 4, 'S')],
        "one undo step"
    );
}

#[test]
fn a_paste_inside_a_span_splits_it_and_every_piece_has_its_own_identity() {
    let mut f = fixture(vec![shape_span(0, 12), shape_span(20, 2)]);
    let clip = copy_frames(layer(&f), 20, 21).expect("clip");
    let cmds = paste_frames(f.scene, layer(&f), 4, &clip);
    run(&mut f, cmds);
    assert_eq!(
        shape_of(&f),
        vec![(0, 4, 'S'), (4, 2, 'S'), (6, 6, 'S'), (20, 2, 'S')]
    );
    let ids = first_ids(layer(&f));
    let unique: std::collections::HashSet<_> = ids.iter().collect();
    assert_eq!(ids.len(), unique.len(), "no two pieces share a shape id");
}

#[test]
fn pasting_past_the_end_or_over_a_gap_works_and_overflow_is_refused() {
    let mut f = fixture(vec![shape_span(0, 2)]);
    let clip = copy_frames(layer(&f), 0, 1).expect("clip");
    let cmds = paste_frames(f.scene, layer(&f), 8, &clip);
    run(&mut f, cmds);
    assert_eq!(shape_of(&f), vec![(0, 2, 'S'), (8, 2, 'S')]);
    assert!(paste_frames(f.scene, layer(&f), u32::MAX, &clip).is_none());
    assert!(paste_frames(f.scene, layer(&f), u32::MAX - 1, &clip).is_none());
    assert!(paste_frames(f.scene, layer(&f), u32::MAX - 2, &clip).is_some());
}

#[test]
fn a_tween_cut_off_from_its_target_is_not_copied() {
    use flare_document::Tween;
    let mut first = shape_span(0, 4);
    first = first.with_tween(Tween::Linear);
    let f = fixture(vec![first, shape_span(4, 4)]);
    let whole = copy_frames(layer(&f), 0, 3).expect("clip");
    assert_eq!(
        whole.spans()[0].tween(),
        Tween::Linear,
        "the span is copied whole"
    );
    let cut = copy_frames(layer(&f), 0, 1).expect("clip");
    assert_eq!(
        cut.spans()[0].tween(),
        Tween::None,
        "cut short, its target is gone"
    );
}

#[test]
fn sound_cues_after_an_inserted_or_removed_frame_move_with_the_spans() {
    let mut f = fixture(vec![shape_span(0, 5), shape_span(5, 5)]);
    let sound = flare_core::SoundId::new();
    let data = flare_document::Sound::new(sound, "s", vec![1], 8000, 1, 8).expect("sound");
    f.session.apply([Command::AddSound(data)]).expect("sound");
    let cues = vec![
        SoundCue { start: 0, sound },
        SoundCue { start: 3, sound },
        SoundCue { start: 7, sound },
    ];
    f.session
        .apply([Command::SetCues {
            scene: f.scene,
            layer: layer(&f).id(),
            cues,
        }])
        .expect("cues");
    let starts = |f: &Fixture| -> Vec<u32> { layer(f).cues().iter().map(|c| c.start).collect() };
    let cmds = insert_frame(f.scene, layer(&f), 3);
    run(&mut f, cmds);
    assert_eq!(
        starts(&f),
        vec![0, 3, 8],
        "only the cue after the frame moved"
    );
    let cmds = remove_frame(f.scene, layer(&f), 3);
    run(&mut f, cmds);
    assert_eq!(starts(&f), vec![0, 3, 7], "and it moves back");
    f.session.undo();
    assert_eq!(
        starts(&f),
        vec![0, 3, 8],
        "one undo step each, spans and cues together"
    );
}

#[test]
fn cues_need_a_sound_and_sounds_in_use_cannot_be_removed() {
    let mut f = fixture(vec![shape_span(0, 5)]);
    let missing = flare_core::SoundId::new();
    let bad = Command::SetCues {
        scene: f.scene,
        layer: layer(&f).id(),
        cues: vec![SoundCue {
            start: 1,
            sound: missing,
        }],
    };
    assert!(f.session.apply([bad]).is_err(), "no such sound");
    assert!(layer(&f).cues().is_empty(), "nothing changed");

    let sound = flare_core::SoundId::new();
    let data = flare_document::Sound::new(sound, "s", vec![1], 8000, 1, 8).expect("sound");
    f.session.apply([Command::AddSound(data)]).expect("add");
    f.session
        .apply([Command::SetCues {
            scene: f.scene,
            layer: layer(&f).id(),
            cues: vec![SoundCue { start: 1, sound }],
        }])
        .expect("cue");
    assert!(
        f.session.apply([Command::RemoveSound(sound)]).is_err(),
        "still played"
    );
    assert!(f.session.apply([Command::RemoveSound(missing)]).is_err());
    // Taking the cue away and the sound out in one step is fine.
    f.session
        .apply([
            Command::SetCues {
                scene: f.scene,
                layer: layer(&f).id(),
                cues: vec![],
            },
            Command::RemoveSound(sound),
        ])
        .expect("both");
    assert!(f.session.document().sounds().is_empty());
}
