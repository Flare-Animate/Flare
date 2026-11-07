//! Wick's tween easings as Flare's. Wick names four: `none` (linear), and quadratic `in`, `out`
//! and `in-out`, which Flare's quadratic family has exactly.

use flare_document::SpanInterp;
use flare_tween::{Easing, HandleKind, Interpolation, Params};

use super::error::WickError;

/// The interpolation a Wick easing names, or `None` for linear motion.
pub(super) fn interp(name: &str) -> Result<Option<SpanInterp>, WickError> {
    let easing = match name {
        "none" => return Ok(None),
        "in" => Easing::In,
        "out" => Easing::Out,
        "in-out" => Easing::InOut,
        _ => return Err(WickError::BadField("easingType")),
    };
    SpanInterp::new(
        Interpolation::Quad,
        easing,
        HandleKind::Auto,
        [0.0; 4],
        Params::default(),
    )
    .map(Some)
    .map_err(|_| WickError::BadField("easingType"))
}
