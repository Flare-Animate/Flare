//! The Home page, laid out like Animate's: a narrow sidebar with New File and Open, and in the
//! main area the presets to create a project from, then the recent projects as cards.

use egui::{Align2, Color32, FontId, Rect, Sense, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};
use flare_core::{DocumentId, FrameRate, LayerId, SceneId};
use flare_document::{Document, Layer, Scene, Stage, Timeline};
use flare_ui::{chrome, icons, theme};
use std::path::PathBuf;

use super::*;

/// The sizes offered for a new project: name, width, height, frames per second.
pub(super) const PRESETS: [(&str, u32, u32, u32); 5] = [
    ("Character Animation", 1920, 1080, 24),
    ("Social", 1080, 1080, 30),
    ("Portrait", 1080, 1920, 30),
    ("Web", 1280, 720, 24),
    ("Classic", 550, 400, 24),
];

/// Most recent projects the Home page lists.
const RECENT_SHOWN: usize = 12;

const SIDEBAR_WIDTH: f32 = 190.0;
const PAGE_MARGIN: f32 = 28.0;
const GAP: f32 = 12.0;
const TILE_WIDTH: f32 = 168.0;
const PREVIEW_HEIGHT: f32 = 96.0;
const LABEL_HEIGHT: f32 = 46.0;

impl Workspace {
    /// Shows or hides the Home page over the document.
    pub fn set_home(&mut self, shown: bool) {
        self.windows.home = shown;
    }

    pub fn home_shown(&self) -> bool {
        self.windows.home
    }

    /// Whether the top bar shows undo, redo, play and the document's name, and the window its
    /// status bar: not on Home, where no document is on show.
    pub(super) fn document_controls_shown(&self) -> bool {
        !self.windows.home
    }

    /// A new project with a stage of `width` by `height` at `fps`: in place of an untouched new
    /// one, else in a new tab. Refuses a size or rate the document cannot have.
    pub fn new_project(&mut self, width: u32, height: u32, fps: u32) {
        match Self::blank_document(width, height, fps) {
            Ok(document) => self.show_document(document, None),
            Err(error) => self.report(error),
        }
    }

    fn blank_document(width: u32, height: u32, fps: u32) -> Result<Document, String> {
        let layer = Layer::new(LayerId::new(), "Layer 1", vec![]).map_err(|e| e.to_string())?;
        let scene = Scene::new(SceneId::new(), "Scene 1", Timeline::new(vec![layer]));
        let rate = FrameRate::new(fps, 1).map_err(|e| e.to_string())?;
        let document = Document::new(DocumentId::new(), rate, vec![scene], vec![])
            .map_err(|diagnostics| format!("{diagnostics:?}"))?;
        let stage =
            Stage::new(width, height, document.stage().background()).map_err(|e| e.to_string())?;
        Ok(document.with_stage(stage))
    }

