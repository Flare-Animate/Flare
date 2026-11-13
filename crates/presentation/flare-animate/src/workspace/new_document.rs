//! The New Document dialog, laid out as Animate's: the kinds of project as tabs across the top,
//! the chosen kind's sizes as cards on the left, and the details (width, height, units, frame
//! rate, background) with Create on the right. It is drawn as the rest of Flare's dialogs are:
//! in the middle of the window over a dimmed backdrop, with Home's cards and headings.

use egui::{Align2, Color32, Context, FontId, Rect, Sense, Stroke, Ui, Vec2, vec2};
use flare_core::Rgba8;
use flare_document::MAX_STAGE_SIDE;
use flare_ui::{chrome, theme};

use super::home::{chosen_tile, section_title, stage_glyph};
use super::home_custom::NewDocument;
use super::new_document_presets::{self as presets, Preset};
use super::*;

/// The most frames per second a project can be made at (the document settings allow the same).
const MAX_FPS: u32 = 240;
const TAB: Vec2 = vec2(92.0, 58.0);
/// Room for three of Home's cards side by side.
const PRESETS_WIDTH: f32 = 3.0 * 168.0 + 2.0 * 12.0;
const DETAILS_WIDTH: f32 = 210.0;

/// What the dialog's buttons asked for on this pass.
#[derive(Default)]
struct Asked {
    create: bool,
    cancel: bool,
}

