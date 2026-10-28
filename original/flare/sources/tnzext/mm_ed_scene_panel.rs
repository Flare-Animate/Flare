//! Window > Scene (Shift+F2): a floating panel listing the document's scenes in play order. A
//! click goes to a scene, a double-click renames it, a drag moves it; the buttons below add,
//! duplicate and delete scenes, Delete asking first (docs/animate-ui.md §7.4).

use eframe::egui::{self, Align, Align2, Color32, FontId, Frame, Id, Layout, Margin, Rect, RichText, Sense, Stroke, Ui, WidgetInfo, WidgetType, pos2, vec2};
use minnamate_core::scenes;

use crate::app::{Action, Editor};
use crate::icons::{self, Icon};
use crate::{home, theme};

const ROW_H: f32 = 22.0;
const WIDTH: f32 = 230.0;

#[derive(Default)]
pub struct State {
    pub open: bool,
    /// The scene being renamed, and the name typed so far.
    pub renaming: Option<(usize, String)>,
    /// The scene Delete asks about.
    pub deleting: Option<usize>,
    /// The scene being dragged.
    dragging: Option<usize>,
}

pub fn show(ctx: &egui::Context, ed: &mut Editor) {
    if !ed.scene_panel.open || ed.home.shown {
        return;
    }
    let frame = Frame::NONE.fill(theme::PANEL_BG).stroke(Stroke::new(1.0, theme::POPUP_BORDER)).shadow(ctx.global_style().visuals.popup_shadow);
    let start = ed.stage_rect.right_top() + vec2(-WIDTH - 24.0, 24.0);
    egui::Window::new("Scene").id(Id::new("scene-panel")).title_bar(false).resizable(false).frame(frame).default_pos(start).show(ctx, |ui| {
        ui.set_width(WIDTH);
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        let mut close = false;
        theme::tab_strip_menu(ui, &["Scene"], 0, Some(|ui: &mut Ui| {
            theme::menu_item(ui, false, "Help", "");
            ui.separator();
            close |= theme::menu_item(ui, true, "Close", "").clicked();
            close |= theme::menu_item(ui, true, "Close Group", "").clicked();
        }));
        list(ui, ed);
        toolbar(ui, ed);
        if close {
            ed.scene_panel.open = false;
        }
    });
    delete_question(ctx, ed);
}

/// The scenes, the current one highlighted; where a dragged one would land, a line.
fn list(ui: &mut Ui, ed: &mut Editor) {
    let count = ed.doc.scenes.len();
    let (area, _) = ui.allocate_exact_size(vec2(WIDTH, (count as f32 * ROW_H).max(ROW_H * 6.0) + 8.0), Sense::hover());
    let top = area.top() + 4.0;
    let pointer = ui.input(|i| i.pointer.hover_pos());
    let gap = |y: f32| (((y - top) / ROW_H).round().max(0.0) as usize).min(count);
    let mut go = None;
    let mut moved = None;
    for i in 0..count {
        let row = Rect::from_min_size(pos2(area.left(), top + i as f32 * ROW_H), vec2(WIDTH, ROW_H));
        let resp = ui.interact(row, ui.id().with(("scene-row", i)), Sense::click_and_drag());
        let name = ed.doc.scenes[i].name.clone();
        resp.widget_info(|| WidgetInfo::selected(WidgetType::SelectableLabel, true, i == ed.scene, &name));
        let p = ui.painter();
        if i == ed.scene {
            p.rect_filled(row, 0.0, theme::SELECTION);
        } else if resp.hovered() {
            p.rect_filled(row, 0.0, theme::BUTTON_HOVER);
        }
        let color = if i == ed.scene { Color32::WHITE } else { theme::TEXT };
        icons::paint(p, Rect::from_center_size(pos2(row.left() + 16.0, row.center().y), vec2(14.0, 14.0)), Icon::Scene, color);
        match &mut ed.scene_panel.renaming {
            Some((r, text)) if *r == i => {
                let field = Rect::from_min_max(pos2(row.left() + 30.0, row.top() + 1.0), pos2(row.right() - 6.0, row.bottom() - 1.0));
                let edit = ui.put(field, egui::TextEdit::singleline(text).margin(Margin::symmetric(2, 1)));
                if !edit.has_focus() && !edit.lost_focus() {
                    edit.request_focus();
                }
                if edit.lost_focus() {
                    let typed = text.clone();
                    ed.scene_panel.renaming = None;
                    if !ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        rename(ed, i, &typed);
                    }
                }
            }
            _ => {
                p.text(pos2(row.left() + 30.0, row.center().y), Align2::LEFT_CENTER, &name, FontId::proportional(12.0), color);
            }
        }
        if resp.double_clicked() {
            ed.scene_panel.renaming = Some((i, name.clone()));
        } else if resp.clicked() {
            go = Some(i);
        }
        if resp.drag_started() {
            ed.scene_panel.dragging = Some(i);
        }
        if resp.drag_stopped()
            && let (Some(from), Some(at)) = (ed.scene_panel.dragging.take(), pointer)
        {
            moved = Some((from, gap(at.y)));
        }
    }
    if let (Some(_), Some(at)) = (ed.scene_panel.dragging, pointer) {
        let y = top + gap(at.y) as f32 * ROW_H;
        ui.painter().hline(area.left() + 4.0..=area.right() - 4.0, y, Stroke::new(2.0, theme::PLAYHEAD));
    }
    if let Some(i) = go {
        ed.go_to_scene(i);
    }
    if let Some((from, gap)) = moved {
        // Its own gap or the one after leaves it where it is.
        if gap != from && gap != from + 1 {
            let current = ed.doc.scenes[ed.scene].name.clone();
            ed.checkpoint();
            scenes::move_to_gap(&mut ed.doc, from, gap);
            ed.scene = ed.doc.scenes.iter().position(|s| s.name == current).unwrap_or(0);
        }
    }
}

