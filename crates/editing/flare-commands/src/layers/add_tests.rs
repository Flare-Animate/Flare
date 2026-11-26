#![allow(clippy::expect_used)]

use super::*;
use crate::apply_all;
use flare_core::{DocumentId, FrameRate, SceneId};
use flare_document::{Scene, Timeline};

fn named(name: &str) -> Layer {
    Layer::new(LayerId::new(), name, vec![]).expect("layer")
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
    let name = |id| {
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

fn add(document: &Document, scene: SceneId, under: Option<LayerId>) -> Document {
    let commands = add_under(document, scene, named("New"), under).expect("add");
    apply_all(document, commands)
        .expect("apply")
        .expect("changed")
}

fn plain(names: &[&str]) -> Vec<(String, Option<String>)> {
    names.iter().map(|n| ((*n).to_owned(), None)).collect()
}

#[test]
fn a_new_layer_goes_just_under_the_one_worked_on() {
    let (a, b, c) = (named("A"), named("B"), named("C"));
    let under = a.id();
    let (document, scene) = build(vec![a, b, c]);
    let done = add(&document, scene, Some(under));
    assert_eq!(order(&done), plain(&["A", "New", "B", "C"]));
    // Under the last layer, and with none named, it is at the bottom.
    let last = document.scenes()[0].timeline().layers()[2].id();
    assert_eq!(
        order(&add(&document, scene, Some(last))),
        plain(&["A", "B", "C", "New"])
    );
    assert_eq!(
        order(&add(&document, scene, None)),
        plain(&["A", "B", "C", "New"])
    );
}

#[test]
fn under_a_layer_in_a_folder_it_joins_the_folder_and_under_a_folder_it_follows_its_contents() {
    let folder = named("F").with_folder(true);
    let inner = named("In").with_parent(Some(folder.id()));
    let (f, i) = (folder.id(), inner.id());
    let (document, scene) = build(vec![folder, inner, named("Out")]);
    let into = |name: &str, parent: Option<&str>| (name.to_owned(), parent.map(str::to_owned));
    assert_eq!(
        order(&add(&document, scene, Some(i))),
        vec![
            into("F", None),
            into("In", Some("F")),
            into("New", Some("F")),
            into("Out", None),
        ]
    );
    assert_eq!(
        order(&add(&document, scene, Some(f))),
        vec![
            into("F", None),
            into("In", Some("F")),
            into("New", None),
            into("Out", None),
        ]
    );
}

#[test]
fn a_mask_keeps_the_layer_it_masks() {
    let mask = named("Mask").with_mask(true);
    let id = mask.id();
    let (document, scene) = build(vec![mask, named("Masked"), named("Below")]);
    assert_eq!(
        order(&add(&document, scene, Some(id))),
        plain(&["Mask", "Masked", "New", "Below"])
    );
}

#[test]
fn a_layer_not_in_the_scene_is_refused_and_nothing_is_built() {
    let (document, scene) = build(vec![named("A")]);
    assert_eq!(
        add_under(&document, scene, named("New"), Some(LayerId::new())),
        Err(LayerRefusal::NoSuchLayer)
    );
}
