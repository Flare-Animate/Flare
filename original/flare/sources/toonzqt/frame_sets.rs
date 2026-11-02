//! Exact frame sets: holes remain holes in editing, clipboard and drag operations.
use super::*;
use ac_core::{frames, FrameSel, Layer, Timeline};
use std::collections::BTreeMap;

#[derive(Clone)]
pub(super) struct FramePiece {
    row: usize,
    start: u32,
    clip: frames::FrameClip,
}

fn normalized(ranges: Vec<FrameSel>) -> Vec<FrameSel> {
    let mut rows: BTreeMap<usize, Vec<(u32, u32)>> = BTreeMap::new();
    for s in ranges {
        for row in s.layer0..=s.layer1 {
            rows.entry(row).or_default().push((s.f0, s.f1));
        }
    }
    let mut out: Vec<FrameSel> = vec![];
    for (row, mut spans) in rows {
        spans.sort_unstable();
        for (f0, f1) in spans {
            if let Some(last) = out.last_mut().filter(|s| s.layer0 == row && f0 <= s.f1 + 1) {
                last.f1 = last.f1.max(f1);
            } else {
                out.push(FrameSel {
                    layer0: row,
                    layer1: row,
                    f0,
                    f1,
                });
            }
        }
    }
    out
}

impl Studio {
    pub(super) fn add_frame_span(&mut self, layer: usize, frame: u32) {
        let mut ranges = self.frame_ranges();
        self.editor.select_span(layer, frame);
        ranges.push(self.editor.frame_target());
        self.clear_layer_selection();
        self.install_frame_ranges(ranges);
        self.timeline_state.focused = true;
        self.playing = false;
    }
    pub(super) fn install_frame_ranges(&mut self, ranges: Vec<FrameSel>) {
        let ranges = normalized(ranges);
        let first = ranges[0];
        let rectangular = ranges
            .iter()
            .enumerate()
            .all(|(i, s)| s.layer0 == first.layer0 + i && s.f0 == first.f0 && s.f1 == first.f1);
        if rectangular {
            self.editor.select_frames(FrameSel {
                layer1: ranges.last().unwrap().layer1,
                ..first
            });
            self.timeline_state.ranges.clear();
        } else {
            self.editor.select_frames(first);
            self.timeline_state.ranges = ranges;
        }
    }
    pub(super) fn frame_ranges(&self) -> Vec<FrameSel> {
        if self.timeline_state.ranges.is_empty() {
            normalized(vec![self.editor.frame_target()])
        } else {
            self.timeline_state.ranges.clone()
        }
    }
    pub(super) fn frame_selected(&self, layer: usize, frame: u32) -> bool {
        if self.timeline_state.ranges.is_empty() {
            self.editor.frame_target().contains(layer, frame)
        } else {
            self.timeline_state
                .ranges
                .iter()
                .any(|s| s.contains(layer, frame))
        }
    }
    pub(super) fn toggle_frame_cell(&mut self, layer: usize, frame: u32) {
        let mut ranges = self.frame_ranges();
        if self.prefs.span_selection && self.timeline_state.ranges.is_empty() {
            // Ctrl in span mode starts individual-frame selection.
            ranges.clear();
        }
        if ranges.iter().any(|s| s.contains(layer, frame)) {
            ranges = ranges
                .into_iter()
                .flat_map(|s| {
                    if !s.contains(layer, frame) {
                        return vec![s];
                    }
                    let mut split = vec![];
                    if s.f0 < frame {
                        split.push(FrameSel { f1: frame - 1, ..s });
                    }
                    if frame < s.f1 {
                        split.push(FrameSel { f0: frame + 1, ..s });
                    }
                    split
                })
                .collect();
        } else {
            ranges.push(FrameSel::cell(layer, frame));
        }
        if ranges.is_empty() {
            ranges.push(FrameSel::cell(layer, frame));
        }
        self.clear_layer_selection();
        self.timeline_state.ranges = normalized(ranges);
        self.editor.select_frames(FrameSel::cell(layer, frame));
        self.timeline_state.focused = true;
        self.timeline_state.anchor = Some((layer, frame));
        self.playing = false;
        self.sync_forms();
    }
    fn copy_frame_pieces(&self) -> Vec<FramePiece> {
        let ranges = self.frame_ranges();
        let row = ranges.iter().map(|s| s.layer0).min().unwrap();
        let start = ranges.iter().map(|s| s.f0).min().unwrap();
        ranges
            .iter()
            .map(|s| FramePiece {
                row: s.layer0 - row,
                start: s.f0 - start,
                clip: frames::copy_frames(&self.editor.timeline().layers[s.layer0], s.f0, s.f1),
            })
            .collect()
    }
    fn prepared_frame_paste(
        &self,
        overwrite: bool,
    ) -> Option<(ac_core::Document, Timeline, Vec<FrameSel>)> {
        let pieces = &self.timeline_state.pieces;
        if pieces.is_empty() {
            return None;
        }
        let payload: Vec<Layer> = pieces
            .iter()
            .map(|p| {
                let mut l = Layer::new("", 0);
                l.end = p.clip.len;
                l.keyframes = p.clip.keyframes.iter().cloned().collect();
                l
            })
            .collect();
        let (doc, payload) =
            self.prepare_clipboard(self.timeline_state.source.as_ref()?, &payload)?;
        let mut tl = self.editor.timeline().clone();
        let at = self.editor.frame_target();
        let mut selected: Vec<FrameSel> = vec![];
        // Descending offsets allow replacement to shift the tail without redirecting
        // subsequent pieces. Gaps in a sparse clipboard are never cleared.
        for (p, l) in pieces.iter().zip(payload).rev() {
            let row = at.layer0.checked_add(p.row)?;
            let start = at.f0.checked_add(p.start)?;
            if !self.timeline_editable(row) || start.checked_add(p.clip.len)? > document::MAX_FRAMES
            {
                return None;
            }
            let clip = frames::FrameClip {
                len: p.clip.len,
                keyframes: l.keyframes.into_iter().collect(),
            };
            if overwrite {
                frames::paste_frames(&mut tl.layers[row], start, &clip);
            } else {
                // Multiple disjoint pieces on one row preserve their holes and
                // offsets. Only a single contiguous piece replaces a selection.
                let count = if pieces.iter().filter(|other| other.row == p.row).count() > 1 {
                    clip.len
                } else {
                    at.frames()
                };
                let removed = count.min(tl.layers[row].end.saturating_sub(start));
                let delta = clip.len as i64 - removed as i64;
                for s in selected.iter_mut().filter(|s| s.layer0 == row) {
                    s.f0 = (s.f0 as i64 + delta) as u32;
                    s.f1 = (s.f1 as i64 + delta) as u32;
                }
                replace_frames(&mut tl.layers[row], start, count, &clip);
            }
            if tl.layers[row].end > document::MAX_FRAMES {
                return None;
            }
            selected.push(FrameSel {
                layer0: row,
                layer1: row,
                f0: start,
                f1: start + clip.len - 1,
            });
        }
        Some((doc, tl, normalized(selected)))
    }
    pub(super) fn frame_set_enabled(&self, command: Command) -> Option<bool> {
        use Command::*;
        if let Some(enabled) = self.folder_frames_enabled(command) {
            return Some(enabled);
        }
        if !matches!(
            command,
            CopyFrames
                | CutFrames
                | PasteFrames
                | PasteOverwriteFrames
                | ClearFrames
                | ReverseFrames
                | FrameByFrame
                | InsertFrame
                | RemoveFrame
                | ClearKeyframe
                | Keyframe
                | BlankKeyframe
                | Tween
                | MoveFramesEarlier
                | MoveFramesLater
                | CopyFramesEarlier
                | CopyFramesLater
                | Delete
        ) {
            return None;
        }
        if command == Delete && !self.timeline_state.focused {
            return None;
        }
        if self.layer_selection_active() && self.selected_layer_roots().len() > 1 {
            return Some(false);
        }
        if matches!(command, PasteFrames | PasteOverwriteFrames) {
            return Some(
                self.prepared_frame_paste(command == PasteOverwriteFrames)
                    .is_some(),
            );
        }
        let ranges = self.frame_ranges();
        if command == CopyFrames {
            return Some(
                ranges
                    .iter()
                    .all(|s| self.timeline_enabled_for(command, *s) == Some(true)),
            );
        }
        if self.timeline_state.ranges.is_empty() {
            return None;
        }
        if matches!(
            command,
            MoveFramesEarlier | MoveFramesLater | CopyFramesEarlier | CopyFramesLater
        ) {
            return Some(ranges.iter().all(|s| {
                self.can_retime(
                    *s,
                    if matches!(command, MoveFramesEarlier | CopyFramesEarlier) {
                        -1
                    } else {
                        1
                    },
                    0,
                )
            }));
        }
        if command == ClearKeyframe {
            return Some(
                ranges.iter().all(|s| self.timeline_editable(s.layer0))
                    && ranges
                        .iter()
                        .any(|s| self.timeline_enabled_for(command, *s) == Some(true)),
            );
        }
        let valid = ranges.iter().all(|s| {
            self.timeline_enabled_for(
                if command == CutFrames {
                    ClearFrames
                } else {
                    command
                },
                *s,
            ) == Some(true)
        });
        if command == InsertFrame && valid {
            let mut tl = self.editor.timeline().clone();
            for s in ranges.iter().rev() {
                frames::insert_frames(&mut tl.layers[s.layer0], s.f1, s.frames());
            }
            return Some(tl.layers.iter().all(|l| l.end <= document::MAX_FRAMES));
        }
        Some(valid)
    }
    pub(super) fn dispatch_frame_set(&mut self, command: Command) -> bool {
        use Command::*;
        if self.dispatch_folder_frames(command) {
            return true;
        }
        if matches!(command, CopyFrames | CutFrames) {
            self.timeline_state.folder_clipboard.clear();
            self.timeline_state.pieces = self.copy_frame_pieces();
            self.timeline_state.source = Some(self.clipboard_source());
            let sampled = self.frame_ranges().iter().any(|s| {
                let l = &self.editor.timeline().layers[s.layer0];
                l.keyframes.range(..=s.f1).any(|(&start, k)| {
                    k.tween != ac_core::Tween::None
                        && l.span_end(start) > s.f0
                        && (!k.tween.is_motion()
                            && k.tween_boundary.is_none()
                            && l.span_end(start) > s.f1
                            && !l.keyframes.contains_key(&l.span_end(start)))
                })
            });
            if sampled {
                self.status = self.i18n.text("status-partial-tween-copy");
                self.error = false;
            } else {
                self.status = self.i18n.text("status-frames-copied");
                self.error = false;
            }

            if command == CopyFrames {
                return true;
            }
        }
        if matches!(command, PasteFrames | PasteOverwriteFrames) {
            let Some((doc, tl, ranges)) =
                self.prepared_frame_paste(command == PasteOverwriteFrames)
            else {
                return true;
            };
            self.editor.begin_edit("timeline.paste");
            self.editor.doc = doc;
            *self.editor.timeline_mut() = tl;
            self.editor.selection.clear();
            self.install_frame_ranges(ranges);
            return true;
        }
        if command == CutFrames && self.timeline_state.ranges.is_empty() {
            self.editor.clear_frames();
            return true;
        }
        if self.timeline_state.ranges.is_empty() || self.frame_set_enabled(command).is_none() {
            return false;
        }
        if matches!(
            command,
            MoveFramesEarlier | MoveFramesLater | CopyFramesEarlier | CopyFramesLater
        ) {
            self.retime_frame_set(
                if matches!(command, MoveFramesEarlier | CopyFramesEarlier) {
                    -1
                } else {
                    1
                },
                0,
                matches!(command, CopyFramesEarlier | CopyFramesLater),
            );
            return true;
        }
        let ranges = self.frame_ranges();
        let depth = self.editor.undo_depth();
        // The legacy single-range transactions are reused in descending order;
        // one preflight and one squashed history entry cover the entire exact set.
        self.timeline_state.ranges.clear();
        for s in ranges.iter().rev() {
            self.editor.select_frames(*s);
            self.dispatch_timeline(if command == CutFrames {
                ClearFrames
            } else {
                command
            });
        }
        self.editor.squash_undo(depth, "timeline.selection");
        if !matches!(command, InsertFrame | RemoveFrame) {
            self.timeline_state.ranges = ranges;
        }
        self.editor.selection.clear();
        true
    }
    pub(super) fn retime_frame_set(&mut self, delta: i64, dl: i64, copy: bool) -> bool {
        let ranges = self.frame_ranges();
        if (delta == 0 && dl == 0) || !ranges.iter().all(|s| self.can_retime(*s, delta, dl)) {
            return false;
        }
        let clips: Vec<_> = ranges
            .iter()
            .map(|s| frames::copy_frames(&self.editor.timeline().layers[s.layer0], s.f0, s.f1))
            .collect();
        self.editor.begin_edit("timeline.move.selection");
        if !copy {
            for s in &ranges {
                frames::clear_frames(&mut self.editor.timeline_mut().layers[s.layer0], s.f0, s.f1);
            }
        }
        let mut targets = vec![];
        for (s, clip) in ranges.iter().zip(clips) {
            let row = (s.layer0 as i64 + dl) as usize;
            let f0 = (s.f0 as i64 + delta) as u32;
            frames::paste_frames(&mut self.editor.timeline_mut().layers[row], f0, &clip);
            targets.push(FrameSel {
                layer0: row,
                layer1: row,
                f0,
                f1: f0 + clip.len - 1,
            });
        }
        self.editor.select_frames(targets[0]);
        self.timeline_state.ranges = normalized(targets);
        self.texture = None;
        true
    }
}

fn replace_frames(layer: &mut Layer, at: u32, selected: u32, clip: &frames::FrameClip) {
    if at >= layer.end {
        frames::paste_frames(layer, at, clip);
        return;
    }
    let tail_start = at.saturating_add(selected).min(layer.end);
    // Pin the prefix before changing layer.end: its source-time mapping must
    // not depend on the replacement length or the shifted tail.
    frames::convert_to_keyframes(layer, at, at, false);
    let tail =
        (tail_start < layer.end).then(|| frames::copy_frames(layer, tail_start, layer.end - 1));
    layer.keyframes.retain(|&f, _| f < at);
    layer.end = at.max(1);
    frames::paste_frames(layer, at, clip);
    if let Some(tail) = tail {
        frames::paste_frames(layer, at + clip.len, &tail);
    }
}
