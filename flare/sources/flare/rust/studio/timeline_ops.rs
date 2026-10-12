//! Timeline transactions and capability checks. Editor remains the document/history owner.
use super::*;
use ac_core::{frames, FrameSel, LayerKind};

#[derive(Default)]
pub(super) struct TimelineState {
    pub focused: bool,
    pub anchor: Option<(usize, u32)>,
    pub gesture: Option<FrameGesture>,
    pub duration_drag: Option<(usize, u32)>,
    pub layer_drag: Option<usize>,
    pub viewport: Option<Rect>,
    pub(super) thumbnails: thumbnails::ThumbnailCache,
    pub(super) ranges: Vec<FrameSel>,
    pub(super) pieces: Vec<super::frame_sets::FramePiece>,
    pub(super) folder_clipboard: Vec<ac_core::Layer>,
    pub(super) source: Option<super::clipboard::Source>,
    pub(super) form_identity: Option<(u64, usize, Option<u32>, usize, u32)>,
    label: String,
    duration: u32,
}

#[derive(Clone, Copy)]
pub(super) struct FrameGesture {
    pub source: FrameSel,
    pub origin: (usize, u32),
    pub target: (usize, u32),
    pub moving: bool,
    pub copy: bool,
    pub resize_left: Option<bool>,
}

impl Studio {
    pub(super) fn resized_static_span(
        &self,
        s: FrameSel,
        left: bool,
        delta: i64,
    ) -> Option<ac_core::Layer> {
        if s.layer0 != s.layer1 || !self.timeline_editable(s.layer0) || delta == 0 {
            return None;
        }
        let mut layer = self.editor.timeline().layers.get(s.layer0)?.clone();
        let key = layer.keyframes.get(&s.f0)?;
        if key.tween != Tween::None || key.motion.is_some() || layer.span_end(s.f0) != s.f1 + 1 {
            return None;
        }
        if left {
            let (prev, key) = layer.keyframes.range(..s.f0).next_back()?;
            let start = s.f0 as i64 + delta;
            if start <= *prev as i64
                || start > s.f1 as i64
                || key.tween != Tween::None
                || key.motion.is_some()
            {
                return None;
            }
            let key = layer.keyframes.remove(&s.f0)?;
            layer.keyframes.insert(start as u32, key);
        } else {
            let end = s.f1 as i64 + 1 + delta;
            let total = layer.end as i64 + delta;
            if end <= s.f0 as i64 || total > document::MAX_FRAMES as i64 {
                return None;
            }
            if delta > 0 {
                frames::insert_frames(&mut layer, s.f1, delta as u32);
            } else {
                frames::remove_frames(&mut layer, end as u32, s.f1);
            }
        }
        Some(layer)
    }
    pub(super) fn resize_static_span(&mut self, s: FrameSel, left: bool, delta: i64) -> bool {
        let Some(layer) = self.resized_static_span(s, left, delta) else {
            return false;
        };
        self.editor.begin_edit("timeline.resize.span");
        self.editor.timeline_mut().layers[s.layer0] = layer;
        self.timeline_state.ranges.clear();
        self.editor.select_span(
            s.layer0,
            if left {
                (s.f0 as i64 + delta) as u32
            } else {
                s.f0
            },
        );
        self.texture = None;
        true
    }
    pub(super) fn stage_hit_test(&self, p: ac_core::geom::Pt) -> Option<ac_core::Selection> {
        self.editor
            .evaluate_current(self.editor.frame)
            .iter()
            .rev()
            .find_map(|item| {
                let (layer, element) = item.src?;
                (self.timeline_editable(layer) && item.hit(p))
                    .then_some(ac_core::Selection { layer, element })
            })
    }
    pub(super) fn timeline_properties(&mut self, ui: &mut egui::Ui) {
        let identity = (
            self.editor.revision,
            self.editor.scene,
            self.editor.editing,
            self.editor.layer,
            self.editor.frame,
        );
        if self.timeline_state.form_identity != Some(identity) {
            self.timeline_state.form_identity = Some(identity);
            self.timeline_state.label = self
                .editor
                .current_keyframe()
                .and_then(|(_, k)| k.label.clone())
                .unwrap_or_default();
            self.timeline_state.duration = self.editor.timeline().layers[self.editor.layer].end;
        }
        self.section(ui, "section-timing");
        let enabled = self.timeline_editable(self.editor.layer);
        ui.add_enabled_ui(enabled, |ui| {
            egui::Grid::new("studio.timing.fields")
                .num_columns(2)
                .show(ui, |ui| {
                    let label = ui.label(self.i18n.text("field-duration"));
                    ui.add(
                        egui::DragValue::new(&mut self.timeline_state.duration)
                            .range(1..=document::MAX_FRAMES),
                    )
                    .labelled_by(label.id);
                    ui.end_row();
                    let label = ui.label(self.i18n.text("field-frame-label"));
                    widgets::text_edit(
                        ui,
                        egui::TextEdit::singleline(&mut self.timeline_state.label)
                            .desired_width(130.0),
                    )
                    .labelled_by(label.id);
                    ui.end_row();
                });
            if widgets::button(ui, self.i18n.text("action-apply-timing")).clicked() {
                let depth = self.editor.undo_depth();
                let label = self.timeline_state.label.clone();
                if self
                    .editor
                    .current_keyframe()
                    .is_some_and(|(_, k)| k.label.as_deref().unwrap_or("") != label)
                {
                    self.editor.set_frame_label(&label);
                }
                self.set_timeline_duration(self.editor.layer, self.timeline_state.duration);
                self.editor.squash_undo(depth, "timeline.properties");
                self.playing = false;
            }
        });
        ui.add_space(10.0);
    }
    pub(super) fn reset_timeline_gesture(&mut self) {
        self.timeline_state.ranges.clear();
        self.clear_layer_selection();
        self.timeline_state.anchor = None;
        self.timeline_state.gesture = None;
        self.timeline_state.duration_drag = None;
        self.timeline_state.layer_drag = None;
    }

