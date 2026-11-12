//! Timeline zoom: how wide a frame is drawn, and which layers get a row.
//!
//! A frame is a small fixed cell (`BASE_FRAME_WIDTH` points at zoom 1), widened or narrowed by the
//! zoom; the track scrolls horizontally, and empty cells go on to the panel's edge. Fit sets the
//! zoom at which the scene just fills the panel.

pub const MIN_ZOOM: f32 = 0.3;
pub const MAX_ZOOM: f32 = 4.0;
pub const STEP: f32 = 1.5;

/// The layers that get a row, top to bottom, as (index in the timeline, how deep in folders).
/// Layers inside a collapsed folder, at any depth, are left out.
pub fn visible_layers(
    layers: &[flare_document::Layer],
    collapsed: &std::collections::HashSet<flare_core::LayerId>,
) -> Vec<(usize, usize)> {
    // (depth, hidden) of each layer seen so far. A parent always comes earlier in the list.
    let mut seen: std::collections::HashMap<flare_core::LayerId, (usize, bool)> =
        std::collections::HashMap::new();
    let mut rows = Vec::new();
    for (index, layer) in layers.iter().enumerate() {
        let (depth, hidden) = layer
            .parent()
            .and_then(|parent| seen.get(&parent).copied().map(|(d, h)| (parent, d, h)))
            .map_or((0, false), |(parent, depth, hidden)| {
                (depth + 1, hidden || collapsed.contains(&parent))
            });
        seen.insert(layer.id(), (depth, hidden));
        if !hidden {
            rows.push((index, depth));
        }
    }
    rows
}

/// How many frames a sound `seconds` long covers at `per_second` frames a second: at least one,
/// so even a click of a sound has a cell to show.
pub fn sound_frames(seconds: f64, per_second: f64) -> u64 {
    let frames = (seconds * per_second).ceil();
    if frames.is_finite() && frames >= 1.0 {
        frames as u64
    } else {
        1
    }
}

/// The frame after the last one on which a sound cued in `timeline` still plays, or 0 when it
/// cues none. The timeline scrolls at least this far, so a sound longer than the scene is never
/// cut off at the panel's edge.
pub fn sounds_end(document: &flare_document::Document, timeline: &flare_document::Timeline) -> u64 {
    let per_second = document.frame_rate().per_second();
    timeline
        .cues()
        .filter_map(|(_, cue)| {
            let sound = document.sound(cue.sound)?;
            Some(u64::from(cue.start).saturating_add(sound_frames(sound.seconds(), per_second)))
        })
        .max()
        .unwrap_or(0)
}

/// `zoom` multiplied by `factor`, kept in range; unusable factors leave it alone.
pub fn zoomed(zoom: f32, factor: f32) -> f32 {
    if !factor.is_finite() || factor <= 0.0 {
        return zoom;
    }
    (zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM)
}

/// How wide one frame is at zoom 1, in points: a small cell, as in Animate, so a short scene does
/// not stretch its frames across the whole panel.
pub const BASE_FRAME_WIDTH: f32 = 10.0;

/// Points per frame at `zoom`. Never smaller than one point, so a degenerate zoom still draws.
pub fn frame_width(zoom: f32) -> f32 {
    let zoom = if zoom.is_finite() {
        zoom.clamp(MIN_ZOOM, MAX_ZOOM)
    } else {
        1.0
    };
    (BASE_FRAME_WIDTH * zoom).max(1.0)
}

/// The zoom at which `frames` frames just fill `available` points, kept in range.
pub fn fit_zoom(available: f32, frames: u64) -> f32 {
    if !available.is_finite() || available <= 0.0 {
        return 1.0;
    }
    (available / frames.max(1) as f32 / BASE_FRAME_WIDTH).clamp(MIN_ZOOM, MAX_ZOOM)
}

/// How many frames to draw cells for: the scene's, and more until `available` points are filled
/// (with one to spare), as Animate shows empty cells to the panel's edge.
pub fn shown_frames(available: f32, per_frame: f32, frames: u64) -> u64 {
    if !available.is_finite() || !per_frame.is_finite() || per_frame <= 0.0 {
        return frames.max(1);
    }
    let to_fill = (available.max(0.0) / per_frame).ceil() as u64 + 1;
    frames.max(to_fill).max(1)
}

