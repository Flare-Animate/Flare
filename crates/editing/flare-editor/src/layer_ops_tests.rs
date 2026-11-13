#![allow(clippy::expect_used)]

use std::num::NonZeroU32;

use flare_core::{DocumentId, FrameRate, Rgba8, ShapeId};
use flare_document::{FrameSpan, Scene, Shape, Timeline, Tween};
use flare_history::EditorSession;

use super::*;

fn shapes(start: u32, duration: u32) -> FrameSpan {
    let shape = Shape::rect(ShapeId::new(), 0.0, 0.0, 5.0, 5.0, Rgba8::WHITE).expect("rect");
    FrameSpan::new(
        start,
        NonZeroU32::new(duration).expect("nonzero"),
        Content::Shape(shape),
    )
}

/// Top to bottom: folder `F` holding `A` and `B`, then `C`, then `D`, all drawn on except the
/// folder.
fn session() -> (EditorSession, SceneId, Vec<LayerId>) {
    let ids: Vec<LayerId> = (0..5).map(|_| LayerId::new()).collect();
    let art = |i: usize, name: &str, parent: Option<LayerId>| {
        Layer::new(
            ids[i],
            name,
            vec![shapes(0, 3), shapes(3, 2).with_tween(Tween::Linear)],
        )
        .expect("layer")
        .with_parent(parent)
    };
    let layers = vec![
        Layer::new(ids[0], "F", vec![])
            .expect("folder")
            .with_folder(true),
        art(1, "A", Some(ids[0])),
        art(2, "B", Some(ids[0])),
        art(3, "C", None),
        art(4, "D", None),
    ];
    let scene = SceneId::new();
    let rate = FrameRate::new(24, 1).expect("rate");
    let doc = Document::new(
        DocumentId::new(),
        rate,
        vec![Scene::new(scene, "S", Timeline::new(layers))],
        vec![],
    )
    .expect("doc");
    (EditorSession::new(doc), scene, ids)
}

fn layers(s: &EditorSession) -> &[Layer] {
    s.document().scenes()[0].timeline().layers()
}

fn flags(s: &EditorSession, f: fn(&Layer) -> bool) -> Vec<bool> {
    layers(s).iter().map(f).collect()
}

#[test]
fn hide_others_keeps_the_layer_and_its_folder_and_is_one_undo_step() {
    let (mut s, scene, ids) = session();
    let commands = hide_others(s.document(), scene, ids[1]).expect("hide");
    s.apply(commands).expect("valid");
    assert_eq!(
        flags(&s, Layer::is_visible),
        vec![true, true, false, false, false]
    );
    assert!(
        hide_others(s.document(), scene, ids[1])
            .expect("again")
            .is_empty(),
        "nothing left to change"
    );
    s.undo();
    assert!(flags(&s, Layer::is_visible).iter().all(|v| *v));
    // Show All brings them back, and has nothing to do when they all show.
    s.redo();
    s.apply(show_all(s.document(), scene)).expect("valid");
    assert!(flags(&s, Layer::is_visible).iter().all(|v| *v));
    assert!(show_all(s.document(), scene).is_empty());
}

#[test]
fn hide_others_on_a_folder_keeps_what_it_holds_and_shows_the_layer_if_hidden() {
    let (mut s, scene, ids) = session();
    s.apply(vec![Command::UpdateLayer {
        scene,
        layer: ids[3],
        change: LayerChange::Visible(false),
    }])
    .expect("hide C");
    s.apply(hide_others(s.document(), scene, ids[0]).expect("hide"))
        .expect("valid");
    assert_eq!(
        flags(&s, Layer::is_visible),
        vec![true, true, true, false, false]
    );
    s.apply(hide_others(s.document(), scene, ids[3]).expect("hide"))
        .expect("valid");
    assert_eq!(
        flags(&s, Layer::is_visible),
        vec![false, false, false, true, false],
        "C is shown again, everything else hidden"
    );
}

#[test]
fn lock_others_locks_the_rest_and_unlocks_the_layer_and_its_folder() {
    let (mut s, scene, ids) = session();
    s.apply(vec![Command::UpdateLayer {
        scene,
        layer: ids[0],
        change: LayerChange::Locked(true),
    }])
    .expect("lock F");
    s.apply(lock_others(s.document(), scene, ids[2]).expect("lock"))
        .expect("valid");
    assert_eq!(
        flags(&s, Layer::is_locked),
        vec![false, true, false, true, true]
    );
}

#[test]
fn an_unknown_layer_is_refused() {
    let (s, scene, _) = session();
    let nobody = LayerId::new();
    assert_eq!(
        hide_others(s.document(), scene, nobody),
        Err(LayerOpRefusal::NoSuchLayer)
    );
    assert_eq!(
        lock_others(s.document(), scene, nobody),
        Err(LayerOpRefusal::NoSuchLayer)
    );
    assert_eq!(
        duplicate(s.document(), scene, nobody).map(|_| ()),
        Err(LayerOpRefusal::NoSuchLayer)
    );
}

#[test]
fn duplicating_a_layer_puts_a_copy_with_new_elements_above_it() {
    let (mut s, scene, ids) = session();
    let (commands, copy) = duplicate(s.document(), scene, ids[3]).expect("duplicate");
    s.apply(commands).expect("the copy is a valid document");
    let names: Vec<&str> = layers(&s).iter().map(Layer::name).collect();
    assert_eq!(names, vec!["F", "A", "B", "C copy", "C", "D"]);
    let (copied, original) = (&layers(&s)[3], &layers(&s)[4]);
    assert_eq!(copied.id(), copy);
    assert_eq!(copied.spans().len(), original.spans().len());
    for (a, b) in copied.spans().iter().zip(original.spans()) {
        assert_eq!(
            (a.start(), a.duration(), a.tween()),
            (b.start(), b.duration(), b.tween())
        );
        let (Content::Shape(x), Content::Shape(y)) = (a.content(), b.content()) else {
            panic!("shapes");
        };
        assert_ne!(x.id(), y.id(), "every element its own");
        assert_eq!(x.path(), y.path());
    }
    // Again: the next name is numbered.
    s.apply(duplicate(s.document(), scene, ids[3]).expect("again").0)
        .expect("valid");
    assert!(layers(&s).iter().any(|l| l.name() == "C copy 2"));
    s.undo();
    s.undo();
    assert_eq!(layers(&s).len(), 5, "each duplicate is one undo step");
}

#[test]
fn duplicating_a_folder_copies_what_it_holds_into_the_copy() {
    let (mut s, scene, ids) = session();
    let (commands, copy) = duplicate(s.document(), scene, ids[0]).expect("duplicate");
    s.apply(commands).expect("valid");
    let shown: Vec<(&str, Option<LayerId>)> =
        layers(&s).iter().map(|l| (l.name(), l.parent())).collect();
    assert_eq!(
        shown,
        vec![
            ("F copy", None),
            ("A", Some(copy)),
            ("B", Some(copy)),
            ("F", None),
            ("A", Some(ids[0])),
            ("B", Some(ids[0])),
            ("C", None),
            ("D", None),
        ]
    );
    assert!(layers(&s)[0].is_folder());
}

#[test]
fn the_camera_is_not_duplicated() {
    let (mut s, scene, _) = session();
    let camera = LayerId::new();
    s.apply(vec![Command::AddLayer {
        scene,
        layer: Layer::new(camera, "Camera", vec![])
            .expect("layer")
            .with_camera(true),
    }])
    .expect("camera");
    assert_eq!(
        duplicate(s.document(), scene, camera).map(|_| ()),
        Err(LayerOpRefusal::Camera)
    );
}
