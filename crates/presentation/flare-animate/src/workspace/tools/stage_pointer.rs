//! What the pointer looks like over the stage while a tool is in hand. A tool that lays down a
//! mark shows only the tip it lays it with (the outline `assist_overlay` draws), so nothing hides
//! the line being drawn. A tool whose icon has a point where it acts is drawn as the pointer with
//! that point on the spot. Any other tool shows a fine cross on the spot with its icon beside it,
//! so the icon never covers where a click lands. The Selection tool keeps the system's arrow,
//! which is its icon already.

use egui::{Color32, CursorIcon, Painter, Pos2, Rect, Response, Stroke, Ui, Vec2};

use crate::workspace::*;

/// How big a tool's picture is drawn as the pointer, in screen points.
const PICTURE_SIZE: f32 = 22.0;

/// How big the icon beside the cross is drawn, in screen points.
const BADGE_SIZE: f32 = 16.0;

/// How far the cross reaches from the spot, and the gap left at its middle so the spot shows.
const CROSS_REACH: f32 = 7.0;
const CROSS_GAP: f32 = 2.0;

/// The rim drawn around a picture, so a white tool shows on a white stage too.
const RIM: Color32 = Color32::from_rgba_premultiplied(0, 0, 0, 220);

/// The pointer a tool shows over the stage.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::workspace) enum StagePointer {
    /// The system's arrow.
    Arrow,
    /// Only the outline of what the tool lays down.
    Tip,
    /// The icon as the pointer, touching the stage at `tip` in its 24-unit square.
    Picture { icon: &'static str, tip: Vec2 },
    /// A cross on the spot, with the icon below and to the right of it.
    Marked { icon: &'static str },
}

/// The pointer for `tool`. `rubs` is whether the Eraser rubs out (it has a size then) rather than
/// deleting what is clicked.
pub(in crate::workspace) fn stage_pointer(tool: Tool, rubs: bool) -> StagePointer {
    let picture = |icon, x, y| StagePointer::Picture {
        icon,
        tip: Vec2::new(x, y),
    };
    let marked = |icon| StagePointer::Marked { icon };
    // Every tool is named, so a new one has to choose its pointer.
    match tool {
        Tool::Select => StagePointer::Arrow,
        Tool::Brush | Tool::Pencil | Tool::Blur | Tool::Smudge => StagePointer::Tip,
        Tool::Eraser if rubs => StagePointer::Tip,
        Tool::Eraser => marked("tool-eraser"),
        // The bucket's drip, the bottle's base, the dropper's tip, the nib, the arrow's point,
        // the middle of the palm and of the lens.
        Tool::Bucket => picture("tool-bucket", 19.25, 20.5),
        Tool::InkBottle => picture("tool-ink-bottle", 12.0, 20.8),
        Tool::Eyedropper => picture("tool-eyedropper", 4.3, 19.95),
        Tool::Pen => picture("tool-pen", 12.0, 3.0),
        Tool::Subselection => picture("tool-subselection", 6.5, 3.8),
        Tool::Hand => picture("tool-hand", 12.4, 12.0),
        // The middle of the turning arrows.
        Tool::RotateView => picture("tool-rotate-view", 12.0, 12.0),
        Tool::Zoom => picture("tool-zoom", 10.5, 10.5),
        Tool::Rectangle => marked("tool-rectangle"),
        Tool::Ellipse => marked("tool-oval"),
        Tool::PolyStar => marked("tool-polystar"),
        Tool::Line => marked("tool-line"),
        Tool::Text => marked("tool-text"),
        Tool::Lasso => marked("tool-lasso"),
        Tool::PolygonLasso => marked("tool-lasso-polygon"),
        Tool::MagicWand => marked("tool-magic-wand"),
        Tool::Camera => marked("tool-camera"),
        Tool::Bone => marked("tool-bone"),
        Tool::AddAnchor => marked("tool-anchor-add"),
        Tool::DeleteAnchor => marked("tool-anchor-delete"),
        Tool::ConvertAnchor => marked("tool-anchor-convert"),
        Tool::FreeTransform => marked("tool-transform"),
        Tool::Ruler => marked("tool-ruler"),
        Tool::Mirror => marked("tool-mirror"),
    }
}

/// Paints `icon` in white with a dark rim around it.
fn paint_rimmed(ui: &Ui, icon: &str, size: f32, rect: Rect) {
    for step in [
        (-1.0, 0.0),
        (1.0, 0.0),
        (0.0, -1.0),
        (0.0, 1.0),
        (-1.0, -1.0),
        (1.0, 1.0),
        (-1.0, 1.0),
        (1.0, -1.0),
    ] {
        flare_ui::icons::image(icon, size, RIM).paint_at(ui, rect.translate(Vec2::from(step)));
    }
    flare_ui::icons::image(icon, size, Color32::WHITE).paint_at(ui, rect);
}

/// A thin cross on `at`, white over dark so it reads on any artwork.
fn paint_cross(painter: &Painter, at: Pos2) {
    for (width, color) in [(3.0, RIM), (1.0, Color32::WHITE)] {
        let stroke = Stroke::new(width, color);
        for dir in [Vec2::X, -Vec2::X, Vec2::Y, -Vec2::Y] {
            painter.line_segment([at + dir * CROSS_GAP, at + dir * CROSS_REACH], stroke);
        }
    }
}

impl Workspace {
    /// Shows the tool in hand as the pointer while it is over the stage, in place of the system's
    /// arrow.
    pub(in crate::workspace) fn tool_pointer(&self, ui: &Ui, response: &Response) {
        let Some(at) = response.hover_pos() else {
            return;
        };
        // What the stage already chose a pointer for this pass (a guide's grip, a Free Transform
        // handle) keeps it: it says what a press there would do.
        let claimed = ui.ctx().output(|o| o.cursor_icon) != CursorIcon::Default;
        if claimed || self.stage_grab().is_some() {
            return;
        }
        match stage_pointer(self.tool, self.eraser_rubs()) {
            StagePointer::Arrow => {}
            StagePointer::Tip => {
                if self.brush_cursor().is_some() {
                    ui.ctx().set_cursor_icon(CursorIcon::None);
                }
            }
            StagePointer::Picture { icon, tip } => {
                ui.ctx().set_cursor_icon(CursorIcon::None);
                let min = at - tip * (PICTURE_SIZE / 24.0);
                paint_rimmed(
                    ui,
                    icon,
                    PICTURE_SIZE,
                    Rect::from_min_size(min, Vec2::splat(PICTURE_SIZE)),
                );
            }
            StagePointer::Marked { icon } => {
                ui.ctx().set_cursor_icon(CursorIcon::None);
                paint_cross(ui.painter(), at);
                let min = at + Vec2::splat(CROSS_REACH - 1.0);
                paint_rimmed(
                    ui,
                    icon,
                    BADGE_SIZE,
                    Rect::from_min_size(min, Vec2::splat(BADGE_SIZE)),
                );
            }
        }
    }
}
