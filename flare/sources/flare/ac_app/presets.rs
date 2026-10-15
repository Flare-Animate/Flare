//! Window › Motion Presets: ready-made movements (our own designs, stored as
//! JSON in ac-core) applied to the selected object as a motion tween, and your
//! own saved from a motion tween. Presets are relative to the object they are
//! applied to, so they work anywhere on the stage.

use crate::App;
use ac_core::motion_presets::{self, MotionPreset};
use eframe::egui::{self, Color32, RichText};

#[derive(Default)]
pub(crate) struct PresetsPanel {
    pub open: bool,
    filter: String,
    /// Name for "Save current motion as preset".
    pub name: String,
    builtin: Vec<MotionPreset>,
}

pub(crate) fn window(app: &mut App, ctx: &egui::Context) {
    let mut p = std::mem::take(&mut app.presets);
    if p.builtin.is_empty() {
        p.builtin = motion_presets::builtin();
    }
    let mut open = p.open;
    let mut apply: Option<MotionPreset> = None;
    let mut delete: Option<String> = None;
    let mut save = false;
    egui::Window::new("Motion Presets").open(&mut open).default_width(260.0).default_height(420.0).show(ctx, |ui| {
        ui.add(egui::TextEdit::singleline(&mut p.filter).hint_text("Search presets").desired_width(f32::INFINITY));
        let q = p.filter.trim().to_lowercase();
        let hit = |m: &MotionPreset| q.is_empty() || m.name.to_lowercase().contains(&q) || m.category.to_lowercase().contains(&q);
        ui.label(RichText::new("Select an object, then click a preset: it becomes a motion tween of that object").small().color(Color32::from_gray(150)));
        egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
            let mut cats: Vec<&str> = vec![];
            for m in app.settings.motion_presets.iter().chain(&p.builtin) {
                if !cats.contains(&m.category.as_str()) {
                    cats.push(m.category.as_str());
                }
            }
            for c in cats {
                let list: Vec<&MotionPreset> = app.settings.motion_presets.iter().chain(&p.builtin).filter(|m| m.category == c && hit(m)).collect();
                if list.is_empty() {
                    continue;
                }
                egui::CollapsingHeader::new(RichText::new(if c.is_empty() { "Other" } else { c }).strong()).default_open(true).show(ui, |ui| {
                    for m in list {
                        let mine = app.settings.motion_presets.iter().any(|u| u.name == m.name && u.category == m.category);
                        ui.horizontal(|ui| {
                            let tip = format!("{}\n{} frames{}", if m.description.is_empty() { &m.name } else { &m.description }, m.last_frame() + 1, m.ease.as_deref().map(|e| format!(" · {e}")).unwrap_or_default());
                            if ui.button(&m.name).on_hover_text(tip).clicked() {
                                apply = Some(m.clone());
                            }
                            if mine && ui.small_button("✕").on_hover_text("Delete this preset").clicked() {
                                delete = Some(m.name.clone());
                            }
                        });
                    }
                });
            }
        });
        ui.separator();
        ui.label(RichText::new("Save your own").strong());
        let can = app.ed.current_motion().is_some();
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut p.name).hint_text("Preset name").desired_width(120.0));
            if ui.add_enabled(can && !p.name.trim().is_empty(), egui::Button::new("Save from motion tween")).on_hover_text("Saves the motion tween under the playhead, relative to its object").clicked() {
                save = true;
            }
        });
        if !can {
            ui.label(RichText::new("Put the playhead on a motion tween to save it").small().color(Color32::from_gray(140)));
        }
    });
    p.open = open;
    if let Some(m) = apply {
        app.status = match app.ed.apply_motion_preset(&m) {
            Ok(()) => format!("Applied motion preset '{}'", m.name),
            Err(e) => e,
        };
    }
    if let Some(n) = delete {
        app.settings.motion_presets.retain(|m| m.name != n);
        app.settings.save();
    }
    if save {
        let name = p.name.trim().to_string();
        match app.ed.motion_preset_from_tween(&name) {
            Ok(m) => {
                app.settings.motion_presets.retain(|u| u.name != name);
                app.settings.motion_presets.push(m);
                app.settings.save();
                app.status = format!("Saved motion preset '{name}'");
                p.name.clear();
            }
            Err(e) => app.status = e,
        }
    }
    app.presets = p;
}
