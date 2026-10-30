//! Timeline zoom: how wide a frame is drawn, and which layers get a row.
//!
//! At zoom 1.0 the whole scene fits the panel; zooming in widens each frame and the track
//! scrolls horizontally.

pub const MIN_ZOOM: f32 = 1.0;
pub const MAX_ZOOM: f32 = 16.0;
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
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
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