    pub(super) fn timeline_editable(&self, layer: usize) -> bool {
        let layers = &self.editor.timeline().layers;
        let Some(l) = layers.get(layer) else {
            return false;
        };
        if !self.editor.layer_editable(layer) || l.kind == LayerKind::Folder {
            return false;
        }
        let mut parent = l.parent;
        while let Some(p) = parent {
            let Some(l) = layers.get(p) else { return false };
            if l.locked || !l.visible {
                return false;
            }
            parent = l.parent;
        }
        true
    }

    fn valid_frame_range(&self, s: FrameSel) -> bool {
        s.layer0 <= s.layer1
            && s.layer1 < self.editor.timeline().layers.len()
            && s.f0 <= s.f1
            && s.f1 < document::MAX_FRAMES
    }

    fn editable_range(&self, s: FrameSel) -> bool {
        self.valid_frame_range(s) && (s.layer0..=s.layer1).all(|i| self.timeline_editable(i))
    }

    fn occupied_range(&self, s: FrameSel) -> bool {
        self.valid_frame_range(s)
            && (s.layer0..=s.layer1).all(|i| s.f1 < self.editor.timeline().layers[i].end)
    }

    pub(super) fn timeline_enabled(&self, command: Command) -> Option<bool> {
        self.timeline_enabled_for(command, self.editor.frame_target())
    }
    pub(super) fn timeline_enabled_for(&self, command: Command, s: FrameSel) -> Option<bool> {
        use Command::*;
        let editable = self.editable_range(s);
        let occupied = self.occupied_range(s);
        let layers = &self.editor.timeline().layers;
        if self.layer_selection_active()
            && self.selected_layer_roots().len() > 1
            && matches!(
                command,
                CopyFrames
                    | CutFrames
                    | PasteFrames
                    | ClearFrames
                    | FrameByFrame
                    | Delete
                    | ReverseFrames
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
                    | IndentLayer
                    | OutdentLayer
            )
        {
            return Some(false);
        }
        Some(match command {
            CopyFrames => {
                occupied && (s.layer0..=s.layer1).all(|i| layers[i].kind != LayerKind::Folder)
            }
            CutFrames | ClearFrames | FrameByFrame => editable && occupied,
            Delete if self.timeline_state.focused => editable && occupied,

            ReverseFrames => editable && occupied && s.f1 > s.f0,
            SelectAllFrames => !layers.is_empty(),
            InsertFrame => {
                editable
                    && (s.layer0..=s.layer1).all(|i| {
                        let end = layers[i].end;
                        s.f1 >= end
                            || end
                                .checked_add(s.frames())
                                .is_some_and(|end| end <= document::MAX_FRAMES)
                    })
            }
            RemoveFrame => editable && (s.layer0..=s.layer1).any(|i| s.f0 < layers[i].end),
            ClearKeyframe => {
                editable
                    && (s.layer0..=s.layer1).any(|i| {
                        layers[i]
                            .keyframes
                            .keys()
                            .any(|f| *f > 0 && (s.f0..=s.f1).contains(f))
                            || layers[i].keyframes.iter().any(|(&start, key)| {
                                key.motion.as_ref().is_some_and(|m| {
                                    m.key_frames()
                                        .iter()
                                        .any(|&f| f > 0 && (s.f0..=s.f1).contains(&(start + f)))
                                })
                            })
                    })
            }
            Keyframe | BlankKeyframe | Tween => editable,
            MoveFramesEarlier | CopyFramesEarlier => self.can_retime(s, -1, 0),
            MoveFramesLater | CopyFramesLater => self.can_retime(s, 1, 0),
            AddFolder => layers.len() < 256 && self.layer_block_unlocked(self.editor.layer),
            LayerUp => {
                self.layer_neighbor(false).is_some() && self.layer_block_unlocked(self.editor.layer)
            }
            LayerDown => {
                self.layer_neighbor(true).is_some() && self.layer_block_unlocked(self.editor.layer)
            }
            IndentLayer => {
                self.indent_parent().is_some() && self.layer_block_unlocked(self.editor.layer)
            }
            OutdentLayer => {
                layers[self.editor.layer].parent.is_some()
                    && self.layer_block_unlocked(self.editor.layer)
            }
            _ => return None,
        })
    }

