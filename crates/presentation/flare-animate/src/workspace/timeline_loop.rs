//! The loop range on the ruler, as Animate draws it while Loop playback is on: a bar along the
//! top of the ruler over the frames that play round and round, with a handle at each end to
//! drag. Without a range of its own the bar covers the whole scene. A double click on the bar
//! gives the whole scene back. The rules are `flare_editor::loop_range`'s.

use egui::{Color32, Painter, Pos2, Rect, Stroke, Vec2};
use flare_editor::loop_range::{LoopEnd, LoopRange};

use crate::workspace::timeline::{Layout, ROW};
use crate::workspace::*;

/// Loop playback: whether it is on (kept with the preferences), the frames it goes round (kept
/// with the open document, never saved) and the end of the bar being dragged.
#[derive(Debug, Clone, Copy)]
pub(in crate::workspace) struct Looping {
    pub(in crate::workspace) on: bool,
    pub(in crate::workspace) range: Option<LoopRange>,
    pub(in crate::workspace) drag: Option<LoopEnd>,
}

impl Default for Looping {
    fn default() -> Self {
        Self {
            on: true,
            range: None,
            drag: None,
        }
    }
}

/// How near a handle a press must be, in points, to take it.
const HANDLE_REACH: f32 = 5.0;
/// The bar's height, along the top of the ruler, above the frame numbers.
const BAR: f32 = 4.0;

impl Layout {
    /// The left edge of the range's first frame and the right edge of its last.
    pub(in crate::workspace) fn loop_bar(&self, range: LoopRange) -> (f32, f32) {
        (
            self.frame_x(range.from() as f32),
            self.frame_x(range.to().saturating_add(1) as f32),
        )
    }

    /// Whether `p` is on the top half of the ruler, where the loop bar is; the onion-skin
    /// markers keep the bottom half.
    pub(in crate::workspace) fn on_loop_strip(&self, p: Pos2) -> bool {
        p.y >= self.ruler_top && p.y < self.ruler_top + ROW / 2.0 && !self.on_labels(p)
    }

    /// The end of the bar a press at `p` takes, if it is near one.
    pub(in crate::workspace) fn loop_end_at(&self, p: Pos2, range: LoopRange) -> Option<LoopEnd> {
        if !self.on_loop_strip(p) {
            return None;
        }
        let (start, end) = self.loop_bar(range);
        let (to_start, to_end) = ((p.x - start).abs(), (p.x - end).abs());
        if to_start <= HANDLE_REACH && to_start <= to_end {
            Some(LoopEnd::Start)
        } else if to_end <= HANDLE_REACH {
            Some(LoopEnd::End)
        } else {
            None
        }
    }
}

impl Workspace {
    /// The frames Loop playback goes round, or `None` for the whole scene.
    pub fn loop_range(&self) -> Option<LoopRange> {
        self.looping
            .range
            .and_then(|range| range.within(self.scene_frames()))
    }

    /// Sets the frames Loop playback goes round; `None` loops the whole scene.
    pub fn set_loop_range(&mut self, range: Option<LoopRange>) {
        self.looping.range = range;
    }

    /// The range the bar shows: the one set, or the whole scene.
    fn shown_loop_range(&self) -> LoopRange {
        self.loop_range()
            .unwrap_or(LoopRange::new(0, self.scene_frames().saturating_sub(1)))
    }

    /// Takes, moves and lets go of an end of the loop bar, and a double click on the bar
    /// clears the range; true while the bar has the press.
    pub(in crate::workspace) fn loop_bar_input(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        l: &Layout,
    ) -> bool {
        if !self.looping.on {
            self.looping.drag = None;
            return false;
        }
        if response.double_clicked()
            && let Some(p) = response.interact_pointer_pos()
            && l.on_loop_strip(p)
        {
            let (start, end) = l.loop_bar(self.shown_loop_range());
            if p.x >= start - HANDLE_REACH && p.x <= end + HANDLE_REACH {
                self.looping.range = None;
                return true;
            }
        }
        if response.drag_started()
            && let Some(p) = ui.input(|i| i.pointer.press_origin())
        {
            self.looping.drag = l.loop_end_at(p, self.shown_loop_range());
        }
        let Some(end) = self.looping.drag else {
            return false;
        };
        if let Some(p) = response.interact_pointer_pos() {
            let at = l.frame_at(p.x).round().max(0.0) as u32;
            // The start handle sits on a frame's left edge, the end handle on its right.
            let frame = match end {
                LoopEnd::Start => at,
                LoopEnd::End => at.saturating_sub(1),
            };
            self.looping.range =
                LoopRange::dragged(self.looping.range, end, frame, self.scene_frames());
        }
        if response.drag_stopped() || !response.dragged() {
            self.looping.drag = None;
        }
        true
    }

    /// Paints the loop bar along the top of the ruler `band`, while Loop playback is on.
    pub(in crate::workspace) fn loop_bar(&self, painter: &Painter, l: &Layout, band: Rect) {
        if !self.looping.on {
            return;
        }
        let range = self.shown_loop_range();
        let (start, end) = l.loop_bar(range);
        // A range of its own is drawn strong; the whole scene, faintly, as a bar to grab.
        let accent = flare_ui::theme::accent();
        let fill = if self.loop_range().is_some() {
            accent
        } else {
            accent.gamma_multiply(0.4)
        };
        let bar = Rect::from_min_max(
            Pos2::new(start.max(l.label_edge), band.top()),
            Pos2::new(end, band.top() + BAR),
        );
        if bar.width() > 0.0 {
            painter.rect_filled(bar, 0.0, fill);
        }
        for x in [start, end] {
            if x < l.label_edge {
                continue;
            }
            let handle =
                Rect::from_center_size(Pos2::new(x, band.top() + 4.0), Vec2::new(3.0, 8.0));
            painter.rect_filled(handle, 1.0, accent);
            painter.rect_stroke(
                handle,
                1.0,
                Stroke::new(1.0, Color32::BLACK.gamma_multiply(0.5)),
                egui::StrokeKind::Outside,
            );
        }
    }
}
