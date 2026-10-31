use super::*;
use flare_document::{PlayMode, Playback, SymbolKind};

/// A fixture holding a 5-frame symbol of the given kind and one instance span of it, tweened.
fn with_instance(kind: SymbolKind) -> (Fixture, SymbolId) {
    let mut f = fixture();
    let symbol = SymbolId::new();
    let inner = Layer::new(
        LayerId::new(),
        "I",
        vec![rect_span(0, 5, 0.0, 0.0, 10.0, 10.0)],
    )
    .expect("layer");
    f.session
        .apply([Command::AddSymbol(
            Symbol::new(symbol, "Walk", Timeline::new(vec![inner])).with_kind(kind),
        )])
        .expect("symbol");
    let instance = Instance::new(InstanceId::new(), symbol, Transform2D::IDENTITY);
    let span = FrameSpan::new(
        0,
        NonZeroU32::new(6).expect("nonzero"),
        Content::Instance(instance),
    )
    .with_tween(Tween::Linear)
    .with_ease(40);
    add(&mut f, span);
    (f, symbol)
}

fn playback_at(f: &Fixture, start: u32) -> Playback {
    let span = find_span(f.session.document(), f.scene, at(f, start)).expect("span");
    match span.content() {
        Content::Instance(i) => i.playback(),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_graphics_frame_setting_changes_in_one_undoable_step_and_keeps_everything_else() {
    let (mut f, _) = with_instance(SymbolKind::Graphic);
    let before = find_span(f.session.document(), f.scene, at(&f, 0))
        .expect("span")
        .clone();
    let want = Playback {
        mode: PlayMode::PlayOnce,
        first_frame: 2,
    };
    let commands = set_playback(f.session.document(), f.scene, at(&f, 0), want).expect("change");
    f.session.apply(commands).expect("apply");
    assert_eq!(playback_at(&f, 0), want);
    let after = find_span(f.session.document(), f.scene, at(&f, 0)).expect("span");
    assert_eq!(
        (after.duration(), after.tween(), after.ease()),
        (before.duration(), before.tween(), before.ease())
    );
    let (Content::Instance(was), Content::Instance(now)) = (before.content(), after.content())
    else {
        panic!("instances");
    };
    assert_eq!(
        (was.id(), was.symbol(), was.transform()),
        (now.id(), now.symbol(), now.transform())
    );
    assert!(f.session.undo(), "one step back");
    assert_eq!(playback_at(&f, 0), Playback::default(), "one step back");
}

#[test]
fn the_first_frame_stays_inside_the_symbol_and_nothing_changes_for_what_is_not_a_graphic() {
    let (f, _) = with_instance(SymbolKind::Graphic);
    let far = Playback {
        mode: PlayMode::SingleFrame,
        first_frame: 999,
    };
    let commands = set_playback(f.session.document(), f.scene, at(&f, 0), far).expect("change");
    let mut f = f;
    f.session.apply(commands).expect("apply");
    assert_eq!(
        playback_at(&f, 0).first_frame,
        4,
        "the symbol has five frames"
    );
    // The same setting again is no change.
    assert!(set_playback(f.session.document(), f.scene, at(&f, 0), far).is_none());
    // A missing span, and a span of artwork.
    assert!(set_playback(f.session.document(), f.scene, at(&f, 50), far).is_none());
    add(&mut f, rect_span(10, 2, 0.0, 0.0, 5.0, 5.0));
    assert!(set_playback(f.session.document(), f.scene, at(&f, 10), far).is_none());
    for kind in [SymbolKind::MovieClip, SymbolKind::Button] {
        let (f, _) = with_instance(kind);
        assert!(
            set_playback(f.session.document(), f.scene, at(&f, 0), far).is_none(),
            "{kind:?} ignores the setting"
        );
    }
}

#[test]
fn moving_and_copying_keep_the_frame_setting() {
    let (mut f, _) = with_instance(SymbolKind::Graphic);
    let want = Playback {
        mode: PlayMode::SingleFrame,
        first_frame: 3,
    };
    let commands = set_playback(f.session.document(), f.scene, at(&f, 0), want).expect("change");
    f.session.apply(commands).expect("apply");
    // Moved.
    let commands = translate(
        f.session.document(),
        f.scene,
        at(&f, 0),
        DVec2::new(5.0, 0.0),
    )
    .expect("move");
    f.session.apply(commands).expect("apply");
    assert_eq!(playback_at(&f, 0), want, "moving");
    // Copied and pasted: the pasted instance is another with the same setting.
    let item = copy_span(f.session.document(), f.scene, at(&f, 0)).expect("copy");
    let (commands, pasted) = paste_offset(
        f.session.document(),
        f.scene,
        &item,
        20,
        DVec2::new(3.0, 3.0),
    )
    .expect("paste");
    f.session.apply(commands).expect("apply");
    let copy = find_span(f.session.document(), f.scene, pasted).expect("pasted span");
    let Content::Instance(copy) = copy.content() else {
        panic!("an instance");
    };
    assert_eq!(copy.playback(), want, "pasting");
}
