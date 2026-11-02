#![allow(clippy::expect_used)]

use super::*;
use crate::xml;

fn fill(inner: &str) -> Node {
    let text = format!(r##"<FillStyle index="1">{inner}</FillStyle>"##);
    xml::parse(text.as_bytes()).expect("xml")
}

const RED: Rgba8 = Rgba8::opaque(0xFF, 0, 0);
const BLUE: Rgba8 = Rgba8::opaque(0, 0, 0xFF);

/// A gradient square 720 px long running top to bottom over a 1280 by 720 shape, as the SWF
/// convention puts one: a turn of a quarter, so the matrix scale is on b and c.
fn vertical() -> String {
    let k = 720.0 * 20.0 / 32768.0;
    format!(
        r##"<LinearGradient><matrix><Matrix a="0" b="{k}" c="{}" d="0" tx="12800" ty="7200"/></matrix>
<GradientEntry color="#FF0000" ratio="0"/><GradientEntry color="#0000FF" ratio="1"/></LinearGradient>"##,
        -k
    )
}

const BOUNDS: (f64, f64, f64, f64) = (0.0, 0.0, 1280.0, 720.0);

#[test]
fn a_linear_gradient_runs_from_its_first_stop_to_its_last_along_the_matrix() {
    let read = read(&fill(&vertical())).expect("gradient");
    assert!(!read.radial && !read.extra_stops);
    assert_eq!(
        read.flat,
        Rgba8::opaque(127, 0, 127),
        "halfway between the stops"
    );
    let gradient = place(&read.linear.expect("linear"), BOUNDS).expect("placed");
    let (from, to) = (gradient.from().as_vec(), gradient.to().as_vec());
    assert!(
        (from.x - 640.0).abs() < 0.01 && from.y.abs() < 0.01,
        "{from:?}"
    );
    assert!(
        (to.x - 640.0).abs() < 0.01 && (to.y - 720.0).abs() < 0.01,
        "{to:?}"
    );
    assert_eq!((gradient.start(), gradient.end()), (RED, BLUE));
}

#[test]
fn stops_inside_the_square_start_and_end_the_gradient_there() {
    let inner = vertical()
        .replace(r#"ratio="0""#, r#"ratio="0.25""#)
        .replace(r#"ratio="1""#, r#"ratio="0.75""#);
    let read = read(&fill(&inner)).expect("gradient");
    let gradient = place(&read.linear.expect("linear"), BOUNDS).expect("placed");
    assert!(
        (gradient.from().as_vec().y - 180.0).abs() < 0.01,
        "{:?}",
        gradient.from()
    );
    assert!(
        (gradient.to().as_vec().y - 540.0).abs() < 0.01,
        "{:?}",
        gradient.to()
    );
}

#[test]
fn a_gradient_with_equal_stops_has_no_direction_and_is_one_color() {
    let inner = vertical().replace(r#"ratio="1""#, r#"ratio="0""#);
    let read = read(&fill(&inner)).expect("gradient");
    assert!(place(&read.linear.expect("linear"), BOUNDS).is_none());
}

#[test]
fn a_square_of_the_wrong_size_or_in_the_wrong_place_is_not_laid_on_the_shape() {
    let linear = read(&fill(&vertical()))
        .expect("gradient")
        .linear
        .expect("linear");
    // The numbers read as pixels instead of twips: a square 64 px long over a shape 1469 across.
    let as_pixels = fill(
        r##"<LinearGradient><matrix><Matrix a="0.0390625" tx="640" ty="360"/></matrix>
<GradientEntry color="#FF0000" ratio="0"/><GradientEntry color="#0000FF" ratio="1"/></LinearGradient>"##,
    );
    let pixels = read(&as_pixels).expect("gradient").linear.expect("linear");
    assert!(place(&pixels, BOUNDS).is_none());
    // The right square, but its middle is far from the shape.
    let far = fill(&vertical().replace(r#"tx="12800""#, r#"tx="1280000""#));
    let far = read(&far).expect("gradient").linear.expect("linear");
    assert!(place(&far, BOUNDS).is_none());
    // And a shape with no size has nothing to be laid on.
    assert!(place(&linear, (5.0, 5.0, 5.0, 5.0)).is_none());
}

#[test]
fn a_radial_gradient_is_one_color_and_says_so() {
    let radial = fill(
        r##"<RadialGradient><matrix><Matrix a="0.0244" d="0.0244"/></matrix>
<GradientEntry color="#FFFFFF" ratio="0"/><GradientEntry color="#CC0000" ratio="1"/></RadialGradient>"##,
    );
    let read = read(&radial).expect("gradient");
    assert!(read.radial && read.linear.is_none());
    assert_eq!(read.flat, Rgba8::opaque(229, 127, 127));
}

#[test]
fn more_than_two_stops_keep_the_first_and_the_last() {
    let three = vertical().replace(
        r##"<GradientEntry color="#0000FF" ratio="1"/>"##,
        r##"<GradientEntry color="#00FF00" ratio="0.5"/><GradientEntry color="#0000FF" ratio="1"/>"##,
    );
    let read = read(&fill(&three)).expect("gradient");
    assert!(read.extra_stops);
    let linear = read.linear.expect("linear");
    assert_eq!((linear.first, linear.last), (RED, BLUE));
}

#[test]
fn a_gradient_with_one_stop_is_that_color_and_one_with_none_or_bad_colors_is_nothing() {
    let one =
        fill(r##"<LinearGradient><GradientEntry color="#336699" ratio="0"/></LinearGradient>"##);
    let read = read(&one).expect("gradient");
    assert_eq!(read.flat, Rgba8::opaque(0x33, 0x66, 0x99));
    assert!(read.linear.is_none());
    assert!(
        super::read(&fill("<LinearGradient/>")).is_none(),
        "no stops"
    );
    assert!(
        super::read(&fill(
            r##"<LinearGradient><GradientEntry color="nope" ratio="0"/></LinearGradient>"##
        ))
        .is_none()
    );
    assert!(super::read(&fill(r##"<SolidColor color="#FF0000"/>"##)).is_none());
}

#[test]
fn nonsense_numbers_in_the_matrix_fall_back_to_the_defaults_and_a_stop_past_the_end_is_clamped() {
    let nonsense = fill(
        r##"<LinearGradient><matrix><Matrix a="NaN" b="inf" tx="x"/></matrix>
<GradientEntry color="#FF0000" ratio="0"/><GradientEntry color="#0000FF" ratio="9"/></LinearGradient>"##,
    );
    let read = read(&nonsense).expect("gradient");
    // What is not a number is as good as left out, which is the identity matrix: a square 1638
    // px long along x, which happens to be about the size of this shape. The stop at 9 is the end.
    let gradient = place(&read.linear.expect("linear"), BOUNDS).expect("placed");
    let (from, to) = (gradient.from().as_vec(), gradient.to().as_vec());
    assert!((to.x - from.x - 1638.4).abs() < 0.01, "{from:?} {to:?}");
    assert!(from.y.abs() < 1e-9 && to.y.abs() < 1e-9);
}
