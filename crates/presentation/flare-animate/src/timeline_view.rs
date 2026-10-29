//! Timeline zoom: how wide a frame is drawn.
//!
//! At zoom 1.0 the whole scene fits the panel; zooming in widens each frame and the track
//! scrolls horizontally.

pub const MIN_ZOOM: f32 = 1.0;
pub const MAX_ZOOM: f32 = 16.0;
pub const STEP: f32 = 1.5;

/// `zoom` multiplied by `factor`, kept in range; unusable factors leave it alone.
pub fn zoomed(zoom: f32, factor: f32) -> f32 {
    if !factor.is_finite() || factor <= 0.0 {
        return zoom;
    }
    (zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM)
}

/// Points per frame when `frames` frames are shown in `available` points at `zoom`.
/// Never smaller than one point, so a degenerate panel still draws something.
pub fn frame_width(available: f32, frames: u64, zoom: f32) -> f32 {
    let frames = frames.max(1) as f32;
    if !available.is_finite() {
        return 1.0;
    }
    let zoom = if zoom.is_finite() {
        zoom.clamp(MIN_ZOOM, MAX_ZOOM)
    } else {
        MIN_ZOOM
    };
    (available.max(0.0) / frames * zoom).max(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_divides_the_panel_among_the_frames_and_zoom_multiplies_it() {
        assert_eq!(frame_width(480.0, 24, 1.0), 20.0);
        assert_eq!(frame_width(480.0, 24, 2.0), 40.0);
    }

    #[test]
    fn zoom_stays_in_range_and_ignores_bad_factors() {
        assert_eq!(zoomed(1.0, 0.5), MIN_ZOOM, "cannot zoom out past fit");
        assert_eq!(zoomed(10.0, 100.0), MAX_ZOOM);
        for bad in [f32::NAN, f32::INFINITY, 0.0, -1.0] {
            assert_eq!(zoomed(3.0, bad), 3.0);
        }
    }

    #[test]
    fn degenerate_inputs_still_give_a_drawable_width() {
        assert_eq!(frame_width(0.0, 24, 1.0), 1.0);
        assert_eq!(frame_width(f32::NAN, 24, 1.0), 1.0);
        assert_eq!(frame_width(480.0, 0, 1.0), 480.0, "no frames counts as one");
        assert_eq!(
            frame_width(480.0, 24, f32::NAN),
            20.0_f32.max(1.0) * 1.0,
            "NaN zoom clamps"
        );
    }
}
