use super::*;

#[test]
fn stroke_width_is_part_of_the_bounds_and_survives_moves() {
    use flare_document::Stroke;
    let mut f = fixture();
    let stroke = Stroke::new(6.0, RED).expect("stroke");
    let shape = Shape::rect(ShapeId::new(), 10.0, 10.0, 20.0, 20.0, RED)
        .expect("rect")
        .with_stroke(stroke);
    let span = FrameSpan::new(0, NonZeroU32::MIN, Content::Shape(shape));
    add(&mut f, span);
    assert_eq!(
        bounds(&f, 0),
        Rect::from_min_max(Pos2::new(7.0, 7.0), Pos2::new(33.0, 33.0))
    );
    let commands = translate(
        f.session.document(),
        f.scene,
        at(&f, 0),
        DVec2::new(1.0, 1.0),
    )
    .expect("move");
    f.session.apply(commands).expect("apply");
    let moved = find_span(f.session.document(), f.scene, at(&f, 0)).expect("span");
    let Content::Shape(shape) = moved.content() else {
        panic!("shape expected")
    };
    assert_eq!(shape.stroke(), Some(stroke), "moving keeps the stroke");
}

#[test]
fn tweens_toggle_on_instances_only() {
    let mut f = fixture();
    let symbol = SymbolId::new();
    f.session
        .apply([Command::AddSymbol(Symbol::new(
            symbol,
            "S",
            Timeline::default(),
        ))])
        .expect("symbol");
    let instance = Instance::new(InstanceId::new(), symbol, Transform2D::IDENTITY);
    add(
        &mut f,
        FrameSpan::new(
            0,
            NonZeroU32::new(4).expect("nonzero"),
            Content::Instance(instance),
        ),
    );
    add(&mut f, rect_span(4, 2, 0.0, 0.0, 5.0, 5.0));
    let doc = f.session.document();
    assert!(
        set_tween(doc, f.scene, at(&f, 4), Tween::Linear).is_none(),
        "shapes do not tween"
    );
    assert!(
        set_tween(doc, f.scene, at(&f, 0), Tween::None).is_none(),
        "already untweened"
    );
    let commands = set_tween(doc, f.scene, at(&f, 0), Tween::Linear).expect("enable");
    f.session.apply(commands).expect("apply");
    let span = find_span(f.session.document(), f.scene, at(&f, 0)).expect("span");
    assert_eq!((span.tween(), span.duration().get()), (Tween::Linear, 4));
    let commands =
        set_tween(f.session.document(), f.scene, at(&f, 0), Tween::None).expect("disable");
    f.session.apply(commands).expect("apply");
    assert_eq!(
        find_span(f.session.document(), f.scene, at(&f, 0))
            .expect("span")
            .tween(),
        Tween::None
    );
}

#[test]
fn deleting_removes_the_span() {
    let mut f = fixture();
    add(&mut f, rect_span(0, 5, 0.0, 0.0, 5.0, 5.0));
    f.session.apply(delete(f.scene, at(&f, 0))).expect("delete");
    assert!(find_span(f.session.document(), f.scene, at(&f, 0)).is_none());
}

#[test]
fn symbols_made_only_of_other_symbols_have_bounds_through_their_transforms() {
    let (inner, outer) = (SymbolId::new(), SymbolId::new());
    let tl = |spans: Vec<FrameSpan>| {
        Timeline::new(vec![Layer::new(LayerId::new(), "L", spans).expect("layer")])
    };
    let shift =
        Transform2D::new(DAffine2::from_translation(DVec2::new(100.0, 20.0))).expect("finite");
    let placed = FrameSpan::new(
        0,
        NonZeroU32::new(1).expect("nonzero"),
        Content::Instance(Instance::new(InstanceId::new(), inner, shift)),
    );
    let symbols = vec![
        Symbol::new(
            inner,
            "Inner",
            tl(vec![rect_span(0, 1, 0.0, 0.0, 10.0, 5.0)]),
        ),
        Symbol::new(outer, "Outer", tl(vec![placed])),
    ];
    let rate = FrameRate::new(24, 1).expect("rate");
    let scene = Scene::new(SceneId::new(), "S", Timeline::default());
    let doc = Document::new(DocumentId::new(), rate, vec![scene], symbols).expect("doc");
    let bounds = symbol_bounds(&doc, outer).expect("nested bounds");
    assert_eq!(
        (bounds.min, bounds.max),
        (Pos2::new(100.0, 20.0), Pos2::new(110.0, 25.0))
    );
    assert!(
        symbol_bounds(&doc, SymbolId::new()).is_none(),
        "unknown symbol"
    );
}

