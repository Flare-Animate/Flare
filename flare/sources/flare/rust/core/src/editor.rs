//! Editing session: every mutation goes through here so undo/redo, the UI,
//! the CLI and (later) the MCP/scripting surface all share one command set.

use crate::frames::{self, FrameClip};
use crate::geom::{pt, Matrix, Path, Pt};
use crate::model::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Selection {
    pub layer: usize,
    pub element: usize,
}

/// A rectangular block of timeline cells (inclusive on both axes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameSel {
    pub layer0: usize,
    pub layer1: usize,
    pub f0: u32,
    pub f1: u32,
}

impl FrameSel {
    pub fn cell(layer: usize, frame: u32) -> Self {
        FrameSel {
            layer0: layer,
            layer1: layer,
            f0: frame,
            f1: frame,
        }
    }
    pub fn between(a: (usize, u32), b: (usize, u32)) -> Self {
        FrameSel {
            layer0: a.0.min(b.0),
            layer1: a.0.max(b.0),
            f0: a.1.min(b.1),
            f1: a.1.max(b.1),
        }
    }
    pub fn contains(&self, layer: usize, frame: u32) -> bool {
        (self.layer0..=self.layer1).contains(&layer) && (self.f0..=self.f1).contains(&frame)
    }
    pub fn frames(&self) -> u32 {
        self.f1 - self.f0 + 1
    }
    pub fn is_single(&self) -> bool {
        self.layer0 == self.layer1 && self.f0 == self.f1
    }
}

pub struct Editor {
    pub doc: Document,
    /// None = main scene; Some(id) = editing inside a symbol.
    pub editing: Option<SymbolId>,
    pub frame: u32,
    pub layer: usize,
    /// Selected stage elements (sorted, unique).
    pub selection: Vec<Selection>,
    /// Selected timeline cells.
    pub frame_sel: Option<FrameSel>,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    element_clip: Vec<(Element, usize)>,
    frame_clip: Vec<FrameClip>,
    /// Show the scene through the camera layer (playback, Camera tool).
    pub camera_view: bool,
    /// Bumped on every undoable change, undo and redo (dirty tracking, autosave).
    pub revision: u64,
    /// Edit Multiple Frames: stage edits also apply to the same objects on
    /// every other keyframe in this range (inclusive).
    pub multi_frame: Option<(u32, u32)>,
    /// Enable Simple Buttons: the button instance (layer, element) under the
    /// pointer and the frame it shows (1 = Over, 2 = Down).
    pub button_state: Option<((usize, usize), u32)>,
    /// Current scene (index into `doc.scenes()`).
    pub scene: usize,
    /// Scripts: draw.* commands draw into the merge shape instead of adding objects.
    pub script_merge: bool,
}

#[derive(Clone)]
struct Snapshot {
    doc: Document,
    label: String,
}

const UNDO_LIMIT: usize = 200;

impl Editor {
    pub fn new(doc: Document) -> Self {
        Editor {
            doc,
            editing: None,
            frame: 0,
            layer: 0,
            selection: vec![],
            frame_sel: None,
            undo: vec![],
            redo: vec![],
            element_clip: vec![],
            frame_clip: vec![],
            camera_view: false,
            revision: 0,
            multi_frame: None,
            button_state: None,
            scene: 0,
            script_merge: false,
        }
    }

    // ------------------------------------------------------------ plumbing
    fn checkpoint(&mut self, label: &str) {
        self.revision += 1;
        self.undo.push(Snapshot {
            doc: self.doc.clone(),
            label: label.into(),
        });
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
    }
    pub fn undo(&mut self) -> Option<String> {
        let s = self.undo.pop()?;
        self.revision += 1;
        let cur = std::mem::replace(&mut self.doc, s.doc);
        self.redo.push(Snapshot {
            doc: cur,
            label: s.label.clone(),
        });
        self.fix_indices();
        Some(s.label)
    }
    pub fn redo(&mut self) -> Option<String> {
        let s = self.redo.pop()?;
        self.revision += 1;
        let cur = std::mem::replace(&mut self.doc, s.doc);
        self.undo.push(Snapshot {
            doc: cur,
            label: s.label.clone(),
        });
        self.fix_indices();
        Some(s.label)
    }
    pub fn undo_label(&self) -> Option<&str> {
        self.undo.last().map(|s| s.label.as_str())
    }
    pub fn redo_label(&self) -> Option<&str> {
        self.redo.last().map(|s| s.label.as_str())
    }

    fn fix_indices(&mut self) {
        if self.scene >= self.doc.scene_count() {
            self.scene = 0;
        }
        if let Some(id) = self.editing {
            if self.doc.symbol(id).is_none() {
                self.editing = None;
            }
        }
        let n = self.timeline().layers.len();
        self.layer = self.layer.min(n.saturating_sub(1));
        self.selection.clear();
        if let Some(fs) = &mut self.frame_sel {
            if fs.layer1 >= n {
                self.frame_sel = None;
            }
        }
    }

    pub fn timeline(&self) -> &Timeline {
        match self.editing.and_then(|id| self.doc.symbol(id)) {
            Some(s) => &s.timeline,
            None => self.scene_tl(),
        }
    }
    pub fn timeline_mut(&mut self) -> &mut Timeline {
        match self.editing {
            Some(id) if self.doc.symbol(id).is_some() => {
                &mut self.doc.symbol_mut(id).unwrap().timeline
            }
            _ => self.scene_tl_mut(),
        }
    }
    /// The current scene's timeline (even while editing a symbol).
    pub fn scene_tl(&self) -> &Timeline {
        self.doc.scene_at(self.scene).unwrap_or(&self.doc.scene)
    }
    pub fn scene_tl_mut(&mut self) -> &mut Timeline {
        let i = if self.scene < self.doc.scene_count() {
            self.scene
        } else {
            0
        };
        self.doc.scene_at_mut(i).unwrap()
    }

    // ------------------------------------------------------------ scenes
    /// Edit ▸ Edit Scene: switch to scene `i`.
    pub fn goto_scene(&mut self, i: usize) {
        if i < self.doc.scene_count() {
            self.scene = i;
            self.editing = None;
            self.frame = 0;
            self.layer = 0;
            self.selection.clear();
            self.frame_sel = None;
        }
    }
    /// Insert ▸ Scene: a new empty scene after the current one.
    pub fn add_scene(&mut self) -> usize {
        self.checkpoint("Insert Scene");
        let mut n = self.doc.scene_count() + 1;
        while self.doc.scenes().any(|t| t.name == format!("Scene {n}")) {
            n += 1;
        }
        let at = self.scene + 1;
        self.doc
            .more_scenes
            .insert(at - 1, Timeline::new(&format!("Scene {n}")));
        self.goto_scene(at);
        at
    }
    pub fn duplicate_scene(&mut self, i: usize) {
        let Some(t) = self.doc.scene_at(i).cloned() else {
            return;
        };
        self.checkpoint("Duplicate Scene");
        let mut t = t;
        t.name = format!("{} copy", t.name);
        self.doc.more_scenes.insert(i, t);
        self.goto_scene(i + 1);
    }
    pub fn delete_scene(&mut self, i: usize) {
        if self.doc.scene_count() < 2 || i >= self.doc.scene_count() {
            return;
        }
        self.checkpoint("Delete Scene");
        if i == 0 {
            self.doc.scene = self.doc.more_scenes.remove(0);
        } else {
            self.doc.more_scenes.remove(i - 1);
        }
        let cur = if i < self.scene {
            self.scene - 1
        } else {
            self.scene.min(self.doc.scene_count() - 1)
        };
        self.goto_scene(cur);
    }
    pub fn rename_scene(&mut self, i: usize, name: &str) {
        if name.trim().is_empty() || self.doc.scene_at(i).is_none() {
            return;
        }
        self.checkpoint("Rename Scene");
        self.doc.scene_at_mut(i).unwrap().name = name.trim().into();
    }
    /// Reorder scenes (Scene panel drag): move scene `from` to position `to`.
    pub fn move_scene(&mut self, from: usize, to: usize) {
        let n = self.doc.scene_count();
        if from >= n || to >= n || from == to {
            return;
        }
        self.checkpoint("Move Scene");
        let mut all: Vec<Timeline> = std::iter::once(std::mem::take(&mut self.doc.scene))
            .chain(self.doc.more_scenes.drain(..))
            .collect();
        let t = all.remove(from);
        all.insert(to, t);
        let mut it = all.into_iter();
        self.doc.scene = it.next().unwrap();
        self.doc.more_scenes = it.collect();
        if self.scene == from {
            self.scene = to;
        } else if from < self.scene && to >= self.scene {
            self.scene -= 1;
        } else if from > self.scene && to <= self.scene {
            self.scene += 1;
        }
    }

    pub fn breadcrumb(&self) -> Vec<String> {
        let mut v = vec![self.scene_tl().name.clone()];
        if let Some(s) = self.editing.and_then(|id| self.doc.symbol(id)) {
            v.push(s.name.clone());
        }
        v
    }

    /// Draw items for the timeline currently being edited (with `src` set).
    pub fn evaluate_current(&self, frame: u32) -> Vec<DrawItem> {
        let ctx = EvalCtx {
            button: self.button_state,
            ..EvalCtx::new(&self.doc, frame)
        };
        let mut out = Vec::new();
        evaluate(
            &ctx,
            self.timeline(),
            frame,
            &Matrix::IDENTITY,
            1.0,
            0,
            &mut out,
        );
        if self.camera_view && self.editing.is_none() {
            if let Some(v) = camera_view(&self.doc, self.scene_tl(), frame) {
                apply_view(&mut out, &v);
            }
        }
        out
    }

    // ------------------------------------------------------------ navigation
    pub fn frame_count(&self) -> u32 {
        self.timeline().frame_count()
    }
    /// Move the playhead. Keeps the stage selection only if it stays valid.
    pub fn set_frame(&mut self, f: u32) {
        if f != self.frame {
            self.commit_floats();
            self.frame = f;
            self.selection.clear();
        }
    }
    pub fn enter_symbol(&mut self, id: SymbolId) {
        if self.doc.symbol(id).is_some() {
            self.editing = Some(id);
            self.frame = 0;
            self.layer = 0;
            self.selection.clear();
            self.frame_sel = None;
        }
    }
    pub fn exit_to_scene(&mut self) {
        self.editing = None;
        self.frame = 0;
        self.layer = 0;
        self.selection.clear();
        self.frame_sel = None;
    }

    // ------------------------------------------------------------ layers
    pub fn add_layer(&mut self) {
        self.checkpoint("New Layer");
        let end = self.timeline().frame_count();
        let n = self.timeline().layers.len();
        let mut l = Layer::new(&format!("Layer {}", n + 1), n);
        l.end = end;
        let at = self.layer.min(n);
        // a new layer joins the folder/mask of the layer it's inserted above
        l.parent = self.timeline().layers.get(at).and_then(|c| c.parent);
        self.remap_layer_parents(|i| Some(if i >= at { i + 1 } else { i }));
        self.timeline_mut().layers.insert(at, l);
        self.layer = at;
        self.selection.clear();
        self.frame_sel = None;
    }
    pub fn delete_layer(&mut self) {
        if self.timeline().layers.len() <= 1 {
            return;
        }
        self.checkpoint("Delete Layer");
        let i = self.layer;
        let kids: Vec<usize> = (0..self.timeline().layers.len())
            .filter(|&c| self.timeline().layers[c].anim_parent == Some(i))
            .collect();
        for c in kids {
            let grand = self.timeline().layers[i].anim_parent;
            self.reparent_raw(c, grand);
        }
        let up = self.timeline().layers[i].parent;
        // children of a deleted folder/mask move up one level
        for l in &mut self.timeline_mut().layers {
            if l.parent == Some(i) {
                l.parent = up;
            }
        }
        self.timeline_mut().layers.remove(i);
        self.remap_layer_parents(|p| Some(if p > i { p - 1 } else { p }));
        self.frame_sel = None;
        self.fix_indices();
    }
    pub fn move_layer(&mut self, from: usize, to: usize) {
        let n = self.timeline().layers.len();
        if from >= n || to >= n || from == to {
            return;
        }
        self.checkpoint("Move Layer");
        // old index → new index after moving `from` to `to`
        let map = move |i: usize| {
            Some(if i == from {
                to
            } else if from < to && i > from && i <= to {
                i - 1
            } else if to < from && i >= to && i < from {
                i + 1
            } else {
                i
            })
        };
        self.remap_layer_parents(map);
        let l = self.timeline_mut().layers.remove(from);
        self.timeline_mut().layers.insert(to, l);
        self.fix_layer_tree();
        self.layer = to;
        self.selection.clear();
        self.frame_sel = None;
    }
    pub fn rename_layer(&mut self, i: usize, name: &str) {
        self.checkpoint("Rename Layer");
        self.timeline_mut().layers[i].name = name.into();
    }
    pub fn toggle_layer_flag(&mut self, i: usize, flag: LayerFlag) {
        let l = &mut self.timeline_mut().layers[i];
        match flag {
            LayerFlag::Visible => l.visible = !l.visible,
            LayerFlag::Locked => l.locked = !l.locked,
            LayerFlag::Outline => l.outline = !l.outline,
        }
        // Hidden or locked layers can't keep selected objects.
        let tl = self.timeline();
        let keep: Vec<Selection> = self
            .selection
            .iter()
            .copied()
            .filter(|s| {
                tl.layers
                    .get(s.layer)
                    .map(|l| l.visible && !l.locked)
                    .unwrap_or(false)
            })
            .collect();
        self.selection = keep;
    }

    // ------------------------------------------------------------ frame selection
    /// The selected block, or the cell under the playhead on the current layer.
    pub fn frame_target(&self) -> FrameSel {
        self.frame_sel
            .unwrap_or(FrameSel::cell(self.layer, self.frame))
    }
    pub fn select_frames(&mut self, sel: FrameSel) {
        let n = self.timeline().layers.len();
        if n == 0 {
            return;
        }
        let sel = FrameSel {
            layer0: sel.layer0.min(n - 1),
            layer1: sel.layer1.min(n - 1),
            ..sel
        };
        self.frame_sel = Some(sel);
        self.layer = sel.layer0;
        if self.frame != sel.f0 {
            self.frame = sel.f0;
        }
        self.selection.clear();
    }
    /// Double-click a frame: select the whole span it belongs to.
    pub fn select_span(&mut self, layer: usize, frame: u32) {
        let Some(l) = self.timeline().layers.get(layer) else {
            return;
        };
        let sel = match l.span_at(frame) {
            Some((s, _, _)) => FrameSel {
                layer0: layer,
                layer1: layer,
                f0: s,
                f1: l.span_end(s) - 1,
            },
            None => FrameSel::cell(layer, frame),
        };
        self.select_frames(sel);
    }
    /// Click a layer name: select all of that layer's frames.
    pub fn select_layer_frames(&mut self, layer: usize) {
        let Some(l) = self.timeline().layers.get(layer) else {
            return;
        };
        let end = l.end.max(1);
        self.frame_sel = Some(FrameSel {
            layer0: layer,
            layer1: layer,
            f0: 0,
            f1: end - 1,
        });
        self.layer = layer;
        self.selection.clear();
    }

    fn for_each_layer(&mut self, label: &str, mut f: impl FnMut(&mut Layer, &FrameSel)) {
        let sel = self.frame_target();
        self.checkpoint(label);
        self.commit_floats();
        let tl = self.timeline_mut();
        for li in sel.layer0..=sel.layer1.min(tl.layers.len().saturating_sub(1)) {
            f(&mut tl.layers[li], &sel);
        }
        self.selection.clear();
    }

    /// F5 – Insert Frame(s): as many frames as are selected.
    pub fn insert_frame(&mut self) {
        self.for_each_layer("Insert Frames", |l, s| {
            frames::insert_frames(l, s.f1, s.frames())
        });
    }
    /// Shift+F5 – Remove Frames.
    pub fn remove_frame(&mut self) {
        self.for_each_layer("Remove Frames", |l, s| frames::remove_frames(l, s.f0, s.f1));
        if let Some(s) = &mut self.frame_sel {
            s.f1 = s.f0;
        }
    }
    /// F6 – Insert Keyframe (copies what is visible, including mid-tween state).
    /// Inside a motion tween it adds property keyframes instead (all properties).
    pub fn insert_keyframe(&mut self) {
        let s = self.frame_target();
        if s.f0 == s.f1
            && self
                .motion_span(s.layer0, s.f0)
                .is_some_and(|(st, _)| st != s.f0)
            && self.timeline().layers[s.layer0]
                .span_at(s.f0)
                .is_some_and(|(_, k, _)| k.tween_boundary.is_none())
        {
            return self.key_all_properties(s.layer0, s.f0);
        }
        self.for_each_layer("Insert Keyframe", |l, s| {
            frames::convert_to_keyframes(l, s.f0, s.f1, false)
        });
    }
    /// Modify ▸ Timeline ▸ Reverse Frames.
    pub fn reverse_frames(&mut self) {
        self.for_each_layer("Reverse Frames", |l, s| {
            frames::reverse_frames(l, s.f0, s.f1)
        });
    }
    /// Convert to Frame-by-Frame Animation (tweens become keyframes).
    pub fn convert_to_frame_by_frame(&mut self) {
        self.for_each_layer("Convert to Frame-by-Frame", |l, s| {
            frames::convert_to_frame_by_frame(l, s.f0, s.f1)
        });
    }
    /// Drag a layer's end: the last span gets longer/shorter (motion keys stretch).
    pub fn set_layer_end(&mut self, layer: usize, end: u32, label: Option<&str>) {
        if self.timeline().layers.get(layer).is_none() {
            return;
        }
        match label {
            Some(l) => self.checkpoint(l),
            None => self.revision += 1,
        }
        frames::set_last_span_end(&mut self.timeline_mut().layers[layer], end);
    }