    pub(super) fn select_frame_range(&mut self, anchor: (usize, u32), target: (usize, u32)) {
        self.timeline_state.ranges.clear();
        self.clear_layer_selection();
        let last = self.editor.timeline().layers.len() - 1;
        let target = (target.0.min(last), target.1.min(document::MAX_FRAMES - 1));
        let anchor = (anchor.0.min(last), anchor.1.min(document::MAX_FRAMES - 1));
        self.editor.select_frames(FrameSel::between(anchor, target));
        self.timeline_state.anchor = Some(anchor);
        self.timeline_state.focused = true;
        self.playing = false;
        self.sync_forms();
    }

    fn clips_for(&self, s: FrameSel) -> Vec<frames::FrameClip> {
        (s.layer0..=s.layer1)
            .map(|i| frames::copy_frames(&self.editor.timeline().layers[i], s.f0, s.f1))
            .collect()
    }

    fn paste_clips(&mut self, at: FrameSel, clips: &[frames::FrameClip]) {
        for (i, clip) in clips.iter().enumerate() {
            frames::paste_frames(
                &mut self.editor.timeline_mut().layers[at.layer0 + i],
                at.f0,
                clip,
            );
        }
        self.editor.select_frames(FrameSel {
            layer0: at.layer0,
            layer1: at.layer0 + clips.len() - 1,
            f0: at.f0,
            f1: at.f0 + clips[0].len - 1,
        });
    }

    pub(super) fn can_retime(&self, s: FrameSel, delta: i64, layer_delta: i64) -> bool {
        if !self.editable_range(s) || !self.occupied_range(s) {
            return false;
        }
        let f0 = s.f0 as i64 + delta;
        let f1 = s.f1 as i64 + delta;
        let l0 = s.layer0 as i64 + layer_delta;
        let l1 = s.layer1 as i64 + layer_delta;
        if f0 < 0
            || f1 >= document::MAX_FRAMES as i64
            || l0 < 0
            || l1 >= self.editor.timeline().layers.len() as i64
        {
            return false;
        }
        (l0 as usize..=l1 as usize).all(|i| self.timeline_editable(i))
    }

