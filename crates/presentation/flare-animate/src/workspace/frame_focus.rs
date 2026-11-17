//! Copy, Cut, Paste and Delete on the timeline's frames, and dragging a run of selected frames.
//!
//! After a click among the timeline's frames the plain clipboard keys and Delete act on frames,
//! as they would on what is selected on the stage after a click there. Delete takes empty frames
//! out, closing the gap between the keyframes either side; frames that hold something are left to
//! the commands that say what they do to them (Remove Frames, Clear Keyframe).

use flare_document::{Content, Layer};
use flare_editor::frames::{self, FramePlacement};

use super::timeline_layer_drag::Grab;
use super::*;

/// Where the clipboard keys and Delete act.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct FrameFocus {
    /// The last click was among the timeline's frames, not on the stage.
    pub(super) on_frames: bool,
    /// Frames were copied after anything on the stage, so Paste puts frames back.
    pub(super) frames_copied_last: bool,
}

/// Whether frames `from..=to` of `layer` show nothing: no keyframe, or a blank one.
fn all_empty(layer: &Layer, from: u32, to: u32) -> bool {
    layer
        .spans()
        .iter()
        .filter(|s| s.start() <= to && s.end() > u64::from(from))
        .all(|s| s.element_count() == 1 && matches!(s.content(), Content::Empty))
}

impl Workspace {
    /// Copy (Cmd/Ctrl+C): the frames after a click in the timeline, else what is selected.
    pub(super) fn copy_in_focus(&mut self) {
        if self.frame_focus.on_frames {
            self.copy_frames();
        } else {
            self.copy_selection();
        }
    }

    /// Cut (Cmd/Ctrl+X): the frames after a click in the timeline, else what is selected.
    pub(super) fn cut_in_focus(&mut self) {
        if self.frame_focus.on_frames {
            self.cut_frames();
        } else {
            self.cut_selection();
        }
    }

    /// Paste (Cmd/Ctrl+V): frames, at the playhead of the active layer, when the timeline was
    /// clicked last and frames were what was copied last; else what was copied on the stage.
    pub(super) fn paste_in_focus(&mut self) {
        let focus = self.frame_focus;
        if focus.on_frames && focus.frames_copied_last && self.frame_clipboard.is_some() {
            self.paste_frames();
        } else {
            self.paste();
        }
    }

    /// Delete after a click in the timeline: takes the selected frames (or the frame at the
    /// playhead) out of the active layer when none of them shows anything, so the keyframes after
    /// them move up and the gap closes, as one undo step. False when that is not what Delete does
    /// here, so it acts on the stage's selection instead.
    pub(super) fn delete_empty_frames(&mut self) -> bool {
        let (Some(scene), Some((id, from, to))) = (self.scene_id(), self.frames_in_play()) else {
            return false;
        };
        let Some(layer) = self.layer(id) else {
            return false;
        };
        if layer.is_locked() || !all_empty(layer, from, to) {
            return false;
        }
        let Some(commands) = frames::remove_frames(scene, layer, from, to) else {
            return false;
        };
        self.run_batch(commands, "remove frames");
        if self.status.is_none() {
            self.frame_range = None;
            self.set_frame(from);
            self.validate_selection();
        }
        true
    }

    /// Starts dragging the frames selected on `layer` when the press, at the fractional frame
    /// `at`, is inside them. False when it is not, so the keyframe under it is dragged instead.
    pub(super) fn begin_frames_drag(&mut self, layer: LayerId, at: f32) -> bool {
        let Some(range) = self.frame_range.filter(|r| r.layer == layer) else {
            return false;
        };
        if !at.is_finite() || at < 0.0 || self.layer(layer).is_none_or(Layer::is_locked) {
            return false;
        }
        let frame = at.floor() as u32;
        if frame < range.from || frame > range.to {
            return false;
        }
        self.timeline_grab = Some(Grab::Span(SpanDrag {
            layer,
            start: range.from,
            grab: SpanGrab::Range {
                last: range.to,
                offset: frame - range.from,
            },
            frame,
            onto: layer,
            copy: false,
        }));
        true
    }

    /// Whether the frames being dragged are copied (Alt held) rather than moved when let go.
    pub(super) fn set_span_drag_copy(&mut self, copy: bool) {
        if let Some(Grab::Span(drag)) = &mut self.timeline_grab {
            drag.copy = copy;
        }
    }

    /// Lets go of dragged frames, a selected run or a keyframe copied with Alt, at `start` on the
    /// row they are over: they replace what they land on, as one undo step, and are selected there.
    pub(super) fn place_dragged_frames(&mut self, drag: SpanDrag, start: u32, length: u32) {
        let (Some(scene), Some(from), Some(onto)) = (
            self.scene_id(),
            self.layer(drag.layer),
            self.layer(drag.onto),
        ) else {
            return;
        };
        let last = drag.start.saturating_add(length.saturating_sub(1));
        let how = if drag.copy {
            FramePlacement::Copy
        } else {
            FramePlacement::Move
        };
        let unchanged =
            how == FramePlacement::Move && start == drag.start && from.id() == onto.id();
        match frames::place_frames(scene, from, (drag.start, last), onto, start, how) {
            Some(commands) => {
                let what = match how {
                    FramePlacement::Copy => "copy frames",
                    FramePlacement::Move => "move frames",
                };
                self.run_batch(commands, what);
                if self.status.is_none() {
                    if drag.onto != drag.layer {
                        self.select_layer(drag.onto);
                    }
                    self.select_frames(drag.onto, start, start.saturating_add(length - 1));
                    self.set_frame(self.frame);
                    self.validate_selection();
                }
            }
            None if !unchanged => self.report(self.tr("msg-cannot-move-span")),
            None => {}
        }
    }
}
