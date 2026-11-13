//! The frame rate as a number field, in the timeline's toolbar and in the document's
//! properties: dragged or typed, and applied when let go or on Enter, so scrubbing the value is
//! one undo step. Inside a symbol it is dimmed: the rate belongs to the whole document.

use egui::Ui;
use flare_core::FrameRate;

use crate::workspace::Workspace;

impl Workspace {
    pub(super) fn fps_field(&mut self, ui: &mut Ui) -> egui::Response {
        let current = self.document().frame_rate().per_second();
        let mut fps = self.fps_edit.unwrap_or(current);
        let response = ui.add_enabled(
            !self.in_symbol(),
            egui::DragValue::new(&mut fps)
                .range(FrameRate::MIN_PER_SECOND..=FrameRate::MAX_PER_SECOND)
                .speed(0.1)
                .max_decimals(3)
                .suffix(" fps"),
        );
        if response.changed() {
            self.fps_edit = Some(fps);
        }
        if (response.drag_stopped() || response.lost_focus())
            && let Some(fps) = self.fps_edit.take()
        {
            self.set_frame_rate(fps);
        }
        response.on_hover_text(tr!(self, "doc-frame-rate"))
    }
}
