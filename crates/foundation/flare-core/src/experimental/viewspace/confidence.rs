//! Layer 3.5 (spec 8.4): confidence and coverage, and the "suggest next keyframe" helper.
//!
//! Per vertex, confidence = angular x coverage, both in 0..=1:
//! - angular: `1 / (1 + d / REFERENCE)` with d the angle to the nearest contributing sample, so it
//!   is 1 on a keyframe and falls off smoothly.
//! - coverage: the blend-weighted visibility of the vertex in the contributing samples (0 where no
//!   keyframe ever saw the vertex, which the renderer fills from its best guess).

use super::keyframe::ViewParams;
use super::neighbours::{Neighbour, angular_distance};

/// Distance at which angular confidence is one half (20 degrees).
pub const REFERENCE: f32 = 0.349_065_85;

pub fn angular_confidence(nearest_distance: f32) -> f32 {
    1.0 / (1.0 + nearest_distance.max(0.0) / REFERENCE)
}

/// Combine the pieces for one vertex.
pub fn vertex_confidence(neighbours: &[Neighbour], coverage: f32) -> f32 {
    let nearest = neighbours
        .iter()
        .map(|n| n.distance)
        .fold(f32::INFINITY, f32::min);
    if !nearest.is_finite() {
        return 0.0;
    }
    (angular_confidence(nearest) * coverage.clamp(0.0, 1.0)).clamp(0.0, 1.0)
}

/// Heatmap colour: red (low) through yellow to green (high), as premultiplied RGBA.
pub fn heat_colour(c: f32) -> [f32; 4] {
    let c = c.clamp(0.0, 1.0);
    let (r, g) = if c < 0.5 {
        (1.0, c * 2.0)
    } else {
        (1.0 - (c - 0.5) * 2.0, 1.0)
    };
    [r, g, 0.1, 1.0]
}

/// Mean of a confidence slice (0 when empty).
pub fn mean(values: &[f32]) -> f32 {
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<f32>() / values.len() as f32
    }
}

/// Pick the candidate view farthest from every existing sample (max-min angular distance), the
/// view where a new keyframe adds the most coverage. Candidates are a grid of azimuths at each of
/// `elevations_deg` (degrees), `step_deg` apart. Returns None when there are no candidates.
pub fn suggest_next_view(
    existing: &[ViewParams],
    elevations_deg: &[f32],
    step_deg: f32,
) -> Option<(ViewParams, f32)> {
    if !step_deg.is_finite() || step_deg < 1.0 {
        return None;
    }
    let steps = (360.0 / step_deg).floor() as u32;
    let mut best: Option<(ViewParams, f32)> = None;
    for el in elevations_deg
        .iter()
        .filter(|e| e.is_finite() && e.abs() <= 90.0)
    {
        for i in 0..steps.min(720) {
            let cand = ViewParams::from_degrees(-180.0 + i as f32 * step_deg, Some(*el));
            let gap = existing
                .iter()
                .map(|v| angular_distance(&cand, v))
                .fold(f32::INFINITY, f32::min);
            let gap = if gap.is_finite() {
                gap
            } else {
                std::f32::consts::PI
            };
            if best.as_ref().is_none_or(|(_, g)| gap > *g + 1e-6) {
                best = Some((cand, gap));
            }
        }
    }
    best
}

#[cfg(test)]
#[path = "confidence_tests.rs"]
mod tests;
