//! Stage zoom and pan.
//!
//! The stage is drawn fitted to its panel (zoom 1.0) unless the user zooms or pans. All the
//! geometry lives here so the mapping between screen and stage is testable without a UI.

use egui::{Pos2, Rect, Vec2};

pub const MIN_ZOOM: f32 = 0.1;
pub const MAX_ZOOM: f32 = 32.0;
/// One menu or key step of zoom.
pub const STEP: f32 = 1.25;
/// Smallest on-screen scale, so a degenerate panel never produces an empty rectangle.
const MIN_SCALE: f32 = 0.01;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StageView {
    /// Multiplier on the fit-to-panel scale.
    zoom: f32,
    /// How far the stage center is from the panel center, in screen points.
    pan: Vec2,
}

impl Default for StageView {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan: Vec2::ZERO,
        }
    }
}

fn usable(extent: Vec2) -> bool {
    extent.x.is_finite() && extent.y.is_finite() && extent.x > 0.0 && extent.y > 0.0
}

impl StageView {
    #[cfg(test)]
    fn zoom(&self) -> f32 {
        self.zoom
    }

    pub fn is_fit(&self) -> bool {
        *self == Self::default()
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Screen points per stage unit.
    pub fn scale(&self, panel: Rect, extent: Vec2) -> f32 {
        if !usable(extent) {
            return 1.0;
        }
        let fit = (panel.width() / extent.x).min(panel.height() / extent.y);
        (fit * self.zoom).max(MIN_SCALE)
    }

    /// Where the stage is drawn.
    pub fn image_rect(&self, panel: Rect, extent: Vec2) -> Rect {
        let size = if usable(extent) {
            extent * self.scale(panel, extent)
        } else {
            Vec2::splat(1.0)
        };
        Rect::from_center_size(panel.center() + self.pan, size)
    }

    /// The part of the stage that is on screen, in stage units, or `None` when none of it is.
    pub fn visible_region(&self, panel: Rect, extent: Vec2) -> Option<Rect> {
        if !usable(extent) || !panel.min.is_finite() || !panel.max.is_finite() {
            return None;
        }
        let image = self.image_rect(panel, extent);
        let scale = self.scale(panel, extent);
        let seen = image.intersect(panel);
        if !(seen.width() > 0.0 && seen.height() > 0.0) {
            return None;
        }
        let to_stage = |p: Pos2| {
            let v = (p - image.min) / scale;
            Pos2::new(v.x.clamp(0.0, extent.x), v.y.clamp(0.0, extent.y))
        };
        let region = Rect::from_min_max(to_stage(seen.min), to_stage(seen.max));
        (region.width() > 0.0 && region.height() > 0.0).then_some(region)
    }

    pub fn pan_by(&mut self, delta: Vec2) {
        if delta.x.is_finite() && delta.y.is_finite() {
            self.pan += delta;
        }
    }

    /// Multiplies the zoom by `factor`, keeping the stage point under `anchor` where it is.
    pub fn zoom_about(&mut self, anchor: Pos2, factor: f32, panel: Rect, extent: Vec2) {
        let panel_ok = panel.min.is_finite() && panel.max.is_finite() && anchor.is_finite();
        if !factor.is_finite() || factor <= 0.0 || !usable(extent) || !panel_ok {
            return;
        }
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

/// A tick on a ruler: where it is on screen, what stage coordinate it marks, and whether it
/// is a labelled one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tick {
    pub screen: f32,
    pub value: f32,
    pub major: bool,
}

/// Ticks for a ruler along one axis. `origin` is the screen position of stage coordinate 0 and
/// `scale` the screen points per stage unit; ticks are made for the screen span `from..to`. The
/// step between labelled ticks is the first of 1, 2, 5, 10, 20, 50... stage units that leaves at
/// least `min_gap` points between them, and there are ten minor ticks between labelled ones.
pub fn ruler_ticks(origin: f32, scale: f32, from: f32, to: f32, min_gap: f32) -> Vec<Tick> {
    if ![origin, scale, from, to, min_gap]
        .iter()
        .all(|v| v.is_finite())
        || scale <= 0.0
        || to <= from
    {
        return Vec::new();
    }
    let mut step = 1.0_f32;
    let mut guard = 0;
    while step * scale < min_gap && guard < 64 {
        step *= match guard % 3 {
            0 => 2.0,
            1 => 2.5,
            _ => 2.0,
        };
        guard += 1;
    }
    let minor = step / 10.0;
    let first = ((from - origin) / scale / minor).floor() as i64;
    let last = ((to - origin) / scale / minor).ceil() as i64;
    // Never more than a few hundred ticks, whatever the numbers say.
    if last - first > 2_000 {
        return Vec::new();
    }
    (first..=last)
        .filter_map(|i| {
            let value = i as f32 * minor;
            let screen = origin + value * scale;
            // Minor ticks closer than 3 points are noise.
            let major = i % 10 == 0;
            (screen >= from && screen <= to && (major || minor * scale >= 3.0)).then_some(Tick {
                screen,
                value,
                major,
            })
        })
        .collect()
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    fn panel() -> Rect {
        Rect::from_min_size(Pos2::new(100.0, 50.0), Vec2::new(800.0, 400.0))
    }

    const EXTENT: Vec2 = Vec2::new(400.0, 200.0);

    fn near(a: Pos2, b: Pos2) -> bool {
        (a - b).length() < 0.01
    }

    #[test]
    fn default_view_fits_the_stage_centered_in_the_panel() {
        let view = StageView::default();
        let rect = view.image_rect(panel(), EXTENT);
        assert_eq!(view.scale(panel(), EXTENT), 2.0);
        assert_eq!(rect.size(), Vec2::new(800.0, 400.0));
        assert_eq!(rect.center(), panel().center());
        assert!(view.is_fit());
    }

    #[test]
    fn zooming_keeps_the_point_under_the_cursor_fixed() {
        let mut view = StageView::default();
        let anchor = Pos2::new(250.0, 120.0);
        let at = |view: &StageView| {
            let rect = view.image_rect(panel(), EXTENT);
            (anchor - rect.min) / view.scale(panel(), EXTENT)
        };
        let before = at(&view);
        view.zoom_about(anchor, 3.0, panel(), EXTENT);
        let after = at(&view);
        assert!(
            near(Pos2::ZERO + before, Pos2::ZERO + after),
            "{before:?} {after:?}"
        );
        assert_eq!(view.zoom(), 3.0);
        view.zoom_about(anchor, 1.0 / 3.0, panel(), EXTENT);
        assert!(view.zoom() > 0.99 && view.zoom() < 1.01);
        assert!(near(Pos2::ZERO + at(&view), Pos2::ZERO + before));
    }

    #[test]
    fn zoom_is_clamped_and_bad_factors_are_ignored() {
        let mut view = StageView::default();
        view.zoom_center(1000.0, panel(), EXTENT);
        assert_eq!(view.zoom(), MAX_ZOOM);
        view.zoom_center(0.0001, panel(), EXTENT);
        assert_eq!(view.zoom(), MIN_ZOOM);
        let before = view;
        for bad in [f32::NAN, f32::INFINITY, 0.0, -2.0] {
            view.zoom_center(bad, panel(), EXTENT);
        }
        assert_eq!(view, before);
    }

    #[test]
    fn panning_moves_the_stage_and_reset_returns_to_fit() {
        let mut view = StageView::default();
        let home = view.image_rect(panel(), EXTENT);
        view.pan_by(Vec2::new(30.0, -10.0));
        assert_eq!(
            view.image_rect(panel(), EXTENT),
            home.translate(Vec2::new(30.0, -10.0))
        );
        view.pan_by(Vec2::new(f32::NAN, 0.0));
        assert_eq!(
            view.image_rect(panel(), EXTENT),
            home.translate(Vec2::new(30.0, -10.0))
        );
        view.zoom_center(2.0, panel(), EXTENT);
        assert!(!view.is_fit());
        view.reset();
        assert_eq!(view.image_rect(panel(), EXTENT), home);
    }

    #[test]
    fn the_visible_region_is_the_part_of_the_stage_inside_the_panel() {
        let mut view = StageView::default();
        // Fitted, the whole stage is visible.
        let all = view.visible_region(panel(), EXTENT).expect("region");
        assert_eq!((all.min, all.max), (Pos2::ZERO, Pos2::new(400.0, 200.0)));
        // Zoomed in 4x about the panel center: a quarter of each side is visible, centered.
        view.zoom_center(4.0, panel(), EXTENT);
        let r = view.visible_region(panel(), EXTENT).expect("region");
        assert!(
            (r.width() - 100.0).abs() < 0.01 && (r.height() - 50.0).abs() < 0.01,
            "{r:?}"
        );
        assert!(
            (r.center() - Pos2::new(200.0, 100.0)).length() < 0.01,
            "{r:?}"
        );
        // Panned far away, nothing is visible.
        view.pan_by(Vec2::new(1e6, 0.0));
        assert!(view.visible_region(panel(), EXTENT).is_none());
        // Partly off the panel: only the stage's left part shows, starting at its left edge.
        let mut view = StageView::default();
        view.pan_by(Vec2::new(400.0, 0.0));
        let r = view.visible_region(panel(), EXTENT).expect("region");
        assert_eq!(r.min.x, 0.0);
        assert!((r.max.x - 200.0).abs() < 0.01, "{r:?}");
        for bad in [Vec2::ZERO, Vec2::new(f32::NAN, 1.0)] {
            assert!(StageView::default().visible_region(panel(), bad).is_none());
        }
    }

    #[test]
    fn zooming_before_the_panel_is_known_changes_nothing() {
        let mut view = StageView::default();
        view.zoom_center(2.0, Rect::NOTHING, EXTENT);
        view.zoom_about(Pos2::new(f32::NAN, 0.0), 2.0, panel(), EXTENT);
        assert!(view.is_fit(), "no NaN pan from an empty panel");
    }

    #[test]
    fn degenerate_extents_and_panels_do_not_panic_or_vanish() {
        let mut view = StageView::default();
        for extent in [Vec2::ZERO, Vec2::new(f32::NAN, 5.0), Vec2::new(-1.0, 3.0)] {
            assert!(view.image_rect(panel(), extent).is_finite());
            view.zoom_center(2.0, panel(), extent);
            assert_eq!(view.zoom(), 1.0, "nothing to zoom");
        }
        let tiny = Rect::from_min_size(Pos2::ZERO, Vec2::ZERO);
        assert!(view.scale(tiny, EXTENT) >= MIN_SCALE);
    }

    #[test]
    fn ruler_ticks_label_round_numbers_and_never_crowd() {
        // 1 point per unit, labels at least 50 points apart: every 50 units.
        let ticks = ruler_ticks(100.0, 1.0, 100.0, 400.0, 50.0);
        let majors: Vec<f32> = ticks.iter().filter(|t| t.major).map(|t| t.value).collect();
        assert_eq!(majors, vec![0.0, 50.0, 100.0, 150.0, 200.0, 250.0, 300.0]);
        // Zoomed in 10x, labels land on finer round numbers.
        let ticks = ruler_ticks(0.0, 10.0, 0.0, 200.0, 50.0);
        let majors: Vec<f32> = ticks.iter().filter(|t| t.major).map(|t| t.value).collect();
        assert_eq!(majors, vec![0.0, 5.0, 10.0, 15.0, 20.0]);
        for pair in ticks.windows(2) {
            assert!(
                pair[1].screen - pair[0].screen >= 3.0 - 1e-3,
                "minor ticks are not crowded"
            );
        }
        // A span that starts left of the origin still gets negative values.
        let ticks = ruler_ticks(100.0, 1.0, 0.0, 120.0, 50.0);
        assert!(ticks.iter().any(|t| t.major && t.value == -100.0));
    }

    #[test]
    fn ruler_ticks_refuse_bad_numbers() {
        assert!(ruler_ticks(0.0, 0.0, 0.0, 100.0, 50.0).is_empty());
        assert!(ruler_ticks(f32::NAN, 1.0, 0.0, 100.0, 50.0).is_empty());
        assert!(
            ruler_ticks(0.0, 1.0, 100.0, 0.0, 50.0).is_empty(),
            "backwards span"
        );
        assert!(
            ruler_ticks(0.0, 1e-9, 0.0, 1e9, 50.0).len() < 3000,
            "bounded"
        );
    }
}
