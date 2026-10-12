//! Layer roles are a projection of the engine's kind + parent graph.
use super::*;
use ac_core::{Layer, LayerKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LayerType {
    Base(LayerKind),
    Masked,
    Guided,
}
impl LayerType {
    fn label(self) -> &'static str {
        match self {
            Self::Base(LayerKind::Normal) => "layer-type-normal",
            Self::Base(LayerKind::Folder) => "layer-type-folder",
            Self::Base(LayerKind::Mask) => "layer-type-mask",
            Self::Base(LayerKind::Guide) => "layer-type-guide",
            Self::Base(LayerKind::Camera) => "layer-type-camera",
            Self::Masked => "layer-type-masked",
            Self::Guided => "layer-type-guided",
        }
    }
}
impl Studio {
    pub(super) fn layer_type(&self, at: usize) -> LayerType {
        let layers = &self.editor.timeline().layers;
        let l = &layers[at];
        if l.kind == LayerKind::Normal {
            match l.parent.map(|p| layers[p].kind) {
                Some(LayerKind::Mask) => return LayerType::Masked,
                Some(LayerKind::Guide) => return LayerType::Guided,
                _ => {}
            }
        }
        LayerType::Base(l.kind)
    }
    pub(super) fn planned_layer_type(
        &self,
        targets: &[usize],
        role: LayerType,
    ) -> Option<Vec<Layer>> {
        if targets.is_empty()
            || targets
                .iter()
                .any(|&i| !self.layer_name_editable(i) || !self.layer_block_unlocked(i))
        {
            return None;
        }
        let old = &self.editor.timeline().layers;
        let mut layers = old.clone();
        for &i in targets {
            let mut container = old[i].parent;
            if container.is_some_and(|p| matches!(old[p].kind, LayerKind::Mask | LayerKind::Guide))
            {
                container = old[container?].parent;
            }
            match role {
                LayerType::Masked | LayerType::Guided => {
                    if old.iter().any(|l| l.parent == Some(i)) {
                        return None;
                    }
                    let kind = if role == LayerType::Masked {
                        LayerKind::Mask
                    } else {
                        LayerKind::Guide
                    };
                    let parent = (0..i).rev().find(|p| {
                        old[*p].kind == kind && old[*p].parent == container && !targets.contains(p)
                    })?;
                    if !self.layer_name_editable(parent) {
                        return None;
                    }
                    layers[i].kind = LayerKind::Normal;
                    layers[i].parent = Some(parent);
                }
                LayerType::Base(kind) => {
                    if kind == LayerKind::Camera {
                        return None;
                    }
                    layers[i].kind = kind;
                    layers[i].parent = container;
                }
            }
        }
        // Removing a container releases its children into its surviving container.
        for i in 0..layers.len() {
            while let Some(p) = layers[i]
                .parent
                .filter(|&p| layers[p].kind == LayerKind::Normal)
            {
                layers[i].parent = layers[p].parent;
            }
            if layers[i].parent.is_some_and(|p| {
                matches!(layers[p].kind, LayerKind::Mask | LayerKind::Guide)
                    && layers[i].kind != LayerKind::Normal
            }) {
                return None;
            }
        }
        Some(layers)
    }
    pub(super) fn set_layer_type(&mut self, targets: &[usize], role: LayerType) {
        let Some(layers) = self.planned_layer_type(targets, role) else {
            return;
        };
        if layers == self.editor.timeline().layers {
            return;
        }
        fn walk(layers: &[Layer], parent: Option<usize>, out: &mut Vec<usize>) {
            for (i, l) in layers.iter().enumerate() {
                if l.parent == parent {
                    out.push(i);
                    walk(layers, Some(i), out);
                }
            }
        }
        let mut order = vec![];
        walk(&layers, None, &mut order);
        if order.len() != layers.len() {
            return;
        }
        let selected = order
            .iter()
            .enumerate()
            .filter_map(|(new, old)| targets.contains(old).then_some(new))
            .collect();
        self.finish_stage_gesture();
        self.editor.begin_edit("layer.type");
        self.editor.timeline_mut().layers = layers;
        self.reorder_layers(order, self.editor.layer);
        self.reset_timeline_gesture();
        self.set_layer_selection(selected, self.editor.layer);
        self.texture = None;
    }
    pub(super) fn layer_type_picker(&mut self, ui: &mut egui::Ui, targets: &[usize]) {
        ui.horizontal(|ui| {
            let label = ui.label(self.i18n.text("field-layer-type"));
            let current = self.layer_type(self.editor.layer);
            let mixed = targets.iter().any(|&i| self.layer_type(i) != current);
            widgets::combo_box(
                ui,
                "studio.layer.type",
                self.i18n.text(if mixed {
                    "field-mixed"
                } else {
                    current.label()
                }),
                ui.spacing().combo_width,
                |ui| {
                    for candidate in [
                        LayerType::Base(LayerKind::Normal),
                        LayerType::Base(LayerKind::Folder),
                        LayerType::Base(LayerKind::Guide),
                        LayerType::Guided,
                        LayerType::Base(LayerKind::Mask),
                        LayerType::Masked,
                    ] {
                        if ui
                            .add_enabled_ui(
                                match candidate {
                                    LayerType::Base(kind) => self.can_set_layer_kind(targets, kind),
                                    _ => self.planned_layer_type(targets, candidate).is_some(),
                                },
                                |ui| {
                                    widgets::selectable(
                                        ui,
                                        !mixed && current == candidate,
                                        self.i18n.text(candidate.label()),
                                    )
                                },
                            )
                            .inner
                            .clicked()
                        {
                            match candidate {
                                LayerType::Base(kind) => self.set_layer_kind_checked(targets, kind),
                                _ => self.set_layer_type(targets, candidate),
                            }
                            ui.close();
                        }
                    }
                },
            )
            .response
            .labelled_by(label.id);
        });
    }
}
