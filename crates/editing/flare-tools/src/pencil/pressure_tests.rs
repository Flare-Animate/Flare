#![allow(clippy::expect_used)]

use flare_input::{InputSample, PointerKind};

use super::*;

fn drag(kind: PointerKind, pressure: impl Fn(f64) -> f32) -> Vec<InputSample> {
    (0..=100)
        .map(|i| {
            let t = f64::from(i) / 100.0;
            InputSample::new(t * 200.0, 0.0, pressure(t), kind).expect("sample")
        })
        .collect()
}

#[test]
fn only_a_pen_line_follows_pressure_and_only_while_it_is_on() {
    let pen = drag(PointerKind::Pen, |_| 0.5);
    let touch = drag(PointerKind::Touch, |_| 0.5);
    let mouse = drag(PointerKind::Mouse, |_| 1.0);
    let on = PencilPressure::default();
    assert!(on.applies(&pen) && on.applies(&touch));
    assert!(!on.applies(&mouse));
    // A pen held hard all the way draws a line of one width, the same as a mouse.
    assert!(!on.applies(&drag(PointerKind::Pen, |_| 1.0)));
    assert!(on.applies(&drag(PointerKind::Pen, |t| if t > 0.9 { 0.6 } else { 1.0 })));
    assert!(!PencilPressure::new(false, 0.1).applies(&pen));
}

#[test]
fn a_pressed_pencil_line_thins_where_the_pen_is_light() {
    let pencil = PencilPressure::new(true, 0.1);
    let samples = drag(PointerKind::Pen, |t| {
        (t * std::f64::consts::PI).sin() as f32
    });
    let stroke = pencil.stroke(4.0);
    let factors = stroke.width_factors(&crate::brush::BrushStroke::spine(&samples));
    assert!((factors[0] - 0.1).abs() < 1e-6, "lightest touch");
    assert!((factors[50] - 1.0).abs() < 1e-6, "full press");
    assert!(stroke.texture.is_solid());
    assert!(stroke.outline(&samples, 0.25).is_some());
}

#[test]
fn the_lightest_touch_is_kept_in_range() {
    assert_eq!(PencilPressure::new(true, 4.0).light(), 1.0);
    assert_eq!(PencilPressure::new(true, -1.0).light(), 0.0);
    assert_eq!(
        PencilPressure::new(true, f32::NAN).light(),
        DEFAULT_PENCIL_LIGHT
    );
}
