#![allow(clippy::expect_used)]

use super::*;
use crate::apply_all;
use flare_core::{DocumentId, FrameRate, SceneId};
use flare_document::{Scene, Sound, SoundCue, Timeline};

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

fn applied(document: &Document, commands: Vec<Command>) -> Document {
    apply_all(document, commands)
        .expect("valid")
        .expect("something changed")
}

fn layers(document: &Document) -> &[Layer] {
    document.scenes()[0].timeline().layers()
}

#[test]
fn each_toggle_flips_what_it_names_and_flips_back() {
    let layer = named("A");
    let id = layer.id();
    let (document, scene) = build(vec![layer]);
    type Toggle = fn(&Document, SceneId, LayerId) -> Result<Vec<Command>, LayerRefusal>;
    type Case = (Toggle, fn(&Layer) -> bool, bool);
    let cases: [Case; 4] = [
        (toggle_visible, Layer::is_visible, true),
        (toggle_locked, Layer::is_locked, false),
        (toggle_mask, Layer::is_mask, false),
        (toggle_guide, Layer::is_guide, false),
    ];
    for (toggle, read, initially) in cases {
        let once = applied(&document, toggle(&document, scene, id).expect("commands"));
        assert_eq!(read(&layers(&once)[0]), !initially);
        let twice = applied(&once, toggle(&once, scene, id).expect("commands"));
        assert_eq!(read(&layers(&twice)[0]), initially);
    }
    assert_eq!(
        toggle_visible(&document, scene, LayerId::new()),
        Err(LayerRefusal::NoSuchLayer)
    );
}

#[test]
fn a_layer_is_renamed_to_the_trimmed_name_and_never_to_nothing() {
    let layer = named("A");
    let id = layer.id();
    let (document, scene) = build(vec![layer]);
    let renamed = applied(&document, rename(scene, id, "  Sky  ").expect("commands"));
    assert_eq!(layers(&renamed)[0].name(), "Sky");
    assert_eq!(rename(scene, id, "   "), Err(LayerRefusal::NeedsAName));
}

#[test]
fn deleting_lets_a_folders_layers_out_and_keeps_at_least_one_layer() {
    let folder = Layer::new(LayerId::new(), "F", vec![])
        .expect("layer")
        .with_folder(true);
    let inside = named("In").with_parent(Some(folder.id()));
    let (folder_id, inside_id) = (folder.id(), inside.id());
    let (document, scene) = build(vec![folder, inside]);
    let after = applied(
        &document,
        delete(&document, scene, folder_id).expect("commands"),
    );
    assert_eq!(layers(&after).len(), 1);
    assert_eq!(layers(&after)[0].id(), inside_id);
    assert_eq!(
        layers(&after)[0].parent(),
        None,
        "let out of the deleted folder"
    );

    let (alone, alone_scene) = build(vec![named("Only")]);
    let only = layers(&alone)[0].id();
    assert_eq!(
        delete(&alone, alone_scene, only),
        Err(LayerRefusal::OnlyLayer)
    );
    assert_eq!(
        delete(&document, scene, LayerId::new()),
        Err(LayerRefusal::NoSuchLayer)
    );
}

#[test]
fn deleting_takes_the_sounds_only_that_layer_played_and_leaves_shared_ones() {
    let sound = |n: u8| {
        Sound::new(
            flare_core::SoundId::new(),
            format!("s{n}"),
            vec![n; 4],
            8000,
            1,
            4,
        )
        .expect("sound")
    };
    let (own, shared) = (sound(1), sound(2));
    let a = named("A")
        .with_cues(vec![
            SoundCue {
                start: 0,
                sound: own.id(),
            },
            SoundCue {
                start: 4,
                sound: shared.id(),
            },
        ])
        .expect("cues");
    let b = named("B")
        .with_cues(vec![SoundCue {
            start: 0,
            sound: shared.id(),
        }])
        .expect("cues");
    let a_id = a.id();
    let (base, scene) = build(vec![a, b]);
    let with_sounds = applied(
        &base,
        vec![
            Command::AddSound(own.clone()),
            Command::AddSound(shared.clone()),
        ],
    );
    assert_eq!(orphaned_sounds(&with_sounds, a_id), vec![own.id()]);
    let after = applied(
        &with_sounds,
        delete(&with_sounds, scene, a_id).expect("commands"),
    );
    let kept: Vec<_> = after.sounds().iter().map(|s| s.id()).collect();
    assert_eq!(kept, vec![shared.id()], "the shared sound stays");
}

#[test]
fn a_layer_moves_one_place_and_not_past_the_ends() {
    let (a, b, c) = (named("A"), named("B"), named("C"));
    let (a_id, b_id, c_id) = (a.id(), b.id(), c.id());
    let (document, scene) = build(vec![a, b, c]);
    let down = applied(
        &document,
        move_by(&document, scene, a_id, 1).expect("commands"),
    );
    let order: Vec<_> = layers(&down).iter().map(Layer::id).collect();
    assert_eq!(order, vec![b_id, a_id, c_id]);
    assert_eq!(
        move_by(&document, scene, a_id, -1),
        Err(LayerRefusal::AtTop)
    );
    assert_eq!(
        move_by(&document, scene, c_id, 1),
        Err(LayerRefusal::AtBottom)
    );
}

#[test]
fn a_folder_goes_above_the_layer_it_holds_and_in_the_folder_that_layer_was_in() {
    let (a, b) = (named("A"), named("B"));
    let b_id = b.id();
    let (document, scene) = build(vec![a, b]);
    let (commands, folder) = add_folder(&document, scene, Some(b_id)).expect("folder");
    let after = applied(&document, commands);
    let order: Vec<&str> = layers(&after).iter().map(Layer::name).collect();
    assert_eq!(order, vec!["A", "Folder 1", "B"]);
    let held = layers(&after).iter().find(|l| l.id() == b_id).expect("B");
    assert_eq!(held.parent(), Some(folder));
    // With nothing to hold, the folder is at the top and empty.
    let (commands, _) = add_folder(&document, scene, None).expect("folder");
    let empty = applied(&document, commands);
    assert_eq!(layers(&empty)[0].name(), "Folder 1");
    assert!(layers(&empty).iter().all(|l| l.parent().is_none()));
}

#[test]
fn only_folders_above_a_layer_and_not_its_own_are_offered() {
    let top = Layer::new(LayerId::new(), "Top", vec![])
        .expect("layer")
        .with_folder(true);
    let own = Layer::new(LayerId::new(), "Own", vec![])
        .expect("layer")
        .with_folder(true);
    let layer = named("L").with_parent(Some(own.id()));
    let below = Layer::new(LayerId::new(), "Below", vec![])
        .expect("layer")
        .with_folder(true);
    let (top_id, layer_id) = (top.id(), layer.id());
    let (document, scene) = build(vec![top, own, layer, below]);
    let offered = folders_for(&document, scene, layer_id);
    assert_eq!(offered, vec![(top_id, "Top".to_owned())]);
    assert!(folders_for(&document, scene, LayerId::new()).is_empty());
    let moved = applied(&document, set_folder(scene, layer_id, None));
    assert_eq!(layers(&moved)[2].parent(), None);
}