    // ------------------------------------------------------------ shape hints
    /// The shape tween around the playhead on the current layer: (start, end) keyframes.
    pub fn shape_tween_keys(&self) -> Option<(u32, u32)> {
        let l = self.timeline().layers.get(self.layer)?;
        let (st, kf, next) = l.span_at(self.frame)?;
        if matches!(kf.tween, Tween::Shape { .. }) {
            return Some((st, next?));
        }
        // the closing keyframe of a shape tween
        if st == self.frame {
            let (&ps, pk) = l.keyframes.range(..st).next_back()?;
            if matches!(pk.tween, Tween::Shape { .. }) {
                return Some((ps, st));
            }
        }
        None
    }
    /// Modify ▸ Shape ▸ Add Shape Hint (Ctrl+Shift+H): a new hint in the
    /// middle of the shape on both keyframes of the shape tween.
    pub fn add_shape_hint(&mut self) -> Option<usize> {
        let (a, b) = self.shape_tween_keys()?;
        let li = self.layer;
        let center = |kf: &Keyframe| -> Pt {
            match kf.elements.first().map(|e| &e.kind) {
                Some(ElementKind::Shape(s)) => s
                    .path
                    .bounds()
                    .map(|(lo, hi)| pt((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0))
                    .unwrap_or_default(),
                _ => Pt::default(),
            }
        };
        let l = &self.timeline().layers[li];
        if l.keyframes[&a].shape_hints.len() >= 26 {
            return None;
        }
        let (ca, cb) = (center(&l.keyframes[&a]), center(&l.keyframes[&b]));
        self.checkpoint("Add Shape Hint");
        let l = &mut self.timeline_mut().layers[li];
        let n = l.keyframes[&a]
            .shape_hints
            .len()
            .max(l.keyframes[&b].shape_hints.len());
        for (k, c) in [(a, ca), (b, cb)] {
            let h = &mut l.keyframes.get_mut(&k).unwrap().shape_hints;
            while h.len() < n {
                h.push(c);
            }
            h.push(c);
        }
        Some(n)
    }
    /// Move hint `i` on the keyframe at `key` (a point on the shape's outline).
    pub fn move_shape_hint(&mut self, key: u32, i: usize, to: Pt, label: Option<&str>) {
        let li = self.layer;
        match label {
            Some(l) => self.checkpoint(l),
            None => self.revision += 1,
        }
        if let Some(h) = self
            .timeline_mut()
            .layers
            .get_mut(li)
            .and_then(|l| l.keyframes.get_mut(&key))
            .and_then(|k| k.shape_hints.get_mut(i))
        {
            *h = to;
        }
    }
    pub fn remove_shape_hint(&mut self, i: usize) {
        let Some((a, b)) = self.shape_tween_keys() else {
            return;
        };
        self.checkpoint("Remove Shape Hint");
        let li = self.layer;
        let l = &mut self.timeline_mut().layers[li];
        for k in [a, b] {
            let h = &mut l.keyframes.get_mut(&k).unwrap().shape_hints;
            if i < h.len() {
                h.remove(i);
            }
        }
    }
    pub fn remove_all_shape_hints(&mut self) {
        let Some((a, b)) = self.shape_tween_keys() else {
            return;
        };
        self.checkpoint("Remove All Hints");
        let li = self.layer;
        let l = &mut self.timeline_mut().layers[li];
        for k in [a, b] {
            l.keyframes.get_mut(&k).unwrap().shape_hints.clear();
        }
    }

    /// F7 – Insert Blank Keyframe.
    pub fn insert_blank_keyframe(&mut self) {
        self.for_each_layer("Insert Blank Keyframe", |l, s| {
            frames::convert_to_keyframes(l, s.f0, s.f1, true)
        });
    }
    /// Shift+F6 – Clear Keyframe.
    pub fn clear_keyframe(&mut self) {
        let s = self.frame_target();
        if s.f0 == s.f1 {
            if let Some((st, _)) = self.motion_span(s.layer0, s.f0) {
                if st != s.f0 {
                    self.checkpoint("Clear Property Keyframes");
                    let l = &mut self.timeline_mut().layers[s.layer0];
                    if let Some(m) = l.keyframes.get_mut(&st).and_then(|k| k.motion.as_mut()) {
                        m.clear_keys_at(s.f0 - st);
                    }
                    return;
                }
            }
        }
        self.for_each_layer("Clear Keyframe", |l, s| {
            frames::clear_keyframes(l, s.f0, s.f1)
        });
    }
    /// Delete/Backspace on selected frames – Clear Frames.
    pub fn clear_frames(&mut self) {
        self.for_each_layer("Clear Frames", |l, s| frames::clear_frames(l, s.f0, s.f1));
    }
    pub fn set_tween(&mut self, tween: Tween) {
        let label = match tween {
            Tween::None => "Remove Tween",
            Tween::Classic { .. } => "Create Classic Tween",
            Tween::Shape { .. } => "Create Shape Tween",
            Tween::Motion { .. } => return self.create_motion_tween(),
        };
        self.for_each_layer(label, |l, s| frames::set_tween(l, s.f0, s.f1, tween));
    }
    /// Change only the ease/rotation of existing tweens in the target.
    pub fn set_tween_params(&mut self, ease: f32, turns: i32) {
        self.for_each_layer("Tween Settings", |l, s| {
            let starts: Vec<u32> = l
                .keyframes
                .keys()
                .copied()
                .filter(|&k| k <= s.f1 && l.span_end(k) > s.f0)
                .collect();
            for k in starts {
                let kf = l.keyframes.get_mut(&k).unwrap();
                kf.tween = match kf.tween {
                    Tween::Classic { .. } => Tween::Classic {
                        ease,
                        rotate_turns: turns,
                    },
                    Tween::Shape { .. } => Tween::Shape { ease },
                    Tween::Motion { .. } => {
                        if let Some(m) = kf.motion.as_mut() {
                            m.rotate_turns = turns;
                        }
                        Tween::Motion { ease }
                    }
                    Tween::None => Tween::None,
                };
            }
        });
    }
    pub fn copy_frames(&mut self) {
        let s = self.frame_target();
        let tl = self.timeline();
        self.frame_clip = (s.layer0..=s.layer1)
            .filter_map(|i| tl.layers.get(i))
            .map(|l| frames::copy_frames(l, s.f0, s.f1))
            .collect();
    }
    pub fn cut_frames(&mut self) {
        self.copy_frames();
        self.clear_frames();
    }
    pub fn can_paste_frames(&self) -> bool {
        !self.frame_clip.is_empty()
    }
    pub fn paste_frames(&mut self) {
        if self.frame_clip.is_empty() {
            return;
        }
        let s = self.frame_target();
        let clips = self.frame_clip.clone();
        self.checkpoint("Paste Frames");
        let tl = self.timeline_mut();
        for (i, clip) in clips.iter().enumerate() {
            if let Some(l) = tl.layers.get_mut(s.layer0 + i) {
                frames::paste_frames(l, s.f0, clip);
            }
        }
        let len = clips[0].len;
        self.frame_sel = Some(FrameSel {
            layer0: s.layer0,
            layer1: (s.layer0 + clips.len() - 1).min(self.timeline().layers.len() - 1),
            f0: s.f0,
            f1: s.f0 + len - 1,
        });
        self.selection.clear();
    }
    /// Drag the selected frames left/right by `delta`.
    pub fn move_frames(&mut self, delta: i64) {
        let Some(s) = self.frame_sel else { return };
        if delta == 0 {
            return;
        }
        self.checkpoint("Move Frames");
        let tl = self.timeline_mut();
        for li in s.layer0..=s.layer1 {
            if let Some(l) = tl.layers.get_mut(li) {
                frames::move_frames(l, s.f0, s.f1, delta);
            }
        }
        let d = delta.max(-(s.f0 as i64));
        let f0 = (s.f0 as i64 + d) as u32;
        self.frame_sel = Some(FrameSel {
            f0,
            f1: f0 + (s.f1 - s.f0),
            ..s
        });
        self.frame = f0;
        self.selection.clear();
    }

    /// The keyframe governing the playhead on the current layer.
    pub fn current_keyframe(&self) -> Option<(u32, &Keyframe)> {
        let layer = self.timeline().layers.get(self.layer)?;
        layer.span_at(self.frame).map(|(s, k, _)| (s, k))
    }

    pub fn set_frame_label(&mut self, label: &str) {
        let Some((start, _)) = self.current_keyframe() else {
            return;
        };
        self.checkpoint("Frame Label");
        let li = self.layer;
        let l = &mut self.timeline_mut().layers[li];
        l.keyframes.get_mut(&start).unwrap().label = if label.is_empty() {
            None
        } else {
            Some(label.into())
        };
    }

    pub fn set_frame_script(&mut self, script: &str) {
        let li = self.layer;
        let f = self.frame;
        if self.timeline().layers.get(li).is_none() {
            return;
        }
        self.checkpoint("Frame Script");
        let l = &mut self.timeline_mut().layers[li];
        // Scripts live on keyframes; make one here if needed (as Animate does).
        if !l.keyframes.contains_key(&f) {
            frames::convert_to_keyframes(l, f, f, false);
        }
        let kf = l.keyframes.get_mut(&f).unwrap();
        kf.script = if script.trim().is_empty() {
            None
        } else {
            Some(script.into())
        };
    }

    /// Undo-stack depth; pair with `squash_undo` to make a batch (a script run) one step.
    pub fn undo_depth(&self) -> usize {
        self.undo.len()
    }
    pub fn squash_undo(&mut self, since: usize, label: &str) {
        if self.undo.len() > since + 1 {
            self.undo.truncate(since + 1);
        }
        if let Some(s) = self.undo.get_mut(since) {
            s.label = label.into();
        }
    }

    // ------------------------------------------------------------ elements
    /// Which keyframe an edit at the playhead lands in, creating one if needed:
    /// - past the layer's end: a new keyframe (Animate inserts one for you)
    /// - inside a static span: the span's keyframe (same object on every frame)
    /// - inside a tween: a new keyframe here when `pose` (moving/scaling an object
    ///   mid-tween sets a new pose), otherwise the span's keyframe
    fn target_keyframe(&mut self, layer: usize, pose: bool) -> Option<u32> {
        let f = self.frame;
        let l = self.timeline().layers.get(layer)?;
        let key = match l.span_at(f) {
            None => {
                let l = &mut self.timeline_mut().layers[layer];
                frames::convert_to_keyframes(l, f, f, false);
                return Some(f);
            }
            Some((s, _, _)) if s == f => f,
            Some((s, kf, _)) => {
                if pose && !matches!(kf.tween, Tween::None) {
                    let l = &mut self.timeline_mut().layers[layer];
                    frames::convert_to_keyframes(l, f, f, false);
                    // Timeline splitting preserves a live source interval.
                    // Object editing still needs a visible pose until authored
                    // property rebasing is implemented for those intervals.
                    frames::materialize_retained_boundaries(l, f, f);
                    f
                } else {
                    s
                }
            }
        };
        Some(key)
    }

    pub fn layer_editable(&self, layer: usize) -> bool {
        self.timeline()
            .layers
            .get(layer)
            .map(|l| !l.locked && l.visible && l.kind != LayerKind::Camera)
            .unwrap_or(false)
    }
    pub fn layer_locked(&self) -> bool {
        !self.layer_editable(self.layer)
    }

    pub fn add_element(&mut self, el: Element) -> Option<Selection> {
        if self.layer_locked() {
            return None;
        }
        self.commit_floats();
        self.checkpoint(&format!("Add {}", el.name));
        let li = self.layer;
        let mut el = el;
        if let Some(inv) = self.layer_parent_at(li).and_then(|m| m.invert()) {
            el.matrix = inv.concat(&el.matrix);
        }
        let start = self.target_keyframe(li, false)?;
        let kf = self.timeline_mut().layers[li]
            .keyframes
            .get_mut(&start)
            .unwrap();
        kf.elements.push(el);
        let sel = Selection {
            layer: li,
            element: kf.elements.len() - 1,
        };
        self.selection = vec![sel];
        self.frame_sel = None;
        Some(sel)
    }

    /// Topmost element under `p` on unlocked, visible layers.
    pub fn hit_test(&self, p: Pt) -> Option<Selection> {
        let items = self.evaluate_current(self.frame);
        let tl = self.timeline();
        items.iter().rev().find_map(|it| {
            let (li, ei) = it.src?;
            if tl.layers[li].locked {
                return None;
            }
            it.hit(p).then_some(Selection {
                layer: li,
                element: ei,
            })
        })
    }

    /// Topmost editable object under `p` on `layer`, ignoring `skip`.
    pub fn hit_test_on_layer(&self, p: Pt, layer: usize, skip: Option<usize>) -> Option<usize> {
        self.evaluate_current(self.frame)
            .iter()
            .rev()
            .find_map(|it| {
                let (li, ei) = it.src?;
                (li == layer && Some(ei) != skip && it.hit(p)).then_some(ei)
            })
    }

    pub fn is_selected(&self, s: Selection) -> bool {
        self.selection.contains(&s)
    }
    /// Click: replace selection. Shift-click: toggle.
    pub fn select(&mut self, s: Option<Selection>, toggle: bool) {
        if !toggle && s.map(|x| !self.selection.contains(&x)).unwrap_or(true) {
            self.commit_floats();
        }
        match (s, toggle) {
            (Some(s), true) => {
                if let Some(i) = self.selection.iter().position(|x| *x == s) {
                    self.selection.remove(i);
                } else {
                    self.selection.push(s);
                    self.selection.sort();
                }
            }
            (Some(s), false) => self.selection = vec![s],
            (None, false) => self.selection.clear(),
            (None, true) => {}
        }
        if let Some(s) = self.selection.first() {
            self.layer = s.layer;
            self.frame_sel = None;
        }
    }
    pub fn select_all(&mut self) {
        self.commit_floats();
        self.lift_all_canvases();
        let items = self.evaluate_current(self.frame);
        let mut v: Vec<Selection> = items
            .iter()
            .filter_map(|it| it.src)
            .filter(|(l, _)| self.layer_editable(*l))
            .map(|(layer, element)| Selection { layer, element })
            .collect();
        v.sort();
        v.dedup();
        self.selection = v;
        self.frame_sel = None;
    }
    /// Marquee: select elements whose bounds lie fully inside the rectangle.
    pub fn select_in_rect(&mut self, a: Pt, b: Pt, add: bool) {
        if !add {
            self.commit_floats();
        }
        let (lo, hi) = (
            pt(a.x.min(b.x), a.y.min(b.y)),
            pt(a.x.max(b.x), a.y.max(b.y)),
        );
        let mut found: Vec<Selection> = Vec::new();
        let items = self.evaluate_current(self.frame);
        let mut bounds: std::collections::BTreeMap<(usize, usize), (Pt, Pt)> = Default::default();
        for it in &items {
            let Some(src) = it.src else { continue };
            if !self.layer_editable(src.0) {
                continue;
            }
            if let Some((l, h)) = it.path.transformed(&it.matrix).bounds() {
                let e = bounds.entry(src).or_insert((l, h));
                *e = (
                    pt(e.0.x.min(l.x), e.0.y.min(l.y)),
                    pt(e.1.x.max(h.x), e.1.y.max(h.y)),
                );
            }
        }
        for ((layer, element), (l, h)) in bounds {
            // merge shapes are cut by the marquee instead (merge_select_area)
            if matches!(
                self.element_at(Selection { layer, element })
                    .map(|e| e.kind),
                Some(ElementKind::Canvas(_))
            ) {
                continue;
            }
            if l.x >= lo.x && l.y >= lo.y && h.x <= hi.x && h.y <= hi.y {
                found.push(Selection { layer, element });
            }
        }
        if add {
            self.selection.extend(found);
            self.selection.sort();
            self.selection.dedup();
        } else {
            self.selection = found;
        }
        self.frame_sel = None;
    }

    pub fn element_at(&self, s: Selection) -> Option<Element> {
        let layer = self.timeline().layers.get(s.layer)?;
        let (els, _) = layer_elements_at(layer, self.frame)?;
        let mut el = els.get(s.element).cloned()?;
        if let Some(pm) = self.layer_parent_at(s.layer) {
            el.matrix = pm.concat(&el.matrix); // as seen on the stage
        }
        Some(el)
    }
    /// The first selected element (drives the Properties panel).
    pub fn selected_element(&self) -> Option<Element> {
        self.element_at(*self.selection.first()?)
    }

    pub fn bounds_of(&self, sels: &[Selection]) -> Option<(Pt, Pt)> {
        let items = self.evaluate_current(self.frame);
        let mut b: Option<(Pt, Pt)> = None;
        // merge shapes with a floating selection: only the selection counts
        let mut floats = vec![];
        for s in sels {
            if let Some(el) = self.element_at(*s) {
                if let ElementKind::Canvas(c) = &el.kind {
                    if let Some((lo, hi)) = c.float.as_ref().and_then(|f| f.bounds()) {
                        let q =
                            [lo, pt(hi.x, lo.y), hi, pt(lo.x, hi.y)].map(|x| el.matrix.apply(x));
                        let (mut a, mut c2) = (q[0], q[0]);
                        for x in q {
                            a = pt(a.x.min(x.x), a.y.min(x.y));
                            c2 = pt(c2.x.max(x.x), c2.y.max(x.y));
                        }
                        b = Some(match b {
                            None => (a, c2),
                            Some((p0, p1)) => (
                                pt(p0.x.min(a.x), p0.y.min(a.y)),
                                pt(p1.x.max(c2.x), p1.y.max(c2.y)),
                            ),
                        });
                        floats.push(*s);
                    }
                }
            }
        }
        for it in items.iter().filter(|it| {
            it.src
                .map(|(l, e)| {
                    sels.contains(&Selection {
                        layer: l,
                        element: e,
                    }) && !floats.contains(&Selection {
                        layer: l,
                        element: e,
                    })
                })
                .unwrap_or(false)
        }) {
            if let Some((lo, hi)) = it.path.transformed(&it.matrix).bounds() {
                b = Some(match b {
                    None => (lo, hi),
                    Some((a, c)) => (
                        pt(a.x.min(lo.x), a.y.min(lo.y)),
                        pt(c.x.max(hi.x), c.y.max(hi.y)),
                    ),
                });
            }
        }
        b
    }
    pub fn selection_bounds(&self) -> Option<(Pt, Pt)> {
        self.bounds_of(&self.selection)
    }

    /// Apply `f` to every selected element on editable layers. `label` names the
    /// undo step. Returns true when an unsupported retained edit sampled poses.
    pub fn modify_selected(&mut self, label: &str, f: impl Fn(&mut Element)) -> bool {
        if self.selection.is_empty() {
            return false;
        }
        self.checkpoint(label);
        self.modify_selected_continuous(f)
    }

    /// Like `modify_selected` without a new undo step (call `begin_drag` first).
    pub fn modify_selected_continuous(&mut self, f: impl Fn(&mut Element)) -> bool {
        let mut materialized = false;
        let sels = self.selection.clone();
        let mut layers: Vec<usize> = sels.iter().map(|s| s.layer).collect();
        layers.dedup();
        for li in layers {
            if !self.layer_editable(li) {
                continue;
            }
            // parented layers store objects in the parent's space; tools work in stage space
            let pm = self
                .layer_parent_at(li)
                .and_then(|m| m.invert().map(|i| (m, i)));
            let parented = |el: &mut Element| match pm {
                Some((m, inv)) => {
                    el.matrix = m.concat(&el.matrix);
                    f(el);
                    el.matrix = inv.concat(&el.matrix);
                }
                None => f(el),
            };
            // a merge shape with a floating selection: the edit moves the selection only
            let wrapped = |el: &mut Element| {
                if let ElementKind::Canvas(c) = &el.kind {
                    if c.float.is_some() {
                        let mut tmp = el.clone();
                        parented(&mut tmp);
                        let d = el
                            .matrix
                            .invert()
                            .unwrap_or(Matrix::IDENTITY)
                            .concat(&tmp.matrix);
                        if let ElementKind::Canvas(c) = &mut el.kind {
                            if let Some(fl) = c.float.as_mut() {
                                fl.transform(&d);
                            }
                        }
                        return;
                    }
                }
                parented(el)
            };
            let f = &wrapped;
            let frame = self.frame;
            // Classic/shape auto-keying first retains the two source intervals,
            // so the following pose edit can preserve their nonlinear curves.
            if self.multi_frame.is_none()
                && self.timeline().layers[li]
                    .span_at(frame)
                    .is_some_and(|(start, key, _)| {
                        start < frame && key.tween != Tween::None && !key.tween.is_motion()
                    })
            {
                frames::convert_to_keyframes(
                    &mut self.timeline_mut().layers[li],
                    frame,
                    frame,
                    false,
                );
            }
            let selected: Vec<_> = sels
                .iter()
                .filter(|s| s.layer == li)
                .map(|s| s.element)
                .collect();
            if self.multi_frame.is_none()
                && crate::tween_edit::edit_pose(
                    &mut self.timeline_mut().layers[li],
                    frame,
                    &selected,
                    &f,
                )
            {
                continue;
            }
            if self.timeline().layers[li]
                .span_at(frame)
                .is_some_and(|(_, k, _)| k.tween_boundary.is_some())
            {
                frames::materialize_retained_boundaries(
                    &mut self.timeline_mut().layers[li],
                    frame,
                    frame,
                );
                materialized = true;
            }
            if self.motion_span(li, self.frame).is_some() {
                for s in sels.iter().filter(|s| s.layer == li) {
                    self.modify_motion_element(li, s.element, &f);
                }
                continue;
            }
            if let Some((a, b)) = self.multi_frame {
                // Edit Multiple Frames: every keyframe in range, in place (no new keyframes)
                let cur = self.timeline().layers[li]
                    .span_at(self.frame)
                    .map(|(s, _, _)| s);
                let l = &mut self.timeline_mut().layers[li];
                let keys: Vec<u32> = l
                    .keyframes
                    .keys()
                    .copied()
                    .filter(|&k| (k >= a && k <= b) || Some(k) == cur)
                    .collect();
                for k in keys {
                    let kf = l.keyframes.get_mut(&k).unwrap();
                    for s in sels.iter().filter(|s| s.layer == li) {
                        if let Some(el) = kf.elements.get_mut(s.element) {
                            f(el);
                        }
                    }
                }
                continue;
            }
            let Some(start) = self.target_keyframe(li, true) else {
                continue;
            };
            let l = &mut self.timeline_mut().layers[li];
            if let Some(kf) = l.keyframes.get_mut(&start) {
                for s in sels.iter().filter(|s| s.layer == li) {
                    if let Some(el) = kf.elements.get_mut(s.element) {
                        f(el);
                    }
                }
            }
        }
        materialized
    }
    // ------------------------------------------------------------ motion tweens
    /// (span start, span length) when `frame` on `layer` is inside a motion tween.
    pub fn motion_span(&self, layer: usize, frame: u32) -> Option<(u32, u32)> {
        let l = self.timeline().layers.get(layer)?;
        let (st, kf, next) = l.span_at(frame)?;
        (kf.tween.is_motion() && kf.motion.is_some())
            .then(|| (st, next.unwrap_or(l.end).min(l.end) - st))
    }
    /// The motion tween under the playhead on the current layer.
    pub fn current_motion(&self) -> Option<(u32, u32, &crate::motion::Motion, &Keyframe)> {
        let (st, len) = self.motion_span(self.layer, self.frame)?;
        let kf = &self.timeline().layers[self.layer].keyframes[&st];
        Some((st, len, kf.motion.as_deref()?, kf))
    }
    fn motion_mut(&mut self, layer: usize, start: u32) -> Option<&mut crate::motion::Motion> {
        self.timeline_mut()
            .layers
            .get_mut(layer)?
            .keyframes
            .get_mut(&start)?
            .motion
            .as_deref_mut()
    }

    /// Edit the tweened object at the playhead: transform changes become
    /// property keyframes, everything else (fill, filters, name…) changes the
    /// object on the whole span.
    fn modify_motion_element(&mut self, li: usize, ei: usize, f: &impl Fn(&mut Element)) {
        use crate::motion::Pose;
        let Some((st, _)) = self.motion_span(li, self.frame) else {
            return;
        };
        let local = self.frame - st;
        let layer = &self.timeline().layers[li];
        let Some((els, _)) = layer_elements_at(layer, self.frame) else {
            return;
        };
        let kf = &layer.keyframes[&st];
        if ei != 0 {
            let l = &mut self.timeline_mut().layers[li];
            if let Some(el) = l
                .keyframes
                .get_mut(&st)
                .and_then(|k| k.elements.get_mut(ei))
            {
                f(el);
            }
            return;
        }
        let (Some(now), Some(base_el), Some(m)) = (
            els.first().cloned(),
            kf.elements.first().cloned(),
            kf.motion.as_deref(),
        ) else {
            return;
        };
        let pivot = m.pivot;
        let mut after = now.clone();
        f(&mut after);
        let (base, before, after_p) = (
            Pose::of(&base_el, pivot),
            Pose::of(&now, pivot),
            Pose::of(&after, pivot),
        );
        let mut new_base = after.clone();
        if local == 0 {
            // the first frame: the object itself changes (keyed properties get keys below)
            after_p.apply(&mut new_base, pivot);
        } else {
            new_base.matrix = base_el.matrix;
            new_base.alpha = base_el.alpha;
            new_base.tint = if after.tint != now.tint
                && crate::motion::brightness_of(after.tint) == 0.0
                && after.tint.is_some()
            {
                after.tint
            } else {
                base_el.tint
            };
        }
        let l = &mut self.timeline_mut().layers[li];
        let k = l.keyframes.get_mut(&st).unwrap();
        k.elements[0] = new_base;
        if let Some(m) = k.motion.as_deref_mut() {
            m.record(local, &base, &before, &after_p);
        }
    }

    /// Insert ▸ Motion Tween: each selected span becomes a motion tween of its
    /// (single) object. Several objects are grouped first; a one-frame span
    /// grows to one second, as in Animate.
    pub fn create_motion_tween(&mut self) {
        let s = self.frame_target();
        let fps = self.doc.fps.round().max(2.0) as u32;
        let mut plan: Vec<(usize, u32, Pt)> = vec![];
        {
            let tl = self.timeline();
            for li in s.layer0..=s.layer1.min(tl.layers.len().saturating_sub(1)) {
                let l = &tl.layers[li];
                if l.kind != LayerKind::Normal {
                    continue;
                }
                for (&st, kf) in l.keyframes.range(..=s.f1) {
                    if l.span_end(st) <= s.f0
                        || st >= l.end
                        || kf.elements.is_empty()
                        || kf.motion.is_some()
                    {
                        continue;
                    }
                    let el = if kf.elements.len() == 1 {
                        kf.elements[0].clone()
                    } else {
                        Element::group(kf.elements.clone())
                    };
                    let pivot = element_local_center(&EvalCtx::new(&self.doc, self.frame), &el);
                    plan.push((li, st, pivot));
                }
            }
        }
        if plan.is_empty() {
            return;
        }
        self.checkpoint("Create Motion Tween");
        for (li, st, pivot) in plan.into_iter().rev() {
            let l = &mut self.timeline_mut().layers[li];
            let kf = l.keyframes.get_mut(&st).unwrap();
            if kf.elements.len() > 1 {
                kf.elements = vec![Element::group(std::mem::take(&mut kf.elements))];
            }
            kf.tween = Tween::Motion {
                ease: kf.tween.ease(),
            };
            kf.motion = Some(Box::new(crate::motion::Motion::new(pivot)));
            kf.tween_boundary = None;
            if l.span_end(st) - st == 1 {
                frames::insert_frames(l, st, fps - 1);
            }
        }
        self.selection.clear();
    }

    /// F6 inside a motion tween: a keyframe for every property at `frame`.
    pub fn key_all_properties(&mut self, li: usize, frame: u32) {
        use crate::motion::Pose;
        let Some((st, _)) = self.motion_span(li, frame) else {
            return;
        };
        let layer = &self.timeline().layers[li];
        let (Some((els, _)), Some(kf)) =
            (layer_elements_at(layer, frame), layer.keyframes.get(&st))
        else {
            return;
        };
        let (Some(now), Some(base), Some(m)) =
            (els.first(), kf.elements.first(), kf.motion.as_deref())
        else {
            return;
        };
        let (b, n) = (Pose::of(base, m.pivot), Pose::of(now, m.pivot));
        self.checkpoint("Insert Property Keyframes");
        if let Some(m) = self.motion_mut(li, st) {
            m.key_all(frame - st, &b, &n);
        }
    }

    /// Change the current motion tween (Motion Editor, motion path edits).
    /// `label` names the undo step; pass None to merge into the current drag.
    pub fn edit_motion(
        &mut self,
        label: Option<&str>,
        f: impl FnOnce(&mut crate::motion::Motion, &Element),
    ) {
        let Some((st, _)) = self.motion_span(self.layer, self.frame) else {
            return;
        };
        if let Some(l) = label {
            self.checkpoint(l);
        } else {
            self.revision += 1;
        }
        let li = self.layer;
        let l = &mut self.timeline_mut().layers[li];
        let kf = l.keyframes.get_mut(&st).unwrap();
        let Some(base) = kf.elements.first().cloned() else {
            return;
        };
        if let Some(m) = kf.motion.as_deref_mut() {
            f(m, &base);
        }
    }

    /// Motion Editor / scripts: set a property key at absolute `frame` on the
    /// current layer's motion tween.
    pub fn set_motion_value(
        &mut self,
        prop: crate::motion::Prop,
        frame: u32,
        value: f32,
        label: Option<&str>,
    ) {
        use crate::motion::Pose;
        let li = self.layer;
        let Some((st, _)) = self.motion_span(li, frame) else {
            return;
        };
        let layer = &self.timeline().layers[li];
        let (Some((els, _)), Some(kf)) =
            (layer_elements_at(layer, frame), layer.keyframes.get(&st))
        else {
            return;
        };
        let (Some(now), Some(base), Some(m)) =
            (els.first(), kf.elements.first(), kf.motion.as_deref())
        else {
            return;
        };
        let (b, n) = (Pose::of(base, m.pivot), Pose::of(now, m.pivot));
        match label {
            Some(l) => self.checkpoint(l),
            None => self.revision += 1,
        }
        if let Some(m) = self.motion_mut(li, st) {
            m.set_value(prop, frame - st, value, &b, &n);
        }
    }

    /// Custom/preset ease for the span(s) under the frame selection
    /// (classic, shape and motion tweens). None = back to the simple ease.
    pub fn set_ease_curve(&mut self, ease: Option<crate::motion::Ease>) {
        self.for_each_layer("Ease", |l, s| {
            let starts: Vec<u32> = l
                .keyframes
                .keys()
                .copied()
                .filter(|&k| k <= s.f1 && l.span_end(k) > s.f0)
                .collect();
            for k in starts {
                let kf = l.keyframes.get_mut(&k).unwrap();
                if !matches!(kf.tween, Tween::None) {
                    kf.ease_curve = ease.clone();
                }
            }
        });
    }

    pub fn begin_drag(&mut self) {
        self.checkpoint("Move");
    }
    /// Start a continuous edit with a named undo step (pair with the `*_continuous` APIs).
    pub fn begin_edit(&mut self, label: &str) {
        self.checkpoint(label);
    }

    /// Delete the selected elements from the keyframe they live in.
    pub fn delete_selected(&mut self) -> usize {
        let mut sels = self.selection.clone();
        sels.retain(|s| self.layer_editable(s.layer));
        if sels.is_empty() {
            return 0;
        }
        self.checkpoint(if sels.len() == 1 {
            "Delete"
        } else {
            "Delete Objects"
        });
        sels.sort();
        let mut removed = 0;
        // merge shapes: delete the floating selection, keep the rest of the shape
        sels.retain(|s| {
            let Some((key, ei)) = self.canvas_at(s.layer).filter(|(_, ei)| *ei == s.element) else {
                return true;
            };
            match self.canvas_mut(s.layer, key, ei) {
                Some(c) if c.float.is_some() => {
                    c.float = None;
                    removed += 1;
                    false
                }
                _ => true,
            }
        });
        for s in sels.iter().rev() {
            let e = s.element;
            self.remap_bones(s.layer, |i| {
                if i == e {
                    None
                } else if i > e {
                    Some(i - 1)
                } else {
                    Some(i)
                }
            });
            let Some(start) = self.target_keyframe(s.layer, false) else {
                continue;
            };
            let l = &mut self.timeline_mut().layers[s.layer];
            if let Some(kf) = l.keyframes.get_mut(&start) {
                if s.element < kf.elements.len() {
                    let retained: Vec<_> =
                        (0..kf.elements.len()).filter(|&i| i != s.element).collect();
                    crate::tween_edit::remap_elements(kf, &retained);
                    kf.elements.remove(s.element);
                    if let Some(b) = &mut kf.tween_boundary {
                        if s.element < b.elements.len() {
                            b.elements.remove(s.element);
                        }
                    }
                    removed += 1;
                }
                // Keep a following tween's pairing intact: remove the same slot there too.
                let next = l.keyframes.range(start + 1..).next().map(|(k, _)| *k);
                let tweened = !matches!(l.keyframes[&start].tween, Tween::None)
                    && l.keyframes[&start].tween_boundary.is_none();
                if tweened {
                    if let Some(n) = next {
                        let nk = l.keyframes.get_mut(&n).unwrap();
                        if s.element < nk.elements.len() {
                            nk.elements.remove(s.element);
                        }
                    }
                }
            }
        }
        self.selection.clear();
        removed
    }

    pub fn copy_selected(&mut self) -> usize {
        // a merge selection copies just the selected piece
        self.element_clip = self
            .selection
            .iter()
            .filter_map(|s| {
                let mut e = self.element_at(*s)?;
                if let ElementKind::Canvas(c) = &e.kind {
                    if let Some(f) = &c.float {
                        e.kind = ElementKind::Canvas((**f).clone());
                    }
                }
                Some((e, s.layer))
            })
            .collect();
        self.element_clip.len()
    }
    pub fn cut_selected(&mut self) {
        if self.copy_selected() > 0 {
            self.delete_selected();
        }
    }
    pub fn can_paste(&self) -> bool {
        !self.element_clip.is_empty()
    }
    /// Paste onto the current layer (offset unless `in_place`).
    pub fn paste(&mut self, in_place: bool) {
        if self.element_clip.is_empty() || self.layer_locked() {
            return;
        }
        let clip = self.element_clip.clone();
        self.checkpoint(if in_place { "Paste in Place" } else { "Paste" });
        self.commit_floats();
        let li = self.layer;
        // merge-shape pieces land on the layer's shape as a floating selection
        let (pieces, objects): (Vec<_>, Vec<_>) = clip
            .into_iter()
            .partition(|(e, _)| matches!(e.kind, ElementKind::Canvas(_)));
        let mut sel = vec![];
        if !pieces.is_empty() {
            let inv = self.canvas_inv(li);
            let has = self.canvas_at(li).is_some();
            if let Some((key, ei)) = self.canvas_for_drawing(li) {
                for (el, _) in pieces {
                    let ElementKind::Canvas(mut piece) = el.kind else {
                        continue;
                    };
                    piece.float = None;
                    let off = if in_place {
                        Matrix::IDENTITY
                    } else {
                        Matrix::translate(10.0, 10.0)
                    };
                    let m = if has { inv } else { Matrix::IDENTITY }
                        .concat(&off)
                        .concat(&el.matrix);
                    piece.transform(&m);
                    if let Some(c) = self.canvas_mut(li, key, ei) {
                        c.add_to_float(piece);
                    }
                }
                sel.push(Selection {
                    layer: li,
                    element: ei,
                });
            }
        }
        let Some(start) = self.target_keyframe(li, false) else {
            return;
        };
        let kf = self.timeline_mut().layers[li]
            .keyframes
            .get_mut(&start)
            .unwrap();
        let first = kf.elements.len();
        for (mut el, _) in objects {
            if !in_place {
                el.matrix = Matrix::translate(10.0, 10.0).concat(&el.matrix);
            }
            kf.elements.push(el);
        }
        let n = kf.elements.len();
        sel.extend((first..n).map(|element| Selection { layer: li, element }));
        self.selection = sel;
        self.frame_sel = None;
    }
    pub fn duplicate(&mut self) {
        if self.copy_selected() > 0 {
            if let Some(s) = self.selection.first() {
                self.layer = s.layer;
            }
            self.paste(false);
        }
    }

    /// Duplicate exactly on top of the original (used by Copy and Transform).
    pub fn duplicate_in_place(&mut self) {
        if self.copy_selected() > 0 {
            if let Some(s) = self.selection.first() {
                self.layer = s.layer;
            }
            self.paste(true);
        }
    }

    /// Ctrl+Up/Down style arrange within one layer (first selected element).
    pub fn arrange(&mut self, to_front: bool) {
        let Some(&s) = self.selection.first() else {
            return;
        };
        if !self.layer_editable(s.layer) {
            return;
        }
        self.checkpoint(if to_front {
            "Bring to Front"
        } else {
            "Send to Back"
        });
        let Some(start) = self.target_keyframe(s.layer, false) else {
            return;
        };
        let els = &mut self.timeline_mut().layers[s.layer]
            .keyframes
            .get_mut(&start)
            .unwrap()
            .elements;
        if s.element >= els.len() {
            return;
        }
        let n = els.len();
        let e = els.remove(s.element);
        let idx = if to_front { els.len() } else { 0 };
        els.insert(idx, e);
        // keep bone references pointing at the same objects
        let mut order: Vec<usize> = (0..n).collect();
        let moved = order.remove(s.element);
        order.insert(idx, moved);
        let key = self.timeline_mut().layers[s.layer]
            .keyframes
            .get_mut(&start)
            .unwrap();
        crate::tween_edit::remap_elements(key, &order);
        if let Some(b) = &mut key.tween_boundary {
            let old = b.elements.clone();
            b.elements = order.iter().filter_map(|&i| old.get(i).cloned()).collect();
        }
        self.remap_bones(s.layer, |i| order.iter().position(|&o| o == i));
        self.selection = vec![Selection {
            layer: s.layer,
            element: idx,
        }];
    }

    /// F8 – Convert to Symbol. All selected objects on the first selected
    /// layer go into the new symbol; registration point = centre of their bounds.
    pub fn convert_to_symbol(&mut self, name: &str, kind: SymbolKind) -> Option<SymbolId> {
        self.detach_floats();
        let first = *self.selection.first()?;
        let sels: Vec<Selection> = self
            .selection
            .iter()
            .copied()
            .filter(|s| s.layer == first.layer)
            .collect();
        let els: Vec<Element> = sels.iter().filter_map(|s| self.element_at(*s)).collect();
        if els.is_empty() || !self.layer_editable(first.layer) {
            return None;
        }
        let (lo, hi) = self.bounds_of(&sels)?;
        let reg = pt((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0);
        self.checkpoint("Convert to Symbol");
        // Structural symbol conversion still uses visible poses. In particular,
        // do not apply a retained correction a second time to the new instance.
        let frame = self.frame;
        frames::materialize_retained_boundaries(
            &mut self.timeline_mut().layers[first.layer],
            frame,
            frame,
        );
        let inner: Vec<Element> = els
            .iter()
            .map(|el| {
                let mut e = el.clone();
                e.matrix = Matrix::translate(-reg.x, -reg.y).concat(&el.matrix);
                e
            })
            .collect();
        let mut tl = Timeline::new(name);
        tl.layers[0].keyframes.insert(0, Keyframe::with(inner));
        let id = self.doc.add_symbol(name, kind, tl);
        let start = self.target_keyframe(first.layer, false)?;
        let slot = sels[0].element;
        let gone: Vec<usize> = sels.iter().map(|s| s.element).collect();
        let kept: Vec<usize> = gone.iter().skip(1).copied().collect();
        let mapping = move |i: usize| {
            if gone.contains(&i) {
                None
            } else {
                Some(i - kept.iter().filter(|&&k| k < i).count())
            }
        };
        self.remap_bones(first.layer, mapping);
        let kf = self.timeline_mut().layers[first.layer]
            .keyframes
            .get_mut(&start)
            .unwrap();
        for s in sels.iter().skip(1).rev() {
            if s.element < kf.elements.len() {
                kf.elements.remove(s.element);
            }
        }
        if slot < kf.elements.len() {
            kf.elements[slot] = Element::instance(name, id, Matrix::translate(reg.x, reg.y));
        }
        self.selection = vec![Selection {
            layer: first.layer,
            element: slot,
        }];
        Some(id)
    }

    /// Break Apart (Ctrl+B) for symbol instances: replace with the symbol's
    /// current-frame content, keeping the visual result.
    pub fn break_apart(&mut self) {
        let Some(&s) = self.selection.first() else {
            return;
        };
        let Some(el) = self.element_at(s) else { return };
        if matches!(el.kind, ElementKind::Shape(_)) && self.layer_editable(s.layer) {
            // drawing objects (and text broken twice) become merge shape
            let shapes: Vec<Selection> = self
                .selection
                .iter()
                .rev()
                .copied()
                .filter(|x| {
                    matches!(
                        self.element_at(*x).map(|e| e.kind),
                        Some(ElementKind::Shape(_))
                    )
                })
                .collect();
            let mark = self.undo_depth();
            for x in shapes {
                self.shape_to_merge(x);
            }
            self.squash_undo(mark, "Break Apart");
            return;
        }
        if matches!(el.kind, ElementKind::Group(_)) {
            self.ungroup_selected();
            return;
        }
        let ElementKind::Instance(inst) = &el.kind else {
            return;
        };
        let Some(sym) = self.doc.symbol(inst.symbol) else {
            return;
        };
        let mut parts: Vec<Element> = Vec::new();
        for layer in sym.timeline.layers.iter().rev() {
            if let Some((els, _)) =
                layer_elements_at(layer, inst.first_frame.min(sym.timeline.frame_count() - 1))
            {
                for mut e in els {
                    e.matrix = el.matrix.concat(&e.matrix);
                    e.alpha *= el.alpha;
                    parts.push(e);
                }
            }
        }
        if !self.layer_editable(s.layer) {
            return;
        }
        self.checkpoint("Break Apart");
        let Some(start) = self.target_keyframe(s.layer, false) else {
            return;
        };
        let els = &mut self.timeline_mut().layers[s.layer]
            .keyframes
            .get_mut(&start)
            .unwrap()
            .elements;
        if s.element >= els.len() {
            return;
        }
        els.remove(s.element);
        let n = parts.len();
        for (i, p) in parts.into_iter().enumerate() {
            els.insert(s.element + i, p);
        }
        let at = s.element;
        self.remap_bones(s.layer, |i| {
            if i == at {
                None
            } else if i > at {
                Some(i + n - 1)
            } else {
                Some(i)
            }
        });
        self.selection = (s.element..s.element + n)
            .map(|element| Selection {
                layer: s.layer,
                element,
            })
            .collect();
    }

    pub fn place_symbol(&mut self, id: SymbolId, at: Pt) {
        let Some(sym) = self.doc.symbol(id) else {
            return;
        };
        if self.editing == Some(id) {
            return; // can't place a symbol inside itself
        }
        // like Animate, new instances have no instance name until you give one
        let _ = sym;
        let el = Element::instance("", id, Matrix::translate(at.x, at.y));
        self.add_element(el);
    }

    // ---- bitmaps (File › Import)
    /// Add an imported bitmap to the library; returns its id.
    pub fn add_bitmap(&mut self, mut asset: crate::bitmap::BitmapAsset) -> crate::bitmap::BitmapId {
        self.checkpoint("Import");
        asset.id = self.doc.next_bitmap_id();
        let id = asset.id;
        self.doc.bitmaps.push(asset);
        id
    }
    /// Place a bitmap with its top-left at `at`.
    pub fn place_bitmap(&mut self, id: crate::bitmap::BitmapId, at: Pt) -> Option<Selection> {
        self.doc.bitmap(id)?;
        self.add_element(Element::bitmap("", id, Matrix::translate(at.x, at.y)))
    }
    pub fn rename_bitmap(&mut self, id: crate::bitmap::BitmapId, name: &str) {
        if self.doc.bitmap(id).is_some() {
            self.checkpoint("Rename Library Item");
            if let Some(b) = self.doc.bitmaps.iter_mut().find(|b| b.id == id) {
                b.name = name.into();
            }
        }
    }
    fn timelines_mut(&mut self) -> Vec<&mut Timeline> {
        let mut v: Vec<&mut Timeline> = self.doc.more_scenes.iter_mut().collect();
        v.push(&mut self.doc.scene);
        v.extend(self.doc.library.iter_mut().map(|s| &mut s.timeline));
        v
    }
    pub fn bitmap_usage(&self, id: crate::bitmap::BitmapId) -> usize {
        fn count(els: &[Element], id: crate::bitmap::BitmapId) -> usize {
            els.iter()
                .map(|e| match &e.kind {
                    ElementKind::Bitmap(b) if b.bitmap == id => 1,
                    ElementKind::Shape(s) if s.bitmap_fill.is_some_and(|b| b.bitmap == id) => 1,
                    ElementKind::Group(c) => count(c, id),
                    _ => 0,
                })
                .sum()
        }
        self.doc
            .scenes()
            .chain(self.doc.library.iter().map(|s| &s.timeline))
            .flat_map(|t| t.layers.iter())
            .flat_map(|l| l.keyframes.values())
            .map(|k| {
                count(&k.elements, id)
                    + k.tween_boundary
                        .as_ref()
                        .map_or(0, |b| count(&b.elements, id))
            })
            .sum()
    }
    /// Delete a library bitmap and every placed copy of it.
    pub fn delete_bitmap(&mut self, id: crate::bitmap::BitmapId) {
        self.checkpoint("Delete Library Item");
        fn strip(els: &mut Vec<Element>, id: crate::bitmap::BitmapId) {
            els.retain(|e| !matches!(&e.kind, ElementKind::Bitmap(b) if b.bitmap == id));
            for e in els {
                if let ElementKind::Shape(s) = &mut e.kind {
                    if s.bitmap_fill.is_some_and(|b| b.bitmap == id) {
                        s.bitmap_fill = None;
                    }
                }
                if let ElementKind::Group(c) = &mut e.kind {
                    strip(c, id);
                }
            }
        }
        for tl in self.timelines_mut() {
            for l in &mut tl.layers {
                let before: Vec<usize> = l.keyframes.values().map(|k| k.elements.len()).collect();
                for k in l.keyframes.values_mut() {
                    let retained: Vec<_> = k
                        .elements
                        .iter()
                        .enumerate()
                        .filter_map(|(i, e)| {
                            (!matches!(&e.kind, ElementKind::Bitmap(b) if b.bitmap == id))
                                .then_some(i)
                        })
                        .collect();
                    crate::tween_edit::remap_elements(k, &retained);
                    strip(&mut k.elements, id);
                    if let Some(b) = &mut k.tween_boundary {
                        strip(&mut b.elements, id);
                    }
                }
                let after: Vec<usize> = l.keyframes.values().map(|k| k.elements.len()).collect();
                if before != after {
                    l.bones.clear(); // indices changed; bones on that layer can't be trusted
                }
            }
        }
        self.doc.bitmaps.retain(|b| b.id != id);
        self.selection.clear();
    }

    pub fn set_bitmap_smoothing(&mut self, id: crate::bitmap::BitmapId, on: bool) {
        if self
            .doc
            .bitmap(id)
            .map(|b| b.smoothing != on)
            .unwrap_or(false)
        {
            self.checkpoint("Bitmap Properties");
            if let Some(b) = self.doc.bitmaps.iter_mut().find(|b| b.id == id) {
                b.smoothing = on;
            }
        }
    }

    /// Classic tween option: rotate with the motion guide's direction.
    pub fn set_orient_to_path(&mut self, on: bool) {
        self.for_each_layer("Orient to Path", |l, s| {
            let starts: Vec<u32> = l
                .keyframes
                .keys()
                .copied()
                .filter(|&k| k <= s.f1 && l.span_end(k) > s.f0)
                .collect();
            for k in starts {
                l.keyframes.get_mut(&k).unwrap().orient_to_path = on;
            }
        });
    }

    pub fn new_symbol(&mut self, name: &str, kind: SymbolKind) -> SymbolId {
        self.checkpoint("New Symbol");
        let mut tl = Timeline::new(name);
        if kind == SymbolKind::Button {
            // Up, Over, Down, Hit
            if let Some(l) = tl.layers.first_mut() {
                l.end = l.end.max(4);
            }
        }
        let id = self.doc.add_symbol(name, kind, tl);
        self.enter_symbol(id);
        id
    }

    /// Swap Symbol: selected instances show `id` instead, keeping transform and effects.
    pub fn swap_symbol(&mut self, id: SymbolId) {
        if self.doc.symbol(id).is_none() || self.editing == Some(id) {
            return;
        }
        self.modify_selected("Swap Symbol", move |e| {
            if let ElementKind::Instance(i) = &mut e.kind {
                i.symbol = id;
                i.first_frame = 0;
            }
        });
    }

    /// Topmost Button instance under `p` on the main timeline (Enable Simple
    /// Buttons): its Hit frame, or its Up frame without one, is the hit area.
    pub fn button_at(&self, p: Pt) -> Option<(usize, usize)> {
        let tl = self.timeline();
        let mut hit = None;
        for (li, layer) in tl.layers.iter().enumerate().rev() {
            if layer.kind != LayerKind::Normal || !layer_shown(tl, li) {
                continue;
            }
            let Some((els, _)) = layer_elements_at(layer, self.frame) else {
                continue;
            };
            for (ei, el) in els.iter().enumerate() {
                let ElementKind::Instance(inst) = &el.kind else {
                    continue;
                };
                let Some(sym) = self
                    .doc
                    .symbol(inst.symbol)
                    .filter(|s| s.kind == SymbolKind::Button)
                else {
                    continue;
                };
                let f = if sym.timeline.frame_count() > 3 { 3 } else { 0 };
                let ctx = EvalCtx::new(&self.doc, self.frame);
                let mut items = vec![];
                evaluate(&ctx, &sym.timeline, f, &el.matrix, 1.0, 1, &mut items);
                if items.iter().any(|it| {
                    let mut solid = it.clone();
                    solid.fill = solid.fill.or(Some(Color::rgb(0, 0, 0))); // hit areas count when invisible too
                    solid.hit(p)
                }) {
                    hit = Some((li, ei));
                }
            }
        }
        hit
    }

    pub fn rename_symbol(&mut self, id: SymbolId, name: &str) {
        self.checkpoint("Rename Symbol");
        if let Some(s) = self.doc.symbol_mut(id) {
            s.name = name.into();
            s.timeline.name = name.into();
        }
    }

    /// Number of instances of `id` anywhere in the document.
    pub fn symbol_usage(&self, id: SymbolId) -> usize {
        fn n(els: &[Element], id: SymbolId) -> usize {
            els.iter()
                .map(|e| match &e.kind {
                    ElementKind::Instance(i) if i.symbol == id => 1,
                    ElementKind::Group(c) => n(c, id),
                    _ => 0,
                })
                .sum()
        }
        let count = |tl: &Timeline| {
            tl.layers
                .iter()
                .flat_map(|l| l.keyframes.values())
                .map(|k| {
                    n(&k.elements, id) + k.tween_boundary.as_ref().map_or(0, |b| n(&b.elements, id))
                })
                .sum::<usize>()
        };
        self.doc.scenes().map(count).sum::<usize>()
            + self
                .doc
                .library
                .iter()
                .map(|s| count(&s.timeline))
                .sum::<usize>()
    }

    /// Delete a library item and every instance of it.
    pub fn delete_symbol(&mut self, id: SymbolId) {
        self.checkpoint("Delete Library Item");
        fn strip_els(els: &mut Vec<Element>, id: SymbolId) {
            els.retain(|e| !matches!(&e.kind, ElementKind::Instance(i) if i.symbol == id));
            for e in els {
                if let ElementKind::Group(c) = &mut e.kind {
                    strip_els(c, id);
                }
            }
        }
        let strip = |tl: &mut Timeline| {
            for l in &mut tl.layers {
                for k in l.keyframes.values_mut() {
                    let retained: Vec<_> = k
                        .elements
                        .iter()
                        .enumerate()
                        .filter_map(|(i, e)| {
                            (!matches!(&e.kind, ElementKind::Instance(s) if s.symbol == id))
                                .then_some(i)
                        })
                        .collect();
                    crate::tween_edit::remap_elements(k, &retained);
                    strip_els(&mut k.elements, id);
                    if let Some(b) = &mut k.tween_boundary {
                        strip_els(&mut b.elements, id);
                    }
                }
            }
        };
        for t in self.doc.scenes_mut() {
            strip(t);
        }
        for s in &mut self.doc.library {
            strip(&mut s.timeline);
        }
        self.doc.library.retain(|s| s.id != id);
        if self.editing == Some(id) {
            self.editing = None;
        }
        self.fix_indices();
    }

    pub fn duplicate_symbol(&mut self, id: SymbolId) -> Option<SymbolId> {
        let s = self.doc.symbol(id)?.clone();
        self.checkpoint("Duplicate Symbol");
        Some(
            self.doc
                .add_symbol(&format!("{} copy", s.name), s.kind, s.timeline),
        )
    }

    pub fn set_symbol_kind(&mut self, id: SymbolId, kind: SymbolKind) {
        self.checkpoint("Symbol Type");
        if let Some(s) = self.doc.symbol_mut(id) {
            s.kind = kind;
        }
    }

    pub fn set_doc_props(&mut self, w: f32, h: f32, fps: f32, bg: Color) {
        self.checkpoint("Document Settings");
        self.doc.width = w;
        self.doc.height = h;
        self.doc.fps = fps;
        self.doc.background = bg;
    }

    /// 3D perspective: field of view and vanishing point (`None` = stage centre).
    pub fn set_perspective(&mut self, angle: f32, vanishing: Option<Pt>) {
        self.checkpoint("Perspective");
        self.doc.perspective_angle = angle.clamp(1.0, 179.0);
        self.doc.vanishing_point = vanishing;
    }

    /// Paint bucket on an element: sets fill (shapes) or tint (instances).
    pub fn paint_bucket(&mut self, sel: Selection, color: Color) {
        self.selection = vec![sel];
        self.modify_selected("Paint Bucket", |el| match &mut el.kind {
            ElementKind::Shape(s) => s.fill = Some(color),
            ElementKind::Instance(_) | ElementKind::Bitmap(_) | ElementKind::Group(_) => {
                el.tint = Some((color, 0.6))
            }
            ElementKind::Text(t) => t.color = color,
            ElementKind::Canvas(c) => {
                let all: Vec<crate::merge::Part> =
                    (0..c.fills.len()).map(crate::merge::Part::Fill).collect();
                c.set_fill(&all, Some(color), None);
            }
        });
    }
}

// ------------------------------------------------------------ merge drawing
impl Editor {
    /// The merge shape (canvas) on `layer` at the playhead: (keyframe, element index).
    pub fn canvas_at(&self, layer: usize) -> Option<(u32, usize)> {
        let l = self.timeline().layers.get(layer)?;
        let (st, kf, _) = l.span_at(self.frame)?;
        let ei = kf
            .elements
            .iter()
            .position(|e| matches!(e.kind, ElementKind::Canvas(_)))?;
        Some((st, ei))
    }
    fn canvas_mut(
        &mut self,
        layer: usize,
        key: u32,
        ei: usize,
    ) -> Option<&mut crate::merge::MergeShape> {
        match &mut self
            .timeline_mut()
            .layers
            .get_mut(layer)?
            .keyframes
            .get_mut(&key)?
            .elements
            .get_mut(ei)?
            .kind
        {
            ElementKind::Canvas(c) => Some(c),
            _ => None,
        }
    }
    pub fn canvas(&self, layer: usize) -> Option<&crate::merge::MergeShape> {
        let (k, ei) = self.canvas_at(layer)?;
        match &self.timeline().layers[layer].keyframes[&k].elements[ei].kind {
            ElementKind::Canvas(c) => Some(c),
            _ => None,
        }
    }
    /// Stage → canvas coordinates on `layer`.
    fn canvas_inv(&self, layer: usize) -> Matrix {
        let (_, ei) = self.canvas_at(layer).unwrap_or((0, 0));
        self.element_at(Selection { layer, element: ei })
            .and_then(|e| e.matrix.invert())
            .unwrap_or(Matrix::IDENTITY)
    }
    /// Deselect: every floating merge selection in this timeline merges back.
    pub fn commit_floats(&mut self) -> bool {
        let mut any = false;
        for l in &mut self.timeline_mut().layers {
            for kf in l.keyframes.values_mut() {
                for el in &mut kf.elements {
                    if let ElementKind::Canvas(c) = &mut el.kind {
                        any |= c.commit_float();
                    }
                }
            }
        }
        any
    }
    /// Commit everything in every timeline (before saving).
    pub fn commit_all_floats(&mut self) {
        let doc = &mut self.doc;
        let mut tls: Vec<&mut Timeline> = std::iter::once(&mut doc.scene)
            .chain(doc.more_scenes.iter_mut())
            .collect();
        tls.extend(doc.library.iter_mut().map(|s| &mut s.timeline));
        for t in tls {
            for l in &mut t.layers {
                for kf in l.keyframes.values_mut() {
                    for el in &mut kf.elements {
                        if let ElementKind::Canvas(c) = &mut el.kind {
                            c.commit_float();
                        }
                    }
                }
            }
        }
    }
    /// Clear the stage selection (merging floating pieces back).
    pub fn deselect(&mut self) {
        self.commit_floats();
        self.selection.clear();
    }

    /// The canvas on the current layer for drawing, created (at the bottom) if needed.
    fn canvas_for_drawing(&mut self, li: usize) -> Option<(u32, usize)> {
        let key = self.target_keyframe(li, false)?;
        let l = &mut self.timeline_mut().layers[li];
        let kf = l.keyframes.get_mut(&key)?;
        if let Some(ei) = kf
            .elements
            .iter()
            .position(|e| matches!(e.kind, ElementKind::Canvas(_)))
        {
            return Some((key, ei));
        }
        let mut el = Element::shape("Shape", Shape::default());
        el.kind = ElementKind::Canvas(Default::default());
        kf.elements.insert(0, el);
        self.remap_bones(li, |i| Some(i + 1));
        Some((key, 0))
    }

    /// Draw in Merge Drawing mode (Object Drawing off): the path cuts and
    /// merges with the layer's shape. `path` is in stage coordinates.
    pub fn merge_draw(
        &mut self,
        path: &Path,
        fill: Option<Color>,
        gradient: Option<Gradient>,
        stroke: Option<Stroke>,
    ) -> bool {
        if self.layer_locked() {
            return false;
        }
        self.checkpoint("Draw Shape");
        self.commit_floats();
        let li = self.layer;
        let inv = self.canvas_inv(li);
        let Some((key, ei)) = self.canvas_for_drawing(li) else {
            return false;
        };
        let local = path.transformed(&if self.canvas_at(li).is_some() {
            inv
        } else {
            Matrix::IDENTITY
        });
        if let Some(c) = self.canvas_mut(li, key, ei) {
            c.add_path(&local, fill, gradient, stroke);
        }
        self.selection.clear();
        true
    }
    /// Brush in a paint mode; `area` polygons in stage coordinates; `start` is
    /// where the stroke began (Paint Inside).
    pub fn merge_paint(
        &mut self,
        area: &[Vec<Pt>],
        fill: Color,
        mode: crate::merge::PaintMode,
        start: Pt,
    ) -> bool {
        if self.layer_locked() {
            return false;
        }
        let li = self.layer;
        let inv = self.canvas_inv(li);
        let local: Vec<Vec<Pt>> = area
            .iter()
            .map(|c| c.iter().map(|x| inv.apply(*x)).collect())
            .collect();
        let selected = self.float_parts(li);
        self.checkpoint("Brush");
        // Paint Selection works on the floating selection's fills: merge it back first
        let sel_bounds = self.canvas(li).and_then(|c| {
            c.float.as_ref().map(|f| {
                f.fills
                    .iter()
                    .map(|r| r.contours.clone())
                    .collect::<Vec<_>>()
            })
        });
        self.commit_floats();
        let Some((key, ei)) = self.canvas_for_drawing(li) else {
            return false;
        };
        let Some(c) = self.canvas_mut(li, key, ei) else {
            return false;
        };
        let inside = c.fill_at(inv.apply(start));
        let sel: Vec<crate::merge::Part> = match (&sel_bounds, mode) {
            (Some(regions), crate::merge::PaintMode::Selection) => (0..c.fills.len())
                .filter(|&i| {
                    regions.iter().any(|r| {
                        r.first()
                            .and_then(|o| o.first())
                            .map(|x| c.fills[i].contains(*x))
                            .unwrap_or(false)
                    })
                })
                .map(crate::merge::Part::Fill)
                .collect(),
            _ => selected,
        };
        c.paint(&local, fill, mode, &sel, inside);
        true
    }
    /// Eraser in a mode over `area` (stage coordinates).
    pub fn merge_erase(
        &mut self,
        area: &[Vec<Pt>],
        mode: crate::merge::PaintMode,
        start: Pt,
        label: Option<&str>,
    ) -> bool {
        let li = self.layer;
        let Some((key, ei)) = self.canvas_at(li) else {
            return false;
        };
        if !self.layer_editable(li) {
            return false;
        }
        let inv = self.canvas_inv(li);
        let local: Vec<Vec<Pt>> = area
            .iter()
            .map(|c| c.iter().map(|x| inv.apply(*x)).collect())
            .collect();
        match label {
            Some(l) => self.checkpoint(l),
            None => self.revision += 1,
        }
        let Some(c) = self.canvas_mut(li, key, ei) else {
            return false;
        };
        if mode == crate::merge::PaintMode::Selection {
            if let Some(f) = c.float.as_mut() {
                let all: Vec<crate::merge::Part> =
                    (0..f.fills.len()).map(crate::merge::Part::Fill).collect();
                f.erase(&local, mode, &all, None);
            }
            return true;
        }
        let inside = c.fill_at(inv.apply(start));
        c.erase(&local, mode, &[], inside);
        true
    }
    /// Faucet: delete the fill or stroke segment at `p` (stage coordinates).
    pub fn merge_faucet(&mut self, p: Pt) -> bool {
        let li = self.layer;
        let Some((key, ei)) = self.canvas_at(li) else {
            return false;
        };
        if !self.layer_editable(li) {
            return false;
        }
        let x = self.canvas_inv(li).apply(p);
        let Some(part) = self.canvas(li).and_then(|c| c.hit(x, 3.0)) else {
            return false;
        };
        self.checkpoint("Faucet");
        if let Some(c) = self.canvas_mut(li, key, ei) {
            c.extract(&[part]);
        }
        true
    }
    fn float_parts(&self, li: usize) -> Vec<crate::merge::Part> {
        self.canvas(li)
            .and_then(|c| c.float.as_ref())
            .map(|f| (0..f.fills.len()).map(crate::merge::Part::Fill).collect())
            .unwrap_or_default()
    }

    /// The merge part under `p` on an editable layer: (layer, part, is it already selected).
    pub fn merge_part_at(
        &self,
        p: Pt,
        tol: f32,
    ) -> Option<(usize, Option<crate::merge::Part>, bool)> {
        let hit = self.hit_test(p)?;
        let el = self.element_at(hit)?;
        let ElementKind::Canvas(c) = &el.kind else {
            return None;
        };
        let x = el.matrix.invert()?.apply(p);
        if c.float_hit(x, tol) {
            return Some((hit.layer, None, true));
        }
        Some((hit.layer, c.hit(x, tol), false))
    }
    /// Click a merge part: lift it (double-click: with its connected strokes)
    /// into the floating selection. `add` = Shift.
    pub fn merge_select(
        &mut self,
        layer: usize,
        part: crate::merge::Part,
        connected: bool,
        add: bool,
    ) {
        let Some((key, ei)) = self.canvas_at(layer) else {
            return;
        };
        if !add {
            self.commit_floats();
            self.selection.clear();
        }
        self.revision += 1;
        if let Some(c) = self.canvas_mut(layer, key, ei) {
            let parts = if connected {
                c.connected(part)
            } else {
                vec![part]
            };
            c.lift(&parts, true);
        }
        let s = Selection { layer, element: ei };
        if !self.selection.contains(&s) {
            self.selection.push(s);
            self.selection.sort();
        }
        self.layer = layer;
        self.frame_sel = None;
    }
    /// Marquee / lasso over merge shapes: cuts out what is inside and selects it.
    pub fn merge_select_area(&mut self, area: &[Vec<Pt>], add: bool) -> bool {
        let mut any = false;
        for li in 0..self.timeline().layers.len() {
            if !self.layer_editable(li) {
                continue;
            }
            let Some((key, ei)) = self.canvas_at(li) else {
                continue;
            };
            let inv = self.canvas_inv(li);
            let local: Vec<Vec<Pt>> = area
                .iter()
                .map(|c| c.iter().map(|x| inv.apply(*x)).collect())
                .collect();
            let Some(c) = self.canvas_mut(li, key, ei) else {
                continue;
            };
            if !add {
                c.commit_float();
            }
            let piece = c.extract_area(&local);
            if piece.fills.is_empty() && piece.strokes.is_empty() {
                continue;
            }
            c.add_to_float(piece);
            any = true;
            let s = Selection {
                layer: li,
                element: ei,
            };
            if !self.selection.contains(&s) {
                self.selection.push(s);
            }
        }
        if any {
            self.revision += 1;
            self.selection.sort();
        }
        any
    }
    /// Select All on a merge shape lifts all of it.
    fn lift_all_canvases(&mut self) {
        for li in 0..self.timeline().layers.len() {
            if !self.layer_editable(li) {
                continue;
            }
            let Some((key, ei)) = self.canvas_at(li) else {
                continue;
            };
            if let Some(c) = self.canvas_mut(li, key, ei) {
                let parts: Vec<crate::merge::Part> = (0..c.fills.len())
                    .map(crate::merge::Part::Fill)
                    .chain((0..c.strokes.len()).map(crate::merge::Part::Stroke))
                    .collect();
                c.lift(&parts, true);
            }
        }
    }
    /// Change the fill/stroke of the floating merge selection (Properties, bucket, ink bottle).
    pub fn merge_set_paint(
        &mut self,
        fill: Option<Option<Color>>,
        gradient: Option<Option<Gradient>>,
        stroke: Option<Option<Stroke>>,
    ) {
        let layers: Vec<usize> = self.selection.iter().map(|s| s.layer).collect();
        self.checkpoint("Fill and Stroke");
        for li in layers {
            let Some((key, ei)) = self.canvas_at(li) else {
                continue;
            };
            let Some(c) = self.canvas_mut(li, key, ei) else {
                continue;
            };
            let Some(f) = c.float.as_mut() else { continue };
            if fill.is_some() || gradient.is_some() {
                let all: Vec<crate::merge::Part> =
                    (0..f.fills.len()).map(crate::merge::Part::Fill).collect();
                let cur = f
                    .fills
                    .first()
                    .map(|r| (r.fill, r.gradient.clone()))
                    .unwrap_or((None, None));
                f.set_fill(
                    &all,
                    fill.unwrap_or(cur.0),
                    gradient.clone().unwrap_or(cur.1),
                );
            }
            if let Some(st) = stroke {
                match st {
                    Some(st) => {
                        let all: Vec<crate::merge::Part> = (0..f.strokes.len())
                            .map(crate::merge::Part::Stroke)
                            .collect();
                        f.set_stroke(&all, st);
                    }
                    None => f.strokes.clear(),
                }
            }
        }
    }
    /// The floating merge selection (for Properties): (layer, piece).
    pub fn merge_float(&self) -> Option<(usize, &crate::merge::MergeShape)> {
        self.selection.iter().find_map(|s| {
            self.canvas(s.layer)
                .and_then(|c| c.float.as_deref())
                .map(|f| (s.layer, f))
        })
    }

    /// Paint Bucket on merge content: recolour the fill under `p`, or fill
    /// the area the strokes enclose (closing gaps up to `gap`).
    pub fn merge_bucket(&mut self, p: Pt, color: Color, gap: f32) -> bool {
        let li = self.layer;
        if !self.layer_editable(li) {
            return false;
        }
        let x = self.canvas_inv(li).apply(p);
        let Some((key, ei)) = self.canvas_at(li) else {
            return false;
        };
        let target = self
            .canvas(li)
            .map(|c| (c.fill_at(x), !c.strokes.is_empty()));
        match target {
            Some((Some(i), _)) => {
                self.checkpoint("Paint Bucket");
                self.commit_floats();
                if let Some(c) = self.canvas_mut(li, key, ei) {
                    c.set_fill(&[crate::merge::Part::Fill(i)], Some(color), None);
                }
                true
            }
            Some((None, true)) => {
                let mut c2 = self.canvas(li).unwrap().clone();
                c2.commit_float();
                if !c2.fill_enclosed(x, color, gap) {
                    return false;
                }
                self.checkpoint("Paint Bucket");
                if let Some(c) = self.canvas_mut(li, key, ei) {
                    *c = c2;
                }
                true
            }
            _ => false,
        }
    }
    /// Ink Bottle on merge content: restyle the stroke under `p`.
    pub fn merge_ink(&mut self, p: Pt, stroke: Stroke) -> bool {
        let li = self.layer;
        let x = self.canvas_inv(li).apply(p);
        let Some((key, ei)) = self.canvas_at(li) else {
            return false;
        };
        let Some(crate::merge::Part::Stroke(i)) = self.canvas(li).and_then(|c| c.hit(x, 3.0))
        else {
            return false;
        };
        self.checkpoint("Ink Bottle");
        if let Some(c) = self.canvas_mut(li, key, ei) {
            c.set_stroke(&[crate::merge::Part::Stroke(i)], stroke);
        }
        true
    }

    /// Selected merge-shape pieces become their own merge-shape elements (on
    /// the same keyframe) so whole-object commands (group, symbol, copy) work on them.
    pub fn detach_floats(&mut self) {
        let sels = self.selection.clone();
        let mut out = vec![];
        for s in sels {
            let piece = self
                .canvas_at(s.layer)
                .filter(|(_, ei)| *ei == s.element)
                .and_then(|(key, ei)| {
                    self.canvas_mut(s.layer, key, ei)
                        .and_then(|c| c.float.take())
                        .map(|f| (key, f))
                });
            match piece {
                Some((key, f)) => {
                    let mut el = Element::shape("Shape", Shape::default());
                    el.matrix =
                        self.timeline().layers[s.layer].keyframes[&key].elements[s.element].matrix;
                    el.kind = ElementKind::Canvas(*f);
                    let kf = self.timeline_mut().layers[s.layer]
                        .keyframes
                        .get_mut(&key)
                        .unwrap();
                    kf.elements.push(el);
                    out.push(Selection {
                        layer: s.layer,
                        element: kf.elements.len() - 1,
                    });
                }
                None => out.push(s),
            }
        }
        out.sort();
        self.selection = out;
    }

    /// Floating merge selection → one drawing object (Modify ▸ Combine
    /// Objects ▸ Union on a shape selection, or Ctrl+G).
    pub fn merge_to_object(&mut self) -> bool {
        let Some(s) = self.selection.iter().copied().find(|s| {
            self.canvas(s.layer)
                .map(|c| c.float.is_some())
                .unwrap_or(false)
        }) else {
            return false;
        };
        let Some((key, ei)) = self.canvas_at(s.layer) else {
            return false;
        };
        let m = self
            .element_at(Selection {
                layer: s.layer,
                element: ei,
            })
            .map(|e| e.matrix)
            .unwrap_or(Matrix::IDENTITY);
        self.checkpoint("Convert to Drawing Object");
        let Some(piece) = self
            .canvas_mut(s.layer, key, ei)
            .and_then(|c| c.float.take())
        else {
            return false;
        };
        let shapes = piece.to_shapes();
        let el = if shapes.len() == 1 {
            Element {
                matrix: m,
                ..Element::shape("Drawing Object", shapes.into_iter().next().unwrap())
            }
        } else {
            Element {
                matrix: m,
                ..Element::group(
                    shapes
                        .into_iter()
                        .map(|sh| Element::shape("Shape", sh))
                        .collect(),
                )
            }
        };
        let kf = self.timeline_mut().layers[s.layer]
            .keyframes
            .get_mut(&key)
            .unwrap();
        kf.elements.push(el);
        let n = kf.elements.len() - 1;
        self.selection = vec![Selection {
            layer: s.layer,
            element: n,
        }];
        true
    }

    /// Break Apart on a drawing object: it becomes part of the layer's merge shape.
    fn shape_to_merge(&mut self, s: Selection) -> bool {
        let Some(el) = self.element_at(s) else {
            return false;
        };
        let ElementKind::Shape(sh) = &el.kind else {
            return false;
        };
        let path = sh.path.transformed(&el.matrix);
        let (fill, grad, stroke) = (sh.fill, sh.gradient.clone(), sh.stroke);
        self.checkpoint("Break Apart");
        let Some(key) = self.target_keyframe(s.layer, false) else {
            return false;
        };
        self.timeline_mut().layers[s.layer]
            .keyframes
            .get_mut(&key)
            .unwrap()
            .elements
            .remove(s.element);
        let at = s.element;
        self.remap_bones(s.layer, |i| {
            if i == at {
                None
            } else if i > at {
                Some(i - 1)
            } else {
                Some(i)
            }
        });
        let li = s.layer;
        let inv = self.canvas_inv(li);
        let has = self.canvas_at(li).is_some();
        let Some((key, ei)) = self.canvas_for_drawing(li) else {
            return false;
        };
        let local = path.transformed(&if has { inv } else { Matrix::IDENTITY });
        if let Some(c) = self.canvas_mut(li, key, ei) {
            c.commit_float();
            c.add_path(&local, fill, grad, stroke);
        }
        self.selection.clear();
        true
    }

    /// Modify ▸ Combine Objects on the selected drawing objects (same layer).
    pub fn combine_objects(&mut self, op: crate::merge::BoolOp) -> Result<(), String> {
        use crate::merge::{bool_polys, path_polygons, polys_path, BoolOp};
        let mut sels: Vec<Selection> = self.selection.clone();
        sels.retain(|s| {
            matches!(
                self.element_at(*s).map(|e| e.kind),
                Some(ElementKind::Shape(_))
            )
        });
        if sels.len() < 2 {
            return Err(
                "select two or more drawing objects (shapes drawn with Object Drawing on)".into(),
            );
        }
        let li = sels[0].layer;
        if sels.iter().any(|s| s.layer != li) {
            return Err("the objects must be on the same layer".into());
        }
        sels.sort();
        let shapes: Vec<(Vec<Vec<Pt>>, Shape)> = sels
            .iter()
            .filter_map(|s| self.element_at(*s))
            .filter_map(|e| {
                if let ElementKind::Shape(sh) = &e.kind {
                    Some((path_polygons(&sh.path.transformed(&e.matrix)), sh.clone()))
                } else {
                    None
                }
            })
            .collect();
        let (top_polys, top) = shapes.last().cloned().unwrap();
        let mut results: Vec<Shape> = vec![];
        match op {
            BoolOp::Union => {
                // same paint → one shape; otherwise the visible pieces, merged per colour
                let mut m = crate::merge::MergeShape::default();
                for (polys, sh) in &shapes {
                    m.add_fill(polys, sh.fill, sh.gradient.clone(), true);
                }
                if shapes
                    .iter()
                    .all(|(_, sh)| sh.fill == top.fill && sh.gradient == top.gradient)
                {
                    let all: Vec<Vec<Pt>> =
                        m.fills.iter().flat_map(|r| r.contours.clone()).collect();
                    results.push(Shape {
                        path: polys_path(&all),
                        ..top.clone()
                    });
                } else {
                    results.extend(m.fills.iter().map(|r| Shape {
                        path: r.path(),
                        fill: r.fill,
                        gradient: r.gradient.clone(),
                        stroke: top.stroke,
                        ..Default::default()
                    }));
                }
            }
            BoolOp::Intersect => {
                let mut acc = shapes[0].0.clone();
                for (polys, _) in &shapes[1..] {
                    acc = bool_polys(&acc, polys, BoolOp::Intersect);
                }
                if !acc.is_empty() {
                    results.push(Shape {
                        path: polys_path(&acc),
                        width_profile: None,
                        warp: None,
                        ..top.clone()
                    });
                }
            }
            BoolOp::Punch | BoolOp::Crop => {
                for (polys, sh) in &shapes[..shapes.len() - 1] {
                    let r = bool_polys(polys, &top_polys, op);
                    if !r.is_empty() {
                        results.push(Shape {
                            path: polys_path(&r),
                            width_profile: None,
                            warp: None,
                            ..sh.clone()
                        });
                    }
                }
            }
        }
        self.checkpoint(match op {
            BoolOp::Union => "Union",
            BoolOp::Intersect => "Intersect",
            BoolOp::Punch => "Punch",
            BoolOp::Crop => "Crop",
        });
        let Some(key) = self.target_keyframe(li, false) else {
            return Ok(());
        };
        let at = sels[0].element;
        {
            let els = &mut self.timeline_mut().layers[li]
                .keyframes
                .get_mut(&key)
                .unwrap()
                .elements;
            for s in sels.iter().rev() {
                if s.element < els.len() {
                    els.remove(s.element);
                }
            }
            for (i, sh) in results.iter().enumerate() {
                els.insert(
                    (at + i).min(els.len()),
                    Element::shape("Drawing Object", sh.clone()),
                );
            }
        }
        let removed: Vec<usize> = sels.iter().map(|s| s.element).collect();
        let n = results.len();
        self.remap_bones(li, |i| {
            if removed.contains(&i) {
                None
            } else {
                Some(i - removed.iter().filter(|&&r| r < i).count() + if i >= at { n } else { 0 })
            }
        });
        self.selection = (at..at + n)
            .map(|element| Selection { layer: li, element })
            .collect();
        Ok(())
    }

    /// Modify ▸ Shape ▸ Smooth / Straighten / Optimize on drawing objects and
    /// the floating merge selection.
    pub fn shape_cleanup(&mut self, kind: &str) {
        let label = match kind {
            "smooth" => "Smooth",
            "straighten" => "Straighten",
            _ => "Optimize Curves",
        };
        if self.selection.is_empty() {
            return;
        }
        let k = kind.to_string();
        self.modify_selected(label, move |e| match &mut e.kind {
            ElementKind::Shape(sh) => {
                let closed = sh.fill.is_some() || sh.gradient.is_some();
                let mut segs = vec![];
                for (pts, c) in sh.path.flatten(16) {
                    let size = pts.iter().fold((f32::MAX, f32::MIN), |a, p| {
                        (a.0.min(p.x.min(p.y)), a.1.max(p.x.max(p.y)))
                    });
                    let span = (size.1 - size.0).max(1.0);
                    let sub = match k.as_str() {
                        "smooth" => Path::smooth_polyline(&pts, span * 0.02),
                        "straighten" => Path::polygon(&crate::geom::simplify(&pts, span * 0.03)),
                        _ => Path::smooth_polyline(&pts, span * 0.008),
                    };
                    let mut ss = sub.segs;
                    if !(c || closed) {
                        ss.retain(|x| !matches!(x, crate::geom::Seg::Z));
                    } else if !ss.iter().any(|x| matches!(x, crate::geom::Seg::Z)) {
                        ss.push(crate::geom::Seg::Z);
                    }
                    segs.extend(ss);
                }
                if !segs.is_empty() {
                    sh.path = Path { segs };
                }
            }
            ElementKind::Canvas(c) => {
                if let Some(f) = c.float.as_mut() {
                    match k.as_str() {
                        "smooth" => f.smooth(),
                        "straighten" => f.straighten(),
                        _ => f.optimize(0.75),
                    }
                }
            }
            _ => {}
        });
    }

    /// Start bending an edge or dragging a corner of a merge shape at `p`.
    pub fn merge_begin_bend(
        &self,
        p: Pt,
        tol: f32,
    ) -> Option<(usize, u32, usize, crate::merge::Bend, Matrix)> {
        for li in 0..self.timeline().layers.len() {
            if !self.layer_editable(li) {
                continue;
            }
            let Some((key, ei)) = self.canvas_at(li) else {
                continue;
            };
            let inv = self.canvas_inv(li);
            let c = self.canvas(li)?;
            if let Some(b) = c.begin_bend(inv.apply(p), tol) {
                return Some((li, key, ei, b, inv));
            }
        }
        None
    }
    /// Apply a bend (during the drag; `first` makes the undo step).
    pub fn merge_apply_bend(
        &mut self,
        li: usize,
        key: u32,
        ei: usize,
        shape: crate::merge::MergeShape,
        first: bool,
    ) {
        if first {
            self.checkpoint("Bend");
        } else {
            self.revision += 1;
        }
        if let Some(c) = self.canvas_mut(li, key, ei) {
            *c = shape;
        }
    }
    /// After a bend: resolve overlaps the deformation created.
    pub fn merge_finish_bend(&mut self, li: usize, key: u32, ei: usize) {
        if let Some(c) = self.canvas_mut(li, key, ei) {
            c.resolve();
        }
    }
}

// ------------------------------------------------------------ shape armatures
impl Editor {
    /// Posed bones of every shape armature on the stage: (shape, bone, head, tail) in stage coordinates.
    pub fn rig_bones(&self) -> Vec<(Selection, usize, Pt, Pt)> {
        let mut out = vec![];
        for (li, l) in self.timeline().layers.iter().enumerate() {
            if !l.visible || l.kind != LayerKind::Normal {
                continue;
            }
            let Some((els, _)) = layer_elements_at(l, self.frame) else {
                continue;
            };
            for ei in 0..els.len() {
                let s = Selection {
                    layer: li,
                    element: ei,
                };
                let Some(el) = self.element_at(s) else {
                    continue;
                };
                if let ElementKind::Shape(Shape { rig: Some(r), .. }) = &el.kind {
                    for (bi, (h, t)) in r.posed().into_iter().enumerate() {
                        out.push((s, bi, el.matrix.apply(h), el.matrix.apply(t)));
                    }
                }
            }
        }
        out
    }
    /// Bone tool inside one shape: a bone from `head` to `tail` (stage coordinates).
    pub fn rig_add_bone(&mut self, s: Selection, head: Pt, tail: Pt) -> Option<usize> {
        let el = self.element_at(s)?;
        if !matches!(el.kind, ElementKind::Shape(_)) || !self.layer_editable(s.layer) {
            return None;
        }
        let inv = el.matrix.invert()?;
        let (h, t) = (inv.apply(head), inv.apply(tail));
        let snap = 6.0 / el.matrix.decompose().scale_x.abs().max(0.01);
        let out = std::cell::Cell::new(None);
        self.selection = vec![s];
        self.modify_selected("Add Shape Bone", |e| {
            if let ElementKind::Shape(sh) = &mut e.kind {
                if sh.rig.is_none() {
                    // enough points to bend smoothly
                    let size = sh
                        .path
                        .bounds()
                        .map(|(lo, hi)| (hi.x - lo.x).max(hi.y - lo.y))
                        .unwrap_or(100.0);
                    sh.path = crate::rig::densify(&sh.path, (size / 24.0).max(2.0));
                    sh.primitive = None;
                    sh.warp = None;
                }
                let r = sh.rig.get_or_insert_with(Default::default);
                out.set(Some(r.add_bone(h, t, snap)));
            }
        });
        out.get()
    }
    /// Pose a shape armature: drag the tail of `bone` toward `target` (IK).
    /// Mid-tween this makes a pose keyframe; call `begin_edit` first for one undo step.
    pub fn rig_ik(&mut self, s: Selection, bone: usize, target: Pt) {
        let Some(el) = self.element_at(s) else { return };
        let Some(inv) = el.matrix.invert() else {
            return;
        };
        let t = inv.apply(target);
        self.selection = vec![s];
        self.modify_selected_continuous(|e| {
            if let ElementKind::Shape(Shape { rig: Some(r), .. }) = &mut e.kind {
                if bone < r.bones.len() {
                    r.ik(bone, t);
                }
            }
        });
    }
    /// Bind tool: point `idx` (in path order) follows `bone` (None = automatic).
    pub fn rig_bind(&mut self, s: Selection, idx: usize, bone: Option<usize>) {
        self.selection = vec![s];
        self.modify_selected(
            if bone.is_some() {
                "Bind Point"
            } else {
                "Unbind Point"
            },
            |e| {
                if let ElementKind::Shape(Shape { rig: Some(r), .. }) = &mut e.kind {
                    r.set_bind(idx, bone);
                }
            },
        );
    }
    /// Remove a shape's armature (keeps the posed shape).
    pub fn rig_clear(&mut self, s: Selection) {
        self.selection = vec![s];
        self.modify_selected("Remove Shape Bones", |e| {
            if let ElementKind::Shape(sh) = &mut e.kind {
                if let Some(r) = sh.rig.take() {
                    sh.path = r.deform(&sh.path);
                }
            }
        });
    }
}

// ------------------------------------------------------------ bones (IK)
impl Editor {
    pub fn bones(&self, layer: usize) -> &[Bone] {
        self.timeline()
            .layers
            .get(layer)
            .map(|l| l.bones.as_slice())
            .unwrap_or(&[])
    }

    /// Re-index bones after objects on a layer were removed/reordered;
    /// bones touching a removed object are dropped.
    pub fn remap_bones(&mut self, layer: usize, f: impl Fn(usize) -> Option<usize>) {
        let Some(l) = self.timeline_mut().layers.get_mut(layer) else {
            return;
        };
        if l.bones.is_empty() {
            return;
        }
        l.bones = l
            .bones
            .iter()
            .filter_map(|b| {
                Some(Bone {
                    parent: f(b.parent)?,
                    child: f(b.child)?,
                    ..*b
                })
            })
            .collect();
    }

    /// Objects connected below `e` (including `e`).
    pub fn bone_subtree(bones: &[Bone], e: usize) -> Vec<usize> {
        let mut out = vec![e];
        let mut i = 0;
        while i < out.len() {
            let cur = out[i];
            for b in bones.iter().filter(|b| b.parent == cur) {
                if !out.contains(&b.child) {
                    out.push(b.child);
                }
            }
            i += 1;
        }
        out
    }

    /// Joint an object rotates around: where it attaches to its parent, or
    /// (for the root) the head of its first bone.
    pub fn bone_pivot(bones: &[Bone], els: &[Element], e: usize) -> Option<Pt> {
        if let Some(b) = bones.iter().find(|b| b.child == e) {
            return els.get(e).map(|el| el.matrix.apply(b.tail_child));
        }
        bones
            .iter()
            .find(|b| b.parent == e)
            .and_then(|b| els.get(e).map(|el| el.matrix.apply(b.head)))
    }

    /// Bone tool: link `parent` → `child` with a bone from `head` to `tail` (doc coords).
    pub fn add_bone(
        &mut self,
        layer: usize,
        parent: usize,
        child: usize,
        head: Pt,
        tail: Pt,
    ) -> Result<(), String> {
        if parent == child {
            return Err("drag from one object to a different one".into());
        }
        if !self.layer_editable(layer) {
            return Err("the layer is locked or hidden".into());
        }
        let bones = self.bones(layer).to_vec();
        if bones.iter().any(|b| b.child == child) {
            return Err("that object is already attached to a bone".into());
        }
        if Self::bone_subtree(&bones, child).contains(&parent) {
            return Err("that would make a loop in the armature".into());
        }
        let l = &self.timeline().layers[layer];
        let (els, _) = layer_elements_at(l, self.frame).ok_or("nothing on this frame")?;
        let pm = els
            .get(parent)
            .ok_or("no such object")?
            .matrix
            .invert()
            .ok_or("object has no size")?;
        let cm = els
            .get(child)
            .ok_or("no such object")?
            .matrix
            .invert()
            .ok_or("object has no size")?;
        let bone = Bone {
            parent,
            child,
            head: pm.apply(head),
            tail_parent: pm.apply(tail),
            tail_child: cm.apply(tail),
        };
        self.checkpoint("Add Bone");
        self.timeline_mut().layers[layer].bones.push(bone);
        Ok(())
    }

    /// Remove the bone that attaches `child` to its parent.
    pub fn remove_bone(&mut self, layer: usize, child: usize) {
        if !self.bones(layer).iter().any(|b| b.child == child) {
            return;
        }
        self.checkpoint("Remove Bone");
        self.timeline_mut().layers[layer]
            .bones
            .retain(|b| b.child != child);
    }

    pub fn clear_bones(&mut self, layer: usize) {
        if self.bones(layer).is_empty() {
            return;
        }
        self.checkpoint("Remove Bones");
        self.timeline_mut().layers[layer].bones.clear();
    }

    /// Inverse kinematics: pull the point `grab` (local to `element`) towards
    /// `target`, rotating the chain of parents (CCD). Dragging an armature's
    /// root moves the whole armature. Writes a pose keyframe at the playhead.
    /// Call `begin_drag` first so a drag is one undo step.
    pub fn ik_drag(&mut self, layer: usize, element: usize, grab: Pt, target: Pt) {
        let f = self.frame;
        let Some(l) = self.timeline().layers.get(layer) else {
            return;
        };
        let bones = l.bones.clone();
        let Some((mut els, _)) = layer_elements_at(l, f) else {
            return;
        };
        if element >= els.len() {
            return;
        }
        let mut chain = vec![element];
        while let Some(b) = bones.iter().find(|b| b.child == *chain.last().unwrap()) {
            if chain.contains(&b.parent) {
                break;
            }
            chain.push(b.parent);
        }
        let root = *chain.last().unwrap();
        let affected = Self::bone_subtree(&bones, root);
        if chain.len() == 1 {
            let eff = els[element].matrix.apply(grab);
            for &y in &affected {
                els[y].matrix.tx += target.x - eff.x;
                els[y].matrix.ty += target.y - eff.y;
            }
        } else {
            for _ in 0..16 {
                for &x in &chain {
                    let Some(j) = Self::bone_pivot(&bones, &els, x) else {
                        continue;
                    };
                    let eff = els[element].matrix.apply(grab);
                    let a1 = (eff.y - j.y).atan2(eff.x - j.x);
                    let a2 = (target.y - j.y).atan2(target.x - j.x);
                    let mut d = a2 - a1;
                    while d > std::f32::consts::PI {
                        d -= std::f32::consts::TAU;
                    }
                    while d < -std::f32::consts::PI {
                        d += std::f32::consts::TAU;
                    }
                    if eff.dist(j) < 1e-3 {
                        continue;
                    }
                    let r = Matrix::translate(j.x, j.y)
                        .concat(&Matrix::rotate(d))
                        .concat(&Matrix::translate(-j.x, -j.y));
                    for y in Self::bone_subtree(&bones, x) {
                        els[y].matrix = r.concat(&els[y].matrix);
                    }
                }
                if els[element].matrix.apply(grab).dist(target) < 0.25 {
                    break;
                }
            }
        }
        apply_bones(&bones, &mut els);
        let l = &mut self.timeline_mut().layers[layer];
        if !l.keyframes.contains_key(&f) {
            frames::convert_to_keyframes(l, f, f, false);
        }
        if let Some(kf) = l.keyframes.get_mut(&f) {
            for y in affected {
                if let (Some(dst), Some(src)) = (kf.elements.get_mut(y), els.get(y)) {
                    dst.matrix = src.matrix;
                }
            }
        }
    }
}

// ------------------------------------------------------------ camera
impl Editor {
    /// Index of the scene's camera layer, if any.
    pub fn camera_layer(&self) -> Option<usize> {
        self.scene_tl()
            .layers
            .iter()
            .position(|l| l.kind == LayerKind::Camera)
    }

    /// Add a camera layer (top of the scene) if there isn't one.
    pub fn add_camera(&mut self) -> usize {
        if let Some(i) = self.camera_layer() {
            return i;
        }
        self.checkpoint("Add Camera");
        let end = self.scene_tl().frame_count();
        let mut l = Layer::new("Camera", 5);
        l.kind = LayerKind::Camera;
        l.end = end;
        l.keyframes
            .insert(0, Keyframe::with(vec![default_camera(&self.doc)]));
        self.scene_tl_mut().layers.insert(0, l);
        if self.editing.is_none() {
            self.layer = (self.layer + 1).min(self.scene_tl().layers.len() - 1);
        }
        self.fix_layer_selection();
        0
    }

    pub fn remove_camera(&mut self) {
        if let Some(i) = self.camera_layer() {
            self.checkpoint("Remove Camera");
            self.scene_tl_mut().layers.remove(i);
            self.fix_layer_selection();
        }
    }

    fn fix_layer_selection(&mut self) {
        let n = self.timeline().layers.len();
        self.layer = self.layer.min(n.saturating_sub(1));
        self.selection.clear();
        self.frame_sel = None;
    }

    /// The camera element's matrix at `frame` (centre, rotation, zoom).
    pub fn camera_at(&self, frame: u32) -> Option<Matrix> {
        let l = &self.scene_tl().layers[self.camera_layer()?];
        layer_elements_at(l, frame).and_then(|(e, _)| e.first().map(|c| c.matrix))
    }

    /// Set the camera at the playhead (makes a camera keyframe). No undo step:
    /// call `begin_drag` (or `checkpoint` via a command) first.
    pub fn set_camera(&mut self, cam: Matrix) {
        let f = self.frame;
        let Some(i) = self.camera_layer() else { return };
        let doc_cam = default_camera(&self.doc);
        let l = &mut self.scene_tl_mut().layers[i];
        if !l.keyframes.contains_key(&f) {
            frames::convert_to_keyframes(l, f, f, false);
        }
        let kf = l.keyframes.get_mut(&f).unwrap();
        if kf.elements.is_empty() {
            kf.elements.push(doc_cam);
        }
        kf.elements[0].matrix = cam;
    }

    pub fn camera_params(cam: &Matrix) -> (Pt, f32, f32) {
        let d = cam.decompose();
        (pt(cam.tx, cam.ty), d.skew_y.to_degrees(), d.scale_x.abs())
    }
    pub fn camera_from(center: Pt, rotation_deg: f32, zoom: f32) -> Matrix {
        Matrix::translate(center.x, center.y)
            .concat(&Matrix::rotate(rotation_deg.to_radians()))
            .concat(&Matrix::scale(zoom.max(0.01), zoom.max(0.01)))
    }
}

/// Align panel operations (Modify › Align, Ctrl+K).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlignOp {
    Left,
    HCenter,
    Right,
    Top,
    VCenter,
    Bottom,
    DistLeft,
    DistHCenter,
    DistRight,
    DistTop,
    DistVCenter,
    DistBottom,
    MatchWidth,
    MatchHeight,
    MatchSize,
    SpaceH,
    SpaceV,
}

// ------------------------------------------------------------ groups, align, layer tree
impl Editor {
    /// Modify › Group (Ctrl+G): selected objects on each layer become one group.
    pub fn group_selected(&mut self) -> usize {
        self.detach_floats();
        let sels: Vec<Selection> = self
            .selection
            .iter()
            .copied()
            .filter(|s| self.layer_editable(s.layer))
            .collect();
        if sels.is_empty() {
            return 0;
        }
        self.checkpoint("Group");
        let mut layers: Vec<usize> = sels.iter().map(|s| s.layer).collect();
        layers.sort();
        layers.dedup();
        let mut new_sel = Vec::new();
        for li in layers {
            let Some(start) = self.target_keyframe(li, true) else {
                continue;
            };
            let mut idx: Vec<usize> = sels
                .iter()
                .filter(|s| s.layer == li)
                .map(|s| s.element)
                .collect();
            idx.sort();
            let els = &mut self.timeline_mut().layers[li]
                .keyframes
                .get_mut(&start)
                .unwrap()
                .elements;
            idx.retain(|i| *i < els.len());
            if idx.is_empty() {
                continue;
            }
            let at = idx[0];
            let mut children = Vec::new();
            for i in idx.iter().rev() {
                children.push(els.remove(*i));
            }
            children.reverse();
            els.insert(at, Element::group(children));
            let gone = idx.clone();
            self.remap_bones(li, move |i| {
                if gone.contains(&i) {
                    None
                } else {
                    Some(i - gone.iter().filter(|g| **g < i).count() + usize::from(i > at))
                }
            });
            new_sel.push(Selection {
                layer: li,
                element: at,
            });
        }
        let n = new_sel.len();
        self.selection = new_sel;
        n
    }

