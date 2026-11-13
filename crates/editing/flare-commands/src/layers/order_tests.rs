#![allow(clippy::expect_used)]

use super::*;
use crate::apply_all;
use flare_core::{DocumentId, FrameRate, SceneId};
use flare_document::{Scene, Timeline};

fn named(name: &str) -> Layer {
    Layer::new(LayerId::new(), name, vec![]).expect("layer")
}

fn folder(name: &str) -> Layer {
    named(name).with_folder(true)
}

fn build(layers: Vec<Layer>) -> (Document, SceneId) {
    let scene = SceneId::new();
    let document = Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![Scene::new(scene, "S", Timeline::new(layers))],
        vec![],
    )
    .expect("document");
    (document, scene)
}

/// The layers' names top first, each with the name of the folder it is in.
fn order(document: &Document) -> Vec<(String, Option<String>)> {
    let layers = document.scenes()[0].timeline().layers();
    let name = |id: LayerId| {
        layers
            .iter()
            .find(|l| l.id() == id)
            .map(|l| l.name().to_owned())
    };
    layers
        .iter()
        .map(|l| (l.name().to_owned(), l.parent().and_then(name)))
        .collect()
}

fn names(document: &Document) -> Vec<String> {
    order(document).into_iter().map(|(n, _)| n).collect()
}

fn moved(
    document: &Document,
    scene: SceneId,
    layer: LayerId,
    before: Option<LayerId>,
    into: Option<LayerId>,
) -> Document {
    let commands = move_to(document, scene, layer, before, into).expect("commands");
    apply_all(document, commands)
        .expect("valid")
        .expect("something changed")
}

#[test]
fn a_layer_goes_above_any_other_or_to_the_bottom_in_one_batch() {
    let [a, b, c, d] = ["A", "B", "C", "D"].map(named);
    let (ids, (document, scene)) = ([a.id(), b.id(), c.id(), d.id()], build(vec![a, b, c, d]));
    let down = moved(&document, scene, ids[0], Some(ids[3]), None);
    assert_eq!(names(&down), ["B", "C", "A", "D"]);
    let up = moved(&document, scene, ids[3], Some(ids[1]), None);
    assert_eq!(names(&up), ["A", "D", "B", "C"]);
    let bottom = moved(&document, scene, ids[1], None, None);
    assert_eq!(names(&bottom), ["A", "C", "D", "B"]);
    let top = moved(&document, scene, ids[2], Some(ids[0]), None);
    assert_eq!(names(&top), ["C", "A", "B", "D"]);
}

#[test]
fn dropping_a_layer_where_it_already_is_changes_nothing() {
    let [a, b] = ["A", "B"].map(named);
    let ids = [a.id(), b.id()];
    let (document, scene) = build(vec![a, b]);
    assert_eq!(
        move_to(&document, scene, ids[0], Some(ids[1]), None),
        Ok(vec![])
    );
    assert_eq!(move_to(&document, scene, ids[1], None, None), Ok(vec![]));
    let to_bottom = move_to(&document, scene, ids[0], None, None).expect("commands");
    assert_eq!(to_bottom.len(), 1, "one step down is one move");
}

#[test]
fn a_folder_takes_everything_in_it_and_a_layer_goes_into_a_folder_and_out() {
    let f = folder("F");
    let inner = folder("G").with_parent(Some(f.id()));
    let x = named("X").with_parent(Some(f.id()));
    let y = named("Y").with_parent(Some(inner.id()));
    let (z, w) = (named("Z"), named("W"));
    let ids = (f.id(), inner.id(), x.id(), y.id(), z.id(), w.id());
    let (document, scene) = build(vec![z, f, inner, y, x, w]);

    let folder_down = moved(&document, scene, ids.0, None, None);
    assert_eq!(names(&folder_down), ["Z", "W", "F", "G", "Y", "X"]);
    let some = |s: &str| Some(s.to_owned());
    assert_eq!(order(&folder_down)[3], ("G".to_owned(), some("F")));
    assert_eq!(order(&folder_down)[4], ("Y".to_owned(), some("G")));

    // W into F, just above X.
    let into = moved(&document, scene, ids.5, Some(ids.2), Some(ids.0));
    assert_eq!(names(&into), ["Z", "F", "G", "Y", "W", "X"]);
    assert_eq!(order(&into)[4], ("W".to_owned(), some("F")));
    // And back out to the top.
    let out = moved(&into, scene, ids.5, Some(ids.4), None);
    assert_eq!(order(&out)[0], ("W".to_owned(), None));
    // Out of its folder without moving: only the folder changes.
    let commands = move_to(&document, scene, ids.2, Some(ids.5), None).expect("commands");
    assert_eq!(commands.len(), 1);
    let freed = apply_all(&document, commands)
        .expect("valid")
        .expect("changed");
    assert_eq!(order(&freed)[4], ("X".to_owned(), None));
}

#[test]
fn a_layer_cannot_go_inside_itself_or_into_a_folder_below_it_or_a_plain_layer() {
    let f = folder("F");
    let x = named("X").with_parent(Some(f.id()));
    let (a, b) = (named("A"), named("B"));
    let ids = (f.id(), x.id(), a.id(), b.id());
    let (document, scene) = build(vec![a, f, x, b]);
    let refused = |layer, before, into| move_to(&document, scene, layer, before, into);
    assert!(matches!(
        refused(ids.0, Some(ids.1), None),
        Err(LayerRefusal::Invalid(_))
    ));
    assert!(matches!(
        refused(ids.0, None, Some(ids.0)),
        Err(LayerRefusal::Invalid(_))
    ));
    // A above A's own place, but into F which would then be below it.
    assert!(matches!(
        refused(ids.3, Some(ids.2), Some(ids.0)),
        Err(LayerRefusal::Invalid(_))
    ));
    assert!(matches!(
        refused(ids.3, None, Some(ids.2)),
        Err(LayerRefusal::Invalid(_)),
    ));
    assert_eq!(
        refused(LayerId::new(), None, None),
        Err(LayerRefusal::NoSuchLayer)
    );
    assert_eq!(
        refused(ids.2, Some(LayerId::new()), None),
        Err(LayerRefusal::NoSuchLayer)
    );
}
