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

fn dropped(f: &mut Fixture, from: usize, start: u32, onto: usize, to: u32) {
    let commands =
        drop_span(f.scene, &layer(f, from), start, &layer(f, onto), to).expect("the drop applies");
    f.session.apply(commands).expect("valid");
}

#[test]
fn a_keyframe_dropped_on_empty_frames_moves_there_and_leaves_its_old_frames_empty() {
    let mut f = fixture(vec![vec![drawn(0, 2, 10)]]);
    dropped(&mut f, 0, 0, 0, 5);
    assert_eq!(spans(&f, 0), [(5, 2, 10)]);
}

#[test]
fn a_keyframe_dropped_on_another_replaces_it() {
    let mut f = fixture(vec![vec![
        drawn(0, 2, 10),
        drawn(2, 2, 20),
        drawn(4, 2, 30),
    ]]);
    dropped(&mut f, 0, 4, 0, 0);
    assert_eq!(spans(&f, 0), [(0, 2, 30), (2, 2, 20)], "the first is gone");
    assert_eq!(f.session.undo_labels().count(), 1, "one undo step");
}

#[test]
fn a_drop_over_part_of_a_keyframe_keeps_the_frames_either_side_of_it() {
    let mut f = fixture(vec![vec![drawn(0, 10, 10), drawn(12, 2, 20)]]);
    dropped(&mut f, 0, 12, 0, 4);
    assert_eq!(spans(&f, 0), [(0, 4, 10), (4, 2, 20), (6, 4, 10)]);
    // The far part is a copy with its own identities, so the document stays valid.
    let ids: Vec<_> = layer(&f, 0)
        .spans()
        .iter()
        .filter_map(|s| match s.content() {
            Content::Shape(shape) => Some(shape.id()),
            _ => None,
        })
        .collect();
    assert_eq!(ids.len(), 3);
    assert!(ids[0] != ids[2]);
}

#[test]
fn a_keyframe_dropped_on_another_layer_moves_there_replacing_what_it_covers() {
    let mut f = fixture(vec![
        vec![drawn(0, 3, 10)],
        vec![drawn(0, 2, 20), drawn(2, 4, 30)],
    ]);
    dropped(&mut f, 0, 0, 1, 1);
    assert_eq!(spans(&f, 0), [], "it left the first layer");
    assert_eq!(spans(&f, 1), [(0, 1, 20), (1, 3, 10), (4, 2, 30)]);
}

#[test]
fn a_drop_that_moves_nothing_or_lands_on_a_folder_or_past_the_end_is_refused() {
    let f = fixture(vec![vec![drawn(0, 3, 10)], vec![]]);
    let (one, two) = (layer(&f, 0), layer(&f, 1));
    assert!(
        drop_span(f.scene, &one, 0, &one, 0).is_none(),
        "no movement"
    );
    assert!(
        drop_span(f.scene, &one, 1, &one, 5).is_none(),
        "no span starts there"
    );
    assert!(drop_span(f.scene, &one, 0, &one, u32::MAX - 1).is_none());
    let folder = two.clone().with_folder(true);
    assert!(drop_span(f.scene, &one, 0, &folder, 2).is_none());
    assert!(
        drop_span(f.scene, &one, 0, &two, 0).is_some(),
        "the same frames of another layer"
    );
}