    /// Modify › Ungroup (Ctrl+Shift+G); Break Apart also ungroups.
    pub fn ungroup_selected(&mut self) -> usize {
        let mut sels: Vec<Selection> = self
            .selection
            .iter()
            .copied()
            .filter(|s| {
                self.layer_editable(s.layer)
                    && matches!(
                        self.element_at(*s).map(|e| e.kind),
                        Some(ElementKind::Group(_))
                    )
            })
            .collect();
        if sels.is_empty() {
            return 0;
        }
        self.checkpoint("Ungroup");
        sels.sort_by(|a, b| b.cmp(a)); // last first so indices stay valid
        let mut new_sel = Vec::new();
        let mut count = 0;
        for s in sels {
            let Some(start) = self.target_keyframe(s.layer, true) else {
                continue;
            };
            let els = &mut self.timeline_mut().layers[s.layer]
                .keyframes
                .get_mut(&start)
                .unwrap()
                .elements;
            let Some(g) = els.get(s.element).cloned() else {
                continue;
            };
            let ElementKind::Group(children) = g.kind else {
                continue;
            };
            els.remove(s.element);
            let n = children.len();
            for (i, mut c) in children.into_iter().enumerate() {
                c.matrix = g.matrix.concat(&c.matrix);
                c.alpha *= g.alpha;
                if c.tint.is_none() {
                    c.tint = g.tint;
                }
                els.insert(s.element + i, c);
            }
            let at = s.element;
            self.remap_bones(s.layer, move |i| {
                if i == at {
                    None
                } else if i > at {
                    Some(i + n - 1)
                } else {
                    Some(i)
                }
            });
            for sel in &mut new_sel {
                let sel: &mut Selection = sel;
                if sel.layer == s.layer && sel.element > at {
                    sel.element += n - 1;
                }
            }
            new_sel.extend((at..at + n).map(|element| Selection {
                layer: s.layer,
                element,
            }));
            count += 1;
        }
        new_sel.sort();
        self.selection = new_sel;
        count
    }

