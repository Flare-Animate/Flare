#![allow(clippy::expect_used, clippy::unwrap_used)]
//! Behaviour of the classical inbetweener on synthetic pictures whose motion is known.

use super::*;

/// A smooth colour field with no hard edges, so the flow has gradients to follow everywhere.
fn texture(x: f32, y: f32) -> [u8; 4] {
    let r = 128.0 + 60.0 * (0.3 * x).sin() * (0.25 * y).cos();
    let g = 128.0 + 60.0 * (0.17 * (x + y)).sin();
    let b = 128.0 + 50.0 * (0.21 * x - 0.13 * y).cos();
    [r as u8, g as u8, b as u8, 255]
}

fn picture(width: u32, height: u32, shift: (f32, f32)) -> Vec<u8> {
    let mut out = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            let p = texture(x as f32 - shift.0, y as f32 - shift.1);
            out.extend_from_slice(&p);
        }
    }
    out
}

/// The mean absolute difference over the channels of the pixels at least `margin` from the edge.
fn interior_error(got: &[u8], want: &[u8], width: u32, height: u32, margin: u32) -> f32 {
    let mut total = 0.0f32;
    let mut count = 0.0f32;
    for y in margin..height - margin {
        for x in margin..width - margin {
            let i = ((y * width + x) * 4) as usize;
            for k in 0..3 {
                total += (f32::from(got[i + k]) - f32::from(want[i + k])).abs();
                count += 1.0;
            }
        }
    }
    total / count
}

fn median(mut values: Vec<f32>) -> f32 {
    values.sort_by(f32::total_cmp);
    values[values.len() / 2]
}

#[test]
fn identical_pictures_are_a_fixed_point() {
    let (w, h) = (40, 32);
    let pixels = picture(w, h, (0.0, 0.0));
    let a = Raster::new(w, h, &pixels).expect("raster");
    let inbetweener = Inbetweener::new(a, a, FlowParams::default()).expect("flow");
    for t in [0.0, 0.25, 0.5, 0.75, 1.0] {
        assert_eq!(inbetweener.frame(t).expect("frame"), pixels, "t = {t}");
    }
    assert!(
        inbetweener
            .forward_flow()
            .iter()
            .all(|f| f[0] == 0.0 && f[1] == 0.0),
        "identical pictures have no motion"
    );
}

#[test]
fn pure_translation_is_recovered() {
    let (w, h) = (48, 40);
    let first = picture(w, h, (0.0, 0.0));
    let second = picture(w, h, (2.0, 1.0));
    let a = Raster::new(w, h, &first).expect("raster");
    let b = Raster::new(w, h, &second).expect("raster");
    let inbetweener = Inbetweener::new(a, b, FlowParams::default()).expect("flow");
    let flow = inbetweener.forward_flow();
    let mut xs = Vec::new();
    let mut ys = Vec::new();
    for y in 8..h - 8 {
        for x in 8..w - 8 {
            let f = flow[(y * w + x) as usize];
            xs.push(f[0]);
            ys.push(f[1]);
        }
    }
    let (mx, my) = (median(xs), median(ys));
    assert!((mx - 2.0).abs() < 0.2, "x offset {mx}");
    assert!((my - 1.0).abs() < 0.2, "y offset {my}");
}

#[test]
fn the_middle_picture_is_halfway_along_the_motion() {
    let (w, h) = (48, 40);
    let first = picture(w, h, (0.0, 0.0));
    let second = picture(w, h, (2.0, 1.0));
    let halfway = picture(w, h, (1.0, 0.5));
    let a = Raster::new(w, h, &first).expect("raster");
    let b = Raster::new(w, h, &second).expect("raster");
    let inbetweener = Inbetweener::new(a, b, FlowParams::default()).expect("flow");
    let middle = inbetweener.frame(0.5).expect("frame");
    let error = interior_error(&middle, &halfway, w, h, 8);
    let unmoved = interior_error(&middle, &first, w, h, 8);
    assert!(error < 4.0, "mean error {error} from the halfway picture");
    let to_second = interior_error(&middle, &second, w, h, 8);

    assert!(
        error * 2.0 < unmoved && error * 2.0 < to_second,
        "error {error}, first {unmoved}, second {to_second}"
    );
}

