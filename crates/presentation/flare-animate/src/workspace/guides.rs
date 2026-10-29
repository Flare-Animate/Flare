//! Guides: lines pulled out of the rulers to line things up against. They belong to the view
//! of one document and are not saved with it.

use egui::{Color32, Pos2, Rect, Sense, Stroke, Vec2};

use super::Workspace;

/// Most guides a document view holds, so dragging cannot grow the list without end.
pub const MAX_GUIDES: usize = 200;

/// Thickness of the rulers the guides are pulled from.
const RULER: f32 = 16.0;

/// One guide: a vertical line at stage x `at`, or a horizontal one at stage y `at`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Guide {
    pub vertical: bool,
    pub at: f32,
}

/// `point` moved onto a guide that is within `reach` stage units, each axis on its own.
pub fn snap_to_guides(point: Pos2, guides: &[Guide], reach: f32) -> Pos2 {
    let nearest = |vertical: bool, value: f32| {
        guides
            .iter()
            .filter(|g| g.vertical == vertical && (g.at - value).abs() <= reach)
            .min_by(|a, b| (a.at - value).abs().total_cmp(&(b.at - value).abs()))
            .map_or(value, |g| g.at)
    };
    Pos2::new(nearest(true, point.x), nearest(false, point.y))
}

impl Workspace {
    /// Starts a guide at `at` and returns its index, unless there are too many already or `at`
    /// is not a number.
    pub fn begin_guide(&mut self, vertical: bool, at: f32) -> Option<usize> {
        if self.guides.len() >= MAX_GUIDES || !at.is_finite() {
            return None;
        }
        self.guides.push(Guide { vertical, at });
        Some(self.guides.len() - 1)
    }

    /// Moves guide `index` to `at`; a number that is not finite is ignored.
    pub fn move_guide(&mut self, index: usize, at: f32) {
        if let Some(guide) = self.guides.get_mut(index)
            && at.is_finite()
        {
            guide.at = at;
        }
    }

    /// Ends a drag of guide `index`; a guide dropped back on a ruler is taken away.
    pub fn release_guide(&mut self, index: usize, remove: bool) {
        if remove && index < self.guides.len() {
            self.guides.remove(index);
        }
        self.guide_drag = None;
    }

    pub fn guides(&self) -> &[Guide] {
        &self.guides
    }

    /// Draws the guides and handles pulling them out of, moving them along and returning them
    /// to the rulers. `image` is the stage's rectangle on screen.
    pub(super) fn stage_guides(&mut self, ui: &mut egui::Ui, panel: Rect, image: Rect, scale: f32) {
        let top = Rect::from_min_size(panel.min, Vec2::new(panel.width(), RULER));
        let left = Rect::from_min_size(panel.min, Vec2::new(RULER, panel.height()));
        if self.show_rulers {
            let ids = (
                ui.make_persistent_id("guide-top"),
                ui.make_persistent_id("guide-left"),
            );
            let from_top = ui.interact(top, ids.0, Sense::drag());
            let from_left = ui.interact(left, ids.1, Sense::drag());
            if self.guide_drag.is_none() {
                for (response, vertical) in [(&from_top, false), (&from_left, true)] {
                    if response.drag_started()
                        && let Some(p) = response.interact_pointer_pos()
                    {
                        let at = if vertical {
                            (p.x - image.min.x) / scale
                        } else {
                            (p.y - image.min.y) / scale
                        };
                        self.guide_drag = self.begin_guide(vertical, at);
                    }
                }
            }
        }
        if !self.show_guides {
            return;
        }
        let ink = Stroke::new(1.0, Color32::from_rgb(0, 200, 220));
        let painter = ui.painter_at(panel);
        let mut grabbed = None;
        for (index, guide) in self.guides.clone().into_iter().enumerate() {
            let screen = if guide.vertical {
                image.min.x + guide.at * scale
            } else {
                image.min.y + guide.at * scale
            };
            let band = if guide.vertical {
                Rect::from_min_max(
                    Pos2::new(screen - 3.0, panel.min.y),
                    Pos2::new(screen + 3.0, panel.max.y),
                )
            } else {
                Rect::from_min_max(
                    Pos2::new(panel.min.x, screen - 3.0),
                    Pos2::new(panel.max.x, screen + 3.0),
                )
            };
            if guide.vertical {
                painter.vline(screen, panel.y_range(), ink);
            } else {
                painter.hline(panel.x_range(), screen, ink);
            }
            let grip = ui.interact(band, ui.make_persistent_id(("guide", index)), Sense::drag());
            if grip.hovered() || grip.dragged() {
                ui.ctx().set_cursor_icon(if guide.vertical {
                    egui::CursorIcon::ResizeHorizontal
                } else {
                    egui::CursorIcon::ResizeVertical
                });
            }
            if grip.drag_started() && self.guide_drag.is_none() {
                self.guide_drag = Some(index);
            }
            if grip.dragged() || grip.drag_stopped() {
                grabbed = Some((index, grip.interact_pointer_pos()));
            }
        }
        let (Some(index), Some((dragged, pointer))) = (self.guide_drag, grabbed) else {
            return;
        };
        if dragged != index {
            return;
        }
        let Some(p) = pointer.or_else(|| ui.input(|i| i.pointer.latest_pos())) else {
            return;
        };
        let vertical = self.guides.get(index).is_some_and(|g| g.vertical);
        let at = if vertical {
            (p.x - image.min.x) / scale
        } else {
            (p.y - image.min.y) / scale
        };
        self.move_guide(index, at);
        if ui.input(|i| i.pointer.any_down()) {
            return;
        }
        // Let go on the ruler it came from (or off the stage): the guide goes back.
        let on_ruler = if vertical {
            left.contains(p)
        } else {
            top.contains(p)
        };
        self.release_guide(index, on_ruler || !panel.contains(p));
    }
}
