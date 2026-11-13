//! Edit Multiple Frames, as Animate has it: while it is on, a bracket along the bottom of the
//! ruler marks a range of frames, every keyframe in it shows on the stage at full strength, and
//! clicking, Select All, the lasso and the marquee pick from all of them, so Move and Free
//! Transform change the whole animation at once (one undo step). Onion skin rests meanwhile: the
//! two would draw over each other. The bracket starts over the whole scene; its ends are
//! dragged, and a double click on it gives the whole scene back. The rules are
//! `flare_tools::multi_frame`'s.

use egui::{Painter, Pos2, Rect, Stroke, Vec2};
use flare_editor::loop_range::{LoopEnd, LoopRange};

use crate::workspace::timeline::{Layout, ROW};
use crate::workspace::*;

/// How near a handle a press must be, in points, to take it.
const HANDLE_REACH: f32 = 5.0;

/// Whether Edit Multiple Frames is on, the range its bracket marks (none for the whole scene)
/// and the end of the bracket being dragged.
#[derive(Debug, Clone, Copy, Default)]
pub(in crate::workspace) struct MultiFrames {
    pub(in crate::workspace) on: bool,
    pub(in crate::workspace) range: Option<LoopRange>,
    pub(in crate::workspace) drag: Option<LoopEnd>,
}

impl Layout {
    /// Whether `p` is on the bottom half of the ruler, where the bracket is.
    pub(in crate::workspace) fn on_multi_strip(&self, p: Pos2) -> bool {
        p.y >= self.ruler_top + ROW / 2.0 && p.y < self.ruler_top + ROW && !self.on_labels(p)
    }
}

impl Workspace {
    /// Whether Edit Multiple Frames is on.
    pub fn editing_multiple_frames(&self) -> bool {
        self.display.multi_frame.on
    }

    /// Turns Edit Multiple Frames on or off.
    pub fn set_edit_multiple_frames(&mut self, on: bool) {
        self.display.multi_frame.on = on;
        self.display.multi_frame.drag = None;
    }

    /// The frames Edit Multiple Frames takes in while it is on: the bracket's, cut to the scene,
    /// or the whole scene.
    pub fn multi_frame_range(&self) -> Option<LoopRange> {
        if !self.display.multi_frame.on {
            return None;
        }
        let frames = self.scene_frames();
        Some(
            self.display
                .multi_frame
                .range
                .and_then(|r| r.within(frames))
                .unwrap_or(LoopRange::new(0, frames.saturating_sub(1))),
        )
    }

    /// Sets the bracket's frames; `None` takes in the whole scene.
    pub fn set_multi_frame_range(&mut self, range: Option<LoopRange>) {
        self.display.multi_frame.range = range;
    }

    /// The frames drawn on the stage besides the playhead's while editing multiple frames.
    pub(in crate::workspace) fn multi_frames_shown(&self) -> Vec<u32> {
        let (Some(range), Some(scene)) = (self.multi_frame_range(), self.scene_id()) else {
            return Vec::new();
        };
        flare_tools::multi_frame::frames_shown(self.document(), scene, range.from(), range.to())
    }

    /// Takes, moves and lets go of an end of the bracket, and a double click on it takes in the
    /// whole scene again; true while the bracket has the press.
    pub(in crate::workspace) fn multi_bar_input(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        l: &Layout,
    ) -> bool {
        let Some(range) = self.multi_frame_range() else {
            return false;
        };
        let (start, end) = l.loop_bar(range);
        if response.double_clicked()
            && let Some(p) = response.interact_pointer_pos()
            && l.on_multi_strip(p)
            && p.x >= start - HANDLE_REACH
            && p.x <= end + HANDLE_REACH
        {
            self.display.multi_frame.range = None;
            return true;
        }
        if response.drag_started()
            && let Some(p) = ui.input(|i| i.pointer.press_origin())
            && l.on_multi_strip(p)
        {
            let (to_start, to_end) = ((p.x - start).abs(), (p.x - end).abs());
            self.display.multi_frame.drag = if to_start <= HANDLE_REACH && to_start <= to_end {
                Some(LoopEnd::Start)
            } else if to_end <= HANDLE_REACH {
                Some(LoopEnd::End)
            } else {
                None
            };
        }
        let Some(grabbed) = self.display.multi_frame.drag else {
            return false;
        };
        if let Some(p) = response.interact_pointer_pos() {
            let at = l.frame_at(p.x).round().max(0.0) as u32;
            // The start handle sits on a frame's left edge, the end handle on its right.
            let frame = match grabbed {
                LoopEnd::Start => at,
                LoopEnd::End => at.saturating_sub(1),
            };
            self.display.multi_frame.range = LoopRange::dragged(
                self.display.multi_frame.range,
                grabbed,
                frame,
                self.scene_frames(),
            );
        }
        if response.drag_stopped() || !response.dragged() {
            self.display.multi_frame.drag = None;
        }
        true
    }

    /// Paints the bracket along the bottom of the ruler `band` while Edit Multiple Frames is on.
    pub(in crate::workspace) fn multi_bar(&self, painter: &Painter, l: &Layout, band: Rect) {
        let Some(range) = self.multi_frame_range() else {
            return;
        };
        let (start, end) = l.loop_bar(range);
        let ink = flare_ui::theme::text();
        let y = band.bottom() - 3.0;
        if end > l.label_edge {
            painter.hline(start.max(l.label_edge)..=end, y, Stroke::new(2.0, ink));
        }
        for x in [start, end] {
            if x < l.label_edge {
                continue;
            }
            let handle = Rect::from_center_size(Pos2::new(x, y - 3.0), Vec2::new(3.0, 9.0));
            painter.rect_filled(handle, 1.0, ink);
        }
    }
}
