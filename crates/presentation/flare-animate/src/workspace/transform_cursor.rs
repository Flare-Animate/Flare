//! The pointer over the Free Transform frame: it shows what a press there would do, and keeps
//! showing it for the whole drag.

use egui::{Color32, CursorIcon, Pos2, Rect, Vec2};
use flare_tools::free_transform::{Grab, GrabAxis, grab_at};

use super::*;

impl Workspace {
    /// What a press at `point` takes hold of on the Free Transform frame, with the frame. `None`
    /// with any other tool, with nothing selected, or off the frame.
    pub(super) fn transform_grab(&self, point: Pos2) -> Option<(Grab, Rect)> {
        if self.tool != Tool::FreeTransform {
            return None;
        }
        let bounds = self.selection_bounds()?;
        grab_at(bounds, point, self.handle_radius, self.skew_modifier).map(|grab| (grab, bounds))
    }

    /// What the stage pointer stands for now: the drag in progress, or else what a press at the
    /// pointer would take hold of.
    pub(super) fn stage_grab(&self) -> Option<Grab> {
        if let Some(drag) = self.drag {
            return match drag.mode {
                DragMode::Scale(handle) => Some(Grab::Scale(handle)),
                DragMode::Skew(handle) => Some(Grab::Skew(handle)),
                DragMode::Rotate => Some(Grab::Rotate),
                DragMode::Move => Some(Grab::Move),
                DragMode::Draw => None,
            };
        }
        self.transform_grab(self.pointer_stage?)
            .map(|(grab, _)| grab)
    }

    /// Sets the pointer for [`Self::stage_grab`]. There is no system pointer for turning, so a
    /// rotation shows the arrow with a turning mark beside it.
    pub(super) fn transform_pointer(&self, ui: &egui::Ui, response: &egui::Response) {
        if !(response.hovered() || self.drag.is_some()) {
            return;
        }
        let Some(grab) = self.stage_grab() else {
            return;
        };
        let icon = match grab.axis() {
            Some(GrabAxis::Horizontal) => CursorIcon::ResizeHorizontal,
            Some(GrabAxis::Vertical) => CursorIcon::ResizeVertical,
            Some(GrabAxis::Falling) => CursorIcon::ResizeNwSe,
            Some(GrabAxis::Rising) => CursorIcon::ResizeNeSw,
            None if grab == Grab::Move => CursorIcon::Move,
            None => {
                if let Some(at) = ui.ctx().pointer_latest_pos() {
                    let mark =
                        Rect::from_center_size(at + Vec2::new(16.0, 16.0), Vec2::splat(16.0));
                    // A dark disc under the white mark reads on any artwork.
                    ui.painter()
                        .circle_filled(mark.center(), 11.0, Color32::from_black_alpha(180));
                    flare_ui::icons::image("rotate-cw", 16.0, Color32::WHITE).paint_at(ui, mark);
                }
                CursorIcon::Default
            }
        };
        ui.ctx().set_cursor_icon(icon);
    }
}
