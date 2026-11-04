//! The interpolation picker of the Properties panel and its read-only curve preview.

use crate::workspace::Workspace;
use egui::{Pos2, Sense, Stroke, Ui, Vec2};
use flare_document::SpanInterp;
use flare_tools::tween::spec;
use flare_ui::{chrome, theme};

/// Samples of the preview line. The curve is two keys, so this is a few dozen cheap evaluations,
/// only while a tweened keyframe is selected.
const PREVIEW_POINTS: usize = 48;

impl Workspace {
    pub(super) fn interpolation_picker(&mut self, ui: &mut Ui, enabled: bool) {
        let current = self.selection_interp();
        let label = tr!(self, "prop-interpolation");
        let hint = tr!(self, "prop-interpolation-hint");
        let off = tr!(self, "prop-interpolation-off");
        let easing_label = tr!(self, "prop-easing");
        let mut chosen: Option<Option<SpanInterp>> = None;
        chrome::row(ui, &label, |ui| {
            let shown = current.map_or(off.clone(), |i| i.interpolation().name().to_owned());
            ui.add_enabled_ui(enabled, |ui| {
                egui::ComboBox::from_id_salt("interp-family")
                    .selected_text(shown)
                    .show_ui(ui, |ui| {
                        if ui.selectable_label(current.is_none(), &off).clicked() {
                            chosen = Some(None);
                        }
                        for family in spec::families() {
                            let on =
                                current.is_some_and(|i| i.interpolation().name() == family.name);
                            if ui.selectable_label(on, family.name).clicked() {
                                let easing = current.map(|i| i.easing().name());
                                chosen =
                                    Some(spec::build_interp(family.name, easing, None, None).ok());
                            }
                        }
                    })
                    .response
                    .on_hover_text(hint);
            });
        });
        if let Some(interp) = current {
            chrome::row(ui, &easing_label, |ui| {
                egui::ComboBox::from_id_salt("interp-easing")
                    .selected_text(interp.easing().name())
                    .show_ui(ui, |ui| {
                        for name in spec::easing_names() {
                            if ui
                                .selectable_label(interp.easing().name() == name, name)
                                .clicked()
                            {
                                let family = interp.interpolation().name();
                                chosen = spec::build_interp(
                                    family,
                                    Some(name),
                                    None,
                                    Some(interp.handles()),
                                )
                                .ok()
                                .map(Some);
                            }
                        }
                    });
            });
            preview(ui, &interp);
        }
        if let Some(interp) = chosen {
            self.set_selection_interp(interp);
        }
    }
}

/// A small read-only plot of the curve: time across, change up.
fn preview(ui: &mut Ui, interp: &SpanInterp) {
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width().min(160.0), 56.0),
        Sense::hover(),
    );
    let painter = ui.painter_at(rect);
    painter.rect_stroke(
        rect,
        2.0,
        Stroke::new(1.0, theme::text_dim()),
        egui::StrokeKind::Inside,
    );
    // Overshooting families (Back, Elastic) leave 0..1; keep the plot inside the box.
    let inner = rect.shrink(4.0);
    let at = |t: f64, v: f64| {
        Pos2::new(
            inner.left() + inner.width() * t as f32,
            inner.bottom()
                - inner.height() * (v as f32).clamp(-0.25, 1.25) / 1.25 * 0.8
                - inner.height() * 0.2,
        )
    };
    let points: Vec<Pos2> = (0..=PREVIEW_POINTS)
        .map(|i| {
            let t = i as f64 / PREVIEW_POINTS as f64;
            at(t, interp.fraction(t))
        })
        .collect();
    painter.add(egui::Shape::line(points, Stroke::new(1.5, theme::accent())));
}
