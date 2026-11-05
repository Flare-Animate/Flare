//! A keyframe's interpolation: the family, easing and handles flare-tween evaluates to bend the
//! tween that starts at it. A span without one keeps its plain `ease` slider.

use flare_tween::{
    Curve, Easing, Extrapolation, Handle, HandleKind, Interpolation, Key, Params, TweenError, ease,
};
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum SpanInterpError {
    #[error(transparent)]
    Tween(#[from] TweenError),
}

/// How the tween that starts at a keyframe moves, as chosen from flare-tween.
///
/// `handles` is `[right.dt, right.dv, left.dt, left.dv]` in the unit square of the segment: the
/// first two belong to the segment's start key and the last two to the key it ends at. They are
/// used by `Bezier` with `HandleKind::Free` (or `Aligned`) only.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpanInterp {
    interpolation: Interpolation,
    easing: Easing,
    handle_kind: HandleKind,
    handles: [f64; 4],
    params: Params,
}

impl SpanInterp {
    /// # Errors
    /// A handle or parameter that is not finite or is out of range.
    pub fn new(
        interpolation: Interpolation,
        easing: Easing,
        handle_kind: HandleKind,
        handles: [f64; 4],
        params: Params,
    ) -> Result<Self, SpanInterpError> {
        let made = Self {
            interpolation,
            easing,
            handle_kind,
            handles,
            params,
        };
        made.curve()?;
        Ok(made)
    }

    pub fn interpolation(&self) -> Interpolation {
        self.interpolation
    }
    pub fn easing(&self) -> Easing {
        self.easing
    }
    pub fn handle_kind(&self) -> HandleKind {
        self.handle_kind
    }
    pub fn handles(&self) -> [f64; 4] {
        self.handles
    }
    pub fn params(&self) -> Params {
        self.params
    }

    /// The two-key curve of the segment, from (0, 0) to (1, 1).
    fn curve(&self) -> Result<Curve, TweenError> {
        let mut a = Key::new(0.0, 0.0);
        a.interpolation = self.interpolation;
        a.easing = self.easing;
        a.handle_kind = self.handle_kind;
        a.right = Handle {
            dt: self.handles[0],
            dv: self.handles[1],
        };
        let mut b = Key::new(1.0, 1.0);
        b.handle_kind = self.handle_kind;
        b.left = Handle {
            dt: self.handles[2],
            dv: self.handles[3],
        };
        Curve::new(vec![a, b], Extrapolation::Constant, self.params)
    }

    /// The same motion played backwards: `f'(t) = 1 - f(1 - t)`. In and Out swap, and a Bezier's
    /// handles swap ends and point the other way, so a reversed tween retraces the same path.
    pub fn mirrored(&self) -> Self {
        let easing = match self.easing {
            Easing::In => Easing::Out,
            Easing::Out => Easing::In,
            other => other,
        };
        let [rdt, rdv, ldt, ldv] = self.handles;
        Self {
            easing,
            handles: [-ldt, -ldv, -rdt, -rdv],
            ..*self
        }
    }

    /// How far along the change is when `t` (0 to 1) of the time has passed. Never fails for
    /// a value made by `new`: a non-finite `t` is the start.
    pub fn fraction(&self, t: f64) -> f64 {
        let t = if t.is_finite() {
            t.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let value = if self.interpolation == Interpolation::Bezier {
            self.curve().and_then(|c| c.evaluate(t))
        } else {
            ease(self.interpolation, self.easing, t, &self.params)
        };
        value.unwrap_or(t)
    }
}

impl Default for SpanInterp {
    fn default() -> Self {
        Self {
            interpolation: Interpolation::Linear,
            easing: Easing::Auto,
            handle_kind: HandleKind::AutoClamped,
            handles: [0.0; 4],
            params: Params::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    #[test]
    fn a_mirrored_curve_is_the_original_played_backwards() {
        for (family, easing) in [
            (Interpolation::Quad, Easing::In),
            (Interpolation::Cubic, Easing::Out),
            (Interpolation::Quad, Easing::InOut),
        ] {
            let i = SpanInterp::new(
                family,
                easing,
                HandleKind::AutoClamped,
                [0.0; 4],
                Params::default(),
            )
            .expect("valid");
            let m = i.mirrored();
            for k in 0..=10 {
                let t = f64::from(k) / 10.0;
                assert!(
                    (m.fraction(t) - (1.0 - i.fraction(1.0 - t))).abs() < 1e-9,
                    "{family:?} {easing:?} {t}"
                );
            }
        }
        let b = SpanInterp::new(
            Interpolation::Bezier,
            Easing::Auto,
            HandleKind::Free,
            [0.2, 0.0, -0.5, -0.3],
            Params::default(),
        )
        .expect("valid");
        let m = b.mirrored();
        assert_eq!(m.handles(), [0.5, 0.3, -0.2, -0.0]);
        for k in 0..=10 {
            let t = f64::from(k) / 10.0;
            assert!(
                (m.fraction(t) - (1.0 - b.fraction(1.0 - t))).abs() < 1e-6,
                "{t}"
            );
        }
        assert_eq!(b.mirrored().mirrored(), b);
    }

    #[test]
    fn families_keep_their_ends_and_differ_inside() {
        let quad = SpanInterp::new(
            Interpolation::Quad,
            Easing::In,
            HandleKind::AutoClamped,
            [0.0; 4],
            Params::default(),
        )
        .expect("valid");
        assert_eq!(quad.fraction(0.0), 0.0);
        assert_eq!(quad.fraction(1.0), 1.0);
        assert!(quad.fraction(0.5) < 0.5);
    }

    #[test]
    fn a_bad_parameter_is_refused() {
        let params = Params {
            back_overshoot: f64::NAN,
            ..Params::default()
        };
        assert!(
            SpanInterp::new(
                Interpolation::Back,
                Easing::Out,
                HandleKind::Auto,
                [0.0; 4],
                params
            )
            .is_err()
        );
    }

    #[test]
    fn bezier_handles_shape_the_fraction() {
        let free = SpanInterp::new(
            Interpolation::Bezier,
            Easing::Auto,
            HandleKind::Free,
            [0.1, 0.0, -0.1, 0.0],
            Params::default(),
        )
        .expect("valid");
        assert!(free.fraction(0.25) < 0.25);
        assert!(free.fraction(f64::NAN).abs() < 1e-9);
    }
}
