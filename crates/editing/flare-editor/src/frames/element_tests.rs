//! Frame operations keep every element of a keyframe.
#![allow(clippy::expect_used)]

use flare_core::{DocumentId, FrameRate, LayerId, Rgba8};
use flare_document::{Document, Scene, Timeline, Tween};
use flare_history::EditorSession;

use super::*;

fn rect(x: f64) -> Content {
    Content::Shape(Shape::rect(ShapeId::new(), x, 0.0, 5.0, 5.0, Rgba8::WHITE).expect("rect"))
}

fn three(start: u32, duration: u32) -> FrameSpan {
    FrameSpan::new(
        start,
        NonZeroU32::new(duration).expect("nonzero"),
        rect(0.0),
    )
    .with_extras(vec![rect(10.0), rect(20.0)])
    .expect("extras")
}

struct Fixture {
    session: EditorSession,
    scene: SceneId,
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

/// (start, duration, elements) of each span.
fn shape_of(f: &Fixture) -> Vec<(u32, u32, usize)> {
    layer(f)
        .spans()
        .iter()
        .map(|s| (s.start(), s.duration().get(), s.element_count()))
        .collect()
}

fn run(f: &mut Fixture, commands: Option<Vec<Command>>) {
    f.session
        .apply(commands.expect("operation applies"))
        .expect("apply");
}

#[test]
fn lengthening_shortening_and_resizing_keep_all_the_elements() {
    let mut f = fixture(vec![three(0, 4)]);
    let commands = insert_frame(f.scene, layer(&f), 1);
    run(&mut f, commands);
    assert_eq!(shape_of(&f), vec![(0, 5, 3)]);
    let commands = remove_frame(f.scene, layer(&f), 1);
    run(&mut f, commands);
    assert_eq!(shape_of(&f), vec![(0, 4, 3)]);
    let commands = resize_span(f.scene, layer(&f), 0, 9);
    run(&mut f, commands);
    assert_eq!(shape_of(&f), vec![(0, 9, 3)]);
    let commands = move_span(f.scene, layer(&f), 0, 3);
    run(&mut f, commands);
    assert_eq!(shape_of(&f), vec![(3, 9, 3)]);
}

#[test]
fn a_new_keyframe_copies_every_element_with_fresh_identities_and_a_blank_one_has_none() {
    let mut f = fixture(vec![three(0, 6)]);
    let commands = insert_keyframe(f.scene, layer(&f), 3);
    run(&mut f, commands);
    assert_eq!(shape_of(&f), vec![(0, 3, 3), (3, 3, 3)]);
    let ids: std::collections::HashSet<_> = layer(&f)
        .spans()
        .iter()
        .flat_map(|s| s.elements())
        .filter_map(|c| match c {
            Content::Shape(s) => Some(s.id()),
            _ => None,
        })
        .collect();
    assert_eq!(ids.len(), 6, "no identity is shared");

    let commands = insert_blank_keyframe(f.scene, layer(&f), 5);
    run(&mut f, commands);
    assert_eq!(shape_of(&f)[2], (5, 1, 1));
    assert!(matches!(layer(&f).spans()[2].content(), Content::Empty));
}

#[test]
fn converting_to_keyframes_clearing_and_removing_ranges_keep_the_elements() {
    let mut f = fixture(vec![three(0, 3)]);
    let commands = convert_to_keyframes(f.scene, layer(&f), 0);
    run(&mut f, commands);
    assert_eq!(shape_of(&f), vec![(0, 1, 3), (1, 1, 3), (2, 1, 3)]);
    let commands = clear_keyframe(f.scene, layer(&f), 1);
    run(&mut f, commands);
    assert_eq!(shape_of(&f), vec![(0, 2, 3), (2, 1, 3)]);
    let commands = remove_frames(f.scene, layer(&f), 0, 0);
    run(&mut f, commands);
    assert_eq!(shape_of(&f), vec![(0, 1, 3), (1, 1, 3)]);
}

#[test]
fn copying_pasting_and_reversing_frames_carry_every_element() {
    let mut f = fixture(vec![three(0, 2), three(2, 2)]);
    let clip = copy_frames(layer(&f), 1, 2).expect("clip");
    assert!(clip.spans().iter().all(|s| s.element_count() == 3));
    let commands = paste_frames(f.scene, layer(&f), 10, &clip);
    run(&mut f, commands);
    let pasted: Vec<_> = shape_of(&f).into_iter().filter(|s| s.0 >= 10).collect();
    assert_eq!(pasted, vec![(10, 1, 3), (11, 1, 3)]);
    let reversed = reverse_frames(f.scene, layer(&f), 0, 3).expect("reverse");
    f.session.apply(reversed).expect("apply");
    assert!(layer(&f).spans().iter().all(|s| s.element_count() == 3));
    // A span that tweened keeps its tween only when it held one element: these never did.
    assert!(layer(&f).spans().iter().all(|s| s.tween() == Tween::None));
}