    /// Align panel. `to_stage` aligns to the stage instead of the selection.
    pub fn align(&mut self, op: AlignOp, to_stage: bool) {
        let sels: Vec<Selection> = self
            .selection
            .iter()
            .copied()
            .filter(|s| self.layer_editable(s.layer))
            .collect();
        let boxes: Vec<(Selection, Pt, Pt)> = sels
            .iter()
            .filter_map(|s| self.bounds_of(&[*s]).map(|(lo, hi)| (*s, lo, hi)))
            .collect();
        if boxes.is_empty()
            || (!to_stage
                && boxes.len() < 2
                && !matches!(
                    op,
                    AlignOp::MatchWidth | AlignOp::MatchHeight | AlignOp::MatchSize
                ))
        {
            return;
        }
        let (rlo, rhi) = if to_stage {
            if self.editing.is_some() {
                (pt(0.0, 0.0), pt(0.0, 0.0))
            } else {
                (pt(0.0, 0.0), pt(self.doc.width, self.doc.height))
            }
        } else {
            boxes.iter().fold(
                (pt(f32::MAX, f32::MAX), pt(f32::MIN, f32::MIN)),
                |(a, b), (_, lo, hi)| {
                    (
                        pt(a.x.min(lo.x), a.y.min(lo.y)),
                        pt(b.x.max(hi.x), b.y.max(hi.y)),
                    )
                },
            )
        };
        let n = boxes.len();
        // per object: (dx, dy, sx, sy) — scale is about the object's top-left
        let mut ops: Vec<(Selection, Pt, f32, f32, Pt)> = boxes
            .iter()
            .map(|(s, lo, _)| (*s, pt(0.0, 0.0), 1.0, 1.0, *lo))
            .collect();
        let cx = |lo: Pt, hi: Pt| (lo.x + hi.x) / 2.0;
        let cy = |lo: Pt, hi: Pt| (lo.y + hi.y) / 2.0;
        let distribute = |key: &dyn Fn(Pt, Pt) -> f32,
                          horizontal: bool,
                          ops: &mut Vec<(Selection, Pt, f32, f32, Pt)>| {
            let mut order: Vec<usize> = (0..n).collect();
            order.sort_by(|a, b| {
                key(boxes[*a].1, boxes[*a].2).total_cmp(&key(boxes[*b].1, boxes[*b].2))
            });
            let (k0, k1) = if to_stage {
                if horizontal {
                    (rlo.x, rhi.x)
                } else {
                    (rlo.y, rhi.y)
                }
            } else {
                (
                    key(boxes[order[0]].1, boxes[order[0]].2),
                    key(boxes[order[n - 1]].1, boxes[order[n - 1]].2),
                )
            };
            for (rank, i) in order.iter().enumerate() {
                let want = if n > 1 {
                    k0 + (k1 - k0) * rank as f32 / (n - 1) as f32
                } else {
                    (k0 + k1) / 2.0
                };
                let d = want - key(boxes[*i].1, boxes[*i].2);
                if horizontal {
                    ops[*i].1.x = d
                } else {
                    ops[*i].1.y = d
                }
            }
        };
        match op {
            AlignOp::Left => ops
                .iter_mut()
                .zip(&boxes)
                .for_each(|(o, b)| o.1.x = rlo.x - b.1.x),
            AlignOp::Right => ops
                .iter_mut()
                .zip(&boxes)
                .for_each(|(o, b)| o.1.x = rhi.x - b.2.x),
            AlignOp::HCenter => ops
                .iter_mut()
                .zip(&boxes)
                .for_each(|(o, b)| o.1.x = (rlo.x + rhi.x) / 2.0 - cx(b.1, b.2)),
            AlignOp::Top => ops
                .iter_mut()
                .zip(&boxes)
                .for_each(|(o, b)| o.1.y = rlo.y - b.1.y),
            AlignOp::Bottom => ops
                .iter_mut()
                .zip(&boxes)
                .for_each(|(o, b)| o.1.y = rhi.y - b.2.y),
            AlignOp::VCenter => ops
                .iter_mut()
                .zip(&boxes)
                .for_each(|(o, b)| o.1.y = (rlo.y + rhi.y) / 2.0 - cy(b.1, b.2)),
            AlignOp::DistLeft => distribute(&|lo, _| lo.x, true, &mut ops),
            AlignOp::DistHCenter => distribute(&|lo, hi| (lo.x + hi.x) / 2.0, true, &mut ops),
            AlignOp::DistRight => distribute(&|_, hi| hi.x, true, &mut ops),
            AlignOp::DistTop => distribute(&|lo, _| lo.y, false, &mut ops),
            AlignOp::DistVCenter => distribute(&|lo, hi| (lo.y + hi.y) / 2.0, false, &mut ops),
            AlignOp::DistBottom => distribute(&|_, hi| hi.y, false, &mut ops),
            AlignOp::MatchWidth | AlignOp::MatchHeight | AlignOp::MatchSize => {
                let (tw, th) = if to_stage {
                    (rhi.x - rlo.x, rhi.y - rlo.y)
                } else {
                    boxes.iter().fold((0.0f32, 0.0f32), |(w, h), (_, lo, hi)| {
                        (w.max(hi.x - lo.x), h.max(hi.y - lo.y))
                    })
                };
                for (o, (_, lo, hi)) in ops.iter_mut().zip(&boxes) {
                    let (w, h) = ((hi.x - lo.x).max(1e-3), (hi.y - lo.y).max(1e-3));
                    if op != AlignOp::MatchHeight && tw > 0.0 {
                        o.2 = tw / w;
                    }
                    if op != AlignOp::MatchWidth && th > 0.0 {
                        o.3 = th / h;
                    }
                }
            }
            AlignOp::SpaceH | AlignOp::SpaceV => {
                let h = op == AlignOp::SpaceH;
                let mut order: Vec<usize> = (0..n).collect();
                order.sort_by(|a, b| {
                    if h {
                        boxes[*a].1.x.total_cmp(&boxes[*b].1.x)
                    } else {
                        boxes[*a].1.y.total_cmp(&boxes[*b].1.y)
                    }
                });
                let total: f32 = boxes
                    .iter()
                    .map(|(_, lo, hi)| if h { hi.x - lo.x } else { hi.y - lo.y })
                    .sum();
                let (a, b) = if h { (rlo.x, rhi.x) } else { (rlo.y, rhi.y) };
                let gap = if n > 1 {
                    ((b - a) - total) / (n - 1) as f32
                } else {
                    0.0
                };
                let mut cur = a;
                for i in order {
                    let (lo, hi) = (boxes[i].1, boxes[i].2);
                    if h {
                        ops[i].1.x = cur - lo.x;
                        cur += hi.x - lo.x + gap;
                    } else {
                        ops[i].1.y = cur - lo.y;
                        cur += hi.y - lo.y + gap;
                    }
                }
            }
        }
        self.checkpoint("Align");
        let keep = self.selection.clone();
        for (s, d, sx, sy, anchor) in ops {
            if d.x.abs() < 1e-4
                && d.y.abs() < 1e-4
                && (sx - 1.0).abs() < 1e-5
                && (sy - 1.0).abs() < 1e-5
            {
                continue;
            }
            let m = Matrix::translate(d.x, d.y)
                .concat(&Matrix::translate(anchor.x, anchor.y))
                .concat(&Matrix::scale(sx, sy))
                .concat(&Matrix::translate(-anchor.x, -anchor.y));
            self.selection = vec![s];
            self.modify_selected_continuous(|e| e.matrix = m.concat(&e.matrix));
        }
        self.selection = keep;
    }

