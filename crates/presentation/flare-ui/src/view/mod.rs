//! Stage zoom, pan and rotation.
//!
//! The stage is drawn fitted to its panel (zoom 1.0, upright) unless the user zooms, pans or turns
//! it. All the geometry lives here so the mapping between screen and stage is testable without a
//! UI. The view turns about the panel's center: what is drawn upright is turned afterwards (see
//! [`turn`]), and every point read from the screen is turned back before it is used, so the tools
//! never see the rotation.

mod rulers;
pub mod turn;

pub use rulers::{Tick, ruler_ticks};

use egui::emath::Rot2;
use egui::{Pos2, Rect, Vec2};

/// Adobe Animate's zoom range (issue #213): 4% to 2000% of the fit-to-panel scale.
pub const MIN_ZOOM: f32 = 0.04;
pub const MAX_ZOOM: f32 = 20.0;
/// One menu or key step of zoom.
pub const STEP: f32 = 1.25;
/// One button step of rotation, and what Shift snaps a turning drag to, in degrees.
pub const ROTATION_STEP_DEGREES: f32 = 15.0;
/// Smallest on-screen scale, so a degenerate panel never produces an empty rectangle.
const MIN_SCALE: f32 = 0.01;
/// A turning drag this close to the center has no direction to read.
const MIN_TURN_RADIUS: f32 = 4.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StageView {
    /// Multiplier on the fit-to-panel scale.
    zoom: f32,
    /// How far the stage center is from the panel center, in screen points, before the turn.
    pan: Vec2,
    /// How far the view is turned clockwise about the panel center, in radians, in `(-π, π]`.
    rotation: f32,
}

impl Default for StageView {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan: Vec2::ZERO,
            rotation: 0.0,
        }
    }
}

/// How far the pasteboard reaches past each edge of the stage, in lengths of the stage's longer
/// side. Artwork can be drawn and kept anywhere on it; publishing shows only the stage.
pub const PASTEBOARD_REACH: f32 = 2.0;

/// The pasteboard in stage units: the stage and the room around it where artwork may be.
/// For a stage with no usable size it is that stage's rectangle, which is empty.
pub fn pasteboard(extent: Vec2) -> Rect {
    let stage = Rect::from_min_size(Pos2::ZERO, extent);
    if !usable(extent) {
        return stage;
    }
    stage.expand(extent.x.max(extent.y) * PASTEBOARD_REACH)
}

fn usable(extent: Vec2) -> bool {
    extent.x.is_finite() && extent.y.is_finite() && extent.x > 0.0 && extent.y > 0.0
}

fn panel_ok(panel: Rect) -> bool {
    panel.min.is_finite() && panel.max.is_finite()
}

/// An angle in radians brought into `(-π, π]`.
fn normalized(angle: f32) -> f32 {
    use std::f32::consts::{PI, TAU};
    let a = angle.rem_euclid(TAU);
    if a > PI { a - TAU } else { a }
}

/// `angle` in radians to the nearest multiple of `step_degrees`.
pub fn snap_rotation(angle: f32, step_degrees: f32) -> f32 {
    if !angle.is_finite() || !step_degrees.is_finite() || step_degrees <= 0.0 {
        return angle;
    }
    let step = step_degrees.to_radians();
    (angle / step).round() * step
}

/// The turn that takes the direction from `pivot` to `from` onto the direction from `pivot` to
/// `to`, in radians clockwise; `None` when either point is too near the pivot to have one.
pub fn turn_between(pivot: Pos2, from: Pos2, to: Pos2) -> Option<f32> {
    let (a, b) = (from - pivot, to - pivot);
    let ok = |v: Vec2| v.is_finite() && v.length() >= MIN_TURN_RADIUS;
    (ok(a) && ok(b)).then(|| normalized(b.angle() - a.angle()))
}

impl StageView {
    #[cfg(test)]
    fn zoom(&self) -> f32 {
        self.zoom
    }

    /// Fitted and centered. The rotation is the person's and is not part of the fit.
    pub fn is_fit(&self) -> bool {
        self.zoom == 1.0 && self.pan == Vec2::ZERO
    }

    /// Back to fitted and centered, keeping the rotation.
    pub fn reset(&mut self) {
        *self = Self {
            rotation: self.rotation,
            ..Self::default()
        };
    }

    /// The turn, in radians clockwise.
    pub fn rotation(&self) -> f32 {
        self.rotation
    }

    /// The turn in whole degrees, as the interface shows it, in `(-180, 180]`.
    pub fn rotation_degrees(&self) -> f32 {
        let d = self.rotation.to_degrees().round();
        if d <= -180.0 { d + 360.0 } else { d }
    }

    pub fn is_turned(&self) -> bool {
        self.rotation != 0.0
    }

    /// Turns the view to `angle` radians clockwise; a non-finite angle changes nothing.
    pub fn set_rotation(&mut self, angle: f32) {
        if angle.is_finite() {
            let a = normalized(angle);
            // A full turn and back lands on upright, not on a hair off it.
            self.rotation = if a.abs() < 1e-6 { 0.0 } else { a };
        }
    }

    pub fn rotate_by(&mut self, delta: f32) {
        self.set_rotation(self.rotation + delta);
    }

    pub fn set_rotation_degrees(&mut self, degrees: f32) {
        self.set_rotation(degrees.to_radians());
    }

