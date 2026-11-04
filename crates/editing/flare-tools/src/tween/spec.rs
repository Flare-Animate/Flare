//! The words the command line and the MCP server use for interpolation, in one place so both
//! mean the same thing: list the families, build a keyframe's interpolation from names, evaluate
//! a curve at a time.

use flare_document::SpanInterp;
use flare_tween::{Easing, HandleKind, Interpolation, Params};

/// One interpolation family and the easings that change it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FamilyInfo {
    pub name: &'static str,
    /// Whether the easing (in, out, in-out, auto) changes this family at all.
    pub eased: bool,
}

pub fn families() -> Vec<FamilyInfo> {
    Interpolation::ALL
        .into_iter()
        .map(|i| FamilyInfo {
            name: i.name(),
            eased: !matches!(
                i,
                Interpolation::Constant | Interpolation::Linear | Interpolation::Bezier
            ),
        })
        .collect()
}

pub fn easing_names() -> Vec<&'static str> {
    Easing::ALL.into_iter().map(Easing::name).collect()
}

pub fn handle_names() -> Vec<&'static str> {
    HandleKind::ALL.into_iter().map(HandleKind::name).collect()
}

/// A keyframe's interpolation from names. Easing defaults to `auto`, the handle type to
/// `auto-clamped`; `handles` is `[right.dt, right.dv, left.dt, left.dv]` of a Bezier segment.
///
/// # Errors
/// An unknown name (the message lists the valid ones) or a parameter out of range.
pub fn build_interp(
    family: &str,
    easing: Option<&str>,
    handle: Option<&str>,
    handles: Option<[f64; 4]>,
) -> Result<SpanInterp, String> {
    let family = Interpolation::parse(family).ok_or_else(|| {
        let all: Vec<_> = Interpolation::ALL.iter().map(|i| i.name()).collect();
        format!(
            "unknown interpolation {family:?}; one of {}",
            all.join(", ")
        )
    })?;
    let easing = match easing {
        None => Easing::Auto,
        Some(name) => Easing::parse(name).ok_or_else(|| {
            format!(
                "unknown easing {name:?}; one of {}",
                easing_names().join(", ")
            )
        })?,
    };
    let handle = match handle {
        None => HandleKind::AutoClamped,
        Some(name) => HandleKind::parse(name).ok_or_else(|| {
            format!(
                "unknown handle type {name:?}; one of {}",
                handle_names().join(", ")
            )
        })?,
    };
    SpanInterp::new(
        family,
        easing,
        handle,
        handles.unwrap_or([0.0; 4]),
        Params::default(),
    )
    .map_err(|e| e.to_string())
}

/// The fraction of the change made at `t` (0 to 1) of the time.
///
/// # Errors
/// As `build_interp`, or a `t` that is not a finite number in 0 to 1.
pub fn eval(
    family: &str,
    easing: Option<&str>,
    handle: Option<&str>,
    handles: Option<[f64; 4]>,
    t: f64,
) -> Result<f64, String> {
    if !t.is_finite() || !(0.0..=1.0).contains(&t) {
        return Err(format!("t must be a number from 0 to 1, not {t}"));
    }
    Ok(build_interp(family, easing, handle, handles)?.fraction(t))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;

    #[test]
    fn lists_every_family_and_marks_the_unaffected_ones() {
        let all = families();
        assert_eq!(all.len(), Interpolation::ALL.len());
        assert!(
            !all.iter()
                .find(|f| f.name == "linear")
                .expect("linear")
                .eased
        );
        assert!(
            all.iter()
                .find(|f| f.name == "bounce")
                .expect("bounce")
                .eased
        );
    }

    #[test]
    fn eval_matches_the_curve_and_refuses_bad_input() {
        assert_eq!(eval("linear", None, None, None, 0.25), Ok(0.25));
        assert!(eval("quad", Some("in"), None, None, 0.5).expect("quad") < 0.5);
        assert!(
            eval("wobble", None, None, None, 0.5)
                .unwrap_err()
                .contains("one of")
        );
        assert!(eval("quad", Some("sideways"), None, None, 0.5).is_err());
        assert!(eval("quad", None, Some("bent"), None, 0.5).is_err());
        assert!(eval("quad", None, None, None, 1.5).is_err());
        assert!(eval("quad", None, None, None, f64::NAN).is_err());
        assert!(
            build_interp(
                "bezier",
                None,
                Some("free"),
                Some([f64::NAN, 0.0, 0.0, 0.0])
            )
            .is_err()
        );
    }
}
