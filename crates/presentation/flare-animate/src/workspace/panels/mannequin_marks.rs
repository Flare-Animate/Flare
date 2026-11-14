//! The picture the mannequin is posed from, in the panel: it stands behind the figure in the
//! view, see-through, turning with it; the joints of the person found in it show as marks, a
//! stick figure, and one put wrong is dragged onto the person's joint, and a right click on one
//! says it is nearer or farther than the part it hangs from (a plus or a minus beside it). Under
//! the view: bringing a picture in, Copy Pose to pose the figure again from the marks, the marks
//! put back on the figure, how see-through it is, and taking it away.

use egui::{Color32, Pos2, Stroke, Ui};
use flare_mannequin::{Depth, Joint, MARK_LINES, MARKED, Placement, Side};
use flare_ui::chrome;

use super::mannequin_view::pos;
use crate::workspace::Workspace;

/// How near a mark a press takes it, in points.
pub(crate) const MARK_REACH: f32 = 9.0;
/// Squares the picture is cut into across and down, so a slanted picture keeps its shape.
const PICTURE_CELLS: usize = 8;

fn mark_color(joint: Joint) -> Color32 {
    match joint.side() {
        Side::Left => Color32::from_rgb(80, 150, 245),
        Side::Right => Color32::from_rgb(245, 140, 60),
        Side::Center => Color32::from_rgb(240, 240, 240),
    }
}

impl Workspace {
    /// Where each mark is seen in the view, when the marks are shown.
    fn seen_marks(&self, placement: Placement) -> Vec<(Joint, Pos2)> {
        let Some(reference) = self.mannequin.reference.as_ref().filter(|r| r.show_marks) else {
            return Vec::new();
        };
        let projector = self.mannequin.figure.projector(placement);
        MARKED
            .into_iter()
            .filter_map(|j| {
                let p = reference.board.point(reference.marks.get(j)?);
                Some((j, pos(projector.project(p).at)))
            })
            .collect()
    }

