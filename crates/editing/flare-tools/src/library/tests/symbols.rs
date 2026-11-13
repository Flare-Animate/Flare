use super::*;
use crate::ops::span_bounds;

/// A rectangle from (20, 40) to (120, 90) at frame 0 of the fixture, and nothing else.
fn rectangle() -> Fixture {
    let mut f = fixture();
    add(&mut f, rect_span(0, 3, 20.0, 40.0, 100.0, 50.0));
    f
}

fn convert(f: &mut Fixture, name: &str, kind: SymbolKind, registration: Registration) -> SymbolId {
    let (commands, id) = convert_to_symbol(
        f.session.document(),
        f.scene,
        at(f, 0),
        name,
        kind,
        registration,
    )
    .expect("a conversion");
    f.session.apply(commands).expect("apply");
    id
}

#[test]
fn create_symbol_adds_an_empty_symbol_of_the_chosen_name_and_type() {
    let mut f = fixture();
    let (commands, id) =
        create_symbol(f.session.document(), "  Walk cycle ", SymbolKind::MovieClip).expect("made");
    f.session.apply(commands).expect("apply");
    let symbol = f.session.document().symbol(id).expect("in the library");
    assert_eq!(symbol.name(), "Walk cycle", "the name is trimmed");
    assert_eq!(symbol.kind(), SymbolKind::MovieClip);
    assert!(symbol.timeline().layers().is_empty());
}

#[test]
fn a_blank_long_or_taken_name_is_refused() {
    let mut f = fixture();
    let document = f.session.document();
    assert_eq!(
        create_symbol(document, " \t ", SymbolKind::Graphic).err(),
        Some(SymbolError::NameEmpty)
    );
    let long = "x".repeat(MAX_SYMBOL_NAME_CHARS + 1);
    assert_eq!(
        create_symbol(document, &long, SymbolKind::Graphic).err(),
        Some(SymbolError::NameTooLong)
    );
    let longest = "x".repeat(MAX_SYMBOL_NAME_CHARS);
    assert!(create_symbol(document, &longest, SymbolKind::Graphic).is_ok());
    let (commands, _) = create_symbol(document, "Ball", SymbolKind::Graphic).expect("made");
    f.session.apply(commands).expect("apply");
    assert_eq!(
        create_symbol(f.session.document(), " Ball", SymbolKind::Button).err(),
        Some(SymbolError::NameTaken)
    );
}

#[test]
fn the_free_name_skips_the_ones_the_library_holds() {
    let mut f = fixture();
    let pattern = |n: u32| format!("Symbol {n}");
    assert_eq!(free_symbol_name(f.session.document(), pattern), "Symbol 1");
    for name in ["Symbol 1", "Symbol 3"] {
        let (commands, _) =
            create_symbol(f.session.document(), name, SymbolKind::Graphic).expect("made");
        f.session.apply(commands).expect("apply");
    }
    assert_eq!(free_symbol_name(f.session.document(), pattern), "Symbol 2");
}

#[test]
fn convert_keeps_the_artwork_in_place_with_the_origin_where_asked() {
    for (registration, origin) in [
        (Registration::TopLeft, dvec2(20.0, 40.0)),
        (Registration::Center, dvec2(70.0, 65.0)),
        (Registration::BottomRight, dvec2(120.0, 90.0)),
        (Registration::Top, dvec2(70.0, 40.0)),
        (Registration::Left, dvec2(20.0, 65.0)),
    ] {
        let mut f = rectangle();
        let before = bounds(&f, 0);
        let id = convert(&mut f, "Box", SymbolKind::Button, registration);
        let symbol = f.session.document().symbol(id).expect("in the library");
        assert_eq!((symbol.name(), symbol.kind()), ("Box", SymbolKind::Button));
        let place = Transform2D::new(glamx::DAffine2::from_translation(origin)).expect("finite");
        assert!(
            same(transform_at(&f, 0), place),
            "{registration:?}: the instance stands on the registration point"
        );
        let inside = crate::ops::shape_bounds(&artwork(&f, id)).expect("bounds");
        let corner = dvec2(f64::from(inside.min.x), f64::from(inside.min.y));
        assert!(
            (corner - (dvec2(20.0, 40.0) - origin)).length() < 1e-4,
            "{registration:?}: the artwork is drawn around the symbol's origin"
        );
        let after = span_bounds(f.session.document(), f.scene, at(&f, 0)).expect("bounds");
        assert!(
            (after.min - before.min).length() < 1e-3 && (after.max - before.max).length() < 1e-3,
            "{registration:?}: nothing moves on the stage"
        );
    }
}

#[test]
fn convert_refuses_a_taken_name_and_anything_but_a_shape_and_changes_nothing() {
    let mut f = rectangle();
    let (commands, _) =
        create_symbol(f.session.document(), "Box", SymbolKind::Graphic).expect("made");
    f.session.apply(commands).expect("apply");
    let document = f.session.document();
    let refused = |name: &str, at: SpanRef| {
        convert_to_symbol(
            document,
            f.scene,
            at,
            name,
            SymbolKind::Graphic,
            Registration::Center,
        )
        .err()
    };
    assert_eq!(refused("Box", at(&f, 0)), Some(SymbolError::NameTaken));
    assert_eq!(refused("", at(&f, 0)), Some(SymbolError::NameEmpty));
    assert_eq!(refused("Other", at(&f, 7)), Some(SymbolError::NotAShape));
    // An instance is not a drawn shape.
    let id = convert(&mut f, "Other", SymbolKind::Graphic, Registration::TopLeft);
    let document = f.session.document();
    assert_eq!(
        convert_to_symbol(
            document,
            f.scene,
            at(&f, 0),
            "Third",
            SymbolKind::Graphic,
            Registration::TopLeft
        )
        .err(),
        Some(SymbolError::NotAShape)
    );
    assert_eq!(document.symbols().len(), 2);
    let _ = id;
}

#[test]
fn registration_reads_the_spellings_every_surface_takes() {
    for registration in Registration::ALL {
        assert_eq!(Registration::parse(registration.name()), Some(registration));
    }
    assert_eq!(Registration::parse("Top Left"), Some(Registration::TopLeft));
    assert_eq!(
        Registration::parse("bottom_right"),
        Some(Registration::BottomRight)
    );
    assert_eq!(Registration::parse("centre"), Some(Registration::Center));
    assert_eq!(Registration::parse("middle"), None);
}