    // ---- layer tree (folders, masks, guides)
    fn remap_layer_parents(&mut self, f: impl Fn(usize) -> Option<usize>) {
        for l in &mut self.timeline_mut().layers {
            l.parent = l.parent.and_then(&f);
            l.anim_parent = l.anim_parent.and_then(&f);
        }
    }

    // ------------------------------------------------------------ layer parenting
    /// Does `anc` appear in `li`'s parenting chain (or is it `li`)?
    fn parent_chain_has(&self, li: usize, anc: usize) -> bool {
        let tl = self.timeline();
        let mut cur = Some(li);
        let mut guard = 0;
        while let (Some(c), true) = (cur, guard < 64) {
            if c == anc {
                return true;
            }
            cur = tl.layers.get(c).and_then(|l| l.anim_parent);
            guard += 1;
        }
        false
    }
    /// Layers that can parent `li` (normal layers not already following it).
    pub fn parent_candidates(&self, li: usize) -> Vec<usize> {
        (0..self.timeline().layers.len())
            .filter(|&p| {
                p != li
                    && self.timeline().layers[p].kind == LayerKind::Normal
                    && !self.parent_chain_has(p, li)
            })
            .collect()
    }
    /// Layer parenting: make layer `li` follow `parent`'s object (None = free).
    /// Objects keep where they are on every keyframe.
    pub fn set_anim_parent(&mut self, li: usize, parent: Option<usize>) {
        let Some(l) = self.timeline().layers.get(li) else {
            return;
        };
        if l.anim_parent == parent || l.kind != LayerKind::Normal {
            return;
        }
        if let Some(p) = parent {
            if !self.parent_candidates(li).contains(&p) {
                return;
            }
        }
        self.checkpoint(if parent.is_some() {
            "Set Parent Layer"
        } else {
            "Remove Parent Layer"
        });
        self.reparent_raw(li, parent);
    }
    fn reparent_raw(&mut self, li: usize, parent: Option<usize>) {
        let keys: Vec<u32> = self.timeline().layers[li]
            .keyframes
            .keys()
            .copied()
            .collect();
        let old: Vec<Matrix> = keys
            .iter()
            .map(|&k| layer_parent_matrix(self.timeline(), li, k))
            .collect();
        self.timeline_mut().layers[li].anim_parent = parent;
        let new: Vec<Matrix> = keys
            .iter()
            .map(|&k| layer_parent_matrix(self.timeline(), li, k))
            .collect();
        let l = &mut self.timeline_mut().layers[li];
        for (i, k) in keys.iter().enumerate() {
            let conv = new[i].invert().unwrap_or(Matrix::IDENTITY).concat(&old[i]);
            if let Some(kf) = l.keyframes.get_mut(k) {
                for el in &mut kf.elements {
                    el.matrix = conv.concat(&el.matrix);
                }
                if let Some(b) = &mut kf.tween_boundary {
                    for el in &mut b.elements {
                        el.matrix = conv.concat(&el.matrix);
                    }
                    for edit in &mut b.pose_edits {
                        edit.transform_space(conv);
                    }
                }
                if let Some(m) = kf.motion.as_deref_mut() {
                    // motion paths are in parent space too: move them with the object
                    for pk in &mut m.path {
                        pk.at = conv.apply(pk.at);
                    }
                }
            }
        }
    }
    /// What layer `li`'s objects inherit at the playhead.
    pub fn layer_parent_at(&self, li: usize) -> Option<Matrix> {
        let l = self.timeline().layers.get(li)?;
        l.anim_parent
            .map(|_| layer_parent_matrix(self.timeline(), li, self.frame))
    }

