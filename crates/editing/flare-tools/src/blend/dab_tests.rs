use super::*;

#[test]
fn a_hard_dab_covers_its_middle_and_ramps_over_one_pixel_at_the_rim() {
    let dab = Dab::new(10.0, 10.5, 8.0, 1.0, 1.0);
    assert_eq!(dab.coverage(10, 10), 1.0, "the middle is covered");
    assert_eq!(dab.coverage(12, 10), 1.0, "a pixel inside");
    // Pixel (13, 10) has its center 3.5 from the middle: half a pixel inside a radius of 4.
    assert!((dab.coverage(13, 10) - 0.5).abs() < 1e-4);
    assert_eq!(dab.coverage(14, 10), 0.0, "past the rim");
}

#[test]
fn a_soft_dab_fades_from_its_middle_to_nothing_at_its_reach() {
    let dab = Dab::new(50.0, 50.0, 40.0, 0.0, 1.0);
    let middle = dab.coverage(49, 49);
    let halfway = dab.coverage(49 + 10, 49);
    let near_rim = dab.coverage(49 + 30, 49);
    assert!(
        middle > halfway && halfway > near_rim,
        "{middle} {halfway} {near_rim}"
    );
    assert!(middle > 0.9);
    // A soft dab is drawn wider than its size: 1.75 times at no hardness.
    assert!((dab.radius - 35.0).abs() < 1e-4);
    assert_eq!(dab.coverage(49 + 36, 49), 0.0);
}

#[test]
fn hardness_flattens_the_top_of_a_soft_dab() {
    let soft = Dab::new(50.0, 50.0, 40.0, 0.0, 1.0);
    let firm = Dab::new(50.0, 50.0, 40.0, 0.8, 1.0);
    // Ten pixels out the firmer dab is still at full strength.
    assert!(firm.coverage(59, 49) > soft.coverage(59, 49));
    assert_eq!(firm.coverage(59, 49), 1.0);
}

#[test]
fn a_dab_thinner_than_the_least_drawn_is_drawn_wider_and_fainter() {
    let dab = Dab::new(5.0, 5.0, 0.9, 1.0, 1.0);
    assert!((dab.radius - MIN_DRAWN / 2.0).abs() < 1e-6);
    assert!((dab.strength - 0.5).abs() < 1e-6, "{}", dab.strength);
    let none = Dab::new(5.0, 5.0, 0.0, 1.0, 1.0);
    assert_eq!(none.strength, 0.0, "no pressure, no effect");
}

#[test]
fn a_small_soft_dab_is_hardened_and_keeps_its_weight() {
    let dab = Dab::new(5.0, 5.0, 3.0, 0.0, 1.0);
    // Pushed most of the way to 252.45 of 255, and a little weaker for it.
    assert!(
        dab.strength < 1.0 && dab.strength > 0.15,
        "{}",
        dab.strength
    );
    assert!(dab.coverage(4, 4) > 0.1);
}

#[test]
fn a_bad_size_gives_an_empty_dab_not_a_panic() {
    for size in [f32::NAN, f32::INFINITY, -4.0] {
        let dab = Dab::new(5.0, 5.0, size, 0.5, 1.0);
        assert_eq!(dab.strength, 0.0);
    }
}

#[test]
fn bounds_are_cut_to_the_bitmap_and_none_off_it() {
    let dab = Dab::new(2.0, 2.0, 10.0, 1.0, 1.0);
    assert_eq!(dab.bounds(100, 100), Some([0, 8, 0, 8]));
    let off = Dab::new(-50.0, 2.0, 10.0, 1.0, 1.0);
    assert_eq!(off.bounds(100, 100), None);
}

#[test]
fn spacing_is_closer_for_hard_dabs_and_each_close_dab_adds_less() {
    assert!((spacing(1.0) - 0.06).abs() < 1e-6);
    assert!((spacing(0.0) - 0.213).abs() < 1e-4);
    assert!((per_dab(0.1) - 0.3).abs() < 1e-6);
    assert_eq!(per_dab(0.5), 1.0);
}