impl Workspace {
    pub(super) fn new_document_window(&mut self, ctx: &Context) {
        // Opened from Home, it belongs to it: a tab chosen over Home takes it along.
        if self.home.new_document_from_home && !self.windows.home {
            self.home.new_document = None;
        }
        let Some(mut doc) = self.home.new_document else {
            return;
        };
        let mut asked = Asked::default();
        // A dialog in the middle of the window over a dimmed backdrop, as Preferences and
        // Publish are: Escape or a click outside closes it.
        let modal =
            flare_ui::dialog::modal(ctx, egui::Id::unique("new_document_dialog")).show(ctx, |ui| {
                let title = self.tr("start-new-document");
                let close = self.tr("close");
                egui::Sides::new().show(
                    ui,
                    |ui| {
                        ui.heading(title);
                    },
                    |ui| {
                        if chrome::icon_button(ui, "close", 14.0, false, true, &close).clicked() {
                            asked.cancel = true;
                        }
                    },
                );
                ui.add_space(6.0);
                self.new_document_tabs(ui, &mut doc);
                ui.add_space(12.0);
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = 16.0;
                    ui.vertical(|ui| self.new_document_presets(ui, &mut doc));
                    ui.vertical(|ui| asked = self.new_document_details(ui, &mut doc, asked.cancel));
                });
            });
        if modal.should_close() {
            asked.cancel = true;
        }
        self.home.new_document = Some(doc);
        if asked.create {
            self.home.new_document = None;
            self.new_project_with(doc.width, doc.height, doc.fps, doc.background);
        } else if asked.cancel {
            self.home.new_document = None;
        }
    }

    /// The kinds of project, each an icon over its name; the chosen one is underlined.
    fn new_document_tabs(&mut self, ui: &mut Ui, doc: &mut NewDocument) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            for (index, kind) in presets::CATEGORIES.iter().enumerate() {
                let (rect, response) = ui.allocate_exact_size(TAB, Sense::click());
                let chosen = index == self.home.new_document_kind;
                if response.hovered() && !chosen {
                    ui.painter().rect_filled(rect, 6.0, theme::hover());
                }
                let ink = if chosen || response.hovered() {
                    theme::text_strong()
                } else {
                    theme::text_dim()
                };
                flare_ui::icons::image(kind.icon, 24.0, ink).paint_at(
                    ui,
                    Rect::from_center_size(rect.center_top() + vec2(0.0, 19.0), Vec2::splat(24.0)),
                );
                ui.painter().text(
                    rect.center_top() + vec2(0.0, 43.0),
                    Align2::CENTER_CENTER,
                    self.tr(kind.name),
                    FontId::proportional(12.0),
                    ink,
                );
                if chosen {
                    ui.painter().hline(
                        (rect.left() + 12.0)..=(rect.right() - 12.0),
                        rect.bottom() - 1.0,
                        Stroke::new(2.0, theme::accent()),
                    );
                }
                if response.clicked() && !chosen {
                    self.home.new_document_kind = index;
                    if let Some(first) = kind.presets.first() {
                        apply(doc, first);
                    }
                }
            }
        });
        ui.separator();
    }

    /// The chosen kind's sizes as Home's cards, each drawn with its stage's proportions.
    fn new_document_presets(&mut self, ui: &mut Ui, doc: &mut NewDocument) {
        ui.set_width(PRESETS_WIDTH);
        let kind = presets::category(self.home.new_document_kind);
        let chosen = presets::matching(kind, doc.width, doc.height, doc.fps);
        let heading = tr!(
            self,
            "start-presets",
            &[("count", &kind.presets.len().to_string())]
        );
        section_title(ui, &heading);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(12.0, 12.0);
            for (index, preset) in kind.presets.iter().enumerate() {
                let (w, h, fps) = (preset.width, preset.height, preset.fps);
                let response = chosen_tile(
                    ui,
                    &self.tr(preset.name),
                    &[&format!("{w} × {h} · {fps} fps")],
                    chosen == Some(index),
                    |painter, rect| stage_glyph(painter, rect, w, h),
                );
                if response.clicked() {
                    apply(doc, preset);
                }
            }
        });
    }

    /// The numbers the project is made with, one under another, and Create.
    fn new_document_details(&self, ui: &mut Ui, doc: &mut NewDocument, cancel: bool) -> Asked {
        ui.set_width(DETAILS_WIDTH);
        let mut asked = Asked {
            create: false,
            cancel,
        };
        section_title(ui, &self.tr("start-details"));
        let field = |ui: &mut Ui, label: String| {
            ui.label(egui::RichText::new(label).color(theme::text_dim()));
        };
        egui::Frame::new()
            .fill(theme::panel())
            .stroke(Stroke::new(1.0, theme::border()))
            .corner_radius(6.0)
            .inner_margin(12.0)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.columns(2, |cols| {
                    field(&mut cols[0], self.tr("start-width"));
                    cols[0].add(egui::DragValue::new(&mut doc.width).range(1..=MAX_STAGE_SIDE));
                    field(&mut cols[1], self.tr("start-height"));
                    cols[1].add(egui::DragValue::new(&mut doc.height).range(1..=MAX_STAGE_SIDE));
                });
                ui.add_space(8.0);
                field(ui, self.tr("start-units"));
                // Pixels are the only unit a stage has; the list is there because Animate's is.
                ui.add_enabled_ui(false, |ui| {
                    egui::ComboBox::from_id_salt("units_of_a_new_document")
                        .width(ui.available_width())
                        .selected_text(self.tr("start-pixels"))
                        .show_ui(ui, |_| {});
                });
                ui.add_space(8.0);
                field(ui, self.tr("start-frame-rate"));
                ui.add(
                    egui::DragValue::new(&mut doc.fps)
                        .range(1..=MAX_FPS)
                        .suffix(" fps"),
                );
                ui.add_space(8.0);
                field(ui, self.tr("start-background"));
                let [r, g, b, a] = doc.background.to_array();
                let mut color = Color32::from_rgba_unmultiplied(r, g, b, a);
                if ui.color_edit_button_srgba(&mut color).changed() {
                    let [r, g, b, a] = color.to_array();
                    doc.background = Rgba8::new(r, g, b, a);
                }
            });
        ui.add_space(16.0);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let create = egui::Button::new(
                egui::RichText::new(self.tr("start-create")).color(Color32::WHITE),
            )
            .fill(theme::accent())
            .min_size(vec2(76.0, 26.0));
            asked.create = ui.add(create).clicked();
            if ui.button(self.tr("start-cancel")).clicked() {
                asked.cancel = true;
            }
        });
        asked
    }
}

/// Takes a preset's numbers, keeping the background chosen.
fn apply(doc: &mut NewDocument, preset: &Preset) {
    doc.width = preset.width;
    doc.height = preset.height;
    doc.fps = preset.fps;
}
