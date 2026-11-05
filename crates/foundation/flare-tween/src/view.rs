//! The view of a curve editor: how world coordinates (time across, change up) map to pixels, and
//! how panning and zooming move that map. Pure geometry, so the panel only feeds it pixels.

/// Pixels per world unit, at the least and the most. Bounds zoom so a wheel cannot run away.
pub const MIN_SCALE: f64 = 4.0;
pub const MAX_SCALE: f64 = 100_000.0;
/// How far the world origin may be from the pixel origin, so a drag cannot overflow.
const MAX_OFFSET: f64 = 1.0e7;

/// A map between world coordinates and pixels. Pixel y grows downward and world y upward.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct View {
    /// The pixel position of the world origin.
    origin: (f64, f64),
    /// Pixels per world unit in x and y.
    scale: (f64, f64),
}

impl Default for View {
    fn default() -> Self {
        Self {
            origin: (0.0, 0.0),
            scale: (100.0, 100.0),
        }
    }
}

fn finite_or(value: f64, fallback: f64) -> f64 {
    if value.is_finite() { value } else { fallback }
}

impl View {
    /// A view that shows the world range `x` by `y` inside a `width` by `height` pixel box with
    /// `margin` pixels kept free all round. A degenerate range or box gives the default scale.
    #[must_use]
    pub fn fit(width: f64, height: f64, margin: f64, x: (f64, f64), y: (f64, f64)) -> Self {
        let span_x = finite_or(x.1 - x.0, 0.0);
        let span_y = finite_or(y.1 - y.0, 0.0);
        let inner_w = finite_or(width - 2.0 * margin, 0.0);
        let inner_h = finite_or(height - 2.0 * margin, 0.0);
        if span_x <= 0.0 || span_y <= 0.0 || inner_w <= 0.0 || inner_h <= 0.0 {
            return Self::default();
        }
        let scale = (
            (inner_w / span_x).clamp(MIN_SCALE, MAX_SCALE),
            (inner_h / span_y).clamp(MIN_SCALE, MAX_SCALE),
        );
        Self {
            origin: (margin - x.0 * scale.0, height - margin + y.0 * scale.1),
            scale,
        }
    }

    #[must_use]
    pub fn scale(&self) -> (f64, f64) {
        self.scale
    }

    /// World to pixels.
    #[must_use]
    pub fn to_screen(&self, world: (f64, f64)) -> (f64, f64) {
        (
            self.origin.0 + world.0 * self.scale.0,
            self.origin.1 - world.1 * self.scale.1,
        )
    }

    /// Pixels to world.
    #[must_use]
    pub fn to_world(&self, screen: (f64, f64)) -> (f64, f64) {
        (
            (screen.0 - self.origin.0) / self.scale.0,
            (self.origin.1 - screen.1) / self.scale.1,
        )
    }

    /// Moves the view by a pixel offset. A non-finite offset changes nothing.
    pub fn pan(&mut self, dx: f64, dy: f64) {
        if !(dx.is_finite() && dy.is_finite()) {
            return;
        }
        self.origin.0 = (self.origin.0 + dx).clamp(-MAX_OFFSET, MAX_OFFSET);
        self.origin.1 = (self.origin.1 + dy).clamp(-MAX_OFFSET, MAX_OFFSET);
    }

    /// Zooms by `factor_x` and `factor_y` about a pixel, which stays under the pointer. A factor
    /// that is not finite and positive changes nothing; the scale stays within its limits.
    pub fn zoom_about(&mut self, anchor: (f64, f64), factor_x: f64, factor_y: f64) {
        let ok = |f: f64| f.is_finite() && f > 0.0;
        if !(ok(factor_x) && ok(factor_y) && anchor.0.is_finite() && anchor.1.is_finite()) {
            return;
        }
        let world = self.to_world(anchor);
        self.scale.0 = (self.scale.0 * factor_x).clamp(MIN_SCALE, MAX_SCALE);
        self.scale.1 = (self.scale.1 * factor_y).clamp(MIN_SCALE, MAX_SCALE);
        let moved = self.to_screen(world);
        self.pan(anchor.0 - moved.0, anchor.1 - moved.1);
    }
}
