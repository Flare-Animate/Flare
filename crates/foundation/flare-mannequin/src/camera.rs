//! Moving round the figure in a view, as a 3D program does: the camera looks at a point (its
//! focus) and turns round it, slides across, and comes nearer or goes further. A zoom keeps the
//! point under the pointer where it is, so one zooms into a hand by pointing at it; framing
//! brings the whole figure into view wherever it was moved; looking at a part makes the camera
//! turn round that part. The turn itself is the figure's [`View`]; the camera holds the rest,
//! which only the view on the screen has (a picture or a placed figure looks at the middle).

use crate::figure::Mannequin;
use crate::math::Vec3;
use crate::view::{MAX_FOCUS, MIDDLE, Placement, Projector, View};

/// The furthest out and the nearest in a camera zooms, as a multiple of the usual size.
pub const ZOOM_RANGE: (f64, f64) = (0.2, 16.0);
/// How much of the view a framed figure fills.
const FRAME_SHARE: f64 = 0.88;
/// Rounds of putting a framed figure in the middle.
const RECENTER_ROUNDS: usize = 3;

/// Where the camera looks and how near it is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    focus: Vec3,
    zoom: f64,
}

impl Default for Camera {
    /// Looking at the middle of where the figure stands, at the usual size.
    fn default() -> Self {
        Self {
            focus: Vec3::new(0.0, MIDDLE, 0.0),
            zoom: 1.0,
        }
    }
}

fn hold(v: f64) -> f64 {
    v.clamp(-MAX_FOCUS, MAX_FOCUS)
}

impl Camera {
    /// The point of the figure's space the camera looks at and turns round.
    pub fn focus(&self) -> Vec3 {
        self.focus
    }

    pub fn zoom(&self) -> f64 {
        self.zoom
    }

    /// Zooms to `zoom` about the middle of the view, held to [`ZOOM_RANGE`].
    pub fn set_zoom(&mut self, zoom: f64) {
        if zoom.is_finite() {
            self.zoom = zoom.clamp(ZOOM_RANGE.0, ZOOM_RANGE.1);
        }
    }

    /// Looks at `point`, which comes to the middle of the view; held inside the space the camera
    /// may look at.
    pub fn look_at(&mut self, point: Vec3) {
        if point.is_finite() {
            self.focus = Vec3::new(hold(point.x), hold(point.y), hold(point.z));
        }
    }

    /// Where the figure goes in a view whose middle is `center`, the figure `height` tall there
    /// at a zoom of one.
    pub fn placement(&self, center: [f64; 2], height: f64) -> Option<Placement> {
        Placement::new(center, height * self.zoom)?.with_focus(self.focus)
    }

    fn projector(&self, view: &View, center: [f64; 2], height: f64) -> Option<Projector> {
        let placement = self.placement(center, height)?;
        Some(Projector::new(view, placement))
    }

    /// Slides the camera across so what is seen moves by `by` in the view, as if dragged.
    pub fn pan(&mut self, view: &View, center: [f64; 2], height: f64, by: [f64; 2]) {
        if !(by[0].is_finite() && by[1].is_finite()) {
            return;
        }
        if let Some(projector) = self.projector(view, center, height) {
            self.look_at(projector.unproject([center[0] - by[0], center[1] - by[1]], 0.0));
        }
    }

    /// Zooms by `factor` (above one nearer), keeping the point of the view at `at` where it is.
    pub fn zoom_about(
        &mut self,
        view: &View,
        center: [f64; 2],
        height: f64,
        at: [f64; 2],
        factor: f64,
    ) {
        if !(factor.is_finite() && factor > 0.0 && at[0].is_finite() && at[1].is_finite()) {
            return;
        }
        let Some(before) = self.projector(view, center, height) else {
            return;
        };
        // The point under the pointer, on the plane through the focus that faces the camera.
        let under = before.unproject(at, 0.0);
        self.set_zoom(self.zoom * factor);
        let Some(after) = self.projector(view, center, height) else {
            return;
        };
        // Sliding the focus along that plane moves the point back under the pointer.
        let landed = after.project(under).at;
        self.focus = self.focus + (after.unproject(landed, 0.0) - after.unproject(at, 0.0));
        self.look_at(self.focus);
    }

    /// Brings the whole figure into a view `size` across round `center`, filling most of it.
    pub fn frame(&mut self, figure: &Mannequin, center: [f64; 2], height: f64, size: [f64; 2]) {
        if !(size[0] > 1.0 && size[1] > 1.0 && size[0].is_finite() && size[1].is_finite()) {
            return;
        }
        let posed = figure.posed();
        let (mut low, mut high) = ([f64::MAX; 3], [f64::MIN; 3]);
        for p in posed.at {
            for (k, v) in [p.x, p.y, p.z].into_iter().enumerate() {
                low[k] = low[k].min(v);
                high[k] = high[k].max(v);
            }
        }
        // The joints stop short of the crown and the soles: take those in too.
        let crown = posed.point(
            crate::Joint::Head,
            Vec3::new(0.0, 0.12 * figure.build().head, 0.0),
        );
        low[1] = low[1].min(0.0);
        high[1] = high[1].max(crown.y);
        self.look_at(Vec3::new(
            (low[0] + high[0]) / 2.0,
            (low[1] + high[1]) / 2.0,
            (low[2] + high[2]) / 2.0,
        ));
        self.zoom = 1.0;
        let Some(placement) = self.placement(center, height) else {
            return;
        };
        let (lo, hi) = figure.bounds(placement, false);
        let (w, h) = (hi[0] - lo[0], hi[1] - lo[1]);
        if w > 1e-9 && h > 1e-9 {
            self.set_zoom((size[0] * FRAME_SHARE / w).min(size[1] * FRAME_SHARE / h));
        }
        // The picture's box is not round the focus when the figure is seen at a slant. A slide
        // is exact only on the plane through the focus, so with perspective it is done again.
        for _ in 0..RECENTER_ROUNDS {
            let Some(placement) = self.placement(center, height) else {
                return;
            };
            let (lo, hi) = figure.bounds(placement, false);
            let middle = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0];
            self.pan(
                &figure.view,
                center,
                height,
                [center[0] - middle[0], center[1] - middle[1]],
            );
        }
    }
}
