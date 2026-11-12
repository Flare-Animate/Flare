//! The Home page, laid out like Animate's: a narrow sidebar with New File and Open, and in the
//! main area the presets to create a project from, then the recent projects as cards.

use egui::{Align2, Color32, FontId, Rect, Sense, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};
use flare_core::{DocumentId, FrameRate, LayerId, Rgba8, SceneId};
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
/// Room for the title, and for each line of detail under it.
const LABEL_BASE: f32 = 32.0;
const DETAIL_HEIGHT: f32 = 14.0;

/// What the Home page keeps: the recent projects, what is known of each one's file, how the list is
/// filtered and sorted, and the Custom Size window while it is open.
#[derive(Default)]
pub(super) struct HomeState {
    pub(super) recent: Vec<PathBuf>,
    pub(super) info: flare_recent::RecentInfo,
    pub(super) search: String,
    pub(super) sort: flare_recent::Sorting,
    pub(super) new_document: Option<super::home_custom::NewDocument>,
    /// The kind of project the New Document dialog shows the sizes of.
    pub(super) new_document_kind: usize,
    /// Whether the dialog was opened from Home, so leaving Home closes it.
    pub(super) new_document_from_home: bool,
}

impl Workspace {
    /// Shows or hides the Home page over the document.
    pub fn set_home(&mut self, shown: bool) {
        if shown && !self.windows.home {
            // Files saved since the last look have a new time and size.
            self.home.info.refresh();
        }
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
        self.new_project_with(width, height, fps, Rgba8::WHITE);
    }

    /// A new project of the given stage size, frame rate and background color.
    pub fn new_project_with(&mut self, width: u32, height: u32, fps: u32, background: Rgba8) {
        match self.blank_document(width, height, fps, background) {
            Ok(document) => self.show_document(document, None),
            Err(error) => self.report(error),
        }
    }