    pub(super) fn retime_frames(
        &mut self,
        s: FrameSel,
        delta: i64,
        layer_delta: i64,
        copy: bool,
    ) -> bool {
        if !self.timeline_state.ranges.is_empty() {
            return self.retime_frame_set(delta, layer_delta, copy);
        }
        if (delta == 0 && layer_delta == 0) || !self.can_retime(s, delta, layer_delta) {
            return false;
        }
        self.editor.select_frames(s);
        if !copy && layer_delta == 0 {
            self.editor.move_frames(delta);
        } else {
            let clips = self.clips_for(s);
            self.editor.begin_edit(if copy {
                "timeline.copy"
            } else {
                "timeline.move"
            });
            if !copy {
                for i in s.layer0..=s.layer1 {
                    frames::clear_frames(&mut self.editor.timeline_mut().layers[i], s.f0, s.f1);
                }
            }
            self.paste_clips(
                FrameSel::cell(
                    (s.layer0 as i64 + layer_delta) as usize,
                    (s.f0 as i64 + delta) as u32,
                ),
                &clips,
            );
        }
        self.texture = None;
        self.playing = false;
        self.timeline_state.anchor = Some((self.editor.layer, self.editor.frame));
        self.sync_forms();
        true
    }

    pub(super) fn set_timeline_duration(&mut self, layer: usize, end: u32) {
        if !self.timeline_editable(layer) || end == 0 || end > document::MAX_FRAMES {
            return;
        }
        let l = &self.editor.timeline().layers[layer];
        let min = l.keyframes.keys().next_back().copied().unwrap_or(0) + 1;
        let end = end.max(min);
        if end != l.end {
            self.editor
                .set_layer_end(layer, end, Some("timeline.duration"));
            self.texture = None;
        }
    }

    pub(super) fn layer_block(&self, at: usize) -> std::ops::Range<usize> {
        let layers = &self.editor.timeline().layers;
        let mut end = at + 1;
        while end < layers.len() {
            let mut parent = layers[end].parent;
            let mut under = false;
            while let Some(p) = parent {
                if p == at {
                    under = true;
                    break;
                }
                parent = layers[p].parent;
            }
            if !under {
                break;
            }
            end += 1;
        }
        at..end
    }

    pub(super) fn layer_block_unlocked(&self, at: usize) -> bool {
        let layers = &self.editor.timeline().layers;
        at < layers.len()
            && self
                .layer_block(at)
                .all(|i| !layers[i].locked && layers[i].kind != LayerKind::Camera)
            && {
                let mut p = layers[at].parent;
                let mut unlocked = true;
                while let Some(i) = p {
                    unlocked &= !layers[i].locked;
                    p = layers[i].parent;
                }
                unlocked
            }
    }

    fn layer_neighbor(&self, down: bool) -> Option<usize> {
        let at = self.editor.layer;
        let layers = &self.editor.timeline().layers;
        if down {
            let next = self.layer_block(at).end;
            (next < layers.len() && layers[next].parent == layers[at].parent).then_some(next)
        } else {
            (0..at)
                .rev()
                .find(|&i| layers[i].parent == layers[at].parent)
        }
    }

    fn indent_parent(&self) -> Option<usize> {
        self.layer_neighbor(false).filter(|&i| {
            self.editor.timeline().layers[i].kind == LayerKind::Folder
                && !self.editor.timeline().layers[i].locked
        })
    }

    // Reorder whole subtrees, remapping BOTH hierarchy and animation-parent references.
    pub(super) fn reorder_layers(&mut self, order: Vec<usize>, selected: usize) {
        let old = self.editor.timeline().layers.clone();
        let mut map = vec![0; old.len()];
        for (new, &old) in order.iter().enumerate() {
            map[old] = new;
        }
        let layers = order
            .iter()
            .map(|&i| {
                let mut l = old[i].clone();
                l.parent = l.parent.map(|p| map[p]);
                l.anim_parent = l.anim_parent.map(|p| map[p]);
                l
            })
            .collect();
        self.editor.timeline_mut().layers = layers;
        self.editor.layer = map[selected];
        self.editor.selection.clear();
        self.editor.frame_sel = None;
        self.reset_timeline_gesture();
    }

