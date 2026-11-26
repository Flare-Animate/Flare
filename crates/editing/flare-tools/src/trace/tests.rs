#![allow(clippy::expect_used)]

use std::num::NonZeroU32;

use flare_commands::{Command, apply_all};
use flare_core::{BitmapId, InstanceId, Transform2D};
use flare_document::{Bitmap, BitmapInstance, Content, FrameSpan};
use glamx::{DAffine2, DVec2};

use super::*;
use crate::test_support::fixture;

const RED: [u8; 4] = [255, 0, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];
const CLEAR: [u8; 4] = [0, 0, 0, 0];

/// A `w` by `h` picture whose pixel `(x, y)` is `color(x, y)`.
fn picture(w: u32, h: u32, color: impl Fn(u32, u32) -> [u8; 4]) -> Bitmap {
    let rgba = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .flat_map(|(x, y)| color(x, y))
        .collect();
    Bitmap::new(BitmapId::new(), "p", w, h, rgba).expect("bitmap")
}

#[test]
fn a_clear_picture_traces_to_nothing() {
    let clear = picture(8, 8, |_, _| CLEAR);
    assert!(trace_colors(&clear).expect("trace").is_empty());
}

#[test]
fn two_colors_are_stacked_the_larger_under_and_covering_the_smaller() {
    // A 12 by 8 red field with a 4 by 4 blue square in it, clear all around.
    let bitmap = picture(14, 10, |x, y| {
        if !(1..13).contains(&x) || !(1..9).contains(&y) {
            CLEAR
        } else if (5..9).contains(&x) && (3..7).contains(&y) {
            BLUE
        } else {
            RED
        }
    });
    let colors = trace_colors(&bitmap).expect("trace");
    assert_eq!(colors.len(), 2);
    let (under, over) = (&colors[0], &colors[1]);
    assert_eq!(
        under.0,
        Rgba8::new(255, 0, 0, 255),
        "the larger color is first"
    );
    assert_eq!(over.0, Rgba8::new(0, 0, 255, 255));
    // The red shape covers the whole field, the blue square included: no seam can open.
    let xs: Vec<f64> = under.1.iter().flatten().map(|p| p.0).collect();
    let ys: Vec<f64> = under.1.iter().flatten().map(|p| p.1).collect();
    let span = |v: &[f64]| {
        (
            v.iter().copied().fold(f64::INFINITY, f64::min),
            v.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        )
    };
    assert_eq!(span(&xs), (1.0, 13.0));
    assert_eq!(span(&ys), (1.0, 9.0));
    assert_eq!(
        under.1.len(),
        1,
        "one outline: the blue is inside it, not a hole"
    );
    let bx: Vec<f64> = over.1.iter().flatten().map(|p| p.0).collect();
    assert_eq!(span(&bx), (5.0, 9.0));
}

#[test]
fn many_shades_are_gathered_into_a_bounded_number_of_colors() {
    let gradient = picture(256, 4, |x, _| [x as u8, 0, 255 - x as u8, 255]);
    let colors = trace_colors(&gradient).expect("trace");
    assert!(
        !colors.is_empty() && colors.len() <= MAX_COLORS,
        "{}",
        colors.len()
    );
}

#[test]
fn a_keyframes_pictures_become_shapes_on_the_stage_as_one_batch() {
    let mut f = fixture();
    let bitmap = picture(10, 10, |x, y| if x < 5 && y < 5 { RED } else { CLEAR });
    let id = bitmap.id();
    // Two pixels to a stage unit, placed at (100, 50).
    let matrix =
        DAffine2::from_scale_angle_translation(DVec2::splat(0.5), 0.0, DVec2::new(100.0, 50.0));
    let placed = BitmapInstance::new(
        InstanceId::new(),
        id,
        Transform2D::new(matrix).expect("finite"),
    );
    f.session
        .apply([
            Command::AddBitmap(bitmap),
            Command::InsertSpan {
                scene: f.scene,
                layer: f.layer,
                span: FrameSpan::new(0, NonZeroU32::MIN, Content::Bitmap(placed)),
            },
        ])
        .expect("place");
    let commands = trace_keyframe(f.session.document(), f.scene, f.layer, 0).expect("trace");
    let after = apply_all(f.session.document(), commands)
        .expect("valid")
        .expect("changed");
    assert!(after.bitmaps().is_empty(), "the picture left the library");
    let span = after.scenes()[0].timeline().layers()[0]
        .span_at(0)
        .expect("keyframe");
    let shapes: Vec<&flare_document::Shape> = span
        .elements()
        .filter_map(|c| match c {
            Content::Shape(s) => Some(s),
            _ => None,
        })
        .collect();
    assert_eq!(shapes.len(), 1);
    assert_eq!(shapes[0].fill(), Rgba8::new(255, 0, 0, 255));
    let points: Vec<DVec2> = shapes[0]
        .path()
        .iter()
        .filter_map(|seg| match seg {
            flare_document::PathSegment::MoveTo(p) | flare_document::PathSegment::LineTo(p) => {
                Some(p.as_vec())
            }
            _ => None,
        })
        .collect();
    let min = points.iter().copied().fold(DVec2::INFINITY, DVec2::min);
    let max = points.iter().copied().fold(DVec2::NEG_INFINITY, DVec2::max);
    // Five pixels at half a unit each, from (100, 50).
    assert!((min - DVec2::new(100.0, 50.0)).length() < 1e-6, "{min}");
    assert!((max - DVec2::new(102.5, 52.5)).length() < 1e-6, "{max}");

    // Nothing to trace is nothing to do.
    assert!(
        trace_keyframe(&after, f.scene, f.layer, 0)
            .expect("no picture")
            .is_empty()
    );
}
