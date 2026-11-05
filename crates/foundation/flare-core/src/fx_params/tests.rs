#![allow(clippy::expect_used)]
use super::*;

#[derive(Debug, PartialEq)]
struct Radius(f64);
impl Scalable for Radius {
    fn scale(&mut self, factor: f64) {
        self.0 *= factor;
    }
}

fn input(scale: f64) -> InputParam {
    InputParam {
        scale,
        ..InputParam::default()
    }
}

#[test]
fn from_input_holds_one_param_and_the_scale() {
    let p = Params::from_input(&input(2.0), Radius(3.0)).expect("valid");
    assert_eq!(p.params(), &[Radius(3.0)]);
    assert_eq!(p.scale_factor(), 2.0);
}

#[test]
fn apply_scale_uses_own_scale_and_scale_by_uses_given() {
    let mut p = Params::from_input(&input(2.0), Radius(3.0)).expect("valid");
    p.apply_scale();
    assert_eq!(p.params()[0], Radius(6.0));
    p.scale_by(0.5).expect("valid");
    assert_eq!(p.params()[0], Radius(3.0));
}

#[test]
fn econf_is_refused_by_name() {
    let i = InputParam {
        is_econf: true,
        ..input(1.0)
    };
    assert_eq!(
        Params::from_input(&i, Radius(1.0)),
        Err(FxParamsError::EconfUnsupported)
    );
}

#[test]
fn bad_scales_are_refused_and_change_nothing() {
    for s in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert_eq!(
            Params::from_input(&input(s), Radius(1.0)),
            Err(FxParamsError::BadScale)
        );
    }
    let mut p = Params::from_input(&input(1.0), Radius(4.0)).expect("valid");
    assert_eq!(p.scale_by(f64::NAN), Err(FxParamsError::BadScale));
    assert_eq!(p.params()[0], Radius(4.0));
}

#[test]
fn push_is_bounded() {
    let mut p: Params<Radius> = Params::default();
    for _ in 0..MAX_PARAMS {
        p.push(Radius(1.0)).expect("room");
    }
    assert_eq!(p.push(Radius(1.0)), Err(FxParamsError::TooMany));
    assert_eq!(p.params().len(), MAX_PARAMS);
}
