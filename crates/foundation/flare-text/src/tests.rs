#![allow(clippy::expect_used)]

use super::*;
use flare_core::DocumentPoint;

/// An engine that knows the Hack font by name, so tests do not depend on what the machine
/// has installed.
fn engine() -> TextEngine {
    let mut engine = TextEngine::new();
    let names = engine.register_font(epaint_default_fonts::HACK_REGULAR.to_vec());
    assert!(names.iter().any(|n| n == "Hack"), "{names:?}");
    engine
}

fn style(size: f32) -> TextStyle {
    TextStyle {
        bold: false,
        italic: false,
        family: "Hack".into(),
        size,
    }
}

fn bounds(path: &[PathSegment]) -> (f64, f64, f64, f64) {
    let mut b = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    let mut add = |p: &DocumentPoint| {
        let v = p.as_vec();
        b = (b.0.min(v.x), b.1.min(v.y), b.2.max(v.x), b.3.max(v.y));
    };
    for s in path {
        match s {
            PathSegment::MoveTo(p) | PathSegment::LineTo(p) => add(p),
            PathSegment::CubicTo(a, c, p) => {
                add(a);
                add(c);
                add(p);
            }
            PathSegment::Close => {}
        }
    }
    b
}

#[test]
fn letters_become_closed_outlines_inside_their_line() {
    let out = engine().outline("Hi", &style(40.0)).expect("outline");
    assert!(matches!(out.path[0], PathSegment::MoveTo(_)));
    assert!(out.path.iter().any(|s| matches!(s, PathSegment::Close)));
    let (min_x, min_y, max_x, max_y) = bounds(&out.path);
    assert!(
        min_x >= -1.0 && max_x <= f64::from(out.width) + 1.0,
        "{min_x}..{max_x} in {}",
        out.width
    );
    assert!(
        min_y >= -1.0 && max_y <= f64::from(out.height) + 1.0,
        "{min_y}..{max_y} in {}",
        out.height
    );
    assert!(
        max_y - min_y > 20.0,
        "a 40 unit capital is tall: {}",
        max_y - min_y
    );
}

#[test]
fn text_scales_with_the_size_and_grows_with_its_length() {
    let mut e = engine();
    let small = e.outline("ab", &style(20.0)).expect("small");
    let big = e.outline("ab", &style(40.0)).expect("big");
    assert!(
        (big.width / small.width - 2.0).abs() < 0.05,
        "{} vs {}",
        big.width,
        small.width
    );
    let longer = e.outline("abab", &style(20.0)).expect("longer");
    assert!(
        (longer.width / small.width - 2.0).abs() < 0.05,
        "a monospace font"
    );
}

#[test]
fn newlines_start_new_lines() {
    let mut e = engine();
    let one = e.outline("ab", &style(30.0)).expect("one line");
    let two = e.outline("ab\nab", &style(30.0)).expect("two lines");
    assert!(
        (two.height / one.height - 2.0).abs() < 0.1,
        "{} vs {}",
        two.height,
        one.height
    );
    assert!(
        (two.width - one.width).abs() < 1.0,
        "the width is that of the widest line"
    );
}

#[test]
fn a_glyph_with_a_hole_keeps_it_as_a_second_contour() {
    let out = engine().outline("o", &style(60.0)).expect("outline");
    let contours = out
        .path
        .iter()
        .filter(|s| matches!(s, PathSegment::MoveTo(_)))
        .count();
    assert_eq!(contours, 2, "outer and inner contour of an o");
    assert!(
        out.path
            .iter()
            .any(|s| matches!(s, PathSegment::CubicTo(..))),
        "curves"
    );
}

#[test]
fn unusable_requests_are_refused_with_reasons() {
    let mut e = engine();
    assert_eq!(
        e.outline("", &style(20.0)).expect_err("refused"),
        TextError::Empty
    );
    assert_eq!(
        e.outline("  \n ", &style(20.0)).expect_err("refused"),
        TextError::Empty
    );
    for size in [0.0, 0.5, MAX_SIZE + 1.0, f32::NAN, f32::INFINITY, -4.0] {
        assert_eq!(
            e.outline("a", &style(size)).expect_err("refused"),
            TextError::BadSize,
            "{size}"
        );
    }
    let long = "a".repeat(MAX_CHARS + 1);
    assert_eq!(
        e.outline(&long, &style(10.0)).expect_err("refused"),
        TextError::TooLong
    );
    assert!(
        e.outline(&"a".repeat(MAX_CHARS), &style(10.0)).is_ok(),
        "the limit itself works"
    );
}

#[test]
fn an_unknown_family_falls_back_instead_of_failing() {
    let mut e = engine();
    let style = TextStyle {
        bold: false,
        italic: false,
        family: "No Such Font, Hack".into(),
        size: 30.0,
    };
    assert!(e.outline("a", &style).is_ok());
}

#[test]
fn bold_and_italic_are_accepted_and_still_give_outlines() {
    let mut engine = engine();
    for (bold, italic) in [(true, false), (false, true), (true, true)] {
        let styled = TextStyle {
            bold,
            italic,
            ..style(32.0)
        };
        let outline = engine.outline("Flare", &styled).expect("outline");
        assert!(!outline.path.is_empty() && outline.width > 0.0);
    }
}