    pub(super) fn move_layer_block(&mut self, from: usize, to: usize) -> bool {
        let layers = &self.editor.timeline().layers;
        if from >= layers.len()
            || to >= layers.len()
            || from == to
            || layers[from].parent != layers[to].parent
            || !self.layer_block_unlocked(from)
        {
            return false;
        }
        let block = self.layer_block(from);
        let target = if to > from {
            self.layer_block(to).end
        } else {
            to
        };
        let mut order: Vec<_> = (0..layers.len()).filter(|i| !block.contains(i)).collect();
        let pos = order
            .iter()
            .position(|&i| i >= target)
            .unwrap_or(order.len());
        order.splice(pos..pos, block);
        self.editor.begin_edit("layer.move");
        self.reorder_layers(order, from);
        true
    }

    pub(super) fn dispatch_timeline(&mut self, command: Command) -> bool {
        use Command::*;
        let s = self.editor.frame_target();
        match command {
            ClearFrames => self.editor.clear_frames(),
            Delete if self.timeline_state.focused => self.editor.clear_frames(),
            ReverseFrames => self.editor.reverse_frames(),
            FrameByFrame => self.editor.convert_to_frame_by_frame(),
            SelectAllFrames => self.select_frame_range(
                (0, 0),
                (
                    self.editor.timeline().layers.len() - 1,
                    self.editor.timeline().frame_count() - 1,
                ),
            ),
            InsertFrame => self.editor.insert_frame(),
            RemoveFrame => {
                self.editor.remove_frame();
                self.editor.frame = self
                    .editor
                    .frame
                    .min(self.editor.timeline().frame_count() - 1);
            }
            ClearKeyframe => {
                self.editor.begin_edit("timeline.clear.keys");
                for i in s.layer0..=s.layer1 {
                    let layer = &mut self.editor.timeline_mut().layers[i];
                    for (&start, key) in &mut layer.keyframes {
                        if let Some(m) = &mut key.motion {
                            for f in m.key_frames() {
                                if f > 0 && (s.f0..=s.f1).contains(&(start + f)) {
                                    m.clear_keys_at(f);
                                }
                            }
                        }
                    }
                    frames::clear_keyframes(layer, s.f0, s.f1);
                }
                self.editor.selection.clear();
            }
            Keyframe => {
                let depth = self.editor.undo_depth();
                for i in s.layer0..=s.layer1 {
                    self.editor.select_frames(FrameSel {
                        layer0: i,
                        layer1: i,
                        ..s
                    });
                    self.editor.insert_keyframe();
                }
                self.editor.squash_undo(depth, "timeline.keyframe");
                self.editor.select_frames(s);
            }
            BlankKeyframe => {
                self.editor.begin_edit("timeline.blank");
                for i in s.layer0..=s.layer1 {
                    let l = &mut self.editor.timeline_mut().layers[i];
                    ac_core::frames::convert_to_keyframes(l, s.f0, s.f1, false);
                    if s.f1 + 1 < l.end {
                        ac_core::frames::convert_to_keyframes(l, s.f1 + 1, s.f1 + 1, false);
                    }
                    for f in s.f0..=s.f1 {
                        let k = l
                            .keyframes
                            .entry(f)
                            .or_insert_with(ac_core::Keyframe::blank);
                        // Blank the artwork without silently removing authored frame metadata.
                        k.elements.clear();
                        k.tween = ac_core::Tween::None;
                        k.motion = None;
                        k.tween_boundary = None;
                        k.ease_curve = None;
                        k.shape_hints.clear();
                        k.orient_to_path = false;
                    }
                    l.end = l.end.max(s.f1 + 1);
                }
                self.editor.selection.clear();
            }
            MoveFramesEarlier | MoveFramesLater | CopyFramesEarlier | CopyFramesLater => {
                let delta = if matches!(command, MoveFramesEarlier | CopyFramesEarlier) {
                    -1
                } else {
                    1
                };
                self.retime_frames(
                    s,
                    delta,
                    0,
                    matches!(command, CopyFramesEarlier | CopyFramesLater),
                );
            }
            AddFolder => {
                self.editor
                    .wrap_layer(LayerKind::Folder, &self.i18n.text("default-folder"));
                self.reset_timeline_gesture();
            }
            LayerUp | LayerDown => {
                if let Some(to) = self.layer_neighbor(command == LayerDown) {
                    self.move_layer_block(self.editor.layer, to);
                }
            }
            IndentLayer => {
                if let Some(parent) = self.indent_parent() {
                    self.editor
                        .set_layer_parent(self.editor.layer, Some(parent));
                }
            }
            OutdentLayer => {
                let at = self.editor.layer;
                let parent = self.editor.timeline().layers[at].parent.unwrap();
                let outer = self.editor.timeline().layers[parent].parent;
                let end = self.layer_block(parent).end;
                let block = self.layer_block(at);
                let mut order: Vec<_> = (0..self.editor.timeline().layers.len())
                    .filter(|i| !block.contains(i))
                    .collect();
                let pos = order.iter().position(|&i| i >= end).unwrap_or(order.len());
                order.splice(pos..pos, block);
                self.editor.begin_edit("layer.outdent");
                self.editor.timeline_mut().layers[at].parent = outer;
                self.reorder_layers(order, at);
            }
            ExpandFolders | CollapseFolders => {
                let collapsed = command == CollapseFolders;
                if self
                    .editor
                    .timeline()
                    .layers
                    .iter()
                    .any(|l| l.kind == LayerKind::Folder && l.collapsed != collapsed)
                {
                    self.editor.begin_edit("layer.collapse");
                    for l in &mut self.editor.timeline_mut().layers {
                        if l.kind == LayerKind::Folder {
                            l.collapsed = collapsed;
                        }
                    }
                }
            }
            ShowAllLayers | HideAllLayers | LockAllLayers | UnlockAllLayers => {
                let lock = matches!(command, LockAllLayers | UnlockAllLayers);
                let value = matches!(command, ShowAllLayers | LockAllLayers);
                let targets: Vec<_> = (0..self.editor.timeline().layers.len()).collect();
                self.set_layer_flags(&targets, lock, value);
            }
            _ => return false,
        }
        true
    }

