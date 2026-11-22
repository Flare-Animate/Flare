//! The frame picker: the frame count in the status bar opens a popup to jump to a frame by its
//! number, by the previous or next keyframe, or by clicking its cell in a grid of the scene's
//! frames. Where a request lands is `flare_editor::frame_picker`'s, as for `flare goto` and the
//! MCP server's `goto_frame`.

use egui::{Color32, Popup, PopupCloseBehavior, RichText, Sense, Ui, vec2};
use flare_editor::frame_picker::{self, Goto, GotoError};
use flare_ui::theme;

use crate::workspace::Workspace;

/// Points one cell of the grid is wide and tall.
const CELL: f32 = 18.0;

impl Workspace {
    /// Moves the playhead the way `goto` says, from where it is now. The error says why not (a
    /// frame past the end, no keyframe that way); the playhead stays.
    pub fn go_to(&mut self, goto: Goto) -> Result<u32, GotoError> {
        let timeline = self
            .document()
            .scenes()
            .first()
            .map(|scene| scene.timeline())
            .ok_or(GotoError::NoFrames)?;
        let frame = frame_picker::resolve(timeline, self.frame, goto)?;
        self.set_frame(frame);
        Ok(frame)
    }

    /// The frame count as a button, and the picker's popup under it while it is open.
    pub(super) fn frame_picker_button(&mut self, ui: &mut Ui, text: RichText) {
        let tip = tr!(self, "bar-frame-picker");
        let response = ui
            .add(egui::Button::new(text).frame(false))
            .on_hover_text(tip);
        Popup::from_toggle_button_response(&response)
            .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                ui.set_width(300.0);
                self.frame_picker(ui);
            });
    }

    fn frame_picker(&mut self, ui: &mut Ui) {
        ui.strong(tr!(self, "bar-frame-picker"));
        ui.add_space(4.0);
        let frames = self.scene_frames();
        // Shown from 1, as the timeline's ruler and the status bar count.
        let mut number = self.frame + 1;
        ui.horizontal(|ui| {
            ui.label(tr!(self, "bar-frame-number"));
            let before = number;
            ui.add(egui::DragValue::new(&mut number).range(1..=frames));
            if number != before {
                self.set_frame(number - 1);
            }
        });
        let (previous, next) = (
            tr!(self, "bar-frame-previous-keyframe"),
            tr!(self, "bar-frame-next-keyframe"),
        );
        ui.horizontal(|ui| {
            for (label, goto) in [
                (previous, Goto::PreviousKeyframe),
                (next, Goto::NextKeyframe),
            ] {
                // At the first or last keyframe there is none to go to: the button is dimmed.
                let possible = self.document().scenes().first().is_some_and(|scene| {
                    frame_picker::resolve(scene.timeline(), self.frame, goto).is_ok()
                });
                if ui.add_enabled(possible, egui::Button::new(label)).clicked() {
                    let _ = self.go_to(goto);
                }
            }
        });
        ui.add_space(4.0);
        let cells = self
            .document()
            .scenes()
            .first()
            .map(|scene| frame_picker::cells(scene.timeline(), 0, frame_picker::MAX_CELLS))
            .unwrap_or_default();
        let mut picked = None;
        egui::ScrollArea::vertical()
            .max_height(180.0)
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
                    for cell in &cells {
                        let (rect, response) =
                            ui.allocate_exact_size(vec2(CELL, CELL), Sense::click());
                        let here = cell.frame == self.frame;
                        let fill = if here {
                            theme::accent()
                        } else if cell.keyframes > 0 {
                            theme::text_dim()
                        } else if cell.shows_art {
                            Color32::from_gray(70)
                        } else {
                            Color32::from_gray(45)
                        };
                        ui.painter().rect_filled(rect, 2.0, fill);
                        if response.hovered() {
                            ui.painter().rect_stroke(
                                rect,
                                2.0,
                                egui::Stroke::new(1.0, theme::accent()),
                                egui::StrokeKind::Inside,
                            );
                        }
                        if response.clicked() {
                            picked = Some(cell.frame);
                        }
                        response.on_hover_text((cell.frame + 1).to_string());
                    }
                });
            });
        if let Some(frame) = picked {
            self.set_frame(frame);
        }
    }
}