    /// Parents must be above their children and be a folder, mask or guide.
    fn fix_layer_tree(&mut self) {
        let kinds: Vec<LayerKind> = self.timeline().layers.iter().map(|l| l.kind).collect();
        for (i, l) in self.timeline_mut().layers.iter_mut().enumerate() {
            if let Some(p) = l.parent {
                if p >= i
                    || !matches!(
                        kinds[p],
                        LayerKind::Folder | LayerKind::Mask | LayerKind::Guide
                    )
                {
                    l.parent = None;
                }
            }
        }
    }

    /// How deep a layer is nested (for indenting the Timeline).
    pub fn layer_depth(&self, i: usize) -> usize {
        let tl = self.timeline();
        let mut d = 0;
        let mut cur = tl.layers.get(i).and_then(|l| l.parent);
        while let Some(p) = cur {
            d += 1;
            if d > 32 {
                break;
            }
            cur = tl.layers.get(p).and_then(|l| l.parent);
        }
        d
    }

    /// Is layer `i` inside a collapsed folder (hidden from the Timeline list)?
    pub fn layer_collapsed_away(&self, i: usize) -> bool {
        let tl = self.timeline();
        let mut cur = tl.layers.get(i).and_then(|l| l.parent);
        let mut guard = 0;
        while let Some(p) = cur {
            if tl
                .layers
                .get(p)
                .map(|l| l.kind == LayerKind::Folder && l.collapsed)
                .unwrap_or(false)
            {
                return true;
            }
            cur = tl.layers.get(p).and_then(|l| l.parent);
            guard += 1;
            if guard > 32 {
                break;
            }
        }
        false
    }

