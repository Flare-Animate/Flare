//! Small functions the panels share.

use egui::{Key, Modifiers, Pos2, Rect, Vec2};

use super::*;

/// Whether Enter was pressed this pass.
pub(super) fn enter_pressed(ctx: &egui::Context) -> bool {
    key_press(ctx, Key::Enter).is_some()
}

/// The layer rows (first index, how many) that intersect `clip`, given the top of the track area,
/// the row height, and one header row above the first layer. Rows partly in view are included.
pub(super) fn visible_rows(top: f32, row: f32, clip: Rect) -> (usize, usize) {
    if !(row > 0.0 && row.is_finite())
        || !top.is_finite()
        || !clip.min.y.is_finite()
        || !clip.max.y.is_finite()
        || clip.is_negative()
    {
        return (0, 0);
    }
    // Layer `i` occupies y in [top + row * (i + 1), top + row * (i + 2)).
    let first = (((clip.min.y - top) / row).floor() - 1.0).max(0.0);
    let last = (((clip.max.y - top) / row).ceil() - 2.0).max(0.0);
    if !first.is_finite() || !last.is_finite() || last < first {
        return (0, 0);
    }
    (first as usize, (last - first) as usize + 1)
}

/// The largest texture side to ask for: the backend's own limit, and never more than we want.
pub(super) fn texture_side_limit(ctx: &egui::Context) -> u32 {
    let backend = ctx.input(|i| i.max_texture_side);
    u32::try_from(backend)
        .unwrap_or(u32::MAX)
        .min(MAX_TEXTURE_SIDE)
}

/// Modifiers held when `key` was pressed this pass, if it was pressed.
pub(super) fn key_press(ctx: &egui::Context, key: Key) -> Option<Modifiers> {
    ctx.input(|i| {
        i.events.iter().find_map(|event| match event {
            egui::Event::Key {
                key: k,
                pressed: true,
                modifiers,
                ..
            } if *k == key => Some(*modifiers),
            _ => None,
        })
    })
}

/// How far a duplicate is moved from its original, in stage units.
pub(super) const DUPLICATE_OFFSET: f64 = 12.0;

/// Scale Up / Scale Down factor.
pub(super) const SCALE_STEP: f64 = 1.25;

/// Screen position to stage (document) coordinates, kept on the pasteboard.
pub(super) fn screen_to_stage(p: Pos2, image_rect: Rect, extent: Vec2) -> Pos2 {
    let u = (p.x - image_rect.min.x) / image_rect.width().max(f32::EPSILON);
    let v = (p.y - image_rect.min.y) / image_rect.height().max(f32::EPSILON);
    // Off the stage is the pasteboard, where artwork may be drawn too; past it, nothing.
    let board = flare_ui::view::pasteboard(extent);
    Pos2::new(
        (u * extent.x).clamp(board.min.x, board.max.x),
        (v * extent.y).clamp(board.min.y, board.max.y),
    )
}
