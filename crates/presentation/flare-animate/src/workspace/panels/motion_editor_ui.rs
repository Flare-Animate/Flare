//! Drawing the Motion Editor and turning the pointer into the calls of `motion_editor.rs`.

use crate::workspace::Workspace;
use egui::{Pos2, Sense, Stroke, Ui, Vec2};
use flare_tools::tween::spec;
use flare_tween::{HandleKind, Interpolation, effective_handles, points};
use flare_ui::{chrome, theme};

const HANDLE_DOT: f32 = 4.5;

impl Workspace {
    pub(crate) fn motion_editor_panel(&mut self, ui: &mut Ui) {
        let Some(shown) = self.motion_shown() else {
            let message = if self.selection_tween().is_some() {
                tr!(self, "motion-no-curve")
            } else {
                tr!(self, "motion-nothing")
            };
            // A tween without a curve yet can be given one here.
            chrome::empty_state(ui, "panel-properties", &message);
            if self.selection_tween().is_some() {
                let label = tr!(self, "motion-start");
                if ui.button(label).clicked() {
                    self.motion_set_family("bezier");
                }
            }
            return;
        };
        self.motion_controls(ui, &shown);
        let size = Vec2::new(
            ui.available_width().max(60.0),
            (ui.available_height() - 4.0).max(60.0),
        );
        let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
        self.motion_layout(f64::from(rect.width()), f64::from(rect.height()));
        self.motion_pointer(ui, rect, &response);
        // Read again: the pointer may have started or ended a drag.
        let Some(shown) = self.motion_shown() else {
            return;
        };
        self.motion_paint(ui, rect, &shown);
    }

    fn motion_controls(&mut self, ui: &mut Ui, shown: &flare_document::SpanInterp) {
        let (family_label, easing_label, handle_label) = (
            tr!(self, "motion-family"),
            tr!(self, "prop-easing"),
            tr!(self, "motion-handle"),
        );
        let mut family = None;
        let mut easing = None;
        let mut kind = None;
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(&family_label).color(theme::text_dim()));
            egui::ComboBox::from_id_salt("motion-family")
                .selected_text(shown.interpolation().name())
                .show_ui(ui, |ui| {
                    for f in spec::families() {
                        let on = shown.interpolation().name() == f.name;
                        if ui.selectable_label(on, f.name).clicked() {
                            family = Some(f.name);
                        }
                    }
                });
            ui.label(egui::RichText::new(&easing_label).color(theme::text_dim()));
            egui::ComboBox::from_id_salt("motion-easing")
                .selected_text(shown.easing().name())
                .show_ui(ui, |ui| {
                    for name in spec::easing_names() {
                        if ui
                            .selectable_label(shown.easing().name() == name, name)
                            .clicked()
                        {
                            easing = Some(name);
                        }
                    }
                });
            if shown.interpolation() == Interpolation::Bezier {
                ui.label(egui::RichText::new(&handle_label).color(theme::text_dim()));
                egui::ComboBox::from_id_salt("motion-handle")
                    .selected_text(shown.handle_kind().name())
                    .show_ui(ui, |ui| {
                        for k in HandleKind::ALL {
                            if ui
                                .selectable_label(shown.handle_kind() == k, k.name())
                                .clicked()
                            {
                                kind = Some(k);
                            }
                        }
                    });
            }
        });
        let (copy, paste, reset) = (
            tr!(self, "motion-copy"),
            tr!(self, "motion-paste"),
            tr!(self, "motion-reset"),
        );
        let can_paste = self.motion().has_clipboard();
        ui.horizontal(|ui| {
            if ui.button(copy).clicked() {
                self.motion_copy();
            }
            if ui
                .add_enabled(can_paste, egui::Button::new(paste))
                .clicked()
            {
                self.motion_paste();
            }
            if ui.button(reset).clicked() {
                self.motion_reset();
            }
        });
        if let Some(f) = family {
            self.motion_set_family(f);
        }
        if let Some(e) = easing {
            self.motion_set_easing(e);
        }
        if let Some(k) = kind {
            self.motion_set_handle_kind(k);
        }
    }

    fn motion_pointer(&mut self, ui: &Ui, rect: egui::Rect, response: &egui::Response) {
        let local = |p: Pos2| (f64::from(p.x - rect.min.x), f64::from(p.y - rect.min.y));
        if response.drag_started_by(egui::PointerButton::Primary)
            && let Some(p) = response.interact_pointer_pos()
        {
            self.motion_press(local(p));
        }
        if response.dragged_by(egui::PointerButton::Primary)
            && let Some(p) = response.interact_pointer_pos()
        {
            let snap = ui.input(|i| i.modifiers.shift);
            self.motion_drag(local(p), snap);
        }
        if response.drag_stopped() {
            self.motion_release();
        }
        // Pan with the middle or right button, zoom with the wheel about the pointer.
        if response.dragged_by(egui::PointerButton::Middle)
            || response.dragged_by(egui::PointerButton::Secondary)
        {
            let d = response.drag_delta();
            self.motion_mut()
                .view_mut()
                .pan(f64::from(d.x), f64::from(d.y));
        }
        if response.hovered()
            && let Some(p) = response.hover_pos()
        {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                let f = f64::from((scroll / 200.0).exp());
                let anchor = local(p);
                let both = ui.input(|i| i.modifiers.shift);
                self.motion_mut()
                    .view_mut()
                    .zoom_about(anchor, if both { 1.0 } else { f }, f);
            }
        }
    }

    fn motion_paint(&mut self, ui: &Ui, rect: egui::Rect, shown: &flare_document::SpanInterp) {
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 2.0, theme::bg());
        let view = *self.motion().view();
        let at = |w: (f64, f64)| {
            let s = view.to_screen(w);
            Pos2::new(rect.min.x + s.0 as f32, rect.min.y + s.1 as f32)
        };
        let dim = Stroke::new(1.0, theme::border());
        // The unit square the segment lives in: start and end of time and of change.
        for (a, b) in [
            ((0.0, 0.0), (1.0, 0.0)),
            ((0.0, 1.0), (1.0, 1.0)),
            ((0.0, 0.0), (0.0, 1.0)),
            ((1.0, 0.0), (1.0, 1.0)),
        ] {
            painter.line_segment([at(a), at(b)], dim);
        }
        // The curve, segment by segment so nothing is allocated to draw it.
        let samples = self.motion_mut().samples_for(shown);
        let n = samples.len().saturating_sub(1).max(1) as f64;
        let line = Stroke::new(1.75, theme::accent());
        for (i, pair) in samples.windows(2).enumerate() {
            let (t0, t1) = (i as f64 / n, (i + 1) as f64 / n);
            painter.line_segment([at((t0, pair[0])), at((t1, pair[1]))], line);
        }
        if shown.interpolation() == Interpolation::Bezier
            && let Ok(handles) = effective_handles(shown.handle_kind(), shown.handles())
        {
            let p = points(handles);
            let stalk = Stroke::new(1.0, theme::text_dim());
            painter.line_segment([at(p[0]), at(p[1])], stalk);
            painter.line_segment([at(p[3]), at(p[2])], stalk);
            for end in [p[0], p[3]] {
                painter.circle_filled(at(end), HANDLE_DOT - 1.0, theme::text());
            }
            for tip in [p[1], p[2]] {
                painter.circle_filled(at(tip), HANDLE_DOT, theme::accent());
                painter.circle_stroke(at(tip), HANDLE_DOT, Stroke::new(1.0, theme::text()));
            }
        }
    }
}
