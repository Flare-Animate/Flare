//! Modify > Convert to Symbol (F8): Animate's dialog with Name, Type and
//! Registration. Insert > New Symbol (Ctrl+F8) uses it too, as Create New
//! Symbol: no registration, and an empty symbol that opens for editing.

use eframe::egui::{self, Align, Color32, Key, Layout, RichText, Sense, Stroke, StrokeKind, vec2};
use minnamate_core::doc::SymbolKind;
use minnamate_core::{hit, symbols};

use crate::app::{Editor, PropsTab};
use crate::theme;

pub struct Dialog {
    name: String,
    kind: SymbolKind,
    /// Registration cell, column and row 0..=2; Animate starts at the top left.
    registration: (u8, u8),
    /// Insert > New Symbol rather than Convert to Symbol.
    new: bool,
}

/// Opens the dialog for the current selection; does nothing without one.
pub fn open(ed: &mut Editor) {
    // ponytail: converts on a scene's timeline only; inside a symbol it needs
    // symbols::convert_to_symbol to take any timeline.
    if !ed.editing.is_empty() {
        ed.log("Convert to Symbol works on the scene; leave the symbol first.");
        return;
    }
    if !ed.selection.is_empty() {
        ed.convert = Some(Dialog { name: symbols::next_symbol_name(&ed.doc), kind: SymbolKind::MovieClip, registration: (0, 0), new: false });
    }
}

/// Opens it as Create New Symbol.
pub fn open_new(ed: &mut Editor) {
    ed.convert = Some(Dialog { name: symbols::next_symbol_name(&ed.doc), kind: SymbolKind::MovieClip, registration: (0, 0), new: true });
}

pub fn show(ctx: &egui::Context, ed: &mut Editor) {
    let Some(dialog) = &mut ed.convert else { return };
    let mut ok = ctx.input(|i| i.key_pressed(Key::Enter));
    let mut cancel = false;
    let modal = egui::Modal::new(egui::Id::new("convert-to-symbol")).show(ctx, |ui| {
        ui.set_width(380.0);
        let title = if dialog.new { "Create New Symbol" } else { "Convert to Symbol" };
        ui.label(RichText::new(title).heading().color(theme::TEXT));
        ui.add_space(8.0);
        egui::Grid::new("convert-fields").num_columns(2).spacing([12.0, 10.0]).show(ui, |ui| {
            ui.label(RichText::new("Name").color(theme::TEXT_DIM));
            ui.add(egui::TextEdit::singleline(&mut dialog.name).desired_width(240.0)).request_focus();
            ui.end_row();
            ui.label(RichText::new("Type").color(theme::TEXT_DIM));
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt("symbol-type").selected_text(kind_name(dialog.kind)).width(130.0).show_ui(ui, |ui| {
                    for kind in [SymbolKind::MovieClip, SymbolKind::Button, SymbolKind::Graphic] {
                        ui.selectable_value(&mut dialog.kind, kind, kind_name(kind));
                    }
                });
                if !dialog.new {
                    ui.add_space(12.0);
                    ui.label(RichText::new("Registration").color(theme::TEXT_DIM));
                    registration_grid(ui, &mut dialog.registration);
                }
            });
            ui.end_row();
            ui.label(RichText::new("Folder").color(theme::TEXT_DIM));
            ui.label("Library root");
            ui.end_row();
        });
        ui.add_space(12.0);
        ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
            cancel |= ui.button("  Cancel  ").clicked();
            ok |= theme::cta_button(ui, "    OK    ").clicked();
        });
    });
    if modal.should_close() {
        cancel = true;
    }
    if cancel {
        ed.convert = None;
    } else if ok {
        convert(ed);
    }
}

fn convert(ed: &mut Editor) {
    let Some(dialog) = ed.convert.take() else { return };
    let name = dialog.name.trim().to_string();
    // Work on a copy, so a refused name leaves no undo step behind.
    let mut doc = ed.doc.clone();
    let made = if dialog.new {
        symbols::new_symbol(&mut doc, &name, dialog.kind).map(|()| None)
    } else {
        let Some(b) = hit::bounds(&ed.doc, ed.timeline(), ed.frame, &ed.selection) else { return };
        let (col, row) = (dialog.registration.0 as f32 / 2.0, dialog.registration.1 as f32 / 2.0);
        let registration = (b.min.0 + (b.max.0 - b.min.0) * col, b.min.1 + (b.max.1 - b.min.1) * row);
        symbols::convert_to_symbol(&mut doc, ed.scene, ed.frame, &ed.selection, &name, dialog.kind, registration).map(Some)
    };
    match made {
        Ok(instance) => {
            ed.checkpoint();
            ed.doc = doc;
            match instance {
                Some(instance) => {
                    ed.selection = vec![instance];
                    ed.layer = instance.layer;
                    ed.props_tab = PropsTab::Object;
                }
                None => {
                    ed.library_selection = Some(name.clone());
                    ed.edit_library_symbol(&name);
                }
            }
        }
        Err(e) => {
            ed.error = Some(e.to_string());
            ed.convert = Some(dialog);
        }
    }
}

fn kind_name(kind: SymbolKind) -> &'static str {
    match kind {
        SymbolKind::MovieClip => "Movie Clip",
        SymbolKind::Button => "Button",
        SymbolKind::Graphic => "Graphic",
    }
}

/// Animate's 3×3 registration picker: nine small squares, the chosen one filled.
/// Document Settings' Anchor is the same picker.
pub fn registration_grid(ui: &mut egui::Ui, cell: &mut (u8, u8)) {
    let (rect, resp) = ui.allocate_exact_size(vec2(30.0, 30.0), Sense::click());
    let p = ui.painter_at(rect.expand(1.0));
    for row in 0..3u8 {
        for col in 0..3u8 {
            let c = rect.min + vec2(5.0 + col as f32 * 10.0, 5.0 + row as f32 * 10.0);
            let square = egui::Rect::from_center_size(c, vec2(6.0, 6.0));
            if *cell == (col, row) {
                p.rect_filled(square, 0.0, Color32::WHITE);
            } else {
                p.rect_stroke(square, 0.0, Stroke::new(1.0, theme::TEXT_DIM), StrokeKind::Inside);
            }
        }
    }
    if resp.clicked()
        && let Some(pos) = resp.interact_pointer_pos()
    {
        let col = ((pos.x - rect.left()) / 10.0).clamp(0.0, 2.0) as u8;
        let row = ((pos.y - rect.top()) / 10.0).clamp(0.0, 2.0) as u8;
        *cell = (col, row);
    }
}
