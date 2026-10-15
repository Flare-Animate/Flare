//! History panel (Window › History, Ctrl+F10): every undoable step, oldest
//! first, with the steps that can be redone greyed below. Click a step to go
//! back (or forward) to just after it; click the top row to go back to how
//! the document was opened. Steps come from the editor's undo stack (up to
//! 200). Our own code; the list-of-steps panel follows PhotoCraft's history
//! panel and Animate's.

use crate::App;
use eframe::egui::{self, Color32, RichText};

pub(crate) fn window(app: &mut App, ctx: &egui::Context) {
    if !app.history_open {
        return;
    }
    let mut open = true;
    let mut go: Option<usize> = None;
    let mut clear = false;
    egui::Window::new("History").open(&mut open).default_width(240.0).default_height(320.0).default_pos(app.stage_rect.right_top() + egui::vec2(-260.0, 40.0)).show(ctx, |ui| {
        let (done, undone) = app.ed.history();
        let n = done.len();
        egui::ScrollArea::vertical().max_height(320.0).auto_shrink([false, true]).stick_to_bottom(true).show(ui, |ui| {
            if ui.selectable_label(n == 0, RichText::new("Opened").italics()).on_hover_text("Back to before the first step listed").clicked() {
                go = Some(0);
            }
            for (i, l) in done.iter().enumerate() {
                if ui.selectable_label(i + 1 == n, format!("{:>3}  {l}", i + 1)).clicked() {
                    go = Some(i + 1);
                }
            }
            for (i, l) in undone.iter().enumerate() {
                if ui.selectable_label(false, RichText::new(format!("{:>3}  {l}", n + i + 1)).color(Color32::from_gray(120))).on_hover_text("Undone: click to redo up to here").clicked() {
                    go = Some(n + i + 1);
                }
            }
        });
        ui.separator();
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{} step{}", n, if n == 1 { "" } else { "s" })).small().color(Color32::from_gray(150)));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("Clear History").on_hover_text("Forget the steps (the document stays as it is; this can't be undone)").clicked() {
                    clear = true;
                }
            });
        });
    });
    if let Some(k) = go {
        app.ed.go_to_step(k);
        app.status = format!("History: {k} step{} done", if k == 1 { "" } else { "s" });
    }
    if clear {
        app.ed.clear_history();
        app.status = "History cleared".into();
    }
    if !open {
        app.history_open = false;
    }
}