    /// The Home page, filling the area the dock otherwise uses.
    pub(super) fn home_page(&mut self, ui: &mut Ui) {
        let mut chosen: Option<(u32, u32, u32)> = None;
        let mut open: Option<PathBuf> = None;
        let mut dialog = None;
        let area = ui.available_rect_before_wrap();
        let side = Rect::from_min_max(area.min, pos2(area.min.x + SIDEBAR_WIDTH, area.max.y));
        let main = Rect::from_min_max(pos2(side.max.x, area.min.y), area.max);
        ui.painter().rect_filled(side, 0.0, theme::PANEL);
        ui.painter().vline(
            side.max.x - 0.5,
            side.y_range(),
            Stroke::new(1.0, theme::BORDER),
        );
        ui.scope_builder(egui::UiBuilder::new().max_rect(side.shrink(14.0)), |ui| {
            self.home_sidebar(ui, &mut chosen, &mut dialog);
        });
        ui.scope_builder(
            egui::UiBuilder::new().max_rect(main.shrink2(vec2(PAGE_MARGIN, 0.0))),
            |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink(false)
                    .show(ui, |ui| {
                        ui.add_space(PAGE_MARGIN);
                        self.home_main(ui, &mut chosen, &mut open);
                        ui.add_space(PAGE_MARGIN);
                    });
            },
        );
        if let Some((w, h, fps)) = chosen {
            self.new_project(w, h, fps);
        }
        if let Some(path) = open {
            self.open(&path);
        }
        if dialog.is_some() {
            self.dialog = dialog;
        }
    }

    fn home_sidebar(
        &mut self,
        ui: &mut Ui,
        chosen: &mut Option<(u32, u32, u32)>,
        dialog: &mut Option<DialogRequest>,
    ) {
        ui.horizontal(|ui| {
            ui.add(icons::image("home", 22.0, theme::ACCENT));
            ui.label(
                egui::RichText::new("Flare")
                    .font(FontId::proportional(20.0))
                    .strong()
                    .color(theme::TEXT),
            );
        });
        ui.label(
            egui::RichText::new(format!("Version {}", env!("CARGO_PKG_VERSION")))
                .font(FontId::proportional(11.0))
                .color(theme::TEXT_DIM),
        );
        ui.add_space(18.0);
        nav_item(ui, "home", "Home", true);
        // Planned, not built: shown dimmed, as the rest of the interface does.
        ui.add_enabled_ui(false, |ui| {
            nav_item(ui, "panel-library", "Learn", false);
        })
        .response
        .on_disabled_hover_text(chrome::NOT_YET);
        ui.add_space(18.0);
        let width = ui.available_width();
        if wide_button(ui, width, "file-new", "New File", true).clicked() {
            let (_, w, h, fps) = PRESETS[0];
            *chosen = Some((w, h, fps));
        }
        ui.add_space(6.0);
        if wide_button(ui, width, "file-open", "Open…", false).clicked() {
            *dialog = Some(DialogRequest::Open);
        }
        ui.add_space(6.0);
        if wide_button(ui, width, "file-import", "Import…", false)
            .on_hover_text("An XFL folder or a .fla file")
            .clicked()
        {
            *dialog = Some(DialogRequest::ImportProject);
        }
    }

    fn home_main(
        &mut self,
        ui: &mut Ui,
        chosen: &mut Option<(u32, u32, u32)>,
        open: &mut Option<PathBuf>,
    ) {
        if let Some(status) = &self.status {
            ui.label(egui::RichText::new(status.as_str()).color(theme::WARNING));
            ui.add_space(8.0);
        }
        section_title(ui, "Create new");
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(GAP, GAP);
            for (name, w, h, fps) in PRESETS {
                let response = tile(
                    ui,
                    name,
                    &format!("{w} × {h} · {fps} fps"),
                    |painter, rect| {
                        stage_glyph(painter, rect, w, h);
                    },
                );
                if response.clicked() {
                    *chosen = Some((w, h, fps));
                }
            }
        });
        ui.add_space(PAGE_MARGIN);
        ui.horizontal(|ui| {
            section_title(ui, "Recent");
            if !self.recent.is_empty() {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("Clear the list").clicked() {
                        self.recent.clear();
                    }
                });
            }
        });
        if self.recent.is_empty() {
            ui.label(
                egui::RichText::new("Projects you open or save show up here.")
                    .color(theme::TEXT_DIM),
            );
            return;
        }
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(GAP, GAP);
            for path in self.recent.iter().take(RECENT_SHOWN) {
                let name = path.file_name().map_or_else(
                    || path.display().to_string(),
                    |n| n.to_string_lossy().into_owned(),
                );
                let folder = path
                    .parent()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default();
                let response = tile(ui, &name, &folder, |painter, rect| {
                    file_glyph(painter, rect);
                })
                .on_hover_text(path.display().to_string());
                if response.clicked() {
                    *open = Some(path.clone());
                }
            }
        });
    }
}

fn section_title(ui: &mut Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .font(FontId::proportional(16.0))
            .strong()
            .color(theme::TEXT),
    );
    ui.add_space(8.0);
}

fn nav_item(ui: &mut Ui, icon: &str, label: &str, selected: bool) {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::click());
    if selected || response.hovered() {
        let fill = if selected {
            theme::PANEL_RAISED
        } else {
            theme::HOVER
        };
        ui.painter().rect_filled(rect, 5.0, fill);
    }
    let ink = if ui.is_enabled() {
        theme::TEXT
    } else {
        theme::TEXT_DISABLED
    };
    let image = icons::image(icon, 16.0, ink);
    let icon_rect =
        Rect::from_center_size(pos2(rect.left() + 16.0, rect.center().y), Vec2::splat(16.0));
    image.paint_at(ui, icon_rect);
    ui.painter().text(
        pos2(rect.left() + 34.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(14.0),
        ink,
    );
}

