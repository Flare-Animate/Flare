use super::*;

#[test]
fn bake_transform_moves_the_picked_transform_into_the_artwork_and_keeps_every_view() {
    let LibraryFixture {
        mut f,
        sym,
        outer,
        late,
        ..
    } = library_fixture();
    let picked_matrix = Transform2D::new(glamx::DAffine2::from_translation(glamx::dvec2(5.0, 7.0)))
        .expect("finite");
    // The late span reads a scale, so the unbake check is not the identity gaining a no-op.
    let late_before = transform_at(&f, late);
    let nested_before = match nested(&f, outer) {
        Content::Instance(instance) => instance.transform(),
        other => panic!("{other:?}"),
    };
    let art_before = artwork(&f, sym);

    let pick = at(&f, 0);
    let scene = f.scene;
    apply(&mut f, |doc| bake_transform(doc, scene, pick));

    let baked_matrix = Transform2D::new(glamx::DAffine2::from_translation(glamx::dvec2(5.0, 7.0)))
        .expect("finite");
    // The artwork has the picked transform now.
    assert_eq!(
        crate::common::content::transform_shape(&art_before, picked_matrix.as_affine()),
        Some(artwork(&f, sym)),
        "the artwork took the transform"
    );
    // The picked one ends with no transform of its own.
    assert!(
        same(transform_at(&f, 0), Transform2D::IDENTITY),
        "the picked instance is plain"
    );
    // Every other view takes the inverse, so what is drawn does not move: the artwork
    // carries the matrix in, and the instance carries the matrix out.
    let new_late = transform_at(&f, late);
    assert!(
        same(
            Transform2D::new(new_late.as_affine() * baked_matrix.as_affine()).expect("finite"),
            late_before
        ),
        "the late instance keeps its view"
    );
    let Content::Instance(nested_instance) = nested(&f, outer) else {
        panic!("nested instance");
    };
    assert!(
        same(
            Transform2D::new(nested_instance.transform().as_affine() * baked_matrix.as_affine())
                .expect("finite"),
            nested_before
        ),
        "the nested instance keeps its view"
    );
}

#[test]
fn bake_transform_preserves_the_playback_of_every_instance() {
    let LibraryFixture { mut f, .. } = library_fixture();
    let pick = at(&f, 0);
    let scene = f.scene;
    apply(&mut f, |doc| bake_transform(doc, scene, pick));
    assert_eq!(playback_at(&f, 0).first_frame, 3);
}

#[test]
fn bake_transform_refuses_the_wrong_pick_and_a_no_op() {
    let mut f = fixture();
    add(&mut f, rect_span(0, 4, 0.0, 0.0, 10.0, 10.0));
    let before = f.session.document().clone();

    assert_eq!(
        bake_transform(f.session.document(), f.scene, at(&f, 0)),
        Err(LibraryError::NotAnInstance),
        "a shape is not a symbol instance"
    );
    assert_eq!(f.session.document(), &before, "a refusal changed nothing");

    let plain = SymbolId::new();
    let inner = Layer::new(
        LayerId::new(),
        "P",
        vec![rect_span(0, 4, 0.0, 0.0, 10.0, 10.0)],
    )
    .expect("layer");
    f.session
        .apply([Command::AddSymbol(Symbol::new(
            plain,
            "Plain",
            Timeline::new(vec![inner]),
        ))])
        .expect("symbol");
    add(
        &mut f,
        instance_span(4, 4, plain, playback(0), Transform2D::IDENTITY),
    );
    let before = f.session.document().clone();

    // An identity transform has nothing to bake.
    assert_eq!(
        bake_transform(f.session.document(), f.scene, at(&f, 4)),
        Err(LibraryError::NoChange),
    );
    assert_eq!(f.session.document(), &before, "nothing changed");
}

#[test]
fn bake_transform_refuses_a_singular_matrix_and_a_tweened_artwork() {
    let mut f = fixture();
    let singular_art = SymbolId::new();
    let inner = Layer::new(
        LayerId::new(),
        "S",
        vec![rect_span(0, 4, 0.0, 0.0, 10.0, 10.0)],
    )
    .expect("layer");
    f.session
        .apply([Command::AddSymbol(Symbol::new(
            singular_art,
            "Flat",
            Timeline::new(vec![inner]),
        ))])
        .expect("symbol");
    let singular =
        Transform2D::new(glamx::DAffine2::from_scale(glamx::dvec2(0.0, 1.0))).expect("finite");
    add(
        &mut f,
        instance_span(0, 4, singular_art, playback(0), singular),
    );

    assert_eq!(
        bake_transform(f.session.document(), f.scene, at(&f, 0)),
        Err(LibraryError::Singular),
        "a flat artwork cannot take the whole geometry"
    );
}

#[test]
fn bake_transform_refuses_artwork_with_a_motion_tween_in_it() {
    let mut f = fixture();
    let leaf = SymbolId::new();
    let leaf_layer = Layer::new(
        LayerId::new(),
        "L",
        vec![rect_span(0, 4, 0.0, 0.0, 10.0, 10.0)],
    )
    .expect("layer");
    let tween_art = SymbolId::new();
    let tween_instance = Instance::new(InstanceId::new(), leaf, Transform2D::IDENTITY);
    let tween_layer = Layer::new(
        LayerId::new(),
        "M",
        vec![
            FrameSpan::new(
                0,
                NonZeroU32::new(4).expect("nonzero"),
                Content::Instance(tween_instance),
            )
            .with_tween(Tween::Linear),
        ],
    )
    .expect("layer");
    f.session
        .apply([
            Command::AddSymbol(Symbol::new(leaf, "Leaf", Timeline::new(vec![leaf_layer]))),
            Command::AddSymbol(Symbol::new(
                tween_art,
                "Moving",
                Timeline::new(vec![tween_layer]),
            )),
        ])
        .expect("symbols");
    let placed = Transform2D::new(glamx::DAffine2::from_translation(glamx::dvec2(1.0, 2.0)))
        .expect("finite");
    add(&mut f, instance_span(0, 4, tween_art, playback(0), placed));

    assert_eq!(
        bake_transform(f.session.document(), f.scene, at(&f, 0)),
        Err(LibraryError::Tweened),
        "a bake would freeze the tween's artwork"
    );
}