#[test]
fn many_span_bounds_match_the_one_at_a_time_answers_and_skip_missing_spans() {
    let mut f = fixture();
    add(&mut f, rect_span(0, 5, 0.0, 0.0, 10.0, 10.0));
    let layer = f.layer;
    let present = SpanRef { layer, start: 0 };
    let missing = SpanRef { layer, start: 9 };
    let gone = SpanRef {
        layer: LayerId::new(),
        start: 0,
    };
    let many = spans_bounds(f.session.document(), f.scene, &[present, missing, gone]);
    let one = span_bounds(f.session.document(), f.scene, present).expect("bounds");
    assert_eq!(many, vec![one]);
    assert!(spans_bounds(f.session.document(), SceneId::new(), &[present]).is_empty());
}

/// A second layer holding a rectangle span, as another selectable span.
fn add_on_new_layer(f: &mut Fixture, span: FrameSpan) -> SpanRef {
    let layer = LayerId::new();
    f.session
        .apply([
            Command::AddLayer {
                scene: f.scene,
                layer: Layer::new(layer, "M", vec![]).expect("layer"),
            },
            Command::InsertSpan {
                scene: f.scene,
                layer,
                span: span.clone(),
            },
        ])
        .expect("insert");
    SpanRef {
        layer,
        start: span.start(),
    }
}

#[test]
fn grouping_shapes_makes_one_symbol_instance_and_breaking_it_apart_restores_the_parts() {
    let mut f = fixture();
    add(&mut f, rect_span(0, 6, 0.0, 0.0, 10.0, 10.0));
    let first = at(&f, 0);
    let second = add_on_new_layer(&mut f, rect_span(0, 6, 40.0, 0.0, 10.0, 10.0));
    assert!(
        group(f.session.document(), f.scene, &[first]).is_none(),
        "one shape is not a group"
    );
    let (commands, symbol) = group(f.session.document(), f.scene, &[first, second]).expect("group");
    f.session.apply(commands).expect("apply");
    let document = f.session.document();
    assert_eq!(
        document
            .symbol(symbol)
            .expect("symbol")
            .timeline()
            .layers()
            .len(),
        2
    );
    assert!(
        find_span(document, f.scene, second).is_none(),
        "the other span is gone"
    );
    assert!(matches!(
        find_span(document, f.scene, first)
            .expect("instance")
            .content(),
        Content::Instance(_)
    ));
    // The group moves as one, and breaking it apart puts each part back where it now is.
    let moved = translate(document, f.scene, first, DVec2::new(100.0, 0.0)).expect("move");
    f.session.apply(moved).expect("apply");
    let (commands, parts) = break_apart(f.session.document(), f.scene, first).expect("break apart");
    assert_eq!(parts.len(), 2);
    f.session.apply(commands).expect("apply");
    let document = f.session.document();
    assert!(
        find_span(document, f.scene, first).is_none(),
        "the instance is gone"
    );
    let mut lefts: Vec<f32> = parts
        .iter()
        .map(|p| span_bounds(document, f.scene, *p).expect("bounds").left())
        .collect();
    lefts.sort_by(f32::total_cmp);
    assert_eq!(lefts, vec![100.0, 140.0]);
    assert!(
        break_apart(document, f.scene, parts[0]).is_none(),
        "a shape cannot be broken apart"
    );
}