#[test]
fn the_ends_are_the_keyframes() {
    let (w, h) = (48, 40);
    let first = picture(w, h, (0.0, 0.0));
    let second = picture(w, h, (2.0, 1.0));
    let a = Raster::new(w, h, &first).expect("raster");
    let b = Raster::new(w, h, &second).expect("raster");
    let inbetweener = Inbetweener::new(a, b, FlowParams::default()).expect("flow");
    assert_eq!(inbetweener.frame(0.0).expect("frame"), first);
    assert_eq!(inbetweener.frame(1.0).expect("frame"), second);
}

/// A red square on blue that is gone in the second picture: it has no match there, so it must not
/// ghost into the in-between.
#[test]
fn a_vanishing_shape_does_not_ghost() {
    let (w, h) = (32, 32);
    let blue = [0u8, 0, 255, 255];
    let red = [255u8, 0, 0, 255];
    let mut first = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let inside = (10..18).contains(&x) && (10..18).contains(&y);
            first.extend_from_slice(if inside { &red } else { &blue });
        }
    }
    let second: Vec<u8> = blue
        .iter()
        .copied()
        .cycle()
        .take((w * h * 4) as usize)
        .collect();
    let a = Raster::new(w, h, &first).expect("raster");
    let b = Raster::new(w, h, &second).expect("raster");
    let inbetweener = Inbetweener::new(a, b, FlowParams::default()).expect("flow");
    let middle = inbetweener.frame(0.5).expect("frame");
    let i = ((13 * w + 13) * 4) as usize;
    assert!(middle[i] < 10, "red {} where the shape vanished", middle[i]);
    assert!(
        middle[i + 2] > 245,
        "blue {} where the background is",
        middle[i + 2]
    );
    assert!(
        !inbetweener.visible_a()[(13 * w + 13) as usize],
        "the vanished pixel is occluded"
    );
}

/// A red square that moves two pixels: halfway it is still red over the middle of its path.
#[test]
fn a_moving_shape_is_shown_in_the_middle() {
    let (w, h) = (32, 32);
    let square = |left: u32| {
        let mut out = Vec::new();
        for y in 0..h {
            for x in 0..w {
                let inside = (left..left + 8).contains(&x) && (10..18).contains(&y);
                out.extend_from_slice(if inside {
                    &[255, 0, 0, 255]
                } else {
                    &[0, 0, 255, 255]
                });
            }
        }
        out
    };
    let first = square(10);
    let second = square(12);
    let a = Raster::new(w, h, &first).expect("raster");
    let b = Raster::new(w, h, &second).expect("raster");
    let inbetweener = Inbetweener::new(a, b, FlowParams::default()).expect("flow");
    let middle = inbetweener.frame(0.5).expect("frame");
    let i = ((13 * w + 14) * 4) as usize;
    assert!(
        middle[i] > 200 && middle[i + 2] < 60,
        "pixel is {:?}",
        &middle[i..i + 4]
    );
}

#[test]
fn a_single_pixel_picture_is_its_own_in_between() {
    let a_pixels = [10u8, 20, 30, 255];
    let b_pixels = [200u8, 100, 50, 255];
    let a = Raster::new(1, 1, &a_pixels).expect("raster");
    let b = Raster::new(1, 1, &b_pixels).expect("raster");
    let inbetweener = Inbetweener::new(a, b, FlowParams::default()).expect("flow");
    assert_eq!(inbetweener.frame(0.0).expect("frame"), a_pixels.to_vec());
    assert_eq!(inbetweener.frame(1.0).expect("frame"), b_pixels.to_vec());
    assert_eq!(inbetweener.frame(0.5).expect("frame").len(), 4);
}