    fn blank_document(
        &self,
        width: u32,
        height: u32,
        fps: u32,
        background: Rgba8,
    ) -> Result<Document, String> {
        let number = [("n", "1")];
        let layer = Layer::new(LayerId::new(), tr!(self, "name-new-layer", &number), vec![])
            .map_err(|e| e.to_string())?;
        let scene = Scene::new(
            SceneId::new(),
            tr!(self, "name-new-scene", &number),
            Timeline::new(vec![layer]),
        );
        let rate = FrameRate::new(fps, 1).map_err(|e| e.to_string())?;
        let document = Document::new(DocumentId::new(), rate, vec![scene], vec![])
            .map_err(|diagnostics| format!("{diagnostics:?}"))?;
        let stage = Stage::new(width, height, background).map_err(|e| e.to_string())?;
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
        ui.painter().rect_filled(side, 0.0, theme::panel());
        ui.painter().vline(
            side.max.x - 0.5,
            side.y_range(),
            Stroke::new(1.0, theme::border()),
        );
        ui.scope_builder(egui::UiBuilder::new().max_rect(side.shrink(14.0)), |ui| {
            self.home_sidebar(ui, &mut dialog);
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

    fn home_sidebar(&mut self, ui: &mut Ui, dialog: &mut Option<DialogRequest>) {
        // The program's own icon beside its name and version, as the About box and the dock show it.
        let version = tr!(
            self,
            "start-version",
            &[("version", env!("CARGO_PKG_VERSION"))]
        );
        ui.horizontal(|ui| {
            ui.add(icons::app_icon(40.0));
            ui.add_space(4.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.label(
                    egui::RichText::new("Flare")
                        .font(FontId::proportional(20.0))
                        .color(theme::text_strong()),
                );
                ui.label(
                    egui::RichText::new(version)
                        .font(FontId::proportional(11.0))
                        .color(theme::text_dim()),
                );
            });
        });
        ui.add_space(18.0);
        nav_item(ui, "home", &tr!(self, "start-nav-home"), true);
        // Planned, not built: shown dimmed, as the rest of the interface does.
        ui.add_enabled_ui(false, |ui| {
            nav_item(ui, "panel-library", &tr!(self, "start-nav-learn"), false);
        })
        .response
        .on_disabled_hover_text(chrome::NOT_YET);
        ui.add_space(18.0);
        let width = ui.available_width();
        if wide_button(ui, width, "file-new", &tr!(self, "start-new-file"), true).clicked() {
            self.show_new_document();
        }
        ui.add_space(6.0);
        if wide_button(ui, width, "file-open", &tr!(self, "start-open"), false).clicked() {
            *dialog = Some(DialogRequest::Open);
        }
    }

    fn home_main(
        &mut self,
        ui: &mut Ui,
        chosen: &mut Option<(u32, u32, u32)>,
        open: &mut Option<PathBuf>,
    ) {
        if let Some(status) = &self.status {
            ui.label(egui::RichText::new(status.as_str()).color(theme::warning()));
            ui.add_space(8.0);
        }
        section_title(ui, &tr!(self, "start-create-new"));
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(GAP, GAP);
            for (name, w, h, fps) in PRESETS {
                let response = tile(
                    ui,
                    &self.preset_title(name),
                    &[&format!("{w} × {h} · {fps} fps")],
                    |painter, rect| {
                        stage_glyph(painter, rect, w, h);
                    },
                );
                if response.clicked() {
                    *chosen = Some((w, h, fps));
                }
            }
            let custom = tile(
                ui,
                &tr!(self, "start-custom-size"),
                &[&tr!(self, "start-custom-hint")],
                plus_glyph,
            );
            if custom.clicked() {
                self.show_custom_size();
            }
        });
        ui.add_space(PAGE_MARGIN);
        ui.horizontal(|ui| {
            section_title(ui, &tr!(self, "start-recent"));
            // Clearing the list is in Preferences: here it sat next to the search box, one
            // slip from Enter's neighbour.
            if !self.home.recent.is_empty() {
                let hint = tr!(self, "start-filter");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.home.search)
                            .hint_text(hint)
                            .desired_width(220.0),
                    );
                });
            }
        });
        if self.home.recent.is_empty() {
            ui.label(egui::RichText::new(tr!(self, "start-recent-empty")).color(theme::text_dim()));
            return;
        }
        let ctx = ui.ctx().clone();
        self.home
            .info
            .ask(&self.home.recent, move || ctx.request_repaint());
        let mut shown: Vec<PathBuf> = self
            .home
            .recent
            .iter()
            .filter(|path| {
                let (name, folder) = name_and_folder(path);
                flare_recent::matches(&name, &folder, &self.home.search)
            })
            .cloned()
            .collect();
        if shown.is_empty() {
            ui.label(
                egui::RichText::new(tr!(
                    self,
                    "start-no-match",
                    &[("search", &self.home.search)]
                ))
                .color(theme::text_dim()),
            );
            return;
        }
        flare_recent::sort(&mut shown, self.home.sort, &self.home.info);
        shown.truncate(RECENT_SHOWN);
        if let Some(path) = self.recent_list(ui, &shown) {
            *open = Some(path);
        }
    }
}

/// The file's name and the folder it is in, as the cards and the search read them.
pub(super) fn name_and_folder(path: &std::path::Path) -> (String, String) {
    let name = path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    let folder = path
        .parent()
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    (name, folder)
}

pub(super) fn section_title(ui: &mut Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .font(FontId::proportional(16.0))
            .strong()
            .color(theme::text()),
    );
    ui.add_space(8.0);
}

