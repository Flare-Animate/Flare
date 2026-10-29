//! The Home page: start a new project, open one, or go back to a recent one.

use egui::{Align, Color32, FontId, Layout, Rect, Sense, Stroke, Ui, Vec2, pos2, vec2};
use flare_core::{DocumentId, FrameRate, LayerId, SceneId};
use flare_document::{Document, Layer, Scene, Stage, Timeline};
use flare_ui::theme;
use std::path::PathBuf;

use super::*;

/// The sizes offered for a new project: name, width, height.
pub(super) const PRESETS: [(&str, u32, u32); 5] = [
    ("HD", 1920, 1080),
    ("720p", 1280, 720),
    ("Square", 1080, 1080),
    ("Portrait", 1080, 1920),
    ("Classic", 550, 400),
];

/// Most recent projects the Home page lists.
const RECENT_SHOWN: usize = 8;

/// Space between the page's edge and its content, and between its blocks.
const PAGE_MARGIN: f32 = 32.0;
const GAP: f32 = 12.0;
const CARD_HEIGHT: f32 = 52.0;

impl Workspace {
    /// Shows or hides the Home page over the document.
    pub fn set_home(&mut self, shown: bool) {
        self.windows.home = shown;
    }

    pub fn home_shown(&self) -> bool {
        self.windows.home
    }

    /// A new project with a stage of `width` by `height`: in place of an untouched new one,
    /// else in a new tab. Refuses a size the stage cannot have.
    pub fn new_project(&mut self, width: u32, height: u32) {
        match Self::blank_document(width, height) {
            Ok(document) => self.show_document(document, None),
            Err(error) => self.report(error),
        }
    }

    fn blank_document(width: u32, height: u32) -> Result<Document, String> {
        let layer = Layer::new(LayerId::new(), "Layer 1", vec![]).map_err(|e| e.to_string())?;
        let scene = Scene::new(SceneId::new(), "Scene 1", Timeline::new(vec![layer]));
        let rate = FrameRate::new(24, 1).map_err(|e| e.to_string())?;
        let document = Document::new(DocumentId::new(), rate, vec![scene], vec![])
            .map_err(|diagnostics| format!("{diagnostics:?}"))?;
        let stage =
            Stage::new(width, height, document.stage().background()).map_err(|e| e.to_string())?;
        Ok(document.with_stage(stage))
    }

    /// The Home page, filling the area the dock otherwise uses.
    pub(super) fn home_page(&mut self, ui: &mut Ui) {
        let mut new_size = None;
        let mut open = None;
        let mut dialog = None;
        let mut leave = false;
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                let width = (ui.available_width() - PAGE_MARGIN * 2.0).clamp(280.0, 820.0);
                ui.add_space(PAGE_MARGIN);
                ui.vertical_centered(|ui| {
                    ui.set_max_width(width);
                    ui.horizontal(|ui| {
                        ui.add(flare_ui::icons::image("home", 28.0, theme::ACCENT));
                        ui.vertical(|ui| {
                            ui.label(
                                egui::RichText::new("Flare")
                                    .font(FontId::proportional(26.0))
                                    .color(theme::TEXT)
                                    .strong(),
                            );
                            ui.label(
                                egui::RichText::new(format!(
                                    "Version {} · early alpha, keep backups of your work",
                                    env!("CARGO_PKG_VERSION")
                                ))
                                .color(theme::TEXT_DIM),
                            );
                        });
                    });
                    ui.add_space(PAGE_MARGIN * 0.75);
                    let columns = if width >= 560.0 { 2 } else { 1 };
                    let column = (width - GAP * (columns - 1) as f32) / columns as f32;
                    ui.with_layout(Layout::left_to_right(Align::TOP), |ui| {
                        ui.spacing_mut().item_spacing = vec2(GAP, GAP);
                        ui.allocate_ui(vec2(column, 0.0), |ui| {
                            ui.vertical(|ui| {
                                heading(ui, "New project");
                                for (name, w, h) in PRESETS {
                                    let detail = format!("{w} × {h} px · 24 fps");
                                    if card(ui, column, name, &detail).clicked() {
                                        new_size = Some((w, h));
                                    }
                                }
                                heading(ui, "Open");
                                if card(ui, column, "Open a project…", "A .flare file").clicked()
                                {
                                    dialog = Some(DialogRequest::Open);
                                }
                                if card(ui, column, "Import…", "An XFL folder or a .fla file")
                                    .clicked()
                                {
                                    dialog = Some(DialogRequest::ImportProject);
                                }
                            });
                        });
                        if columns == 2 {
                            ui.allocate_ui(vec2(column, 0.0), |ui| {
                                self.recent_column(ui, column, &mut open);
                            });
                        }
                    });
                    if columns == 1 {
                        ui.add_space(GAP);
                        self.recent_column(ui, width, &mut open);
                    }
                    ui.add_space(PAGE_MARGIN * 0.5);
                    if ui.link("Back to the document").clicked() {
                        leave = true;
                    }
                });
                ui.add_space(PAGE_MARGIN);
            });
        if let Some((w, h)) = new_size {
            self.new_project(w, h);
        }
        if let Some(path) = open {
            self.open(&path);
        }
        if dialog.is_some() {
            self.dialog = dialog;
        }
        if leave {
            self.windows.home = false;
        }
    }

    fn recent_column(&mut self, ui: &mut Ui, width: f32, open: &mut Option<PathBuf>) {
        ui.vertical(|ui| {
            heading(ui, "Recent");
            if self.recent.is_empty() {
                ui.label(
                    egui::RichText::new("Projects you open or save show up here.")
                        .color(theme::TEXT_DIM),
                );
            }
            for path in self.recent.iter().take(RECENT_SHOWN) {
                let name = path.file_name().map_or_else(
                    || path.display().to_string(),
                    |n| n.to_string_lossy().into_owned(),
                );
                let folder = path
                    .parent()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default();
                if card(ui, width, &name, &folder).clicked() {
                    *open = Some(path.clone());
                }
            }
            if !self.recent.is_empty() && ui.small_button("Clear the list").clicked() {
                self.recent.clear();
            }
        });
    }
}

fn heading(ui: &mut Ui, text: &str) {
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new(text.to_uppercase())
            .font(FontId::proportional(11.0))
            .color(theme::TEXT_DIM)
            .strong(),
    );
}

/// A wide button with a title over a smaller line of detail.
fn card(ui: &mut Ui, width: f32, title: &str, detail: &str) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(width, CARD_HEIGHT), Sense::click());
    if ui.is_rect_visible(rect) {
        let fill = if response.hovered() {
            theme::HOVER
        } else {
            theme::PANEL_RAISED
        };
        let painter = ui.painter();
        painter.rect(
            rect,
            6.0,
            fill,
            Stroke::new(1.0, theme::BORDER),
            egui::StrokeKind::Inside,
        );
        let inner = Rect::from_min_max(rect.min + Vec2::splat(12.0), rect.max - Vec2::splat(12.0));
        let clipped = painter.with_clip_rect(inner);
        clipped.text(
            pos2(inner.left(), inner.top()),
            egui::Align2::LEFT_TOP,
            title,
            FontId::proportional(14.0),
            theme::TEXT,
        );
        clipped.text(
            pos2(inner.left(), inner.bottom()),
            egui::Align2::LEFT_BOTTOM,
            detail,
            FontId::proportional(12.0),
            Color32::from_gray(0x8a),
        );
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}