#[test]
fn anchors_are_the_segment_ends_and_moving_one_reshapes_the_outline() {
    let mut f = fixture();
    add(&mut f, rect_span(0, 4, 0.0, 0.0, 10.0, 10.0));
    let span = at(&f, 0);
    let Content::Shape(shape) = find_span(f.session.document(), f.scene, span)
        .expect("span")
        .content()
    else {
        panic!("shape");
    };
    let points = anchors(shape);
    assert_eq!(points.len(), 4, "a rectangle has four corners");
    assert_eq!(points[2], (2, Pos2::new(10.0, 10.0)));
    let commands = move_anchor(
        f.session.document(),
        f.scene,
        span,
        2,
        DVec2::new(20.0, 30.0),
    )
    .expect("move");
    f.session.apply(commands).expect("apply");
    let b = bounds(&f, 0);
    assert_eq!((b.min, b.max), (Pos2::new(0.0, 0.0), Pos2::new(20.0, 30.0)));
    assert!(
        move_anchor(
            f.session.document(),
            f.scene,
            span,
            2,
            DVec2::new(20.0, 30.0)
        )
        .is_none(),
        "no movement"
    );
    assert!(
        move_anchor(f.session.document(), f.scene, span, 4, DVec2::ZERO).is_none(),
        "Close is not an anchor"
    );
    assert!(move_anchor(f.session.document(), f.scene, span, 99, DVec2::ZERO).is_none());
}

#[test]
fn moving_a_curve_anchor_carries_its_handles() {
    let mut f = fixture();
    let ellipse = Shape::ellipse(ShapeId::new(), 10.0, 10.0, 10.0, 10.0, RED).expect("ellipse");
    add(
        &mut f,
        FrameSpan::new(0, NonZeroU32::MIN, Content::Shape(ellipse)),
    );
    let span = at(&f, 0);
    let before = bounds(&f, 0);
    // The first segment is the move to the right-hand point; drag it 6 units to the right.
    let commands = move_anchor(
        f.session.document(),
        f.scene,
        span,
        0,
        DVec2::new(26.0, 10.0),
    )
    .expect("move");
    f.session.apply(commands).expect("apply");
    let after = bounds(&f, 0);
    assert!(
        after.right() > before.right() + 5.0,
        "{before:?} then {after:?}"
    );
    assert_eq!(after.left(), before.left());
}

#[test]
fn a_mirror_copy_is_a_new_layer_reflected_across_the_axis_and_the_original_stays() {
    let mut f = fixture();
    add(&mut f, rect_span(0, 4, 10.0, 0.0, 20.0, 10.0));
    let before = f.session.document().clone();
    let mirror = DAffine2::from_translation(DVec2::new(100.0, 0.0))
        * DAffine2::from_scale(DVec2::new(-1.0, 1.0))
        * DAffine2::from_translation(DVec2::new(-100.0, 0.0));
    let (commands, copy) =
        duplicate_through(&before, f.scene, at(&f, 0), mirror, "mirror").expect("mirror copy");
    f.session.apply(commands).expect("apply");
    assert_eq!(
        span_bounds(f.session.document(), f.scene, copy).expect("copy bounds"),
        Rect::from_min_max(Pos2::new(170.0, 0.0), Pos2::new(190.0, 10.0))
    );
    assert_eq!(
        bounds(&f, 0),
        span_bounds(&before, f.scene, at(&f, 0)).expect("old")
    );
    let names: Vec<&str> = f.session.document().scenes()[0]
        .timeline()
        .layers()
        .iter()
        .map(|l| l.name())
        .collect();
    assert_eq!(names, ["L", "L mirror"]);
    f.session.undo();
    assert_eq!(
        f.session.document(),
        &before,
        "one undo step removes the copy"
    );
}

#[test]
fn a_mirror_copy_has_its_own_identity_and_a_collapsing_matrix_is_refused() {
    let mut f = fixture();
    add(&mut f, rect_span(0, 4, 10.0, 0.0, 20.0, 10.0));
    let document = f.session.document().clone();
    let flip = DAffine2::from_scale(DVec2::new(-1.0, 1.0));
    let (commands, copy) =
        duplicate_through(&document, f.scene, at(&f, 0), flip, "mirror").expect("copy");
    f.session.apply(commands).expect("apply");
    let ids: Vec<ShapeId> = [at(&f, 0), copy]
        .into_iter()
        .filter_map(|s| find_span(f.session.document(), f.scene, s))
        .filter_map(|s| match s.content() {
            Content::Shape(shape) => Some(shape.id()),
            _ => None,
        })
        .collect();
    assert_eq!(ids.len(), 2);
    assert_ne!(ids[0], ids[1]);
    let flat = DAffine2::from_scale(DVec2::new(0.0, 1.0));
    assert!(duplicate_through(&document, f.scene, at(&f, 0), flat, "x").is_none());
    assert!(duplicate_through(&document, f.scene, at(&f, 9), flip, "x").is_none());
}