/// Add, Duplicate and Delete, at the panel's foot.
fn toolbar(ui: &mut Ui, ed: &mut Editor) {
    let (bar, _) = ui.allocate_exact_size(vec2(WIDTH, 26.0), Sense::hover());
    ui.painter().hline(bar.x_range(), bar.top() + 0.5, Stroke::new(1.0, theme::BORDER));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(bar.shrink2(vec2(6.0, 2.0))).layout(Layout::right_to_left(Align::Center)));
    child.spacing_mut().item_spacing.x = 2.0;
    let button = |ui: &mut Ui, icon: Icon, tip: &str, enabled: bool| {
        theme::icon_button(ui, theme::ICON_BUTTON, false, enabled, tip, |p, r, c| icons::paint(p, r.shrink(3.0), icon, c)).clicked()
    };
    let several = ed.doc.scenes.len() > 1;
    if button(&mut child, Icon::Delete, "Delete scene", several) {
        ed.scene_panel.deleting = Some(ed.scene);
    }
    if button(&mut child, Icon::Duplicate, "Duplicate scene", true) {
        ed.checkpoint();
        let at = scenes::duplicate(&mut ed.doc, ed.scene);
        ed.go_to_scene(at);
    }
    if button(&mut child, Icon::NewLayer, "Add scene", true) {
        ed.run(Action::InsertScene, ui.ctx());
    }
}

/// Renames scene `i`; a name refused says why.
fn rename(ed: &mut Editor, i: usize, name: &str) {
    if ed.doc.scenes.get(i).is_none_or(|s| s.name == name.trim()) {
        return;
    }
    let mut doc = ed.doc.clone();
    match scenes::rename(&mut doc, i, name) {
        Ok(()) => {
            ed.checkpoint();
            ed.doc = doc;
        }
        Err(e) => ed.error = Some(e.to_string()),
    }
}

