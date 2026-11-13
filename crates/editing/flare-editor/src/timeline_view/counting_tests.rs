use super::*;

#[test]
fn the_fifth_tenth_and_fifteenth_frames_are_shaded() {
    let shaded: Vec<u32> = (0..20).filter(|&i| is_fifth(i)).map(|i| i + 1).collect();
    assert_eq!(
        shaded,
        [5, 10, 15, 20],
        "counted from one, as the ruler shows them"
    );
    assert!(!is_fifth(u32::MAX), "no overflow at the last frame");
}

#[test]
fn the_ruler_numbers_frame_one_then_every_fifth() {
    let numbered: Vec<u32> = (0..21)
        .filter(|&i| is_numbered(i, 5))
        .map(|i| i + 1)
        .collect();
    assert_eq!(numbered, [1, 5, 10, 15, 20]);
    let tens: Vec<u32> = (0..30)
        .filter(|&i| is_numbered(i, 10))
        .map(|i| i + 1)
        .collect();
    assert_eq!(tens, [1, 10, 20, 30]);
    assert!(
        is_numbered(0, 0) && !is_numbered(4, 0),
        "a zero step numbers only the first"
    );
}

#[test]
fn narrow_frames_number_further_apart_in_round_steps() {
    assert_eq!(ruler_step(10.0), 5, "the usual width");
    assert_eq!(ruler_step(40.0), 5, "never closer than five");
    assert_eq!(ruler_step(4.0), 10);
    assert_eq!(ruler_step(3.0), 20);
    assert_eq!(ruler_step(1.0), 50);
    assert_eq!(ruler_step(0.5), 100);
    assert_eq!(ruler_step(0.1), 500);
    for bad in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        assert_eq!(ruler_step(bad), 5, "{bad}");
    }
    assert!(
        ruler_step(f32::MIN_POSITIVE) > 1_000_000,
        "it stops rather than overflow"
    );
    for width in [0.3, 1.0, 3.0, 10.0] {
        assert!(ruler_step(width) as f32 * width >= MIN_LABEL_GAP, "{width}");
    }
}
