//! The fill and stroke colors in the tool strip: two overlapping squares, the stroke in front as
//! a frame around an empty middle, and under them, as in Animate's toolbar, two small buttons:
//! black and white, and swap. In a strip one tool wide the same box is drawn smaller. No color is
//! at the top of each swatch's picker.

use egui::color_picker::{Alpha, color_picker_color32};
use egui::{Color32, Popup, PopupCloseBehavior, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, vec2};
use flare_ui::{icons, theme};

use crate::workspace::{ColorTarget, Workspace};

/// The sizes of one drawing of the box.
#[derive(Clone, Copy)]
pub(crate) struct Metrics {
    /// Side of each square.
    square: f32,
    /// How far the front square is pushed down and right.
    offset: f32,
    /// Width of the stroke's frame.
    frame: f32,
    /// Side of each of the two small buttons.
    small: f32,
}

/// The space between the squares and the buttons, and between the buttons.
const GAP: f32 = 3.0;

impl Metrics {
    pub(crate) const FULL: Self = Self {
        square: 30.0,
        offset: 18.0,
        frame: 6.0,
        small: 16.0,
    };
    /// For a strip one tool wide (24 px a tool, a little more with its margins): the squares are
    /// 25 px across.
    pub(crate) const COMPACT: Self = Self {
        square: 16.0,
        offset: 9.0,
        frame: 4.0,
        small: 12.0,
    };

    /// The drawing that fits `width`.
    pub(crate) fn for_width(width: f32) -> Self {
        if width < Self::FULL.width() {
            Self::COMPACT
        } else {
            Self::FULL
        }
    }

    /// Across the squares, which are wider than the two buttons under them.
    pub(crate) fn width(self) -> f32 {
        (self.offset + self.square).max(self.small * 2.0 + 1.0)
    }

    pub(crate) fn height(self) -> f32 {
        self.offset + self.square + GAP + self.small
    }
}

/// Where everything in the color section goes, from its top-left corner.
pub(crate) struct WellRects {
    pub(crate) fill: Rect,
    pub(crate) stroke: Rect,
    pub(crate) defaults: Rect,
    pub(crate) swap: Rect,
}

pub(crate) fn rects_with(origin: Pos2, m: Metrics) -> WellRects {
    let at = |x: f32, y: f32, side: f32| Rect::from_min_size(origin + vec2(x, y), vec2(side, side));
    let buttons_y = m.offset + m.square + GAP;
    let left = (m.width() - (m.small * 2.0 + 1.0)) / 2.0;
    // The squares are centered over the buttons if they are the narrower.
    let squares_left = (m.width() - (m.offset + m.square)) / 2.0;
    WellRects {
        fill: at(squares_left, 0.0, m.square),
        stroke: at(squares_left + m.offset, m.offset, m.square),
        defaults: at(left, buttons_y, m.small),
        swap: at(left + m.small + 1.0, buttons_y, m.small),
    }
}

impl Workspace {
    /// The color section and its pickers, centered in the strip.
    pub(crate) fn color_wells(&mut self, ui: &mut Ui) {
        let width = ui.available_width();
        let m = Metrics::for_width(width);
        let (area, _) = ui.allocate_exact_size(vec2(width, m.height()), Sense::hover());
        let origin = Pos2::new(area.center().x - m.width() / 2.0, area.top());
        let rects = rects_with(origin, m);
        let painter = ui.painter().clone();
        // The fill, behind.
        let fill = ui
            .interact(
                rects.fill,
                ui.make_persistent_id("fill_swatch"),
                Sense::click(),
            )
            .on_hover_text("Fill color");
        paint_square(&painter, rects.fill, self.paint.fill);
        // The stroke, in front: a frame around the strip's own color showing through.
        let stroke = ui
            .interact(
                rects.stroke,
                ui.make_persistent_id("stroke_swatch"),
                Sense::click(),
            )
            .on_hover_text("Stroke color");
        paint_square(&painter, rects.stroke, self.paint.stroke_color);
        painter.rect(
            rects.stroke.shrink(m.frame),
            1.0,
            theme::PANEL_RAISED,
            Stroke::new(1.0, theme::BORDER),
            StrokeKind::Inside,
        );
        for (response, target) in [(&fill, ColorTarget::Fill), (&stroke, ColorTarget::Stroke)] {
            let color = match target {
                ColorTarget::Stroke => &mut self.paint.stroke_color,
                _ => &mut self.paint.fill,
            };
            if response.hovered() || self.color_target == target {
                painter.rect_stroke(
                    response.rect,
                    3.0,
                    Stroke::new(1.5, theme::ACCENT),
                    StrokeKind::Outside,
                );
            }
            if response.clicked() {
                self.color_target = target;
            }
            Popup::from_toggle_button_response(response)
                .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
                .show(|ui| {
                    ui.spacing_mut().slider_width = 200.0;
                    // As in Animate's swatch picker: a way to have none of this color.
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

/// A square of `color`. An empty one (no color) is a white square with a red slash.
fn paint_square(painter: &egui::Painter, rect: Rect, color: Color32) {
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

/// One of the small buttons under the squares.
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
