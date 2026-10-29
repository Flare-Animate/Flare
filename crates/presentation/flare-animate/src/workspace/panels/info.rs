//! The Info panel: the size and position of the selection, and what is under the pointer.

use egui::{Color32, Ui};
use flare_core::Rgba8;
use flare_document::Document;
use flare_ui::chrome;
use glamx::DVec2;

use crate::workspace::Workspace;

/// The color of the topmost shape at `point` of `frame`: its fill, or its line where it has
/// none. `None` over empty stage.
pub(crate) fn color_at(document: &Document, frame: u32, point: DVec2) -> Option<Rgba8> {
    let scene = document.scenes().first()?.id();
    let evaluated = flare_eval::evaluate_frame(document, scene, frame).ok()?;
    let top = *flare_render::hit_test_all(&evaluated, point).first()?;
    let shape = evaluated.shapes.get(top)?.shape;
    if let Some(gradient) = shape.gradient() {
        return Some(gradient.start());
    }
    if shape.fill().a > 0 {
        return Some(shape.fill());
    }
    shape.stroke().map(|stroke| stroke.color())
}

impl Workspace {
    pub(crate) fn info_panel(&mut self, ui: &mut Ui) {
        chrome::section(ui, "panel-properties", "Selection");
        let rects = self.selection_rects();
        let bounds = rects.iter().copied().reduce(|a, b| a.union(b));
        let value = |ui: &mut Ui, label: &str, number: Option<f32>| {
            chrome::row(ui, label, |ui| {
                ui.label(number.map_or_else(|| "-".to_owned(), |n| format!("{n:.1}")));
            });
        };
        value(ui, "W", bounds.map(|b| b.width()));
        value(ui, "H", bounds.map(|b| b.height()));
        value(ui, "X", bounds.map(|b| b.min.x));
        value(ui, "Y", bounds.map(|b| b.min.y));
        chrome::section(ui, "tool-eyedropper", "Pointer");
        let pointer = self.pointer_stage;
        value(ui, "X", pointer.map(|p| p.x));
        value(ui, "Y", pointer.map(|p| p.y));
        let under = pointer.and_then(|p| {
            color_at(
                self.document(),
                self.frame,
                DVec2::new(p.x.into(), p.y.into()),
            )
        });
        chrome::row(ui, "Color", |ui| match under {
            Some(c) => {
                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
                ui.painter().rect_filled(
                    rect,
                    2.0,
                    Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a),
                );
                ui.label(format!("{:02X}{:02X}{:02X}  A {}", c.r, c.g, c.b, c.a));
            }
            None => {
                ui.label("-");
            }
        });
    }
}