/// A full-width button with an icon; the primary one is filled with the accent.
fn wide_button(ui: &mut Ui, width: f32, icon: &str, label: &str, primary: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(width, 32.0), Sense::click());
    let fill = match (primary, response.hovered()) {
        (true, true) => theme::ACCENT.gamma_multiply(0.9),
        (true, false) => theme::ACCENT.gamma_multiply(0.75),
        (false, true) => theme::HOVER,
        (false, false) => theme::PANEL_RAISED,
    };
    ui.painter().rect(
        rect,
        5.0,
        fill,
        Stroke::new(
            1.0,
            if primary {
                theme::ACCENT
            } else {
                theme::BORDER
            },
        ),
        StrokeKind::Inside,
    );
    let icon_rect =
        Rect::from_center_size(pos2(rect.left() + 18.0, rect.center().y), Vec2::splat(16.0));
    icons::image(icon, 16.0, Color32::WHITE).paint_at(ui, icon_rect);
    ui.painter().text(
        pos2(rect.left() + 38.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(14.0),
        Color32::WHITE,
    );
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A card: a picture drawn by `preview` over a title and a line of detail.
fn tile(
    ui: &mut Ui,
    title: &str,
    detail: &str,
    preview: impl FnOnce(&egui::Painter, Rect),
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(
        vec2(TILE_WIDTH, PREVIEW_HEIGHT + LABEL_HEIGHT),
        Sense::click(),
    );
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let border = if response.hovered() {
            theme::ACCENT
        } else {
            theme::BORDER
        };
        painter.rect(
            rect,
            6.0,
            theme::PANEL,
            Stroke::new(1.0, border),
            StrokeKind::Inside,
        );
        let top = Rect::from_min_size(
            rect.min + Vec2::splat(1.0),
            vec2(rect.width() - 2.0, PREVIEW_HEIGHT),
        );
        painter.rect_filled(top, 5.0, theme::STAGE_SURROUND);
        preview(&painter.with_clip_rect(top), top);
        let text = painter.with_clip_rect(Rect::from_min_max(
            pos2(rect.left() + 10.0, top.bottom()),
            pos2(rect.right() - 10.0, rect.bottom()),
        ));
        text.text(
            pos2(rect.left() + 10.0, top.bottom() + 8.0),
            Align2::LEFT_TOP,
            title,
            FontId::proportional(13.0),
            theme::TEXT,
        );
        text.text(
            pos2(rect.left() + 10.0, rect.bottom() - 8.0),
            Align2::LEFT_BOTTOM,
            detail,
            FontId::proportional(11.0),
            theme::TEXT_DIM,
        );
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A rectangle with the proportions of a stage, centered in `rect`.
fn stage_glyph(painter: &egui::Painter, rect: Rect, width: u32, height: u32) {
    let room = rect.shrink2(vec2(30.0, 16.0));
    let scale = (room.width() / width as f32).min(room.height() / height as f32);
    let size = vec2(width as f32 * scale, height as f32 * scale);
    let glyph = Rect::from_center_size(rect.center(), size);
    painter.rect(
        glyph,
        2.0,
        Color32::WHITE,
        Stroke::new(1.0, theme::BORDER),
        StrokeKind::Inside,
    );
}

/// A page with a folded corner, for a project that has no picture.
fn file_glyph(painter: &egui::Painter, rect: Rect) {
    let page = Rect::from_center_size(rect.center(), vec2(46.0, 60.0));
    painter.rect(
        page,
        3.0,
        theme::PANEL_RAISED,
        Stroke::new(1.5, theme::TEXT_DIM),
        StrokeKind::Inside,
    );
    for row in 0..3 {
        let y = page.top() + 24.0 + row as f32 * 9.0;
        painter.hline(
            (page.left() + 9.0)..=(page.right() - 9.0),
            y,
            Stroke::new(1.5, theme::BORDER),
        );
    }
}
