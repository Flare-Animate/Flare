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

#[test]
fn a_version_one_file_without_the_new_fields_still_opens() {
    // Written by an earlier build: no drawn curve, no velocity, no flow. The defaults put the
    // brush back exactly as it drew then.
    let old = r#"{"version":1,"profiles":[{"name":"Marker","square":false,"angle":0.0,
        "flatness":100.0,"pressure_min":0.2,"pressure_gamma":1.3}]}"#;
    let presets = BrushProfileFile::decode(old).expect("decode");
    assert_eq!(presets.len(), 1);
    assert_eq!(presets[0].name, "Marker");
    assert_eq!(presets[0].velocity(), 0.0);
    assert_eq!(presets[0].flow(), 1.0);
    assert_eq!(presets[0].pressure_points(), None);
    let round_tripped = BrushProfileFile::encode(&presets).expect("encode");
    assert_eq!(
        BrushProfileFile::decode(&round_tripped).expect("decode"),
        presets
    );
}

#[test]
fn preset_responses_are_sanitized_when_read() {
    let wild = BrushPreset {
        pressure_points: Some(vec![(0.0, 0.2), (0.6, 0.5), (1.2, 4.0), (f32::NAN, 0.3)]),
        velocity: 9.0,
        flow: -3.0,
        ..BrushPreset::from_nib("Wild".into(), Nib::ROUND)
    }
    .sanitized();
    // Outside points are gone and the curve keeps its own floor and its full-width top.
    let points = wild.pressure_points().expect("curve");
    assert_eq!(points.first(), Some(&(0.0, wild.pressure_min())));
    assert_eq!(points.last(), Some(&(1.0, 1.0)));
    assert!(
        points
            .iter()
            .all(|(x, y)| (0.0..=1.0).contains(x) && (0.0..=1.0).contains(y))
    );
    assert_eq!(wild.velocity(), 1.0);
    assert_eq!(wild.flow(), 0.0);
    // A single point is no line: the gamma shape stays.
    let lone = BrushPreset {
        pressure_points: Some(vec![(0.5, 0.5)]),
        ..BrushPreset::from_nib("Lone".into(), Nib::ROUND)
    };
    assert_eq!(lone.pressure_points(), None);
    assert!(lone.pressure_curve().points().is_empty());
    let curve = wild.pressure_curve();
    assert_eq!(curve.width_factor(0.0), wild.pressure_min());
    assert_eq!(curve.width_factor(1.0), 1.0);
}

#[test]
fn importing_keeps_the_first_of_the_batch_chosen_deduplicates_names_and_refuses_a_full_shelf() {
    let preset = |name: &str| BrushPreset::from_nib(name.to_owned(), square(15.0));
    let mut shelf = BrushShelf::default();
    shelf.keep_nib().expect("room"); // "Custom brush 1"
    shelf
        .keep_presets(vec![
            preset("Crayon"),
            preset("Crayon"),
            preset("Custom brush 1"),
        ])
        .expect("room");
    assert_eq!(shelf.presets().len(), 4);
    assert_eq!(
        shelf.presets()[1..]
            .iter()
            .map(|p| p.name.as_str())
            .collect::<Vec<_>>(),
        ["Crayon", "Crayon 3", "Custom brush 1 4"],
    );
    assert_eq!(shelf.selected(), Some(1), "the first import lands in hand");
    assert_eq!(shelf.nib().angle_degrees(), 15.0, "its nib, too");
    assert!(shelf.builtin().is_none());
    // An empty import changes nothing at all.
    shelf.choose(None);
    shelf.keep_presets(vec![]).expect("nothing to keep");
    assert_eq!(shelf.selected(), None);
    // Not enough room refuses the whole batch and the shelf is as it was.
    for _ in shelf.presets().len()..MAX_BRUSHES {
        shelf.keep_nib().expect("room");
    }
    let before = shelf.clone();
    assert_eq!(shelf.keep_presets(vec![preset("Late")]), Err(ShelfFull));
    assert_eq!(shelf, before);
}

#[test]
fn velocity_and_flow_are_kept_set_and_restored() {
    let mut shelf = BrushShelf::default();
    shelf.set_velocity(0.4);
    shelf.set_flow(0.6);
    assert_eq!((shelf.velocity(), shelf.flow()), (0.4, 0.6));
    assert_eq!(
        shelf.builtin(),
        None,
        "editing a response takes the brush off the built-in list"
    );
    assert_eq!(shelf.pressure_curve().width_factor(1.0), 1.0);
    // Bad numbers stay in range.
    shelf.set_velocity(f32::NAN);
    shelf.set_flow(7.0);
    assert_eq!((shelf.velocity(), shelf.flow()), (0.0, 1.0));
    // A kept brush carries them; restoring brings them back.
    shelf.set_velocity(0.5);
    shelf.keep_nib().expect("room");
    assert_eq!(shelf.presets()[0].velocity(), 0.5);
    assert_eq!(shelf.presets()[0].flow(), 1.0);
    let restored = BrushShelf::restore(Nib::ROUND, shelf.presets(), Some(0));
    assert_eq!(
        (
            restored.velocity(),
            restored.flow(),
            restored.pressure_min()
        ),
        (0.5, 1.0, 0.15)
    );
    // Choosing a builtin loads its own response in hand.
    let mut built = BrushShelf::default();
    built.choose_builtin(BuiltinBrushProfile::Pencil);
    assert_eq!(built.velocity(), 0.2);
    assert_eq!(built.flow(), 0.9);
}

#[test]
fn a_pressed_keeps_the_drawn_curve_of_its_preset_in_hand() {
    let drawn = BrushPreset {
        pressure_points: Some(vec![(0.0, 0.3), (0.5, 0.6), (1.0, 1.0)]),
        velocity: 0.3,
        ..BrushPreset::from_nib("Marker".into(), square(0.0))
    };
    let mut shelf = BrushShelf::restore_profile(&drawn, std::slice::from_ref(&drawn), Some(0));
    assert_eq!(
        shelf.pressure_points().expect("curve").len(),
        3,
        "the drawn curve came off the preset"
    );
    assert_eq!(shelf.velocity(), 0.3);
    let curve = shelf.pressure_curve();
    // The curve's floor is its own: the drawn width at no pressure is the `pressure_min`.
    assert!((curve.width_factor(0.25) - (drawn.pressure_min() + 0.6) / 2.0).abs() < 1e-5);
    assert_eq!(curve.width_factor(0.0), drawn.pressure_min());
    // Editing the gamma curve drops the drawn points, and the kept brush loses them too.
    shelf.set_pressure_curve(0.2, 2.0);
    assert!(shelf.pressure_points().is_none());
    assert_eq!(shelf.presets()[0].pressure_points, None);
    assert_eq!(shelf.pressure_curve().width_factor(0.5), 0.4);
}