fn nav_item(ui: &mut Ui, icon: &str, label: &str, selected: bool) {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::click());
    if selected || response.hovered() {
        let fill = if selected {
            theme::panel_raised()
        } else {
            theme::hover()
        };
        ui.painter().rect_filled(rect, 5.0, fill);
    }
    let ink = if ui.is_enabled() {
        theme::text()
    } else {
        theme::text_disabled()
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
        (true, true) => theme::accent().gamma_multiply(0.9),
        (true, false) => theme::accent().gamma_multiply(0.75),
        (false, true) => theme::hover(),
        (false, false) => theme::panel_raised(),
    };
    ui.painter().rect(
        rect,
        5.0,
        fill,
        Stroke::new(
            1.0,
            if primary {
                theme::accent()
            } else {
                theme::border()
            },
        ),
        StrokeKind::Inside,
    );
    let icon_rect =
        Rect::from_center_size(pos2(rect.left() + 18.0, rect.center().y), Vec2::splat(16.0));
    // White on the accent fill; on the plain fill, the palette's own text, which is dark on the
    // light palette.
    let ink = if primary {
        Color32::WHITE
    } else {
        theme::text()
    };
    icons::image(icon, 16.0, ink).paint_at(ui, icon_rect);
    ui.painter().text(
        pos2(rect.left() + 38.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(14.0),
        ink,
    );
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A card: a picture drawn by `preview` over a title and a line of detail.
fn tile(
    ui: &mut Ui,
    title: &str,
    details: &[&str],
    preview: impl FnOnce(&egui::Painter, Rect),
) -> egui::Response {
    chosen_tile(ui, title, details, false, preview)
}

/// A card as [`tile`] draws it, outlined in the accent color when `chosen`, as the New Document
/// dialog marks the size it will make.
pub(super) fn chosen_tile(
    ui: &mut Ui,
    title: &str,
    details: &[&str],
    chosen: bool,
    preview: impl FnOnce(&egui::Painter, Rect),
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(
        vec2(
            TILE_WIDTH,
            PREVIEW_HEIGHT + LABEL_BASE + DETAIL_HEIGHT * details.len() as f32,
        ),
        Sense::click(),
    );
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let border = if chosen {
            Stroke::new(2.0, theme::accent())
        } else if response.hovered() {
            Stroke::new(1.0, theme::accent())
        } else {
            Stroke::new(1.0, theme::border())
        };
        painter.rect(rect, 6.0, theme::panel(), border, StrokeKind::Inside);
        let top = Rect::from_min_size(
            rect.min + Vec2::splat(1.0),
            vec2(rect.width() - 2.0, PREVIEW_HEIGHT),
        );
        painter.rect_filled(top, 5.0, theme::stage_surround());
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
            theme::text(),
        );
        for (row, detail) in details.iter().rev().enumerate() {
            text.text(
                pos2(
                    rect.left() + 10.0,
                    rect.bottom() - 8.0 - row as f32 * DETAIL_HEIGHT,
                ),
                Align2::LEFT_BOTTOM,
                *detail,
                FontId::proportional(11.0),
                theme::text_dim(),
            );
        }
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A frame with the proportions of a stage, centered in `rect`: drawn in the interface's own
/// greys, not as a white sheet, so a row of them reads as shapes to pick between and not as glare.
pub(super) fn stage_glyph(painter: &egui::Painter, rect: Rect, width: u32, height: u32) {
    let room = rect.shrink2(vec2(36.0, 20.0));
    let scale = (room.width() / width as f32).min(room.height() / height as f32);
    let size = vec2(width as f32 * scale, height as f32 * scale);
    let glyph = Rect::from_center_size(rect.center(), size);
    painter.rect(
        glyph,
        3.0,
        theme::panel_raised(),
        Stroke::new(1.5, theme::text_dim()),
        StrokeKind::Inside,
    );
}

/// A plus, for the card that makes a project of a size of the person's choosing.
fn plus_glyph(painter: &egui::Painter, rect: Rect) {
    let c = rect.center();
    let stroke = Stroke::new(2.5, theme::text_dim());
    painter.hline((c.x - 14.0)..=(c.x + 14.0), c.y, stroke);
    painter.vline(c.x, (c.y - 14.0)..=(c.y + 14.0), stroke);
}
