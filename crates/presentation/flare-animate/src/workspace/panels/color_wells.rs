//! The fill and stroke colors in the tool strip: two overlapping squares, the stroke in front as
//! a frame around an empty middle, with the swap and default buttons at the corners.

use egui::color_picker::{Alpha, color_picker_color32};
use egui::{Popup, PopupCloseBehavior, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, vec2};
use flare_ui::{chrome, theme};

use crate::workspace::Workspace;

/// Side of the box the squares sit in.
const BOX: f32 = 66.0;
/// Side of each square, and how far the front one is pushed down and right.
const SQUARE: f32 = 40.0;
const OFFSET: f32 = 24.0;
/// Width of the stroke's frame.
const FRAME: f32 = 8.0;
const SMALL: f32 = 18.0;

/// Where everything in the color box goes, from the box's top-left corner.
pub(crate) struct WellRects {
    pub(crate) fill: Rect,
    pub(crate) stroke: Rect,
    pub(crate) swap: Rect,
    pub(crate) defaults: Rect,
}

pub(crate) fn well_rects(origin: Pos2) -> WellRects {
    let at = |x: f32, y: f32, side: f32| Rect::from_min_size(origin + vec2(x, y), vec2(side, side));
    WellRects {
        fill: at(0.0, 0.0, SQUARE),
        stroke: at(OFFSET, OFFSET, SQUARE),
        swap: at(SQUARE + 2.0, 0.0, SMALL),
        defaults: at(0.0, SQUARE + 2.0, SMALL),
    }
}

impl Workspace {
    /// The color box and its popups, centered in the strip.
    pub(crate) fn color_wells(&mut self, ui: &mut Ui) {
        let width = ui.available_width();
        let (area, _) = ui.allocate_exact_size(vec2(width, BOX), Sense::hover());
        let rects = well_rects(pos2_centered(area));
        let painter = ui.painter().clone();
        // The fill, behind.
        let fill = ui
            .interact(
                rects.fill,
                ui.make_persistent_id("fill_swatch"),
                Sense::click(),
            )
            .on_hover_text("Fill color");
        painter.rect(
            rects.fill,
            3.0,
            self.paint.fill,
            Stroke::new(1.0, theme::BORDER),
            StrokeKind::Inside,
        );
        // The stroke, in front: a frame around the panel showing through.
        let stroke = ui
            .interact(
                rects.stroke,
                ui.make_persistent_id("stroke_swatch"),
                Sense::click(),
            )
            .on_hover_text("Stroke color");
        painter.rect(
            rects.stroke,
            3.0,
            self.paint.stroke_color,
            Stroke::new(1.0, theme::BORDER),
            StrokeKind::Inside,
        );
        painter.rect(
            rects.stroke.shrink(FRAME),
            1.0,
            theme::PANEL,
            Stroke::new(1.0, theme::BORDER),
            StrokeKind::Inside,
        );
        for (response, color) in [
            (&fill, &mut self.paint.fill),
            (&stroke, &mut self.paint.stroke_color),
        ] {
            if response.hovered() {
                painter.rect_stroke(
                    response.rect,
                    3.0,
                    Stroke::new(1.5, theme::ACCENT),
                    StrokeKind::Outside,
                );
            }
            Popup::from_toggle_button_response(response)
                .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
                .show(|ui| {
                    ui.spacing_mut().slider_width = 200.0;
                    color_picker_color32(ui, color, Alpha::OnlyBlend);
                });
        }
        if small_button(ui, rects.swap, "swap-colors", "Swap fill and stroke") {
            self.swap_colors();
        }
        if small_button(ui, rects.defaults, "default-colors", "Black and white") {
            self.default_colors();
        }
    }
}

/// The top-left corner that centers the squares across `area`.
fn pos2_centered(area: Rect) -> Pos2 {
    Pos2::new(area.center().x - (OFFSET + SQUARE) / 2.0, area.top())
}

fn small_button(ui: &mut Ui, rect: Rect, icon: &str, hint: &str) -> bool {
    ui.put(rect, |ui: &mut Ui| {
        chrome::icon_button(ui, icon, 12.0, false, true, hint)
    })
    .clicked()
}