/// The color of a span's bar in the timeline: by what it holds, and a tween has its own so it can
/// be told from a held drawing (motion tweens blue, shape tweens green, as in Animate).
pub fn span_color(content: &flare_document::Content, tween: flare_document::Tween) -> [u8; 3] {
    use flare_document::{Content, Tween};
    match (content, tween) {
        (_, Tween::Linear) => [100, 140, 220],
        (_, Tween::Shape) => [100, 180, 110],
        (Content::Empty, _) => [90, 90, 90],
        (Content::Shape(_), _) => [70, 130, 180],
        (Content::Instance(_), _) => [120, 170, 90],
        (Content::Bitmap(_), _) => [180, 120, 170],
        // The camera's keyframes, in Animate's camera orange.
        (Content::Camera(_), _) => [220, 150, 70],
        // An armature's poses, in a green of their own.
        (Content::Armature(_), _) => [150, 185, 80],
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_is_a_small_fixed_cell_and_zoom_scales_it() {
        assert_eq!(frame_width(1.0), BASE_FRAME_WIDTH);
        assert_eq!(frame_width(2.0), BASE_FRAME_WIDTH * 2.0);
        // It does not depend on how many frames there are or how wide the panel is.
        assert!(frame_width(1.0) < 20.0);
    }

    #[test]
    fn fit_makes_the_frames_just_fill_the_panel_within_the_zoom_range() {
        assert_eq!(fit_zoom(480.0, 48), 1.0);
        assert_eq!(fit_zoom(480.0, 24), 2.0);
        assert_eq!(
            fit_zoom(480.0, 1),
            MAX_ZOOM,
            "one frame is not stretched past the range"
        );
        assert_eq!(fit_zoom(480.0, 100_000), MIN_ZOOM);
        assert_eq!(fit_zoom(0.0, 24), 1.0);
        assert_eq!(fit_zoom(f32::NAN, 24), 1.0);
    }

    #[test]
    fn empty_cells_fill_the_panel_past_the_last_frame() {
        // 24 frames of 10 points use 240 of 800: cells go on to the edge, and one more.
        assert_eq!(shown_frames(800.0, 10.0, 24), 81);
        // A long scene shows all of its frames.
        assert_eq!(shown_frames(800.0, 10.0, 500), 500);
        assert_eq!(shown_frames(f32::NAN, 10.0, 24), 24);
        assert_eq!(shown_frames(800.0, 0.0, 0), 1);
    }

    #[test]
    fn zoom_stays_in_range_and_ignores_bad_factors() {
        assert_eq!(
            zoomed(1.0, 0.0001),
            MIN_ZOOM,
            "cannot zoom out past the range"
        );
        assert_eq!(zoomed(10.0, 100.0), MAX_ZOOM);
        for bad in [f32::NAN, f32::INFINITY, 0.0, -1.0] {
            assert_eq!(zoomed(3.0, bad), 3.0);
        }
    }

    #[test]
    fn degenerate_inputs_still_give_a_drawable_width() {
        assert!(frame_width(f32::NAN) >= 1.0);
        assert!(frame_width(f32::INFINITY) >= 1.0);
        assert!(frame_width(-3.0) >= 1.0);
        assert!(frame_width(0.0) >= 1.0);
    }

    fn layer(
        name: &str,
        folder: bool,
        parent: Option<flare_core::LayerId>,
    ) -> flare_document::Layer {
        flare_document::Layer::new(flare_core::LayerId::new(), name, vec![])
            .expect("layer")
            .with_folder(folder)
            .with_parent(parent)
    }

    #[test]
    fn collapsed_folders_hide_what_they_hold_at_every_depth() {
        let outer = layer("outer", true, None);
        let inner = layer("inner", true, Some(outer.id()));
        let leaf = layer("leaf", false, Some(inner.id()));
        let loose = layer("loose", false, None);
        let layers = [outer.clone(), inner.clone(), leaf, loose];
        let none = std::collections::HashSet::new();
        assert_eq!(
            visible_layers(&layers, &none),
            [(0, 0), (1, 1), (2, 2), (3, 0)],
            "everything shows, indented by depth"
        );
        let inner_closed = std::collections::HashSet::from([inner.id()]);
        assert_eq!(
            visible_layers(&layers, &inner_closed),
            [(0, 0), (1, 1), (3, 0)]
        );
        let outer_closed = std::collections::HashSet::from([outer.id()]);
        assert_eq!(visible_layers(&layers, &outer_closed), [(0, 0), (3, 0)]);
    }
}

#[cfg(test)]
#[path = "timeline_view_sound_tests.rs"]
mod sound_tests;
