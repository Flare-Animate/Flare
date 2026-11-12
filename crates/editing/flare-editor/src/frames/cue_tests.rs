//! Sound cues and playback options through frame edits.

use super::*;

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
fn frames_added_to_a_sound_layer_leave_its_cues_where_they_are() {
    let mut f = fixture(vec![]);
    let sound = flare_core::SoundId::new();
    let data = flare_document::Sound::new(sound, "s", vec![1], 8000, 1, 8).expect("sound");
    f.session.apply([Command::AddSound(data)]).expect("sound");
    let cues = vec![SoundCue { start: 2, sound }, SoundCue { start: 30, sound }];
    f.session
        .apply([Command::SetCues {
            scene: f.scene,
            layer: layer(&f).id(),
            cues,
        }])
        .expect("cues");
    let cmds = insert_frame(f.scene, layer(&f), 24);
    run(&mut f, cmds);
    assert_eq!(shape_of(&f), vec![(0, 25, 'E')]);
    let starts: Vec<u32> = layer(&f).cues().iter().map(|c| c.start).collect();
    assert_eq!(starts, vec![2, 30], "nothing was inserted before them");
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

#[test]
fn a_new_keyframe_of_a_graphic_keeps_how_it_plays() {
    use flare_core::{InstanceId, SymbolId, Transform2D};
    use flare_document::{Instance, PlayMode, Playback, Symbol};
    let symbol = SymbolId::new();
    let playback = Playback {
        mode: PlayMode::PlayOnce,
        first_frame: 2,
    };
    let instance =
        Instance::new(InstanceId::new(), symbol, Transform2D::IDENTITY).with_playback(playback);
    let span = FrameSpan::new(
        0,
        NonZeroU32::new(8).expect("nonzero"),
        Content::Instance(instance),
    );
    let (scene, layer_id) = (SceneId::new(), LayerId::new());
    let layer_value = Layer::new(layer_id, "L", vec![span]).expect("layer");
    let scene_value = Scene::new(scene, "S", Timeline::new(vec![layer_value]));
    let doc = Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![scene_value],
        vec![Symbol::new(symbol, "Walk", Timeline::default())],
    )
    .expect("doc");
    let mut f = Fixture {
        session: EditorSession::new(doc),
        scene,
    };
    let commands = insert_keyframe(f.scene, layer(&f), 4);
    run(&mut f, commands);
    let playbacks: Vec<Playback> = layer(&f)
        .spans()
        .iter()
        .map(|s| match s.content() {
            Content::Instance(i) => i.playback(),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(
        playbacks,
        [playback, playback],
        "the copy plays like the original"
    );
}
