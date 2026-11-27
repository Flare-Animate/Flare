//! The erasers the Eraser can be, picked from its list in Properties as a brush is picked from
//! the Brush's: the Eraser that rubs out, and the vector erasers after OpenToonz's. Segment and
//! Whole Line work along the drag with the round tip, as the rubbing Eraser does (see
//! `eraser.rs`, which they share); Rectangle and Freehand gather the drag as the box from its
//! start to the pointer, or as the loop it draws, show it, and take what is inside on release.
//! All of them keep to the Eraser's layer and frame, and each drag is one undo step.

use egui::{Pos2, Stroke, Ui};
use flare_input::InputSample;
use flare_tools::eraser::vector::{self, Take, VectorEraserType};
use flare_ui::{chrome, icons, theme};
use glamx::DVec2;

use crate::workspace::*;

/// Which eraser the Eraser is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::workspace) enum EraserKind {
    /// Rubs out the area it passes over, fills and lines alike (or with Faucet deletes what is
    /// clicked; on a raster layer it rubs out pixels).
    #[default]
    Rub,
    /// A vector eraser, taking lines by the piece or whole.
    Vector(VectorEraserType),
}

impl EraserKind {
    /// Every eraser, in the order the list shows them.
    pub(in crate::workspace) const ALL: [Self; 5] = [
        Self::Rub,
        Self::Vector(VectorEraserType::Segment),
        Self::Vector(VectorEraserType::Whole),
        Self::Vector(VectorEraserType::Rectangular),
        Self::Vector(VectorEraserType::Freehand),
    ];

    fn icon(self) -> &'static str {
        match self {
            Self::Rub => "tool-eraser",
            Self::Vector(_) => "tool-vector-eraser",
        }
    }
}

impl Workspace {
    /// The vector eraser in hand, if the Eraser is one.
    pub(in crate::workspace) fn vector_eraser(&self) -> Option<VectorEraserType> {
        match self.eraser_kind {
            EraserKind::Vector(kind) if self.tool == Tool::Eraser => Some(kind),
            _ => None,
        }
    }

    /// Picks the eraser the Eraser is.
    pub(in crate::workspace) fn set_eraser_kind(&mut self, kind: EraserKind) {
        self.eraser_kind = kind;
    }

    /// Whether the eraser in hand takes what lies inside a box or loop dragged, rather than what
    /// its tip touches.
    pub(in crate::workspace) fn vector_encloses(&self) -> bool {
        self.vector_eraser().is_some_and(|kind| !kind.uses_tip())
    }

    /// The box or loop the drag through `samples` makes, for the eraser in hand; empty for one
    /// that is not an area yet.
    pub(in crate::workspace) fn vector_area(&self, samples: &[InputSample]) -> Vec<DVec2> {
        let point = |s: &InputSample| DVec2::new(s.x, s.y);
        match self.vector_eraser() {
            Some(VectorEraserType::Rectangular) => match (samples.first(), samples.last()) {
                (Some(a), Some(b)) => vector::rectangle(point(a), point(b)),
                _ => Vec::new(),
            },
            Some(VectorEraserType::Freehand) => {
                vector::freehand_loop(&samples.iter().map(point).collect::<Vec<_>>())
            }
            _ => Vec::new(),
        }
    }

    /// Takes away the shapes wholly inside the area the drag through `samples` made, as one
    /// undo step; nothing when it holds none.
    pub(in crate::workspace) fn finish_vector_area(&mut self, samples: &[InputSample]) {
        let area = self.vector_area(samples);
        let Some(scene) = self.scene_id() else {
            return;
        };
        if area.is_empty() {
            return;
        }
        let commands = vector::erase_with(
            self.document(),
            scene,
            self.frame,
            &[area],
            Take::Inside,
            self.eraser_layer(),
        );
        if !commands.is_empty() {
            self.run_batch(commands, &self.tr("msg-erase"));
        }
    }

