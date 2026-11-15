#![allow(clippy::expect_used)]

use std::num::NonZeroU32;

use flare_commands::{Command, LayerChange};
use flare_core::{InstanceId, LayerId, SoundId, SymbolId, Transform2D};
use flare_document::{
    Content, FrameSpan, Instance, Layer, PlayMode, Sound, SoundCue, Symbol, SymbolKind, Timeline,
};

use super::*;
use crate::test_support::*;
use Viseme::*;

/// A ten-frame mouth symbol of `kind`, one instance of it on the fixture's layer from `start` for
/// `duration` frames, and a sound cued at `cue` on a layer of its own.
fn mouth(kind: SymbolKind, start: u32, duration: u32, cue: u32) -> (Fixture, SoundId, InstanceId) {
    let mut f = fixture();
    let symbol = SymbolId::new();
    let drawings = (0..10)
        .map(|i| rect_span(i, 1, 0.0, 0.0, 10.0, 10.0))
        .collect();
    let inner = Layer::new(LayerId::new(), "Mouths", drawings).expect("layer");
    let sound = SoundId::new();
    let voice = Sound::new(sound, "Voice", vec![1, 2, 3], 8000, 1, 8000).expect("sound");
    let sound_layer = Layer::new(LayerId::new(), "Voice", vec![])
        .expect("layer")
        .with_cues(vec![SoundCue { start: cue, sound }])
        .expect("cues");
    let id = InstanceId::new();
    let instance = Instance::new(id, symbol, Transform2D::IDENTITY);
    f.session
        .apply([
            Command::AddSymbol(
                Symbol::new(symbol, "Mouth", Timeline::new(vec![inner])).with_kind(kind),
            ),
            Command::AddSound(voice),
            Command::AddLayer {
                scene: f.scene,
                layer: sound_layer,
            },
        ])
        .expect("setup");
    add(
        &mut f,
        FrameSpan::new(
            start,
            NonZeroU32::new(duration).expect("nonzero"),
            Content::Instance(instance),
        ),
    );
    (f, sound, id)
}

/// (start, duration, first frame) of every key on the fixture's layer, checking each is a Single
/// Frame instance.
fn keys(f: &Fixture) -> Vec<(u32, u32, u32)> {
    let layer = f.session.document().scenes()[0]
        .timeline()
        .layers()
        .iter()
        .find(|l| l.id() == f.layer)
        .expect("layer");
    layer
        .spans()
        .iter()
        .map(|span| {
            let Content::Instance(i) = span.content() else {
                panic!("{:?}", span.content());
            };
            assert_eq!(i.playback().mode, PlayMode::SingleFrame);
            (
                span.start(),
                span.duration().get(),
                i.playback().first_frame,
            )
        })
        .collect()
}

#[test]
fn a_sound_becomes_one_single_frame_key_per_change_of_mouth_in_one_undo_step() {
    let (mut f, sound, id) = mouth(SymbolKind::Graphic, 0, 3, 2);
    let start = sound_start(f.session.document(), f.scene, sound).expect("cued");
    assert_eq!(start, 2);
    let shapes = [Ai, Ai, Mbp, O, O, O];
    let made = lip_sync(
        f.session.document(),
        f.scene,
        at(&f, 0),
        start,
        &shapes,
        &MouthMap::in_order(),
    )
    .expect("synced");
    assert_eq!((made.start, made.frames, made.keyframes), (0, 8, 4));
    let before = f.session.document().clone();
    f.session.apply(made.commands).expect("apply");
    // Rest before the sound (frames 0-1), then AI, MBP, O; it grew to the sound's end at 8.
    assert_eq!(keys(&f), vec![(0, 2, 0), (2, 2, 2), (4, 1, 1), (5, 3, 4)]);
    let layer = &f.session.document().scenes()[0].timeline().layers()[0];
    let Content::Instance(first) = layer.spans()[0].content() else {
        panic!("instance");
    };
    assert_eq!(first.id(), id, "the first key keeps the instance");
    assert!(f.session.undo(), "undo");
    assert_eq!(f.session.document(), &before);
}