    pub fn set_layer_kind(&mut self, i: usize, kind: LayerKind) {
        if self
            .timeline()
            .layers
            .get(i)
            .map(|l| l.kind == kind || l.kind == LayerKind::Camera)
            .unwrap_or(true)
        {
            return;
        }
        self.checkpoint("Layer Type");
        let was = self.timeline().layers[i].kind;
        self.timeline_mut().layers[i].kind = kind;
        if was != LayerKind::Normal && kind == LayerKind::Normal {
            // children are released
            for l in &mut self.timeline_mut().layers {
                if l.parent == Some(i) {
                    l.parent = None;
                }
            }
        }
        self.fix_layer_tree();
        self.selection.retain(|s| s.layer != i);
    }

    /// Put layer `i` under `parent` (a folder, mask or guide above it), or at top level.
    pub fn set_layer_parent(&mut self, i: usize, parent: Option<usize>) {
        let n = self.timeline().layers.len();
        if i >= n || parent == self.timeline().layers[i].parent {
            return;
        }
        if let Some(p) = parent {
            if p >= i
                || !matches!(
                    self.timeline().layers[p].kind,
                    LayerKind::Folder | LayerKind::Mask | LayerKind::Guide
                )
            {
                return;
            }
        }
        self.checkpoint("Layer Parent");
        self.timeline_mut().layers[i].parent = parent;
    }

