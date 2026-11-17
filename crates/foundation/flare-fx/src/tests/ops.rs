//! Each node's pixel work, on small pictures whose answer can be worked out by hand.

use flare_core::Rgba8;

use super::{square, total_alpha};
use crate::Pixels;
use crate::ops::blur::blur;
use crate::ops::color::{Adjustment, adjust, tint};
use crate::ops::composite::{fade_in_place, over_in_place};
use crate::ops::shadow::shadow;
use crate::ops::snow::{Snowfall, snow};

#[test]
fn a_blur_spreads_an_edge_and_keeps_what_was_there() {
    let mut p = square(40, 40, [10, 10, 30, 30], [255, 0, 0, 255]);
    let before = total_alpha(&p);
    let (w, h) = (40, 40);
    blur(p.data_mut(), w, h, 4, 6.0);
    // Far from any edge the square is still solid; just outside it, it is not empty anymore.
    assert_eq!(p.pixel(20, 20), [255, 0, 0, 255]);
    assert!(p.pixel(8, 20)[3] > 0, "the edge spread out");
    assert!(p.pixel(11, 20)[3] < 255, "and softened inside");
    let after = total_alpha(&p);
    let kept = after as f64 / before as f64;
    assert!((0.97..=1.03).contains(&kept), "about the same ink: {kept}");
    // Premultiplied: no color is ever more than its alpha.
    assert!(p.data().as_chunks::<4>().0.iter().all(|c| c[0] <= c[3]));
}

#[test]
fn a_blur_under_half_a_pixel_changes_nothing() {
    let mut p = square(8, 8, [2, 2, 6, 6], [0, 0, 255, 255]);
    let before = p.clone();
    blur(p.data_mut(), 8, 8, 4, 0.4);
    assert_eq!(p, before);
}

#[test]
fn a_shadow_falls_where_the_offset_says_under_the_picture() {
    let p = square(40, 40, [5, 5, 15, 15], [255, 255, 255, 255]);
    let out = shadow(&p, [10.0, 0.0], 0.0, Rgba8::opaque(0, 0, 0), 1.0);
    assert_eq!(
        out.pixel(10, 10),
        [255, 255, 255, 255],
        "the picture stays on top"
    );
    assert_eq!(
        out.pixel(20, 10),
        [0, 0, 0, 255],
        "the shadow is ten pixels right"
    );
    assert_eq!(out.pixel(30, 10), [0, 0, 0, 0], "and goes no further");
    assert_eq!(out.pixel(10, 20), [0, 0, 0, 0]);
}

#[test]
fn a_half_pixel_offset_shares_the_shadow_between_two_pixels() {
    let p = square(10, 3, [2, 1, 3, 2], [255, 255, 255, 255]);
    let out = shadow(&p, [3.5, 0.0], 0.0, Rgba8::opaque(0, 0, 0), 1.0);
    let (a, b) = (out.pixel(5, 1)[3], out.pixel(6, 1)[3]);
    assert!(
        (120..=135).contains(&a) && (120..=135).contains(&b),
        "{a} {b}"
    );
}

#[test]
fn a_glow_strength_past_one_makes_the_halo_stronger_but_not_past_opaque() {
    let p = square(30, 30, [10, 10, 20, 20], [0, 0, 0, 255]);
    let soft = shadow(&p, [0.0, 0.0], 8.0, Rgba8::opaque(255, 200, 0), 1.0);
    let strong = shadow(&p, [0.0, 0.0], 8.0, Rgba8::opaque(255, 200, 0), 4.0);
    assert!(strong.pixel(7, 15)[3] > soft.pixel(7, 15)[3]);
    assert!(
        strong
            .data()
            .as_chunks::<4>()
            .0
            .iter()
            .all(|c| c[0] <= c[3])
    );
    assert_eq!(
        strong.pixel(15, 15),
        [0, 0, 0, 255],
        "the picture is on top"
    );
}

#[test]
fn hue_turns_red_toward_green_and_saturation_down_makes_grey() {
    let mut p = square(1, 1, [0, 0, 1, 1], [255, 0, 0, 255]);
    let none = Adjustment {
        brightness: 0.0,
        contrast: 0.0,
        saturation: 0.0,
        hue: 0.0,
    };
    adjust(&mut p, Adjustment { hue: 120.0, ..none });
    let [r, g, b, a] = p.pixel(0, 0);
    assert!(
        g > r && g > b,
        "red turned by a third is green: {r} {g} {b}"
    );
    assert_eq!(a, 255);

    let mut p = square(1, 1, [0, 0, 1, 1], [200, 40, 90, 255]);
    adjust(
        &mut p,
        Adjustment {
            saturation: -1.0,
            ..none
        },
    );
    let [r, g, b, _] = p.pixel(0, 0);
    assert!(
        r.abs_diff(g) <= 1 && g.abs_diff(b) <= 1,
        "grey: {r} {g} {b}"
    );

    let mut p = square(1, 1, [0, 0, 1, 1], [100, 100, 100, 255]);
    adjust(
        &mut p,
        Adjustment {
            brightness: 1.0,
            ..none
        },
    );
    assert_eq!(p.pixel(0, 0), [255, 255, 255, 255]);
}