    /// The box or loop being dragged, as a line round it.
    pub(in crate::workspace) fn draw_vector_area(
        &self,
        painter: &egui::Painter,
        to_screen: impl Fn(Pos2) -> Pos2,
    ) {
        if !self.vector_encloses() {
            return;
        }
        let area = self.vector_area(&self.freehand);
        if area.len() < 3 {
            return;
        }
        let points = area
            .iter()
            .map(|p| to_screen(Pos2::new(p.x as f32, p.y as f32)))
            .collect();
        painter.add(egui::Shape::closed_line(
            points,
            Stroke::new(1.5, theme::accent()),
        ));
    }

    /// The name of `kind` in the Eraser's list.
    fn eraser_kind_name(&self, kind: EraserKind) -> String {
        match kind {
            EraserKind::Rub => tr!(self, "opt-eraser-kind-rub"),
            EraserKind::Vector(VectorEraserType::Segment) => tr!(self, "opt-eraser-kind-segment"),
            EraserKind::Vector(VectorEraserType::Whole) => tr!(self, "opt-eraser-kind-whole"),
            EraserKind::Vector(VectorEraserType::Rectangular) => {
                tr!(self, "opt-eraser-kind-rectangular")
            }
            EraserKind::Vector(VectorEraserType::Freehand) => {
                tr!(self, "opt-eraser-kind-freehand")
            }
        }
    }

    /// What `kind` does, shown when its row in the list is pointed at and under the options.
    fn eraser_kind_help(&self, kind: EraserKind) -> String {
        match kind {
            EraserKind::Rub if self.eraser_faucet => tr!(self, "tools-help-eraser-faucet"),
            EraserKind::Rub => tr!(self, "tools-help-eraser"),
            EraserKind::Vector(VectorEraserType::Segment) => tr!(self, "tools-help-vector-eraser"),
            EraserKind::Vector(VectorEraserType::Whole) => {
                tr!(self, "tools-help-vector-eraser-whole")
            }
            EraserKind::Vector(VectorEraserType::Rectangular) => {
                tr!(self, "tools-help-vector-eraser-rectangular")
            }
            EraserKind::Vector(VectorEraserType::Freehand) => {
                tr!(self, "tools-help-vector-eraser-freehand")
            }
        }
    }

    /// The Eraser row: the list of erasers, each with its picture and name, as the Brush row
    /// lists brushes.
    pub(in crate::workspace) fn eraser_picker(&mut self, ui: &mut Ui) {
        let current = self.eraser_kind;
        let entries: Vec<(EraserKind, String, String)> = EraserKind::ALL
            .into_iter()
            .map(|kind| {
                (
                    kind,
                    self.eraser_kind_name(kind),
                    self.eraser_kind_help(kind),
                )
            })
            .collect();
        let shown = self.eraser_kind_name(current);
        let mut pick = None;
        chrome::row(ui, &tr!(self, "opt-eraser"), |ui| {
            egui::ComboBox::from_id_salt("eraser-list")
                .selected_text(shown)
                .width(ui.available_width().min(200.0))
                .truncate()
                .show_ui(ui, |ui| {
                    for (kind, name, help) in &entries {
                        let picture = icons::image(kind.icon(), 16.0, theme::text());
                        let row = egui::Button::image_and_text(picture, name.as_str())
                            .selected(*kind == current)
                            .frame_when_inactive(false);
                        if ui.add(row).on_hover_text(help).clicked() {
                            pick = Some(*kind);
                        }
                    }
                });
        });
        if let Some(kind) = pick {
            self.set_eraser_kind(kind);
        }
    }

    /// A vector eraser's options: the tip it shares with the Eraser, for the types that use one.
    pub(in crate::workspace) fn vector_eraser_options(
        &mut self,
        ui: &mut Ui,
        kind: VectorEraserType,
    ) {
        if kind.uses_tip() {
            self.eraser_tip_options(ui);
        }
        ui.weak(self.eraser_kind_help(EraserKind::Vector(kind)));
    }
}
