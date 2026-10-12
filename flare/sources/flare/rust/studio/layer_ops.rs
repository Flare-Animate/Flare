//! Whole-layer selection and clipboard. Payload indices are local to the clipboard;
//! the Editor remains the sole document/history authority.
use super::*;
use ac_core::{ElementKind, Layer, LayerKind};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
pub(super) struct LayerState {
    selected: BTreeSet<usize>,
    anchor: Option<usize>,
    context: Option<(usize, Option<u32>)>,
    clipboard: Vec<Layer>,
    source: Option<clipboard::Source>,
    rename: Option<LayerRename>,
    flag_drag: Option<FlagGesture>,
    color_depth: Option<usize>,
}

struct LayerRename {
    layer: usize,
    text: String,
    focus: bool,
}

struct FlagGesture {
    lock: bool,
    value: bool,
    last: usize,
    targets: BTreeSet<usize>,
}

impl Studio {
    pub(super) fn set_layer_outlines(&mut self, targets: &[usize], value: bool) {
        if !targets.iter().any(|&i| {
            self.editor
                .timeline()
                .layers
                .get(i)
                .is_some_and(|l| l.outline != value)
        }) {
            return;
        }
        self.finish_stage_gesture();
        self.editor.begin_edit("layer.outline");
        for &i in targets {
            if let Some(l) = self.editor.timeline_mut().layers.get_mut(i) {
                l.outline = value;
            }
        }
        self.texture = None;
    }
    pub(super) fn outline_control(&mut self, ui: &mut egui::Ui, rect: Rect, at: Option<usize>) {
        // A context-menu deletion may have changed the model during this row pass.
        if at.is_some_and(|i| i >= self.editor.timeline().layers.len()) {
            return;
        }
        let targets: Vec<_> = at.map_or_else(
            || (0..self.editor.timeline().layers.len()).collect(),
            |i| vec![i],
        );
        let all = targets
            .iter()
            .all(|&i| self.editor.timeline().layers[i].outline);
        let color = at.map_or(self.prefs.theme.palette().text, |i| {
            let c = self.editor.timeline().layers[i].color;
            Color32::from_rgb(c.r, c.g, c.b)
        });
        let r = rect.shrink(3.0);
        if !all {
            ui.painter().rect_filled(r, 0.0, color);
        }
        ui.painter()
            .rect_stroke(r, 0.0, Stroke::new(1.5, color), egui::StrokeKind::Inside);
        let label = at.map_or_else(
            || self.i18n.text("action-all-outlines"),
            |i| {
                self.i18n.format(
                    "action-layer-outline",
                    &[("name", &self.editor.timeline().layers[i].name)],
                )
            },
        );
        let response = ui
            .interact(rect, ui.id().with(("outline", at)), Sense::click())
            .on_hover_text(&label);
        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &label));
        if response.clicked() {
            if let Some(at) = at.filter(|_| ui.input(|i| i.modifiers.alt)) {
                let others: Vec<_> = (0..self.editor.timeline().layers.len())
                    .filter(|&i| i != at)
                    .collect();
                let outlined = others
                    .iter()
                    .all(|&i| self.editor.timeline().layers[i].outline);
                self.set_layer_outlines(&others, !outlined);
            } else {
                self.set_layer_outlines(&targets, !all);
            }
        }
    }
    fn copying_folder_frames(&self) -> bool {
        self.layer_selection_active()
            && self.selected_layer_roots().len() == 1
            && self.editor.timeline().layers[self.editor.layer].kind == LayerKind::Folder
    }
    fn folder_frame_payload(&self) -> Vec<Layer> {
        let mut indices = self.clipboard_indices();
        indices.remove(&self.editor.layer);
        self.layer_payload(&indices)
    }
    pub(super) fn folder_frames_enabled(&self, command: Command) -> Option<bool> {
        use Command::*;
        if matches!(command, CopyFrames | CutFrames) && self.copying_folder_frames() {
            let payload = self.folder_frame_payload();
            return Some(
                !payload.is_empty()
                    && payload.iter().all(|l| l.kind != LayerKind::Camera)
                    && (command == CopyFrames
                        || self.selected_layer_indices().iter().all(|&i| {
                            self.editor.timeline().layers[i].kind == LayerKind::Folder
                                || self.timeline_editable(i)
                        })),
            );
        }
        if matches!(command, PasteFrames | PasteOverwriteFrames)
            && !self.timeline_state.folder_clipboard.is_empty()
        {
            return Some(
                self.layer_payload_fits(
                    &self.timeline_state.folder_clipboard,
                    self.layer_paste_target(),
                ) && self.timeline_state.source.as_ref().is_some_and(|source| {
                    self.prepare_clipboard(source, &self.timeline_state.folder_clipboard)
                        .is_some()
                }),
            );
        }
        None
    }
    pub(super) fn dispatch_folder_frames(&mut self, command: Command) -> bool {
        let Some(enabled) = self.folder_frames_enabled(command) else {
            return false;
        };
        if !enabled {
            return true;
        }
        match command {
            Command::CopyFrames | Command::CutFrames => {
                self.timeline_state.folder_clipboard = self.folder_frame_payload();
                self.timeline_state.pieces.clear();
                self.timeline_state.source = Some(self.clipboard_source());
                if command == Command::CutFrames {
                    self.editor.begin_edit("timeline.cut.folder");
                    for i in self.selected_layer_indices() {
                        let l = &mut self.editor.timeline_mut().layers[i];
                        if l.kind != LayerKind::Folder {
                            ac_core::frames::clear_frames(l, 0, l.end - 1);
                        }
                    }
                }
            }
            _ => {
                let (at, parent) = self.layer_paste_target().unwrap();
                let (doc, payload) = self
                    .prepare_clipboard(
                        self.timeline_state.source.as_ref().unwrap(),
                        &self.timeline_state.folder_clipboard,
                    )
                    .unwrap();
                let depth = self.editor.undo_depth();
                self.editor.begin_edit("clipboard.assets");
                self.editor.doc = doc;
                self.insert_layer_payload(payload, at, parent);
                self.editor.squash_undo(depth, "timeline.paste.folder");
            }
        }
        self.texture = None;
        true
    }
    pub(super) fn flag_drag_active(&self) -> bool {
        self.layer_state.flag_drag.is_some()
    }
    pub(super) fn clear_layer_selection(&mut self) {
        self.layer_state.selected.clear();
        self.layer_state.anchor = None;
        self.layer_state.context = None;
        self.layer_state.rename = None;
        self.layer_state.flag_drag = None;
    }

    pub(super) fn layer_selection_active(&self) -> bool {
        self.layer_state.context == Some((self.editor.scene, self.editor.editing))
            && !self.layer_state.selected.is_empty()
            && self
                .layer_state
                .selected
                .iter()
                .all(|&i| i < self.editor.timeline().layers.len())
    }

    pub(super) fn layer_row_selected(&self, index: usize) -> bool {
        if self.layer_selection_active() {
            self.layer_state.selected.contains(&index)
        } else {
            self.editor.layer == index
        }
    }

    pub(super) fn set_layer_selection(&mut self, selected: BTreeSet<usize>, active: usize) {
        self.timeline_state.ranges.clear();
        self.layer_state.context = Some((self.editor.scene, self.editor.editing));
        self.layer_state.selected = selected;
        self.editor.layer = active;
        self.editor.selection.clear();
        self.editor.frame_sel = None;
        if self.layer_state.selected.len() == 1 {
            self.editor.select_layer_frames(active);
        }
        self.timeline_state.focused = true;
        self.playing = false;
        self.sync_forms();
    }

    pub(super) fn select_layer_rows(&mut self, at: usize, extend: bool, toggle: bool) {
        if at >= self.editor.timeline().layers.len() {
            return;
        }
        self.layer_state.rename = None;
        let mut selected = if self.layer_selection_active() {
            self.layer_state.selected.clone()
        } else {
            BTreeSet::from([self.editor.layer])
        };
        let anchor = self.layer_state.anchor.unwrap_or(self.editor.layer);
        if extend {
            selected = (at.min(anchor)..=at.max(anchor))
                .filter(|&i| !self.editor.layer_collapsed_away(i))
                .collect();
        } else if toggle {
            if !selected.remove(&at) {
                selected.insert(at);
            }
            // Keep an active row for commands even when toggling the last selected row.
            if selected.is_empty() {
                selected.insert(at);
            }
            self.layer_state.anchor = Some(at);
        } else {
            selected = BTreeSet::from([at]);
            self.layer_state.anchor = Some(at);
        }
        let active = if selected.contains(&at) {
            at
        } else {
            *selected.first().unwrap()
        };
        self.set_layer_selection(selected, active);
    }

    pub(super) fn selected_layer_roots(&self) -> Vec<usize> {
        let selected = if self.layer_selection_active() {
            self.layer_state.selected.clone()
        } else {
            BTreeSet::from([self.editor.layer])
        };
        selected
            .iter()
            .copied()
            .filter(|&i| {
                let mut parent = self.editor.timeline().layers[i].parent;
                while let Some(p) = parent {
                    if selected.contains(&p) {
                        return false;
                    }
                    parent = self.editor.timeline().layers[p].parent;
                }
                true
            })
            .collect()
    }

    fn selected_layer_indices(&self) -> BTreeSet<usize> {
        let roots = self.selected_layer_roots();
        // Follow parent links rather than assuming that a loaded subtree is contiguous.
        (0..self.editor.timeline().layers.len())
            .filter(|&i| {
                let mut p = Some(i);
                while let Some(at) = p {
                    if roots.contains(&at) {
                        return true;
                    }
                    p = self.editor.timeline().layers[at].parent;
                }
                false
            })
            .collect()
    }

    fn clipboard_indices(&self) -> BTreeSet<usize> {
        let mut selected = self.selected_layer_indices();
        // Linked animation parents and mask/guide ancestors are content dependencies.
        // Retain them as editable layers rather than baking or dropping their effects.
        loop {
            let before = selected.len();
            for i in selected.clone() {
                let l = &self.editor.timeline().layers[i];
                if let Some(parent) = l.anim_parent {
                    selected.insert(parent);
                }
                let mut parent = l.parent;
                while let Some(p) = parent {
                    let l = &self.editor.timeline().layers[p];
                    if matches!(l.kind, LayerKind::Mask | LayerKind::Guide) {
                        selected.insert(p);
                    }
                    parent = l.parent;
                }
            }
            if selected.len() == before {
                return selected;
            }
        }
    }

    fn layer_payload(&self, indices: &BTreeSet<usize>) -> Vec<Layer> {
        let layers = &self.editor.timeline().layers;
        let map: BTreeMap<_, _> = indices
            .iter()
            .copied()
            .enumerate()
            .map(|(new, old)| (old, new))
            .collect();
        indices
            .iter()
            .map(|&i| {
                let mut l = layers[i].clone();
                let mut p = l.parent;
                let mut nearest = None;
                while let Some(parent) = p {
                    if let Some(&new) = map.get(&parent) {
                        nearest = Some(new);
                        break;
                    }
                    // Detached ordinary folders still contribute their current visibility/lock state.
                    l.visible &= layers[parent].visible;
                    l.locked |= layers[parent].locked;
                    p = layers[parent].parent;
                }
                l.parent = nearest;
                l.anim_parent = l.anim_parent.and_then(|p| map.get(&p).copied());
                l
            })
            .collect()
    }

    fn selected_layers_deletable(&self) -> bool {
        let selected = self.selected_layer_indices();
        let layers = &self.editor.timeline().layers;
        selected.len() < layers.len()
            && selected.iter().all(|&i| self.layer_block_unlocked(i))
            && layers.iter().enumerate().all(|(i, l)| {
                selected.contains(&i)
                    || !l.anim_parent.is_some_and(|p| selected.contains(&p))
                    || self.timeline_editable(i)
            })
    }

    fn layer_paste_target(&self) -> Option<(usize, Option<usize>)> {
        let layers = &self.editor.timeline().layers;
        let at = self.editor.layer;
        let l = layers.get(at)?;
        let (at, parent) = if l.kind == LayerKind::Folder {
            (at + 1, Some(at))
        } else {
            (at, l.parent)
        };
        let mut p = parent;
        while let Some(i) = p {
            if layers[i].locked {
                return None;
            }
            p = layers[i].parent;
        }
        Some((at, parent))
    }

    fn layer_payload_fits(
        &self,
        payload: &[Layer],
        target: Option<(usize, Option<usize>)>,
    ) -> bool {
        let Some((_, parent)) = target else {
            return false;
        };
        let layers = &self.editor.timeline().layers;
        let mut p = parent;
        while let Some(i) = p {
            if layers[i].locked {
                return false;
            }
            p = layers[i].parent;
        }
        !payload.is_empty()
            && layers.len() + payload.len() <= 256
            && payload.iter().all(|l| l.kind != LayerKind::Camera)
            && !parent.is_some_and(|i| {
                matches!(layers[i].kind, LayerKind::Mask | LayerKind::Guide)
                    && payload
                        .iter()
                        .any(|l| l.parent.is_none() && l.kind != LayerKind::Normal)
            })
    }

    fn clipboard_references_valid(&self, payload: &[Layer]) -> bool {
        fn elements(
            doc: &ac_core::Document,
            els: &[Element],
            target: Option<u32>,
            visiting: &mut BTreeSet<u32>,
            checked: &mut BTreeSet<u32>,
        ) -> bool {
            els.iter().all(|e| match &e.kind {
                ElementKind::Instance(instance) => {
                    let id = instance.symbol;
                    if Some(id) == target || visiting.contains(&id) {
                        return false;
                    }
                    if checked.contains(&id) {
                        return true;
                    }
                    let Some(symbol) = doc.symbol(id) else {
                        return false;
                    };
                    visiting.insert(id);
                    let valid = symbol.timeline.layers.iter().all(|l| {
                        l.keyframes
                            .values()
                            .all(|k| elements(doc, &k.elements, target, visiting, checked))
                    });
                    visiting.remove(&id);
                    if valid {
                        checked.insert(id);
                    }
                    valid
                }
                ElementKind::Group(children) => elements(doc, children, target, visiting, checked),
                ElementKind::Bitmap(b) => doc.bitmap(b.bitmap).is_some(),
                ElementKind::Shape(s) => {
                    s.bitmap_fill.is_none_or(|b| doc.bitmap(b.bitmap).is_some())
                }
                _ => true,
            })
        }
        let mut checked = BTreeSet::new();
        let mut visiting = BTreeSet::new();
        payload.iter().all(|l| {
            l.keyframes.values().all(|k| {
                elements(
                    &self.editor.doc,
                    &k.elements,
                    self.editor.editing,
                    &mut visiting,
                    &mut checked,
                )
            })
        })
    }

    pub(super) fn layer_command_enabled(&self, command: Command) -> Option<bool> {
        use Command::*;
        Some(match command {
            CopyLayers => self
                .clipboard_indices()
                .iter()
                .all(|&i| self.editor.timeline().layers[i].kind != LayerKind::Camera),
            CutLayers => {
                self.selected_layers_deletable()
                    && self
                        .clipboard_indices()
                        .iter()
                        .all(|&i| self.editor.timeline().layers[i].kind != LayerKind::Camera)
            }
            DeleteLayer => self.selected_layers_deletable(),
            PasteLayers => {
                self.layer_payload_fits(&self.layer_state.clipboard, self.layer_paste_target())
                    && self.layer_state.source.as_ref().is_some_and(|source| {
                        self.prepare_clipboard(source, &self.layer_state.clipboard)
                            .is_some()
                    })
            }
            DuplicateLayer => {
                let indices = self.clipboard_indices();
                let payload = self.layer_payload(&indices);
                let at = *indices.first().unwrap();
                self.clipboard_references_valid(&payload)
                    && self.layer_payload_fits(
                        &payload,
                        Some((at, self.editor.timeline().layers[at].parent)),
                    )
            }
            SelectAllLayers => !self.editor.timeline().layers.is_empty(),
            LayerUp | LayerDown
                if self.layer_selection_active() && self.selected_layer_roots().len() > 1 =>
            {
                self.group_neighbor(command == LayerDown).is_some()
            }
            _ => return None,
        })
    }

    fn delete_selected_layers(&mut self) {
        let selected = self.selected_layer_indices();
        let at = *selected.first().unwrap();
        let depth = self.editor.undo_depth();
        // Descending order keeps original indices meaningful and lets Editor preserve
        // surviving animation children when their parent is removed.
        for &i in selected.iter().rev() {
            self.editor.layer = i;
            self.editor.delete_layer();
        }
        self.editor.squash_undo(depth, "layer.delete");
        self.editor.layer = at.min(self.editor.timeline().layers.len() - 1);
        self.reset_timeline_gesture();
        self.select_layer_rows(self.editor.layer, false, false);
    }

    fn insert_layer_payload(&mut self, mut payload: Vec<Layer>, at: usize, parent: Option<usize>) {
        let n = payload.len();
        self.editor.begin_edit("layer.paste");
        let shift = |p| if p >= at { p + n } else { p };
        for l in &mut self.editor.timeline_mut().layers {
            l.parent = l.parent.map(shift);
            l.anim_parent = l.anim_parent.map(shift);
        }
        for l in &mut payload {
            l.parent = l.parent.map(|p| p + at).or(parent.map(shift));
            l.anim_parent = l.anim_parent.map(|p| p + at);
        }
        self.editor.timeline_mut().layers.splice(at..at, payload);
        if let Some(parent) = parent {
            self.editor.timeline_mut().layers[shift(parent)].collapsed = false;
        }
        self.reset_timeline_gesture();
        self.set_layer_selection((at..at + n).collect(), at);
        self.layer_state.anchor = Some(at);
        self.texture = None;
    }

    pub(super) fn dispatch_layers(&mut self, command: Command) -> bool {
        use Command::*;
        match command {
            CopyLayers | CutLayers => {
                let indices = self.clipboard_indices();
                let dependencies = indices.len() - self.selected_layer_indices().len();
                self.layer_state.clipboard = self.layer_payload(&indices);
                self.layer_state.source = Some(self.clipboard_source());
                self.status = self.i18n.format(
                    "status-layer-clipboard",
                    &[
                        ("count", &indices.len().to_string()),
                        ("dependencies", &dependencies.to_string()),
                    ],
                );
                self.error = false;
                if command == CutLayers {
                    self.delete_selected_layers();
                }
            }
            PasteLayers => {
                let (at, parent) = self.layer_paste_target().unwrap();
                let (doc, payload) = self
                    .prepare_clipboard(
                        self.layer_state.source.as_ref().unwrap(),
                        &self.layer_state.clipboard,
                    )
                    .unwrap();
                let depth = self.editor.undo_depth();
                self.editor.begin_edit("clipboard.assets");
                self.editor.doc = doc;
                self.insert_layer_payload(payload, at, parent);
                self.editor.squash_undo(depth, "layer.paste");
            }
            DeleteLayer => self.delete_selected_layers(),
            DuplicateLayer => {
                let indices = self.clipboard_indices();
                let mut payload = self.layer_payload(&indices);
                for l in &mut payload {
                    l.name = self.i18n.format("duplicate-name", &[("name", &l.name)]);
                }
                // Duplicate beside the selected roots, including a folder rather than inside it.
                let at = *indices.first().unwrap();
                let parent = self.editor.timeline().layers[at].parent;
                self.insert_layer_payload(payload, at, parent);
            }
            SelectAllLayers => {
                self.set_layer_selection(
                    (0..self.editor.timeline().layers.len()).collect(),
                    self.editor.layer,
                );
                self.layer_state.anchor = Some(0);
            }
            LayerUp | LayerDown
                if self.layer_selection_active() && self.selected_layer_roots().len() > 1 =>
            {
                if let Some(to) = self.group_neighbor(command == LayerDown) {
                    self.move_selected_layer_group(to, false, None);
                }
            }
            _ => return false,
        }
        self.playing = false;
        self.texture = None;
        true
    }

    fn group_neighbor(&self, down: bool) -> Option<usize> {
        let roots = self.selected_layer_roots();
        let layers = &self.editor.timeline().layers;
        let parent = layers[roots[0]].parent;
        if !roots.iter().all(|&i| layers[i].parent == parent)
            || !self
                .selected_layer_indices()
                .iter()
                .all(|&i| self.layer_block_unlocked(i))
        {
            return None;
        }
        if down {
            let end = self.layer_block(*roots.last().unwrap()).end;
            (end..layers.len()).find(|&i| layers[i].parent == parent)
        } else {
            (0..roots[0]).rev().find(|&i| layers[i].parent == parent)
        }
    }

    fn move_selected_layer_group(&mut self, to: usize, into: bool, after: Option<bool>) -> bool {
        let roots = self.selected_layer_roots();
        let selected = self.selected_layer_indices();
        let layers = &self.editor.timeline().layers;
        if to >= layers.len()
            || selected.contains(&to)
            || !selected.iter().all(|&i| self.layer_block_unlocked(i))
        {
            return false;
        }
        let parent = if into { Some(to) } else { layers[to].parent };
        if parent.is_some_and(|p| {
            matches!(layers[p].kind, LayerKind::Mask | LayerKind::Guide)
                && roots.iter().any(|&i| layers[i].kind != LayerKind::Normal)
        }) {
            return false;
        }
        let mut p = parent;
        while let Some(i) = p {
            if selected.contains(&i) || layers[i].locked {
                return false;
            }
            p = layers[i].parent;
        }
        if into && layers[to].kind != LayerKind::Folder {
            return false;
        }
        let insertion = if into || after.unwrap_or(to > roots[0]) {
            self.layer_block(to).end
        } else {
            to
        };
        let mut order: Vec<_> = (0..layers.len())
            .filter(|i| !selected.contains(i))
            .collect();
        let pos = order
            .iter()
            .position(|&i| i >= insertion)
            .unwrap_or(order.len());
        order.splice(pos..pos, selected.iter().copied());
        if order.iter().copied().eq(0..layers.len())
            && roots.iter().all(|&i| layers[i].parent == parent)
            && (!into || !layers[to].collapsed)
        {
            return false;
        }
        let new_selected: BTreeSet<_> = order
            .iter()
            .enumerate()
            .filter_map(|(new, old)| roots.contains(old).then_some(new))
            .collect();
        let active = self.editor.layer;
        self.editor.begin_edit("layer.move.group");
        for &i in &roots {
            self.editor.timeline_mut().layers[i].parent = parent;
        }
        if into {
            self.editor.timeline_mut().layers[to].collapsed = false;
        }
        self.reorder_layers(order, active);
        self.set_layer_selection(new_selected, self.editor.layer);
        self.texture = None;
        true
    }

    /// A folder's center accepts children; its edges insert siblings.
    pub(super) fn drop_layers_at(&mut self, from: usize, to: usize, position: f32) -> bool {
        if !self.layer_selection_active() || !self.layer_row_selected(from) {
            self.select_layer_rows(from, false, false);
        }
        let into = self
            .editor
            .timeline()
            .layers
            .get(to)
            .is_some_and(|l| l.kind == LayerKind::Folder && (0.25..=0.75).contains(&position));
        self.move_selected_layer_group(to, into, Some(position >= 0.5))
    }

    pub(super) fn layer_name_editable(&self, at: usize) -> bool {
        let layers = &self.editor.timeline().layers;
        let mut p = Some(at);
        while let Some(i) = p {
            let Some(layer) = layers.get(i) else {
                return false;
            };
            if layer.locked || layer.kind == LayerKind::Camera {
                return false;
            }
            p = layer.parent;
        }
        true
    }

    pub(super) fn rename_layer_checked(&mut self, at: usize, name: &str) {
        let name = name.trim();
        if self.layer_name_editable(at)
            && !name.is_empty()
            && self.editor.timeline().layers[at].name != name
        {
            self.editor.rename_layer(at, name);
            self.sync_forms();
        }
    }

    pub(super) fn begin_layer_rename(&mut self, at: usize) {
        if self.layer_name_editable(at) {
            self.select_layer_rows(at, false, false);
            self.layer_state.rename = Some(LayerRename {
                layer: at,
                text: self.editor.timeline().layers[at].name.clone(),
                focus: true,
            });
        }
    }

    pub(super) fn layer_name_editor(&mut self, ui: &mut egui::Ui, at: usize, rect: Rect) -> bool {
        let Some(rename) = self.layer_state.rename.as_mut().filter(|r| r.layer == at) else {
            return false;
        };
        let p = self.prefs.theme.palette();
        let response = ui
            .scope(|ui| {
                ui.visuals_mut().override_text_color =
                    Some(Color32::from_gray(if p.back.r() > 128 { 50 } else { 35 }));
                ui.put(
                    rect,
                    egui::TextEdit::singleline(&mut rename.text)
                        .id_salt(("studio.layer.rename", at))
                        .font(theme::medium(11.0))
                        .clip_text(true),
                )
            })
            .inner;
        theme::field_edges(ui.painter(), response.rect, p);
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::TextEdit,
                true,
                self.i18n.text("field-layer-name"),
            )
        });
        if rename.focus {
            response.request_focus();
            rename.focus = false;
        } else if response.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            let rename = self.layer_state.rename.take().unwrap();
            self.rename_layer_checked(at, &rename.text);
        }
        true
    }

    pub(super) fn layer_properties(&mut self, ui: &mut egui::Ui) {
        let targets: Vec<_> = if self.layer_selection_active() {
            self.layer_state.selected.iter().copied().collect()
        } else {
            vec![self.editor.layer]
        };
        self.section(ui, "section-layer-properties");
        ui.label(self.i18n.count("selected-layer-count", targets.len()));
        for (lock, key) in [(false, "field-layer-visible"), (true, "field-layer-locked")] {
            let values: Vec<_> = targets
                .iter()
                .map(|&i| {
                    let l = &self.editor.timeline().layers[i];
                    if lock {
                        l.locked
                    } else {
                        l.visible
                    }
                })
                .collect();
            let mut value = values.iter().all(|&v| v);
            let mixed = values.iter().any(|&v| v) && !value;
            if ui
                .add(egui::Checkbox::new(&mut value, self.i18n.text(key)).indeterminate(mixed))
                .changed()
            {
                self.set_layer_flags(&targets, lock, value);
            }
        }
        let current = &self.editor.timeline().layers[self.editor.layer];
        let mut outline = targets
            .iter()
            .all(|&i| self.editor.timeline().layers[i].outline);
        let mixed = targets
            .iter()
            .any(|&i| self.editor.timeline().layers[i].outline)
            && !outline;
        let color = current.color;
        let mut c = Color32::from_rgb(color.r, color.g, color.b);
        if ui
            .add(
                egui::Checkbox::new(&mut outline, self.i18n.text("field-layer-outline"))
                    .indeterminate(mixed),
            )
            .changed()
        {
            self.set_layer_outlines(&targets, outline);
        }
        ui.horizontal(|ui| {
            ui.label(self.i18n.text("field-layer-color"));
            if ui.color_edit_button_srgba(&mut c).changed() {
                let depth = *self
                    .layer_state
                    .color_depth
                    .get_or_insert(self.editor.undo_depth());
                self.editor.begin_edit("layer.color");
                for &i in &targets {
                    self.editor.timeline_mut().layers[i].color = Color::rgb(c.r(), c.g(), c.b());
                }
                self.editor.squash_undo(depth, "layer.color");
                self.texture = None;
            }
        });
        if !egui::Popup::is_any_open(ui.ctx()) {
            self.layer_state.color_depth = None;
        }
        self.layer_type_picker(ui, &targets);
        ui.add_enabled_ui(
            targets.len() == 1 && self.layer_name_editable(self.editor.layer),
            |ui| {
                let label = ui.label(self.i18n.text("field-layer-name"));
                widgets::text_edit(ui, egui::TextEdit::singleline(&mut self.layer_name))
                    .labelled_by(label.id);
                if widgets::button(ui, self.i18n.text("action-rename-layer")).clicked() {
                    self.rename_layer_checked(self.editor.layer, &self.layer_name.clone());
                }
            },
        );
        ui.horizontal(|ui| {
            ui.label(self.i18n.text("field-layer-height"));
            let height = self.editor.timeline().layers[self.editor.layer].row_height;
            let mixed = targets
                .iter()
                .any(|&i| self.editor.timeline().layers[i].row_height != height);
            widgets::combo_box(
                ui,
                "studio.layer.height",
                if mixed {
                    self.i18n.text("field-mixed")
                } else {
                    format!("{height}%")
                },
                ui.spacing().combo_width,
                |ui| {
                    for h in [100, 200, 300] {
                        if widgets::selectable(ui, !mixed && height == h, format!("{h}%")).clicked()
                        {
                            self.set_layer_height(&targets, h);
                        }
                    }
                },
            );
        });
    }

    pub(super) fn set_layer_height(&mut self, targets: &[usize], height: u16) {
        if ![100, 200, 300].contains(&height)
            || targets
                .iter()
                .all(|&i| self.editor.timeline().layers[i].row_height == height)
        {
            return;
        }
        self.editor.begin_edit("layer.height");
        for &i in targets {
            self.editor.timeline_mut().layers[i].row_height = height;
        }
    }

    pub(super) fn can_set_layer_kind(&self, targets: &[usize], kind: LayerKind) -> bool {
        self.planned_layer_type(targets, layer_types::LayerType::Base(kind))
            .is_some()
    }
    pub(super) fn set_layer_kind_checked(&mut self, targets: &[usize], kind: LayerKind) {
        self.set_layer_type(targets, layer_types::LayerType::Base(kind));
    }

    pub(super) fn begin_flag_drag(&mut self, at: usize, lock: bool, value: bool) {
        self.timeline_state.gesture = None;
        self.timeline_state.duration_drag = None;
        self.timeline_state.layer_drag = None;
        self.layer_state.flag_drag = Some(FlagGesture {
            lock,
            value,
            last: at,
            targets: BTreeSet::from([at]),
        });
    }

    pub(super) fn flag_preview(&self, at: usize, lock: bool, value: bool) -> bool {
        self.layer_state
            .flag_drag
            .as_ref()
            .filter(|g| g.lock == lock && g.targets.contains(&at))
            .map_or(value, |g| g.value)
    }

    pub(super) fn update_flag_drag(
        &mut self,
        at: usize,
        rows: &[usize],
        release: bool,
        inside: bool,
    ) {
        if let Some(g) = &mut self.layer_state.flag_drag {
            if inside {
                g.targets.extend(
                    rows.iter()
                        .copied()
                        .filter(|&i| i >= g.last.min(at) && i <= g.last.max(at)),
                );
                g.last = at;
            }
        }
        if release {
            if let Some(g) = self.layer_state.flag_drag.take().filter(|_| inside) {
                self.set_layer_flags(&g.targets.into_iter().collect::<Vec<_>>(), g.lock, g.value);
            }
        }
    }
}
