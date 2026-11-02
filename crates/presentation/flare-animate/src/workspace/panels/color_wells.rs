//! The fill and stroke colors in the tool strip: two overlapping squares, the stroke in front as
//! a frame around an empty middle, with the black-and-white and swap buttons side by side above
//! them, as in Animate's toolbar. In a strip one tool wide the same box is drawn smaller. No color
//! is at the top of each swatch's picker.

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

/// Side of the box the squares sit in, in a strip wide enough for it.
const BOX: f32 = 50.0;

/// The space between the squares and the buttons, and between the buttons.
const BUTTON_GAP: f32 = 2.0;

impl Metrics {
    pub(crate) const FULL: Self = Self {
        square: 30.0,
        offset: 18.0,
        frame: 6.0,
        small: 16.0,
    };
    /// For a strip one tool wide (24 px a tool, a little more with its margins): the squares are
    /// 25 px across and the two buttons sit under them in a row.
    pub(crate) const COMPACT: Self = Self {
        square: 16.0,
        offset: 9.0,
        frame: 4.0,
        small: 12.0,
    };

    /// The drawing that fits `width`.
    pub(crate) fn for_width(width: f32) -> Self {
        if width < Self::FULL.offset + Self::FULL.square {
            Self::COMPACT
        } else {
            Self::FULL
        }
    }

    /// The two buttons and the gap between them.
    fn buttons(self) -> f32 {
        self.small * 2.0 + BUTTON_GAP
    }

    pub(crate) fn width(self) -> f32 {
        BOX.min(self.offset + self.square).max(self.buttons())
    }

    pub(crate) fn height(self) -> f32 {
        self.small + BUTTON_GAP + self.offset + self.square
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
    // In a row above the squares, black and white on the left and swap on the right, centered.
    let left = (m.width() - m.buttons()) / 2.0;
    let defaults = at(left, 0.0, m.small);
    let swap = at(left + m.small + BUTTON_GAP, 0.0, m.small);
    // The squares below them, centered in the same width.
    let top = m.small + BUTTON_GAP;
    let x = (m.width() - (m.offset + m.square)) / 2.0;
    WellRects {
        fill: at(x, top, m.square),
        stroke: at(x + m.offset, top + m.offset, m.square),
        defaults,
        swap,
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
        let (fill_hint, stroke_hint) = (tr!(self, "col-fill-color"), tr!(self, "col-stroke-color"));
        let (none_label, no_stroke, no_fill) = (
            tr!(self, "col-none"),
            tr!(self, "col-no-stroke"),
            tr!(self, "col-no-fill"),
        );
        let (defaults_hint, swap_hint) = (tr!(self, "col-black-white"), tr!(self, "col-swap"));
        // The fill, behind.
        let fill = ui
            .interact(
                rects.fill,
                ui.make_persistent_id("fill_swatch"),
                Sense::click(),
            )
            .on_hover_text(fill_hint);
        paint_square(&painter, rects.fill, self.paint.fill);
        // The stroke, in front: a frame around the strip's own color showing through.
        let stroke = ui
            .interact(
                rects.stroke,
                ui.make_persistent_id("stroke_swatch"),
                Sense::click(),
            )
            .on_hover_text(stroke_hint);
        paint_square(&painter, rects.stroke, self.paint.stroke_color);
        painter.rect(
            rects.stroke.shrink(m.frame),
            1.0,
            theme::panel_raised(),
            Stroke::new(1.0, theme::border()),
            StrokeKind::Inside,
        );
        for (response, target) in [(&fill, ColorTarget::Fill), (&stroke, ColorTarget::Stroke)] {
            let color = match target {
                ColorTarget::Stroke => &mut self.paint.stroke_color,
                _ => &mut self.paint.fill,
            };
            if response.hovered() {
                painter.rect_stroke(
                    response.rect,
                    3.0,
                    Stroke::new(1.5, theme::accent()),
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
                            icons::image("no-color", 14.0, theme::text()),
                            &none_label,
                        ))
                        .on_hover_text(match target {
                            ColorTarget::Stroke => &no_stroke,
                            _ => &no_fill,
                        });
                    if none.clicked() {
                        *color = Color32::TRANSPARENT;
                    }
                    color_picker_color32(ui, color, Alpha::OnlyBlend);
                });
        }
        if tiny_button(ui, rects.defaults, "default-colors", &defaults_hint) {
            self.default_colors();
        }
        if tiny_button(ui, rects.swap, "swap-colors", &swap_hint) {
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
        Stroke::new(1.0, theme::border()),
        StrokeKind::Inside,
    );
}

/// One of the small buttons under the squares.
fn tiny_button(ui: &mut Ui, rect: Rect, icon: &str, hint: &str) -> bool {
    let response = ui
        .interact(rect, ui.make_persistent_id(icon), Sense::click())
        .on_hover_text(hint);
    if response.hovered() {
        ui.painter().rect_filled(rect, 2.0, theme::hover());
    }
    let tint = if response.hovered() {
        theme::text_strong()
    } else {
        theme::text()
    };
    icons::image(icon, rect.width(), tint).paint_at(ui, rect.shrink(1.0));
    response.clicked()
}
