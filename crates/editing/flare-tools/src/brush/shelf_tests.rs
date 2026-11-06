#![allow(clippy::expect_used)]

use super::*;

fn square(angle: f64) -> Nib {
    Nib::new(NibShape::Square, angle, 40.0)
}

#[test]
fn keeping_the_nib_names_it_chooses_it_and_stops_at_the_limit() {
    let mut shelf = BrushShelf::default().with_loose_nib(square(30.0));
    shelf.keep_nib().expect("room");
    shelf.keep_nib().expect("room");
    let names: Vec<&str> = shelf.presets().iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["Custom brush 1", "Custom brush 2"]);
    assert_eq!(shelf.selected(), Some(1));
    for _ in 2..MAX_BRUSHES {
        shelf.keep_nib().expect("room");
    }
    assert_eq!(shelf.keep_nib(), Err(ShelfFull));
    assert_eq!(shelf.presets().len(), MAX_BRUSHES);
}

#[test]
fn choosing_puts_a_brushs_nib_in_hand_and_nothing_chosen_is_the_plain_round_brush() {
    let mut shelf = BrushShelf::default().with_loose_nib(square(45.0));
    shelf.keep_nib().expect("room");
    shelf.choose(None);
    assert_eq!((shelf.selected(), shelf.nib()), (None, Nib::ROUND));
    shelf.choose(Some(0));
    assert_eq!(shelf.nib().shape(), NibShape::Square);
    assert!((shelf.nib().angle_degrees() - 45.0).abs() < 1e-4);
    // A place that is not there is the plain brush, not a panic.
    shelf.choose(Some(99));
    assert_eq!((shelf.selected(), shelf.nib()), (None, Nib::ROUND));
}

#[test]
fn changing_the_nib_edits_the_chosen_brush_and_keeps_its_name_and_deleting_it_removes_it() {
    let mut shelf = BrushShelf::default().with_loose_nib(square(10.0));
    shelf.keep_nib().expect("room");
    shelf.set_nib(square(80.0));
    assert_eq!(shelf.presets()[0].name, "Custom brush 1");
    assert!((shelf.presets()[0].angle - 80.0).abs() < 1e-3);
    // With no brush chosen the nib is only in hand.
    shelf.choose(None);
    shelf.set_nib(square(5.0));
    assert!((shelf.presets()[0].angle - 80.0).abs() < 1e-3);
    // The plain brush cannot be deleted.
    shelf.delete_selected();
    assert_eq!(shelf.presets().len(), 1);
    shelf.choose(Some(0));
    shelf.delete_selected();
    assert!(shelf.presets().is_empty());
    assert_eq!(shelf.nib(), Nib::ROUND);
}

#[test]
fn restoring_cleans_up_odd_names_limits_the_count_and_lets_the_chosen_brush_win() {
    let odd = BrushPreset {
        name: format!("  \u{7}{}", "x".repeat(100)),
        square: true,
        angle: -90.0,
        flatness: 0.0,
        ..BrushPreset::default()
    };
    let blank = BrushPreset {
        name: "   ".into(),
        ..BrushPreset::default()
    };
    let many: Vec<BrushPreset> = std::iter::once(odd)
        .chain(std::iter::once(blank))
        .chain((0..MAX_BRUSHES + 10).map(|_| BrushPreset::default()))
        .collect();
    let shelf = BrushShelf::restore(Nib::ROUND, &many, Some(0));
    assert_eq!(shelf.presets().len(), MAX_BRUSHES);
    assert!(shelf.presets()[0].name.chars().count() <= 40);
    assert!(!shelf.presets()[0].name.chars().any(char::is_control));
    assert_eq!(shelf.presets()[1].name, "Custom brush");
    assert_eq!(
        shelf.nib().shape(),
        NibShape::Square,
        "the chosen brush's nib is the one in hand"
    );
    // A chosen brush that is not there leaves the loose nib alone.
    let loose = BrushShelf::restore(square(20.0), &[], Some(3));
    assert_eq!(loose.selected(), None);
    assert_eq!(
        loose.nib(),
        Nib::ROUND,
        "choosing nothing means the plain brush"
    );
}

#[test]
fn built_in_profiles_set_both_the_nib_and_pressure_response() {
    let mut shelf = BrushShelf::default();
    shelf.choose_builtin(BuiltinBrushProfile::Calligraphy);
    assert_eq!(shelf.builtin(), Some(BuiltinBrushProfile::Calligraphy));
    assert_eq!(shelf.nib().shape(), NibShape::Square);
    assert_eq!(shelf.pressure_min(), 0.08);
    assert_eq!(shelf.pressure_gamma(), 1.0);
    shelf.set_pressure_curve(0.2, 2.0);
    assert_eq!(shelf.builtin(), None, "editing a preset makes it custom");
    assert_eq!(shelf.pressure_curve().width_factor(0.5), 0.4);
}

#[test]
fn custom_profiles_round_trip_as_bounded_versioned_json() {
    let preset = BrushPreset {
        pressure_min: 0.12,
        pressure_gamma: 1.7,
        ..BrushPreset::from_nib("Ink".to_owned(), square(25.0))
    };
    let encoded = BrushProfileFile::encode(std::slice::from_ref(&preset)).expect("encode");
    assert_eq!(
        BrushProfileFile::decode(&encoded).expect("decode"),
        [preset]
    );
    assert!(BrushProfileFile::decode(r#"{"version":2,"profiles":[]}"#).is_err());
    assert!(BrushProfileFile::decode(&" ".repeat(MAX_BRUSH_PROFILE_BYTES + 1)).is_err());
}
