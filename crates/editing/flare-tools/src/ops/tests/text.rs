use super::*;
use flare_core::DocumentPoint;
use flare_document::TextSource;

fn text_shape(at: (f64, f64)) -> Shape {
    let source = TextSource::new(
        "Hi",
        "Hack",
        20.0,
        false,
        false,
        DocumentPoint::new(at.0, at.1).expect("point"),
    )
    .expect("source");
    Shape::rect(ShapeId::new(), at.0, at.1, 30.0, 20.0, RED)
        .expect("rect")
        .with_text(source)
}

fn shape_at(f: &Fixture) -> Shape {
    let span = find_span(f.session.document(), f.scene, at(f, 0)).expect("span");
    let Content::Shape(shape) = span.content() else {
        panic!("shape expected")
    };
    shape.clone()
}

fn with_text(f: &mut Fixture) {
    add(
        f,
        FrameSpan::new(0, NonZeroU32::MIN, Content::Shape(text_shape((10.0, 10.0)))),
    );
}

#[test]
fn moving_text_moves_its_source_and_it_stays_text() {
    let mut f = fixture();
    with_text(&mut f);
    let commands = translate(
        f.session.document(),
        f.scene,
        at(&f, 0),
        DVec2::new(5.0, 7.0),
    )
    .expect("move");
    f.session.apply(commands).expect("apply");
    let shape = shape_at(&f);
    let source = shape.text().expect("still text");
    assert_eq!(
        (source.origin().as_vec().x, source.origin().as_vec().y),
        (15.0, 17.0)
    );
    assert_eq!(source.size(), 20.0);
}

#[test]
fn an_even_scale_scales_the_size_and_a_turn_or_a_stretch_makes_outlines() {
    let mut f = fixture();
    with_text(&mut f);
    let doubled = transform(
        f.session.document(),
        f.scene,
        at(&f, 0),
        DAffine2::from_scale(DVec2::splat(2.0)),
    )
    .expect("scale");
    f.session.apply(doubled).expect("apply");
    let source = shape_at(&f).text().cloned().expect("text");
    assert_eq!(source.size(), 40.0);
    assert_eq!(source.origin().as_vec().x, 20.0);

    for matrix in [
        DAffine2::from_angle(0.5),
        DAffine2::from_scale(DVec2::new(2.0, 1.0)),
        DAffine2::from_scale(DVec2::new(-1.0, 1.0)),
    ] {
        let mut f = fixture();
        with_text(&mut f);
        let commands = transform(f.session.document(), f.scene, at(&f, 0), matrix).expect("t");
        f.session.apply(commands).expect("apply");
        assert!(shape_at(&f).text().is_none(), "{matrix:?} makes outlines");
    }
}

#[test]
fn text_is_replaced_by_a_new_version_in_one_undo_step_and_taken_apart_on_request() {
    let mut f = fixture();
    with_text(&mut f);
    let before = shape_at(&f);
    let wider = Shape::rect(before.id(), 10.0, 10.0, 60.0, 20.0, RED)
        .expect("rect")
        .with_text(before.text().cloned().expect("text"));
    assert!(replace_shape(f.session.document(), f.scene, at(&f, 0), before.clone()).is_none());
    let commands =
        replace_shape(f.session.document(), f.scene, at(&f, 0), wider.clone()).expect("replace");
    f.session.apply(commands).expect("apply");
    assert_eq!(shape_at(&f), wider);
    assert!(f.session.undo());
    assert_eq!(shape_at(&f), before, "one undo restores it");

    let commands = break_text_apart(f.session.document(), f.scene, at(&f, 0)).expect("apart");
    f.session.apply(commands).expect("apply");
    let apart = shape_at(&f);
    assert!(apart.text().is_none());
    assert_eq!(apart.path(), before.path(), "the letters stay");
    assert!(
        break_text_apart(f.session.document(), f.scene, at(&f, 0)).is_none(),
        "nothing left to take apart"
    );
}

#[test]
fn a_duplicate_of_text_is_text_of_its_own_in_its_own_place() {
    let mut f = fixture();
    with_text(&mut f);
    let (commands, copy) = duplicate_through(
        f.session.document(),
        f.scene,
        at(&f, 0),
        DAffine2::from_translation(DVec2::new(40.0, 0.0)),
        "",
    )
    .expect("duplicate");
    f.session.apply(commands).expect("apply");
    let span = find_span(f.session.document(), f.scene, copy).expect("copy");
    let Content::Shape(shape) = span.content() else {
        panic!("shape expected")
    };
    assert_ne!(shape.id(), shape_at(&f).id());
    assert_eq!(shape.text().map(|t| t.origin().as_vec().x), Some(50.0));
}