/// "Delete this scene?", while Delete waits on it.
fn delete_question(ctx: &egui::Context, ed: &mut Editor) {
    let Some(i) = ed.scene_panel.deleting else { return };
    let Some(name) = ed.doc.scenes.get(i).map(|s| s.name.clone()) else {
        ed.scene_panel.deleting = None;
        return;
    };
    let mut answer = None;
    home::dialog("delete-scene", ctx, theme::PANEL_BG).show(ctx, |ui| {
        if home::title_bar(ui, "Delete Scene", 380.0) {
            answer = Some(false);
        }
        Frame::NONE.inner_margin(Margin { left: 20, right: 20, top: 16, bottom: 14 }).show(ui, |ui| {
            ui.set_width(340.0);
            ui.label(RichText::new(format!("Delete \"{name}\"? Its layers and frames go with it.")).color(theme::TEXT));
            ui.add_space(16.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.add(egui::Button::new("Cancel").min_size(vec2(72.0, 28.0))).clicked() {
                    answer = Some(false);
                }
                let (_, rect) = ui.allocate_space(vec2(72.0, 28.0));
                if home::push(ui, rect, "OK", true, 4.0).clicked() {
                    answer = Some(true);
                }
            });
        });
    });
    if ed.error.is_none() {
        answer = answer.or_else(|| ctx.input(|k| if k.key_pressed(egui::Key::Enter) { Some(true) } else if k.key_pressed(egui::Key::Escape) { Some(false) } else { None }));
    }
    match answer {
        Some(true) => {
            ed.scene_panel.deleting = None;
            // Out of any symbol first: it was entered from a scene that may go.
            ed.go_to_scene(ed.scene);
            let current = ed.doc.scenes[ed.scene].name.clone();
            let mut doc = ed.doc.clone();
            if scenes::delete(&mut doc, i).is_ok() {
                ed.checkpoint();
                ed.doc = doc;
                // The scene shown stays shown; the one deleted gives way to its neighbour.
                let at = ed.doc.scenes.iter().position(|s| s.name == current).unwrap_or(i.min(ed.doc.scenes.len() - 1));
                if at == ed.scene {
                    ed.set_frame(0);
                    ed.layer = 0;
                } else {
                    ed.go_to_scene(at);
                }
            }
        }
        Some(false) => ed.scene_panel.deleting = None,
        None => {}
    }
}

#[cfg(test)]
mod tests {
    use eframe::egui::{Key, Modifiers};
    use egui_kittest::kittest::Queryable;

    use super::*;
    use crate::app::testkit::*;

    fn names(ed: &Editor) -> Vec<String> {
        ed.doc.scenes.iter().map(|s| s.name.clone()).collect()
    }

    #[test]
    fn the_scene_panel_adds_renames_moves_and_deletes() {
        let mut h = blank();
        h.key_press_modifiers(Modifiers::SHIFT, Key::F2);
        h.run();
        assert!(h.state().scene_panel.open, "Shift+F2 opens it");
        h.get_by_label("Add scene").click();
        h.run();
        h.get_by_label("Duplicate scene").click();
        h.run();
        assert_eq!((names(h.state()), h.state().scene), (vec!["Scene 1".to_string(), "Scene 2".into(), "Scene 2 copy".into()], 2));

        // A click goes to a scene; a double-click renames it.
        h.get_by_label("Scene 1").click();
        h.run();
        assert_eq!(h.state().scene, 0);
        h.state_mut().scene_panel.renaming = Some((0, "Intro".into()));
        h.run();
        h.key_press(Key::Enter);
        h.run();
        assert_eq!(names(h.state())[0], "Intro");

        // Dragged below the last, Intro plays last, and stays the one shown. (The edit bar names
        // the scene shown too, so rows are found from the second one.)
        let second = h.get_by_label("Scene 2").rect().center();
        screen_drag(&mut h, second - vec2(0.0, ROW_H), second + vec2(0.0, ROW_H * 1.6), Modifiers::NONE);
        assert_eq!(names(h.state()), ["Scene 2", "Scene 2 copy", "Intro"]);
        assert_eq!(h.state().doc.scenes[h.state().scene].name, "Intro");

        // Delete asks; OK deletes, and the neighbour shows.
        h.get_by_label("Delete scene").click();
        h.run();
        assert_eq!(h.state().scene_panel.deleting, Some(2));
        h.get_by_label("OK").click();
        h.run();
        assert_eq!((names(h.state()), h.state().scene), (vec!["Scene 2".to_string(), "Scene 2 copy".into()], 1));
        // Every step undoes.
        for _ in 0..5 {
            h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
            h.step();
        }
        assert_eq!(names(h.state()), ["Scene 1"]);
    }

    /// Writes target/screenshots/scene_panel.png: the Scene panel with three scenes, the second shown.
    #[test]
    #[ignore = "needs a GPU adapter; run with --ignored"]
    fn scene_panel_screenshot() {
        let mut h = harness(true);
        h.run();
        let ctx = h.ctx.clone();
        let ed = h.state_mut();
        ed.doc.insert_scene(0);
        ed.doc.scenes[1].name = "Credits".into();
        ed.run(Action::InsertScene, &ctx);
        ed.scene_panel.open = true;
        h.run();
        screenshot(&mut h, "scene_panel");
    }
}
