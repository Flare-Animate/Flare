#![allow(clippy::expect_used)]

use flare_core::{DocumentId, FrameRate, LayerId, SceneId, SoundId};

use crate::{Document, Layer, Scene, Sound, SoundCue, Timeline};

fn named(name: &str) -> Layer {
    Layer::new(LayerId::new(), name, vec![]).expect("layer")
}

fn folder(name: &str) -> Layer {
    named(name).with_folder(true)
}

fn sound(n: u8) -> Sound {
    Sound::new(SoundId::new(), format!("s{n}"), vec![n; 4], 8000, 1, 4).expect("sound")
}

#[test]
fn a_sound_played_only_by_one_layer_is_found_and_a_shared_one_is_not() {
    let (own, shared) = (sound(1), sound(2));
    let cue = |start, sound: &Sound| SoundCue {
        start,
        sound: sound.id(),
    };
    let a = named("A")
        .with_cues(vec![cue(0, &own), cue(4, &shared)])
        .expect("cues");
    let b = named("B").with_cues(vec![cue(0, &shared)]).expect("cues");
    let a_id = a.id();
    let document = Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![Scene::new(SceneId::new(), "S", Timeline::new(vec![a, b]))],
        vec![],
    )
    .expect("document")
    .with_sounds(vec![own.clone(), shared.clone()])
    .expect("sounds");
    assert_eq!(document.sounds_only_played_by(a_id), vec![own.id()]);
    assert!(document.sounds_only_played_by(LayerId::new()).is_empty());
}

#[test]
fn only_folders_above_a_layer_and_not_its_own_are_offered() {
    let (top, own, below) = (folder("Top"), folder("Own"), folder("Below"));
    let layer = named("L").with_parent(Some(own.id()));
    let (top_id, layer_id) = (top.id(), layer.id());
    let timeline = Timeline::new(vec![top, own, layer, below]);
    let offered: Vec<_> = timeline
        .folders_for(layer_id)
        .into_iter()
        .map(|l| (l.id(), l.name().to_owned()))
        .collect();
    assert_eq!(offered, vec![(top_id, "Top".to_owned())]);
    assert!(timeline.folders_for(LayerId::new()).is_empty());
    // A layer with no folder above it is offered none.
    assert!(timeline.folders_for(top_id).is_empty());
}