    /// The mark seen at `p`, if it is near one.
    pub(crate) fn mannequin_mark_at(&self, placement: Placement, p: Pos2) -> Option<Joint> {
        self.seen_marks(placement)
            .into_iter()
            .map(|(j, at)| (j, at.distance(p)))
            .filter(|(_, d)| *d <= MARK_REACH)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(j, _)| j)
    }

    /// Moves `joint`'s mark to the point of the picture under `p`; held where the picture is
    /// seen edge on.
    pub(crate) fn drag_mannequin_mark(&mut self, placement: Placement, joint: Joint, p: [f64; 2]) {
        let projector = self.mannequin.figure.projector(placement);
        if let Some(reference) = &mut self.mannequin.reference
            && let Some(on) = reference.board.under(&projector, p)
        {
            reference.marks.set(joint, Some(on));
        }
    }

    /// The picture, standing where its board is, as the view sees it.
    pub(crate) fn paint_mannequin_reference(
        &mut self,
        ui: &Ui,
        painter: &egui::Painter,
        placement: Placement,
    ) {
        let projector = self.mannequin.figure.projector(placement);
        let Some(reference) = &mut self.mannequin.reference else {
            return;
        };
        if let Some(image) = reference.image.take() {
            let options = egui::TextureOptions::LINEAR;
            reference.texture = Some(ui.ctx().load_texture("mannequin-reference", image, options));
        }
        let Some(texture) = &reference.texture else {
            return;
        };
        let [w, h] = reference.board.size();
        let color = Color32::WHITE.gamma_multiply(reference.opacity.clamp(0.05, 1.0));
        let mut mesh = egui::Mesh::with_texture(texture.id());
        let n = PICTURE_CELLS;
        for row in 0..=n {
            for column in 0..=n {
                let (u, v) = (column as f32 / n as f32, row as f32 / n as f32);
                let p = reference.board.point([f64::from(u) * w, f64::from(v) * h]);
                mesh.vertices.push(egui::epaint::Vertex {
                    pos: pos(projector.project(p).at),
                    uv: Pos2::new(u, v),
                    color,
                });
            }
        }
        for row in 0..n {
            for column in 0..n {
                let i = (row * (n + 1) + column) as u32;
                let below = i + (n + 1) as u32;
                mesh.add_triangle(i, i + 1, below + 1);
                mesh.add_triangle(i, below + 1, below);
            }
        }
        painter.add(egui::Shape::mesh(mesh));
    }

    /// The marks as a stick figure over the picture, each part's depth beside it when given.
    pub(crate) fn paint_mannequin_marks(&self, painter: &egui::Painter, placement: Placement) {
        let seen = self.seen_marks(placement);
        let find = |j: Joint| seen.iter().find(|(m, _)| *m == j).map(|(_, p)| *p);
        let line = Stroke::new(1.5, Color32::WHITE.gamma_multiply(0.7));
        for (a, b) in MARK_LINES {
            if let (Some(a), Some(b)) = (find(a), find(b)) {
                painter.line_segment([a, b], line);
            }
        }
        let dragged = match &self.mannequin.drag {
            Some(crate::workspace::mannequin::MannequinDrag::Mark(j)) => Some(*j),
            _ => None,
        };
        for (joint, p) in &seen {
            let radius = if dragged == Some(*joint) { 6.0 } else { 4.5 };
            painter.circle(
                *p,
                radius,
                mark_color(*joint),
                Stroke::new(1.0, Color32::BLACK),
            );
            let sign = match self.mannequin_mark_depth(*joint) {
                Depth::Auto => continue,
                Depth::Nearer => "+",
                Depth::Farther => "\u{2212}",
            };
            painter.text(
                *p + egui::vec2(7.0, -7.0),
                egui::Align2::LEFT_BOTTOM,
                sign,
                egui::FontId::proportional(12.0),
                Color32::WHITE,
            );
        }
    }

    /// Bringing a picture in to pose from, and what can be done with it once it is.
    pub(crate) fn mannequin_reference_settings(&mut self, ui: &mut Ui) {
        ui.strong(tr!(self, "mannequin-reference"));
        let import = tr!(self, "mannequin-reference-import");
        if self.mannequin.loading.is_some() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.weak(tr!(self, "mannequin-reference-reading"));
            });
            return;
        }
        let Some(name) = self.mannequin.reference.as_ref().map(|r| r.name.clone()) else {
            ui.weak(tr!(self, "mannequin-reference-none"));
            if ui.button(import).clicked() {
                self.ask_mannequin_reference();
            }
            return;
        };
        ui.label(name);
        ui.weak(tr!(self, "mannequin-reference-hint"));
        let labels = [
            tr!(self, "mannequin-copy-pose"),
            tr!(self, "mannequin-reset-marks"),
            tr!(self, "mannequin-show-marks"),
            tr!(self, "mannequin-opacity"),
            tr!(self, "mannequin-reference-remove"),
        ];
        ui.horizontal_wrapped(|ui| {
            if ui.button(&labels[0]).clicked() {
                self.copy_mannequin_pose();
            }
            if ui.button(&labels[1]).clicked() {
                self.reset_mannequin_marks();
            }
        });
        let mut remove = false;
        if let Some(reference) = &mut self.mannequin.reference {
            ui.checkbox(&mut reference.show_marks, &labels[2]);
            chrome::slider(
                ui,
                &labels[3],
                &mut reference.opacity,
                0.05..=1.0,
                "",
                false,
            );
            ui.horizontal(|ui| {
                remove = ui.button(&labels[4]).clicked();
                if ui.button(import).clicked() {
                    self.dialog = Some(crate::DialogRequest::ImportMannequinReference);
                }
            });
        }
        if remove {
            self.remove_mannequin_reference();
        }
    }
}
