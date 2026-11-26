use super::*;

#[test]
fn increment_instances_moves_every_qualifying_instance_on_the_scene_and_in_other_symbols() {
    let LibraryFixture {
        mut f,
        sym,
        outer,
        late,
        ..
    } = library_fixture();
    apply(&mut f, |doc| increment_instances(doc, sym, 4, 2));
    assert_eq!(playback_at(&f, 0).first_frame, 3, "the early one stays");
    assert_eq!(playback_at(&f, late).first_frame, 10, "the late one moves");
    let Content::Instance(nested_instance) = nested(&f, outer) else {
        panic!("nested instance");
    };
    assert_eq!(
        nested_instance.playback().first_frame,
        7,
        "the nested one moves too"
    );
    let _ = sym;
}

#[test]
fn increment_instances_refuses_to_move_a_frame_before_the_symbols_first() {
    let LibraryFixture { f, sym, .. } = library_fixture();
    // The early instance reads frame 3; four frames back leaves the symbol's frames.
    assert_eq!(
        increment_instances(f.session.document(), sym, 0, -4),
        Err(LibraryError::BeforeFirst)
    );
    assert_eq!(
        playback_at(&f, 0).first_frame,
        3,
        "nothing was applied, so nothing moved"
    );
}

#[test]
fn increment_instances_refuses_a_no_op() {
    let LibraryFixture { f, sym, .. } = library_fixture();
    assert_eq!(
        increment_instances(f.session.document(), sym, 0, 0),
        Err(LibraryError::NoChange)
    );
    assert_eq!(
        increment_instances(f.session.document(), sym, u32::MAX, 1),
        Err(LibraryError::NoChange),
        "no instance reads that far"
    );
    assert_eq!(
        increment_instances(f.session.document(), SymbolId::new(), 0, 1),
        Err(LibraryError::NoSuchSymbol)
    );
}

#[test]
fn increment_instances_keeps_each_instances_play_mode() {
    // Single frame, play once and loop all keep their mode; only where they begin moves.
    for mode in [PlayMode::Loop, PlayMode::PlayOnce, PlayMode::SingleFrame] {
        let LibraryFixture { mut f, sym, .. } = library_fixture();
        let start = f.session.document().scenes()[0].timeline().layers()[0].spans()[0].start();
        let _ = start;
        f.session
            .apply([Command::SetInstancePlayback {
                scene: f.scene,
                layer: at(&f, 0).layer,
                start: 0,
                playback: Playback {
                    mode,
                    first_frame: 3,
                },
            }])
            .expect("mode");
        apply(&mut f, |doc| increment_instances(doc, sym, 0, 2));
        let moved = playback_at(&f, 0);
        assert_eq!(moved.mode, mode);
        assert_eq!(moved.first_frame, 5);
    }
}