    /// Insert a new folder/mask/guide layer above the current one and put the
    /// current layer inside it (Insert › Timeline › Layer Folder, Add Classic
    /// Motion Guide, …). Returns the new layer's index.
    pub fn wrap_layer(&mut self, kind: LayerKind, name: &str) -> usize {
        let cur = self
            .layer
            .min(self.timeline().layers.len().saturating_sub(1));
        let outer = self.timeline().layers.get(cur).and_then(|l| l.parent);
        self.add_layer();
        let at = self.layer; // == cur; the old layer is now at cur + 1
        let l = &mut self.timeline_mut().layers[at];
        l.kind = kind;
        l.name = name.into();
        l.parent = outer;
        if kind != LayerKind::Folder {
            l.keyframes.clear();
            l.keyframes.insert(0, Keyframe::blank());
        }
        self.timeline_mut().layers[at + 1].parent = Some(at);
        self.squash_undo(
            self.undo_depth() - 1,
            match kind {
                LayerKind::Folder => "New Folder",
                LayerKind::Mask => "Add Mask",
                LayerKind::Guide => "Add Motion Guide",
                _ => "New Layer",
            },
        );
        at
    }

    /// Layer › Mask: the layer becomes a mask for the layer right below it.
    pub fn make_mask(&mut self, i: usize) {
        let n = self.timeline().layers.len();
        if i + 1 >= n {
            return;
        }
        self.checkpoint("Mask");
        let parent = self.timeline().layers[i].parent;
        let tl = self.timeline_mut();
        tl.layers[i].kind = LayerKind::Mask;
        if tl.layers[i + 1].kind == LayerKind::Normal {
            tl.layers[i + 1].parent = Some(i);
        }
        tl.layers[i].parent = parent;
        // Animate locks both so the mask effect shows while authoring
        tl.layers[i].locked = true;
        tl.layers[i + 1].locked = true;
        self.selection.retain(|s| s.layer != i && s.layer != i + 1);
    }

    /// Is `anc` an ancestor (folder/mask/guide chain) of layer `i`?
    fn layer_is_under(&self, i: usize, anc: usize) -> bool {
        let tl = self.timeline();
        let mut cur = tl.layers.get(i).and_then(|l| l.parent);
        let mut guard = 0;
        while let Some(p) = cur {
            if p == anc {
                return true;
            }
            cur = tl.layers.get(p).and_then(|l| l.parent);
            guard += 1;
            if guard > 32 {
                break;
            }
        }
        false
    }

    /// Timeline drag-and-drop: move layer `from` (with everything inside it)
    /// to row `to`; it joins the folder/mask/guide it lands in.
    pub fn drop_layer(&mut self, from: usize, to: usize) {
        let n = self.timeline().layers.len();
        if from >= n || to >= n || from == to {
            return;
        }
        // the block: `from` and the run of its descendants right below it
        let mut end = from + 1;
        while end < n && self.layer_is_under(end, from) {
            end += 1;
        }
        if (from..end).contains(&to) {
            return; // can't drop a folder into itself
        }
        self.checkpoint("Move Layer");
        let mut order: Vec<usize> = (0..n).filter(|i| !(from..end).contains(i)).collect();
        // insert position in the reduced list: before the target row
        let pos = order.iter().position(|&i| i == to).unwrap_or(order.len());
        let pos = if to > from { pos + 1 } else { pos };
        let block: Vec<usize> = (from..end).collect();
        order.splice(pos..pos, block);
        let mut new_of = vec![0usize; n];
        for (new, &old) in order.iter().enumerate() {
            new_of[old] = new;
        }
        let old_layers = std::mem::take(&mut self.timeline_mut().layers);
        let mut slots: Vec<Option<Layer>> = old_layers.into_iter().map(Some).collect();
        let mut layers: Vec<Layer> = order.iter().map(|&o| slots[o].take().unwrap()).collect();
        for l in &mut layers {
            l.parent = l.parent.map(|p| new_of[p]);
        }
        // nesting of the dropped layer: into the container right above, or a sibling of the layer below
        let head = new_of[from];
        let above = head
            .checked_sub(1)
            .and_then(|a| layers.get(a).map(|l| (a, l.kind, l.collapsed)));
        let below_parent = layers
            .get(head + (end - from))
            .and_then(|l| l.parent)
            .filter(|p| *p < head);
        layers[head].parent = match above {
            Some((a, LayerKind::Folder, false))
            | Some((a, LayerKind::Mask, _))
            | Some((a, LayerKind::Guide, _)) => Some(a),
            _ => below_parent,
        };
        self.timeline_mut().layers = layers;
        self.fix_layer_tree();
        self.layer = head;
        self.selection.clear();
        self.frame_sel = None;
    }

    /// Duplicate a layer (and its content) above itself.
    pub fn duplicate_layer(&mut self) {
        let i = self.layer;
        let Some(mut l) = self.timeline().layers.get(i).cloned() else {
            return;
        };
        if l.kind == LayerKind::Camera {
            return;
        }
        self.checkpoint("Duplicate Layer");
        l.name = format!("{} copy", l.name);
        self.remap_layer_parents(|p| Some(if p >= i { p + 1 } else { p }));
        self.timeline_mut().layers.insert(i, l);
        self.selection.clear();
        self.frame_sel = None;
    }

    /// Layer Properties: colour and flags in one undo step.
    pub fn set_layer_look(
        &mut self,
        i: usize,
        color: Color,
        visible: bool,
        locked: bool,
        outline: bool,
    ) {
        let Some(l) = self.timeline().layers.get(i) else {
            return;
        };
        if (l.color, l.visible, l.locked, l.outline) == (color, visible, locked, outline) {
            return;
        }
        self.checkpoint("Layer Properties");
        let l = &mut self.timeline_mut().layers[i];
        l.color = color;
        l.visible = visible;
        l.locked = locked;
        l.outline = outline;
        let tl = self.timeline();
        let keep: Vec<Selection> = self
            .selection
            .iter()
            .copied()
            .filter(|s| {
                tl.layers
                    .get(s.layer)
                    .map(|l| l.visible && !l.locked)
                    .unwrap_or(false)
            })
            .collect();
        self.selection = keep;
    }

    // ---- ruler guides
    /// Add a guide (vertical = at x). Returns its index.
    pub fn add_guide(&mut self, vertical: bool, pos: f32) -> usize {
        self.checkpoint("Add Guide");
        let v = if vertical {
            &mut self.doc.guides.x
        } else {
            &mut self.doc.guides.y
        };
        v.push(pos);
        v.len() - 1
    }
    /// Move a guide during a drag (call `begin_edit` first).
    pub fn move_guide(&mut self, vertical: bool, i: usize, pos: f32) {
        let v = if vertical {
            &mut self.doc.guides.x
        } else {
            &mut self.doc.guides.y
        };
        if let Some(g) = v.get_mut(i) {
            *g = pos;
        }
    }
    pub fn remove_guide(&mut self, vertical: bool, i: usize) {
        self.checkpoint("Remove Guide");
        let v = if vertical {
            &mut self.doc.guides.x
        } else {
            &mut self.doc.guides.y
        };
        if i < v.len() {
            v.remove(i);
        }
    }
    pub fn clear_guides(&mut self) {
        if !self.doc.guides.is_empty() {
            self.checkpoint("Clear Guides");
            self.doc.guides = Default::default();
        }
    }

    pub fn toggle_folder(&mut self, i: usize) {
        if let Some(l) = self.timeline_mut().layers.get_mut(i) {
            l.collapsed = !l.collapsed;
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum LayerFlag {
    Visible,
    Locked,
    Outline,
}
