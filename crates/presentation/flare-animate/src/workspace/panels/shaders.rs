//! The Shaders panel: the project's named shader graphs, each with a small picture of what it
//! draws, and Import, Export and Remove. The edits are `flare_editor::shader_library`'s, the same
//! `flare shader` and the MCP server make, so each is one undo step; the graph file is
//! `flare_import`'s versioned JSON.

use std::collections::HashMap;

use egui::{TextureHandle, Ui};
use flare_core::ShaderGraph;
use flare_editor::shader_library;
use flare_ui::{chrome, theme};

use crate::workspace::{DialogRequest, Workspace};

/// The side of a graph's picture, in pixels.
const THUMB: u32 = 48;

/// What the panel remembers: the graph chosen and the pictures drawn, each kept while its graph
/// stays as it was (drawing 48 x 48 pixels of a 256-node graph is not free).
#[derive(Default)]
pub(crate) struct ShaderPanel {
    pub(crate) selected: Option<String>,
    thumbs: HashMap<String, (ShaderGraph, TextureHandle)>,
}

impl Workspace {
    /// Adds the graph in `bytes` (a graph file) to the project as `name`, or replaces the graph
    /// of that name: one undo step. Says why when the file is not a graph.
    pub fn import_shader_graph(&mut self, name: &str, bytes: &[u8]) {
        let name = name.trim().to_owned();
        let command = flare_import::import_shader_graph(bytes)
            .map_err(|e| e.to_string())
            .and_then(|graph| {
                shader_library::set_graph(self.document(), &name, graph).map_err(|e| e.to_string())
            });
        match command {
            Ok(command) => {
                if self
                    .active_mut()
                    .apply_labeled([command], "Import a shader graph")
                    .is_ok()
                {
                    self.shaders.selected = Some(name);
                } else {
                    self.report(tr!(
                        self,
                        "shaders-cannot-import",
                        &[("name", &name), ("error", "the project cannot hold it")]
                    ));
                }
            }
            Err(error) => self.report(tr!(
                self,
                "shaders-cannot-import",
                &[("name", &name), ("error", &error)]
            )),
        }
    }

    /// The chosen graph's name and its graph file, for the shell to write.
    pub fn shader_export(&self) -> Option<(String, Vec<u8>)> {
        let name = self.shaders.selected.clone()?;
        let asset = shader_library::find(self.document(), &name)?;
        let bytes = flare_import::export_shader_graph(asset.graph()).ok()?;
        Some((name, bytes))
    }

    /// Says a graph file could not be written.
    pub fn shader_export_failed(&mut self, name: &str, error: &str) {
        self.report(tr!(
            self,
            "shaders-cannot-export",
            &[("name", name), ("error", error)]
        ));
    }

    pub(crate) fn remove_selected_shader(&mut self) {
        let Some(name) = self.shaders.selected.clone() else {
            return;
        };
        if let Ok(command) = shader_library::remove_graph(self.document(), &name)
            && self
                .active_mut()
                .apply_labeled([command], "Remove a shader graph")
                .is_ok()
        {
            self.shaders.selected = None;
            self.shaders.thumbs.remove(&name);
        }
    }

    /// The picture of `graph`, drawn again only when the graph changed.
    fn shader_thumb(
        &mut self,
        ctx: &egui::Context,
        name: &str,
        graph: &ShaderGraph,
    ) -> Option<TextureHandle> {
        if let Some((cached, texture)) = self.shaders.thumbs.get(name)
            && cached == graph
        {
            return Some(texture.clone());
        }
        let image = flare_render::render_shader(graph, THUMB, THUMB, 0.0).ok()?;
        let picture = egui::ColorImage::from_rgba_unmultiplied(
            [THUMB as usize, THUMB as usize],
            image.rgba(),
        );
        let texture = ctx.load_texture(
            format!("shader-{name}"),
            picture,
            egui::TextureOptions::LINEAR,
        );
        self.shaders
            .thumbs
            .insert(name.to_owned(), (graph.clone(), texture.clone()));
        Some(texture)
    }

    pub(crate) fn shaders_panel(&mut self, ui: &mut Ui) {
        let assets: Vec<(String, ShaderGraph)> = self
            .document()
            .shader_assets()
            .iter()
            .map(|a| (a.name().to_owned(), a.graph().clone()))
            .collect();
        if self
            .shaders
            .selected
            .as_ref()
            .is_some_and(|name| !assets.iter().any(|(n, _)| n == name))
        {
            self.shaders.selected = None;
        }
        let chosen = self.shaders.selected.is_some();
        ui.horizontal(|ui| {
            if ui.button(tr!(self, "shaders-import")).clicked() {
                self.dialog = Some(DialogRequest::ImportShader);
            }
            if ui
                .add_enabled(chosen, egui::Button::new(tr!(self, "shaders-export")))
                .on_disabled_hover_text(tr!(self, "shaders-select"))
                .clicked()
            {
                self.dialog = Some(DialogRequest::ExportShader);
            }
            if ui
                .add_enabled(chosen, egui::Button::new(tr!(self, "shaders-remove")))
                .on_disabled_hover_text(tr!(self, "shaders-select"))
                .clicked()
            {
                self.remove_selected_shader();
            }
        });
        ui.separator();
        if assets.is_empty() {
            chrome::empty_state(ui, "panel-shaders", &tr!(self, "shaders-empty"));
            return;
        }
        let ctx = ui.ctx().clone();
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (name, graph) in &assets {
                    let texture = self.shader_thumb(&ctx, name, graph);
                    let selected = self.shaders.selected.as_deref() == Some(name.as_str());
                    ui.horizontal(|ui| {
                        if let Some(texture) = texture {
                            ui.image((texture.id(), egui::vec2(THUMB as f32, THUMB as f32)));
                        }
                        ui.vertical(|ui| {
                            if ui.selectable_label(selected, name).clicked() {
                                self.shaders.selected = Some(name.clone());
                            }
                            let count = graph.nodes().len().to_string();
                            ui.label(
                                egui::RichText::new(tr!(
                                    self,
                                    "shaders-nodes",
                                    &[("count", &count)]
                                ))
                                .color(theme::text_dim()),
                            );
                        });
                    });
                }
                ui.add_space(6.0);
                ui.label(egui::RichText::new(tr!(self, "shaders-note")).color(theme::text_dim()));
            });
    }
}
