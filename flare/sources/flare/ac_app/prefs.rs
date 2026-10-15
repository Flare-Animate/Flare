//! Edit › Preferences (Ctrl+U): General, Drawing, Text and Scripts pages.
//! Saved in the settings and applied at once (and at every start).

use crate::files::Prefs;
use crate::App;
use eframe::egui::{self, Color32, RichText};

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Page {
    #[default]
    General,
    Drawing,
    Text,
    Scripts,
}

/// Put the preferences into effect (start-up and every change).
pub(crate) fn apply(app: &mut App, ctx: &egui::Context) {
    let p = app.settings.prefs.clone();
    ac_core::editor::set_undo_limit(p.undo_levels as usize);
    ctx.set_zoom_factor(p.ui_scale.clamp(0.75, 2.0));
    ctx.style_mut(|s| s.interaction.tooltip_delay = if p.tooltips { 0.5 } else { 1.0e6 });
    app.view.snap_px = p.snap_px;
}

pub(crate) fn window(app: &mut App, ctx: &egui::Context) {
    if !app.prefs_open {
        return;
    }
    let mut open = true;
    let before = app.settings.prefs.clone();
    let mut reset = false;
    egui::Window::new("Preferences").open(&mut open).resizable(false).collapsible(false).default_width(420.0).show(ctx, |ui| {
        ui.horizontal(|ui| {
            for (pg, name) in [(Page::General, "General"), (Page::Drawing, "Drawing"), (Page::Text, "Text"), (Page::Scripts, "Scripts")] {
                ui.selectable_value(&mut app.prefs_page, pg, name);
            }
        });
        ui.separator();
        let p: &mut Prefs = &mut app.settings.prefs;
        egui::Grid::new("prefs").num_columns(2).spacing([14.0, 8.0]).show(ui, |ui| match app.prefs_page {
            Page::General => {
                ui.label("Undo levels");
                ui.add(egui::DragValue::new(&mut p.undo_levels).range(2..=1000));
                ui.end_row();
                ui.label("Auto-recovery");
                ui.horizontal(|ui| {
                    let mut on = p.autosave_secs > 0;
                    if ui.checkbox(&mut on, "every").changed() {
                        p.autosave_secs = if on { 60 } else { 0 };
                    }
                    ui.add_enabled(on, egui::DragValue::new(&mut p.autosave_secs).range(10..=3600).suffix(" s"));
                });
                ui.end_row();
                ui.label("Interface size");
                ui.add(egui::Slider::new(&mut p.ui_scale, 0.75..=2.0).custom_formatter(|v, _| format!("{:.0} %", v * 100.0)));
                ui.end_row();
                ui.label("Selection");
                ui.checkbox(&mut p.shift_select, "Shift select").on_hover_text("Off: clicking adds objects to the selection without holding Shift");
                ui.end_row();
                ui.label("");
                ui.checkbox(&mut p.tooltips, "Show tooltips");
                ui.end_row();
                ui.label("At launch");
                ui.checkbox(&mut app.settings.show_startup, "Show the Startup window");
                ui.end_row();
            }
            Page::Drawing => {
                ui.label("Snap distance");
                egui::ComboBox::from_id_salt("pref-snap").selected_text(match p.snap_px as u32 { 0..=3 => "Must be close", 4..=8 => "Normal", _ => "Can be distant" }).show_ui(ui, |ui| {
                    ui.selectable_value(&mut p.snap_px, 3.0, "Must be close");
                    ui.selectable_value(&mut p.snap_px, 6.0, "Normal");
                    ui.selectable_value(&mut p.snap_px, 12.0, "Can be distant");
                });
                ui.end_row();
                ui.label("New drawings");
                ui.horizontal(|ui| {
                    ui.radio_value(&mut p.object_drawing, false, "Merge Drawing");
                    ui.radio_value(&mut p.object_drawing, true, "Object Drawing");
                });
                ui.end_row();
                ui.label("Tweens");
                ui.checkbox(&mut app.settings.retain_tween_curves, "Keep tween curves when splitting").on_hover_text("Flare's timeline semantics; off = Animate's behaviour");
                ui.end_row();
            }
            Page::Text => {
                ui.label("Default size");
                ui.add(egui::DragValue::new(&mut p.text_size).range(4.0..=400.0).suffix(" pt"));
                ui.end_row();
            }
            Page::Scripts => {
                ui.label("Frame scripts");
                ui.checkbox(&mut p.frame_scripts, "Run frame scripts while playing");
                ui.end_row();
                ui.label("");
                ui.label(RichText::new("Authoring scripts in other languages always ask before running.").small().color(Color32::from_gray(150)));
                ui.end_row();
            }
        });
        ui.separator();
        ui.horizontal(|ui| {
            if ui.button("Restore Defaults").clicked() {
                reset = true;
            }
            ui.label(RichText::new("Changes apply at once").small().color(Color32::from_gray(140)));
        });
    });
    if reset {
        app.settings.prefs = Prefs::default();
    }
    let p = app.settings.prefs.clone();
    if p != before || reset {
        if p.object_drawing != before.object_drawing {
            app.topts.object_draw = p.object_drawing;
        }
        if p.text_size != before.text_size {
            app.topts.text_size = p.text_size;
        }
        if p.frame_scripts != before.frame_scripts {
            app.scripting.frame_scripts = p.frame_scripts;
        }
        apply(app, ctx);
    }
    ac_core::frames::set_retain_tween_curves(app.settings.retain_tween_curves);
    if !open {
        app.prefs_open = false;
        app.settings.save();
    }
}