    pub(super) fn set_layer_flags(&mut self, targets: &[usize], lock: bool, value: bool) {
        let layers = &self.editor.timeline().layers;
        if !targets.iter().any(|&i| {
            layers.get(i).is_some_and(|l| {
                if lock {
                    l.locked != value
                } else {
                    l.visible != value
                }
            })
        }) {
            return;
        }
        self.finish_stage_gesture();
        self.editor.begin_edit("layer.flags");
        for &i in targets {
            if let Some(l) = self.editor.timeline_mut().layers.get_mut(i) {
                if lock {
                    l.locked = value;
                } else {
                    l.visible = value;
                }
            }
        }
        self.editor.selection = self
            .editor
            .selection
            .iter()
            .copied()
            .filter(|s| self.timeline_editable(s.layer))
            .collect();
        self.texture = None;
    }

    pub(super) fn solo_layer_flag(&mut self, at: usize, lock: bool) {
        let layers = &self.editor.timeline().layers;
        let mut related: Vec<_> = self.layer_block(at).collect();
        let mut parent = layers[at].parent;
        while let Some(i) = parent {
            related.push(i);
            parent = layers[i].parent;
        }
        let others: Vec<_> = (0..layers.len()).filter(|i| !related.contains(i)).collect();
        let isolated = others.iter().all(|&i| {
            if lock {
                layers[i].locked
            } else {
                !layers[i].visible
            }
        });
        let depth = self.editor.undo_depth();
        self.set_layer_flags(&others, lock, if lock { !isolated } else { isolated });
        self.set_layer_flags(&related, lock, !lock);
        self.editor.squash_undo(depth, "layer.isolate");
    }
}
