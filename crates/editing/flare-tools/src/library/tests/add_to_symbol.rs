use super::*;

#[test]
fn add_instance_to_symbol_appends_one_frame_to_the_targets_first_layer() {
    let mut f = fixture();
    let target = SymbolId::new();
    let inner = Layer::new(
        LayerId::new(),
        "T",
        vec![rect_span(0, 10, 0.0, 0.0, 10.0, 10.0)],
    )
    .expect("layer");
    f.session
        .apply([Command::AddSymbol(Symbol::new(
            target,
            "Box",
            Timeline::new(vec![inner]),
        ))])
        .expect("symbol");
    add(&mut f, rect_span(0, 6, 40.0, 40.0, 10.0, 10.0));
    let (scene, pick) = (f.scene, at(&f, 0));
    let before = f.session.document().clone();

    apply(&mut f, |doc| {
        add_instance_to_symbol(doc, target, scene, pick)
    });

    let document = f.session.document();
    let spans = document.symbol(target).expect("target").timeline().layers()[0].spans();
    assert_eq!(spans.len(), 2);
    assert_eq!(spans[1].start(), 10, "after the target's own frames");
    assert_eq!(spans[1].duration().get(), 1, "one frame only");
    assert!(
        matches!(spans[1].content(), Content::Shape(_)),
        "the shape was copied"
    );
    assert_eq!(
        document.scenes()[0].timeline(),
        before.scenes()[0].timeline(),
        "the scene keeps its shape"
    );
}

#[test]
fn add_instance_to_symbol_refuses_the_wrong_pick_and_a_missing_target() {
    let mut f = fixture();
    let target = SymbolId::new();
    let inner = Layer::new(
        LayerId::new(),
        "T",
        vec![rect_span(0, 10, 0.0, 0.0, 10.0, 10.0)],
    )
    .expect("layer");
    f.session
        .apply([Command::AddSymbol(Symbol::new(
            target,
            "Box",
            Timeline::new(vec![inner]),
        ))])
        .expect("symbol");
    add(
        &mut f,
        instance_span(0, 6, target, playback(0), Transform2D::IDENTITY),
    );

    // Nothing picked: no span starts there.
    assert_eq!(
        add_instance_to_symbol(f.session.document(), target, f.scene, at(&f, 99)),
        Err(LibraryError::NothingSelected)
    );
    // An instance of the target inside the target: a cycle.
    assert_eq!(
        add_instance_to_symbol(f.session.document(), target, f.scene, at(&f, 0)),
        Err(LibraryError::Cycle)
    );
    // No target named that.
    assert_eq!(
        add_instance_to_symbol(f.session.document(), SymbolId::new(), f.scene, at(&f, 0)),
        Err(LibraryError::NoSuchSymbol)
    );
}

#[test]
fn add_instance_to_symbol_refuses_an_empty_keyframe_and_a_layerless_target() {
    let mut f = fixture();
    add(&mut f, FrameSpan::new(0, NonZeroU32::MIN, Content::Empty));
    let hollow = SymbolId::new();
    f.session
        .apply([Command::AddSymbol(Symbol::new(
            hollow,
            "Empty",
            Timeline::new(vec![]),
        ))])
        .expect("symbol");
    let layerless = SymbolId::new();
    f.session
        .apply([Command::AddSymbol(Symbol::new(
            layerless,
            "NoLayer",
            Timeline::new(vec![]),
        ))])
        .expect("symbol");

    assert_eq!(
        add_instance_to_symbol(f.session.document(), hollow, f.scene, at(&f, 0)),
        Err(LibraryError::NotPlaceable),
        "an empty keyframe holds nothing to copy"
    );

    // With something to copy, a target without layers still cannot take it.
    add(&mut f, rect_span(6, 4, 0.0, 0.0, 10.0, 10.0));
    assert_eq!(
        add_instance_to_symbol(f.session.document(), layerless, f.scene, at(&f, 6)),
        Err(LibraryError::NoLayer)
    );
}