#[test]
fn converting_can_make_a_button_or_movie_clip_and_defaults_to_a_graphic() {
    use flare_document::SymbolKind;
    for (kind, convert) in [
        (SymbolKind::Graphic, None),
        (SymbolKind::Button, Some(SymbolKind::Button)),
        (SymbolKind::MovieClip, Some(SymbolKind::MovieClip)),
    ] {
        let mut f = fixture();
        add(&mut f, rect_span(0, 4, 0.0, 0.0, 10.0, 10.0));
        let (commands, symbol) = match convert {
            Some(kind) => convert_to_symbol_as(f.session.document(), f.scene, at(&f, 0), kind),
            None => convert_to_symbol(f.session.document(), f.scene, at(&f, 0)),
        }
        .expect("convertible");
        f.session.apply(commands).expect("apply");
        let made = f.session.document().symbol(symbol).expect("symbol");
        assert_eq!(made.kind(), kind);
    }
}

#[test]
fn a_shape_tween_fits_a_shape_and_a_motion_tween_an_instance_and_the_ease_is_kept() {
    let mut f = fixture();
    add(&mut f, rect_span(0, 4, 0.0, 0.0, 10.0, 10.0));
    let shape_at = at(&f, 0);
    // A motion tween makes no sense on a shape.
    assert!(set_tween(f.session.document(), f.scene, shape_at, Tween::Linear).is_none());
    let commands =
        set_tween(f.session.document(), f.scene, shape_at, Tween::Shape).expect("a shape tween");
    f.session.apply(commands).expect("apply");
    let commands = set_ease(f.session.document(), f.scene, shape_at, 60).expect("ease");
    f.session.apply(commands).expect("apply");
    let span = find_span(f.session.document(), f.scene, shape_at).expect("span");
    assert_eq!((span.tween(), span.ease()), (Tween::Shape, 60));
    // The same setting again is nothing to do, and an ease is kept when the tween is switched off.
    assert!(set_tween(f.session.document(), f.scene, shape_at, Tween::Shape).is_none());
    assert!(set_ease(f.session.document(), f.scene, shape_at, 60).is_none());
    let commands = set_tween(f.session.document(), f.scene, shape_at, Tween::None).expect("off");
    f.session.apply(commands).expect("apply");
    let span = find_span(f.session.document(), f.scene, shape_at).expect("span");
    assert_eq!((span.tween(), span.ease()), (Tween::None, 60));
}

#[test]
fn an_ease_is_clamped_and_follows_the_span_through_other_edits() {
    let mut f = fixture();
    add(&mut f, rect_span(0, 4, 0.0, 0.0, 10.0, 10.0));
    let commands =
        set_tween(f.session.document(), f.scene, at(&f, 0), Tween::Shape).expect("tween");
    f.session.apply(commands).expect("apply");
    let commands = set_ease(f.session.document(), f.scene, at(&f, 0), 120).expect("ease");
    f.session.apply(commands).expect("apply");
    assert_eq!(
        find_span(f.session.document(), f.scene, at(&f, 0))
            .expect("span")
            .ease(),
        100
    );
    // Moving and copying keep the tween and its ease.
    let moved = translate(
        f.session.document(),
        f.scene,
        at(&f, 0),
        DVec2::new(5.0, 5.0),
    )
    .expect("move");
    f.session.apply(moved).expect("apply");
    let span = find_span(f.session.document(), f.scene, at(&f, 0)).expect("span");
    assert_eq!((span.tween(), span.ease()), (Tween::Shape, 100));
    let (copy, at_copy) = duplicate(
        f.session.document(),
        f.scene,
        at(&f, 0),
        DVec2::new(1.0, 1.0),
    )
    .expect("copy");
    f.session.apply(copy).expect("apply");
    let copied = find_span(f.session.document(), f.scene, at_copy).expect("span");
    assert_eq!((copied.tween(), copied.ease()), (Tween::Shape, 100));
    let item = copy_span(f.session.document(), f.scene, at(&f, 0)).expect("clip");
    let (pasted, at_paste) = paste(f.session.document(), f.scene, &item, 10).expect("paste");
    f.session.apply(pasted).expect("apply");
    let pasted = find_span(f.session.document(), f.scene, at_paste).expect("span");
    assert_eq!((pasted.tween(), pasted.ease()), (Tween::Shape, 100));
}
