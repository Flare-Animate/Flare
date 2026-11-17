#![allow(clippy::expect_used)]

use flare_core::{DocumentId, FrameRate, LayerId, Rgba8};
use flare_document::{Document, Scene, Timeline};
use flare_history::EditorSession;

use super::*;

/// A shape span filled with red `tag`, so each can be told apart after a move.
fn drawn(start: u32, duration: u32, tag: u8) -> FrameSpan {
    let fill = Rgba8::opaque(tag, 0, 0);
    let shape = Shape::rect(ShapeId::new(), 0.0, 0.0, 5.0, 5.0, fill).expect("rect");
    FrameSpan::new(
        start,
        NonZeroU32::new(duration).expect("nonzero"),
        Content::Shape(shape),
    )
}

struct Fixture {
    session: EditorSession,
    scene: SceneId,
}

fn fixture(layers: Vec<Vec<FrameSpan>>) -> Fixture {
    let scene = SceneId::new();
    let layers = layers
        .into_iter()
        .enumerate()
        .map(|(i, spans)| Layer::new(LayerId::new(), format!("L{i}"), spans).expect("layer"))
        .collect();
    let rate = FrameRate::new(24, 1).expect("rate");
    let scenes = vec![Scene::new(scene, "S", Timeline::new(layers))];
    let doc = Document::new(DocumentId::new(), rate, scenes, vec![]).expect("doc");
    Fixture {
        session: EditorSession::new(doc),
        scene,
    }
}

fn layer(f: &Fixture, i: usize) -> Layer {
    f.session.document().scenes()[0].timeline().layers()[i].clone()
}

/// (start, duration, tag of the drawing; 0 for empty frames) of each span of layer `i`.
fn spans(f: &Fixture, i: usize) -> Vec<(u32, u32, u8)> {
    layer(f, i)
        .spans()
        .iter()
        .map(|s| {
            let tag = match s.content() {
                Content::Shape(shape) => shape.fill().r,
                _ => 0,
            };
            (s.start(), s.duration().get(), tag)
        })
        .collect()
}

fn placed(
    f: &mut Fixture,
    from: usize,
    range: (u32, u32),
    onto: usize,
    at: u32,
    how: FramePlacement,
) {
    let commands = place_frames(f.scene, &layer(f, from), range, &layer(f, onto), at, how)
        .expect("the frames go there");
    f.session.apply(commands).expect("valid");
}

#[test]
fn moving_several_keyframes_takes_their_gaps_along_and_leaves_their_frames_empty() {
    let mut f = fixture(vec![vec![
        drawn(0, 2, 10),
        drawn(4, 1, 20),
        drawn(9, 1, 30),
    ]]);
    placed(&mut f, 0, (0, 4), 0, 5, FramePlacement::Move);
    assert_eq!(
        spans(&f, 0),
        [(5, 2, 10), (9, 1, 20)],
        "the third keyframe was landed on and replaced"
    );
    assert_eq!(f.session.undo_labels().count(), 1, "one undo step");
    f.session.undo();
    assert_eq!(spans(&f, 0), [(0, 2, 10), (4, 1, 20), (9, 1, 30)]);
}

#[test]
fn copying_keeps_the_original_and_gives_the_copy_its_own_identities() {
    let mut f = fixture(vec![vec![drawn(0, 3, 10)], vec![drawn(0, 6, 40)]]);
    placed(&mut f, 0, (0, 2), 0, 3, FramePlacement::Copy);
    assert_eq!(spans(&f, 0), [(0, 3, 10), (3, 3, 10)]);
    let ids: Vec<_> = layer(&f, 0)
        .spans()
        .iter()
        .filter_map(|s| match s.content() {
            Content::Shape(shape) => Some(shape.id()),
            _ => None,
        })
        .collect();
    assert_ne!(ids[0], ids[1]);
    // Onto another layer, a copy cuts through what it lands on and keeps both ends of it.
    placed(&mut f, 0, (0, 1), 1, 2, FramePlacement::Copy);
    assert_eq!(spans(&f, 1), [(0, 2, 40), (2, 2, 10), (4, 2, 40)]);
    assert_eq!(spans(&f, 0), [(0, 3, 10), (3, 3, 10)], "the source is kept");
}

#[test]
fn moving_onto_another_layer_takes_the_frames_off_the_first() {
    let mut f = fixture(vec![vec![drawn(0, 4, 10)], vec![]]);
    // Only the middle two frames: the keyframe's ends stay, the far one with a new identity.
    placed(&mut f, 0, (1, 2), 1, 0, FramePlacement::Move);
    assert_eq!(spans(&f, 0), [(0, 1, 10), (3, 1, 10)]);
    assert_eq!(spans(&f, 1), [(0, 2, 10)]);
}

#[test]
fn placing_nothing_or_onto_a_folder_or_past_the_end_is_refused() {
    let f = fixture(vec![vec![drawn(2, 3, 10)], vec![]]);
    let (one, two) = (layer(&f, 0), layer(&f, 1));
    let place = |range, onto: &Layer, at, how| place_frames(f.scene, &one, range, onto, at, how);
    assert!(
        place((2, 4), &one, 2, FramePlacement::Move).is_none(),
        "no movement"
    );
    assert!(
        place((2, 4), &one, 2, FramePlacement::Copy).is_some(),
        "a copy over itself"
    );
    assert!(
        place((4, 2), &one, 8, FramePlacement::Move).is_none(),
        "backwards"
    );
    assert!(
        place((6, 9), &one, 0, FramePlacement::Move).is_none(),
        "only empty frames"
    );
    assert!(place((2, 4), &one, u32::MAX - 1, FramePlacement::Move).is_none());
    let folder = two.clone().with_folder(true);
    assert!(place((2, 4), &folder, 0, FramePlacement::Move).is_none());
}
