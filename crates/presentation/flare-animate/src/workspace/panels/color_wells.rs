//! The stroke and fill colors in the tool strip, arranged as Animate's toolbar has them: a
//! swatch for the stroke over a swatch for the fill, each with a small picture of its tool, and
//! under them two small buttons: black and white, and swap. No color is in each swatch's picker.

use egui::color_picker::{Alpha, color_picker_color32};
use egui::{Color32, Popup, PopupCloseBehavior, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, vec2};
use flare_ui::{icons, theme};

use crate::workspace::{ColorTarget, Workspace};

/// Side of a color swatch, and of the small picture of its tool beside it.
pub(crate) const SWATCH: f32 = 16.0;
pub(crate) const PICTURE: f32 = 10.0;
const GAP: f32 = 2.0;
/// The widest and narrowest the two small buttons are drawn.
const BUTTON_MAX: f32 = 16.0;
const BUTTON_MIN: f32 = 12.0;
/// The space between the two small buttons.
const BUTTON_GAP: f32 = 1.0;

/// Where everything in the color section goes, from its top-left corner.
pub(crate) struct WellRects {
    pub(crate) stroke_picture: Rect,
    pub(crate) stroke: Rect,
    pub(crate) fill_picture: Rect,
    pub(crate) fill: Rect,
    pub(crate) defaults: Rect,
    pub(crate) swap: Rect,
}

/// Width of the swatch rows.
pub(crate) const ROW_WIDTH: f32 = PICTURE + GAP + SWATCH;

/// The side of each of the two small buttons in a strip `width` wide.
pub(crate) fn button_side(width: f32) -> f32 {
    ((width - BUTTON_GAP) / 2.0).clamp(BUTTON_MIN, BUTTON_MAX)
}

/// The height of the section in a strip `width` wide.
pub(crate) fn height(width: f32) -> f32 {
    SWATCH * 2.0 + 4.0 + 6.0 + button_side(width)
}

pub(crate) fn well_rects(origin: Pos2, width: f32) -> WellRects {
    let at = |x: f32, y: f32, w: f32, h: f32| Rect::from_min_size(origin + vec2(x, y), vec2(w, h));
    let side = button_side(width);
    let buttons_y = SWATCH * 2.0 + 4.0 + 6.0;
    // The two buttons are centered under the rows.
    let row = side * 2.0 + BUTTON_GAP;
    let left = (ROW_WIDTH - row) / 2.0;
    let picture_y = (SWATCH - PICTURE) / 2.0;
    WellRects {
        stroke_picture: at(0.0, picture_y, PICTURE, PICTURE),
        stroke: at(PICTURE + GAP, 0.0, SWATCH, SWATCH),
        fill_picture: at(0.0, SWATCH + 4.0 + picture_y, PICTURE, PICTURE),
        fill: at(PICTURE + GAP, SWATCH + 4.0, SWATCH, SWATCH),
        defaults: at(left, buttons_y, side, side),
        swap: at(left + side + BUTTON_GAP, buttons_y, side, side),
    }
}

/// Where the section goes in `area`, which is the width of the strip: everything is centered
/// across it, the rows and the two buttons around the same middle.
pub(crate) fn rects_in(area: Rect) -> WellRects {
    let width = area.width();
    let row = ROW_WIDTH.max(button_side(width) * 2.0 + BUTTON_GAP);
    let origin = Pos2::new(area.center().x - row / 2.0, area.top());
    well_rects(
        Pos2::new(origin.x + (row - ROW_WIDTH) / 2.0, origin.y),
        width,
    )
}

impl Workspace {
    /// The color section and its pickers, centered in the strip.
    pub(crate) fn color_wells(&mut self, ui: &mut Ui) {
        let width = ui.available_width();
        let (area, _) = ui.allocate_exact_size(vec2(width, height(width)), Sense::hover());
        let rects = rects_in(area);
        let painter = ui.painter().clone();
        for (picture, icon) in [
            (rects.stroke_picture, "stroke-color"),
            (rects.fill_picture, "fill-color"),
        ] {
            icons::image(icon, PICTURE, theme::TEXT_DIM).paint_at(ui, picture);
        }
        let wells = [
            (
                rects.stroke,
                ColorTarget::Stroke,
                "stroke_swatch",
                "Stroke color",
            ),
            (rects.fill, ColorTarget::Fill, "fill_swatch", "Fill color"),
        ];
        for (rect, target, id, hint) in wells {
            let response = ui
                .interact(rect, ui.make_persistent_id(id), Sense::click())
                .on_hover_text(hint);
            let color = match target {
                ColorTarget::Stroke => &mut self.paint.stroke_color,
                _ => &mut self.paint.fill,
            };
            paint_swatch(&painter, rect, *color);
            let selected = self.color_target == target;
            if selected || response.hovered() {
                painter.rect_stroke(
                    rect,
                    3.0,
                    Stroke::new(1.5, theme::ACCENT),
                    StrokeKind::Outside,
                );
            }
            if response.clicked() {
                self.color_target = target;
            }
            Popup::from_toggle_button_response(&response)
                .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
                .show(|ui| {
                    ui.spacing_mut().slider_width = 200.0;
                    // As in Animate's swatch picker: a way to have none of this color.
                    ui.horizontal(|ui| {
                        let none = ui
                            .add(egui::Button::image_and_text(
                                icons::image("no-color", 14.0, theme::TEXT),
                                "No color",
                            ))
                            .on_hover_text(match target {
                                ColorTarget::Stroke => "No stroke",
                                _ => "No fill",
                            });
                        if none.clicked() {
                            *color = Color32::TRANSPARENT;
                        }
                    });
                    color_picker_color32(ui, color, Alpha::OnlyBlend);
                });
        }
        if tiny_button(ui, rects.defaults, "default-colors", "Black and white") {
            self.default_colors();
        }
        if tiny_button(ui, rects.swap, "swap-colors", "Swap fill and stroke") {
            self.swap_colors();
        }
    }
}

/// A swatch of `color`. An empty one (no color) is drawn as a white square with a red slash, as
/// Animate draws it.
fn paint_swatch(painter: &egui::Painter, rect: Rect, color: Color32) {
    if color.a() == 0 {
        painter.rect_filled(rect, 3.0, Color32::WHITE);
        painter.line_segment(
            [rect.left_bottom(), rect.right_top()],
            Stroke::new(1.5, Color32::from_rgb(0xd0, 0x2b, 0x2b)),
        );
    } else {
        painter.rect_filled(rect, 3.0, color);
    }
    painter.rect_stroke(
        rect,
        3.0,
        Stroke::new(1.0, theme::BORDER),
        StrokeKind::Inside,
    );
}

/// One of the small buttons under the swatches.
fn tiny_button(ui: &mut Ui, rect: Rect, icon: &str, hint: &str) -> bool {
    let response = ui
        .interact(rect, ui.make_persistent_id(icon), Sense::click())
        .on_hover_text(hint);
    if response.hovered() {
        ui.painter().rect_filled(rect, 2.0, theme::HOVER);
    }
    let tint = if response.hovered() {
        Color32::WHITE
    } else {
        theme::TEXT
    };
    icons::image(icon, rect.width(), tint).paint_at(ui, rect.shrink(1.0));
    response.clicked()
}