    pub fn reset_rotation(&mut self) {
        self.rotation = 0.0;
    }

    fn rot(&self) -> Rot2 {
        Rot2::from_angle(self.rotation)
    }

    /// A point of the upright drawing to where it shows once turned.
    pub fn turn_point(&self, panel: Rect, p: Pos2) -> Pos2 {
        if !self.is_turned() {
            return p;
        }
        let c = panel.center();
        c + self.rot() * (p - c)
    }

    /// A point on screen to where it is in the upright drawing.
    pub fn unturn_point(&self, panel: Rect, p: Pos2) -> Pos2 {
        if !self.is_turned() {
            return p;
        }
        let c = panel.center();
        c + self.rot().inverse() * (p - c)
    }

    /// A movement on screen as a movement of the upright drawing.
    pub fn unturn_vec(&self, v: Vec2) -> Vec2 {
        if !self.is_turned() {
            return v;
        }
        self.rot().inverse() * v
    }

    /// The part of the upright drawing that covers the panel once turned: the panel itself when
    /// upright, else the square around the circle through its corners. Lines drawn across the
    /// panel (grid, guides) are drawn across this, so a turn leaves no corner without them.
    pub fn upright_reach(&self, panel: Rect) -> Rect {
        if !self.is_turned() || !panel_ok(panel) {
            return panel;
        }
        Rect::from_center_size(panel.center(), Vec2::splat(panel.size().length()))
    }

    /// Screen points per stage unit.
    pub fn scale(&self, panel: Rect, extent: Vec2) -> f32 {
        if !usable(extent) {
            return 1.0;
        }
        let fit = (panel.width() / extent.x).min(panel.height() / extent.y);
        (fit * self.zoom).max(MIN_SCALE)
    }

    /// Where the stage is drawn before the turn.
    pub fn image_rect(&self, panel: Rect, extent: Vec2) -> Rect {
        let size = if usable(extent) {
            extent * self.scale(panel, extent)
        } else {
            Vec2::splat(1.0)
        };
        Rect::from_center_size(panel.center() + self.pan, size)
    }

    /// A stage point to where it shows on screen, turn included.
    pub fn to_screen(&self, panel: Rect, extent: Vec2, p: Pos2) -> Pos2 {
        let image = self.image_rect(panel, extent);
        self.turn_point(panel, image.min + p.to_vec2() * self.scale(panel, extent))
    }

    /// A screen point to the stage point under it, turn included. Not clamped.
    pub fn to_stage(&self, panel: Rect, extent: Vec2, p: Pos2) -> Pos2 {
        let image = self.image_rect(panel, extent);
        let upright = self.unturn_point(panel, p);
        ((upright - image.min) / self.scale(panel, extent).max(f32::EPSILON)).to_pos2()
    }

    /// The part of the stage that is on screen, in stage units, or `None` when none of it is.
    pub fn visible_region(&self, panel: Rect, extent: Vec2) -> Option<Rect> {
        self.visible_within(panel, extent, Rect::from_min_size(Pos2::ZERO, extent))
    }

    /// The part of the stage and the pasteboard around it that is on screen, in stage units, or
    /// `None` when none of it is: what the editor draws when artwork off the stage shows.
    pub fn visible_pasteboard(&self, panel: Rect, extent: Vec2) -> Option<Rect> {
        self.visible_within(panel, extent, pasteboard(extent))
    }

    /// The part of `bounds` (in stage units) that is on screen, in stage units. Turned, the
    /// panel's corners fall on a slanted square of the stage: the box around it is what shows.
    fn visible_within(&self, panel: Rect, extent: Vec2, bounds: Rect) -> Option<Rect> {
        if !usable(extent) || !panel_ok(panel) {
            return None;
        }
        let corners = [
            panel.left_top(),
            panel.right_top(),
            panel.right_bottom(),
            panel.left_bottom(),
        ];
        let seen =
            Rect::from_points(&corners.map(|p| self.to_stage(panel, extent, p))).intersect(bounds);
        (seen.width() > 0.0 && seen.height() > 0.0).then_some(seen)
    }

    /// Moves the stage by a movement on screen, whichever way the view is turned.
    pub fn pan_by(&mut self, delta: Vec2) {
        if delta.x.is_finite() && delta.y.is_finite() {
            self.pan += self.unturn_vec(delta);
        }
    }

    /// Multiplies the zoom by `factor`, keeping the stage point under `anchor` where it is.
    pub fn zoom_about(&mut self, anchor: Pos2, factor: f32, panel: Rect, extent: Vec2) {
        let ok = panel_ok(panel) && anchor.is_finite();
        if !factor.is_finite() || factor <= 0.0 || !usable(extent) || !ok {
            return;
        }
        // The zoom happens in the upright drawing, so the anchor is turned back into it first.
        let anchor = self.unturn_point(panel, anchor);
        let before = self.image_rect(panel, extent);
        let scale = self.scale(panel, extent);
        let stage_point = (anchor - before.min) / scale;
        self.zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        let after = self.image_rect(panel, extent);
        let now = after.min + stage_point * self.scale(panel, extent);
        self.pan += anchor - now;
    }

    /// Zooms about the panel center (menu commands).
    pub fn zoom_center(&mut self, factor: f32, panel: Rect, extent: Vec2) {
        self.zoom_about(panel.center(), factor, panel, extent);
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests;