#[test]
fn an_adjustment_keeps_transparent_pixels_transparent_and_half_covered_ones_premultiplied() {
    let mut p = square(2, 1, [0, 0, 1, 1], [64, 0, 0, 128]);
    adjust(
        &mut p,
        Adjustment {
            brightness: 1.0,
            contrast: 0.0,
            saturation: 0.0,
            hue: 0.0,
        },
    );
    assert_eq!(p.pixel(0, 0), [128, 128, 128, 128]);
    assert_eq!(p.pixel(1, 0), [0, 0, 0, 0]);
}

#[test]
fn a_tint_mixes_toward_its_color_and_keeps_the_shape() {
    let mut p = square(2, 1, [0, 0, 1, 1], [0, 0, 0, 255]);
    tint(&mut p, Rgba8::opaque(200, 100, 0), 0.5);
    assert_eq!(p.pixel(0, 0), [100, 50, 0, 255]);
    assert_eq!(
        p.pixel(1, 0),
        [0, 0, 0, 0],
        "nothing is added where nothing was"
    );
}

#[test]
fn over_lays_the_top_on_the_bottom_and_fade_scales_everything() {
    let mut under = square(2, 1, [0, 0, 2, 1], [0, 0, 255, 255]);
    let top = square(2, 1, [0, 0, 1, 1], [128, 0, 0, 128]);
    over_in_place(&mut under, &top);
    assert_eq!(under.pixel(0, 0), [128, 0, 127, 255]);
    assert_eq!(under.pixel(1, 0), [0, 0, 255, 255]);
    fade_in_place(&mut under, 0.5);
    assert_eq!(under.pixel(1, 0), [0, 0, 128, 128]);
}

fn fall() -> Snowfall {
    Snowfall {
        flakes: 200,
        size: 4.0,
        speed: 3.0,
        wind: 1.0,
        sway: 4.0,
        seed: 7,
        color: Rgba8::WHITE,
        opacity: 1.0,
    }
}

fn snowed(frame: u32, fall: &Snowfall) -> Pixels {
    let mut p = Pixels::transparent(120, 80);
    snow(&mut p, fall, [0.0, 0.0, 120.0, 80.0], frame);
    p
}

#[test]
fn snow_is_the_same_on_the_same_frame_and_moves_on_the_next() {
    let a = snowed(10, &fall());
    assert_eq!(a, snowed(10, &fall()), "a frame always shows the same snow");
    assert_ne!(a, snowed(11, &fall()), "and it falls");
    assert_ne!(
        a,
        snowed(10, &Snowfall { seed: 8, ..fall() }),
        "another seed, other flakes"
    );
    assert!(total_alpha(&a) > 0);
    assert_eq!(
        total_alpha(&snowed(
            10,
            &Snowfall {
                flakes: 0,
                ..fall()
            }
        )),
        0
    );
    assert_eq!(
        total_alpha(&snowed(
            10,
            &Snowfall {
                opacity: 0.0,
                ..fall()
            }
        )),
        0
    );
}

#[test]
fn snow_stays_inside_the_stage() {
    let mut p = Pixels::transparent(100, 100);
    snow(
        &mut p,
        &Snowfall {
            flakes: 600,
            ..fall()
        },
        [20.0, 30.0, 60.0, 70.0],
        3,
    );
    for y in 0..100 {
        for x in 0..100 {
            let inside = (20..60).contains(&x) && (30..70).contains(&y);
            if !inside {
                assert_eq!(p.pixel(x, y)[3], 0, "flake outside the stage at {x},{y}");
            }
        }
    }
    assert!(total_alpha(&p) > 0);
}

#[test]
fn snow_off_the_picture_draws_nothing_and_does_not_fail() {
    let mut p = Pixels::transparent(10, 10);
    snow(&mut p, &fall(), [500.0, 500.0, 900.0, 900.0], 2);
    assert_eq!(total_alpha(&p), 0);
    snow(&mut p, &fall(), [0.0, 0.0, 0.0, 0.0], 2);
    assert_eq!(total_alpha(&p), 0);
}