#[test]
fn two_shapes_on_one_drawing_are_one_key_and_a_short_symbol_holds_its_last_frame() {
    let (f, _, _) = mouth(SymbolKind::Graphic, 0, 4, 0);
    let map = MouthMap::in_order().with(Mbp, 7).with(O, 7).with(U, 40);
    let made = lip_sync(
        f.session.document(),
        f.scene,
        at(&f, 0),
        0,
        &[Mbp, O, U, Neutral],
        &map,
    )
    .expect("synced");
    let mut f = f;
    f.session.apply(made.commands).expect("apply");
    assert_eq!(keys(&f), vec![(0, 2, 7), (2, 1, 9), (3, 1, 0)]);
}

#[test]
fn a_span_longer_than_the_sound_rests_after_it_and_the_next_key_is_not_overwritten() {
    let (mut f, _, _) = mouth(SymbolKind::Graphic, 0, 10, 0);
    let made = lip_sync(
        f.session.document(),
        f.scene,
        at(&f, 0),
        0,
        &[Ai, Ai],
        &MouthMap::in_order(),
    )
    .expect("synced");
    f.session.apply(made.commands).expect("apply");
    assert_eq!(keys(&f), vec![(0, 2, 2), (2, 8, 0)]);

    let (mut f, _, _) = mouth(SymbolKind::Graphic, 0, 2, 0);
    add(&mut f, rect_span(4, 3, 0.0, 0.0, 5.0, 5.0));
    let made = lip_sync(
        f.session.document(),
        f.scene,
        at(&f, 0),
        0,
        &[E; 9],
        &MouthMap::in_order(),
    )
    .expect("synced");
    assert_eq!(made.frames, 4, "stops at the key at frame 4");
    f.session.apply(made.commands).expect("apply");
}

#[test]
fn a_very_long_span_is_synced_without_walking_every_frame() {
    let (f, _, _) = mouth(SymbolKind::Graphic, 0, u32::MAX - 1, 5);
    let made = lip_sync(
        f.session.document(),
        f.scene,
        at(&f, 0),
        5,
        &[Ai],
        &MouthMap::in_order(),
    )
    .expect("synced");
    assert_eq!(made.keyframes, 3);
    assert_eq!(made.frames, u32::MAX - 1);
}

#[test]
fn what_cannot_be_lip_synced_is_refused_with_the_reason() {
    let (f, sound, _) = mouth(SymbolKind::MovieClip, 0, 3, 0);
    let sync = |f: &Fixture, at: SpanRef, start: u32| {
        lip_sync(
            f.session.document(),
            f.scene,
            at,
            start,
            &[Ai],
            &MouthMap::in_order(),
        )
    };
    assert_eq!(sync(&f, at(&f, 0), 0), Err(LipSyncError::NotAGraphic));
    assert_eq!(sync(&f, at(&f, 1), 0), Err(LipSyncError::NoSpan));
    assert_eq!(
        sound_start(f.session.document(), f.scene, SoundId::new()),
        None
    );
    assert!(sound_start(f.session.document(), f.scene, sound).is_some());

    let (mut f, _, _) = mouth(SymbolKind::Graphic, 5, 3, 0);
    assert_eq!(sync(&f, at(&f, 5), 0), Err(LipSyncError::NothingToSync));
    f.session
        .apply([Command::UpdateLayer {
            scene: f.scene,
            layer: f.layer,
            change: LayerChange::Locked(true),
        }])
        .expect("lock");
    assert_eq!(sync(&f, at(&f, 5), 5), Err(LipSyncError::Locked));

    let mut f = fixture();
    add(&mut f, rect_span(0, 3, 0.0, 0.0, 5.0, 5.0));
    assert_eq!(sync(&f, at(&f, 0), 0), Err(LipSyncError::NotAGraphic));
}

#[test]
fn a_mapping_reads_from_text_and_says_what_is_wrong_with_it() {
    assert_eq!(MouthMap::parse("").expect("empty"), MouthMap::in_order());
    let map = MouthMap::parse(" MBP=3, o = 5 ,").expect("map");
    assert_eq!((map.frame(Mbp), map.frame(O), map.frame(Ai)), (3, 5, 2));
    assert_eq!(map.pairs().count(), 10);
    assert!(matches!(MouthMap::parse("ah=1"), Err(MapError::UnknownShape(s)) if s == "ah"));
    assert!(matches!(MouthMap::parse("mbp"), Err(MapError::BadPair(_))));
    assert!(matches!(
        MouthMap::parse("mbp=-1"),
        Err(MapError::BadPair(_))
    ));
    assert_eq!(MouthMap::parse(&"x".repeat(2000)), Err(MapError::TooLong));
}
