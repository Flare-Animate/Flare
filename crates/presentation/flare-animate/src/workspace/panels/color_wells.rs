//! The fill and stroke colors in the tool strip: two overlapping squares, the stroke in front as
//! a frame around an empty middle, with the swap and default buttons at the corners. In a strip
//! one tool wide the same box is drawn smaller, with the two buttons stacked beneath it.

use egui::color_picker::{Alpha, color_picker_color32};
use egui::{Popup, PopupCloseBehavior, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, vec2};
use flare_ui::{chrome, theme};

use crate::workspace::Workspace;

/// Side of the box the squares sit in.
pub(crate) const BOX: f32 = 50.0;
/// Side of each square, and how far the front one is pushed down and right.
pub(crate) const SQUARE: f32 = 30.0;
pub(crate) const OFFSET: f32 = 18.0;
/// Width of the stroke's frame.
const FRAME: f32 = 6.0;
pub(crate) const SMALL: f32 = 16.0;

/// The sizes of one drawing of the box.
#[derive(Clone, Copy)]
pub(crate) struct Metrics {
    square: f32,
    offset: f32,
    frame: f32,
    small: f32,
    /// Whether the two buttons are stacked beneath the squares rather than at their corners.
    stacked: bool,
}

impl Metrics {
    pub(crate) const FULL: Self = Self {
        square: SQUARE,
        offset: OFFSET,
        frame: FRAME,
        small: SMALL,
        stacked: false,
    };
    /// For a strip one tool wide (24 px a tool, a little more with its margins): the squares are
    /// 25 px across and the buttons, 16 px, sit under them one above the other.
    pub(crate) const COMPACT: Self = Self {
        square: 16.0,
        offset: 9.0,
        frame: 4.0,
        small: SMALL,
        stacked: true,
    };

    /// The drawing that fits `width`.
    pub(crate) fn for_width(width: f32) -> Self {
        if width < OFFSET + SQUARE {
            Self::COMPACT
        } else {
            Self::FULL
        }
    }

    pub(crate) fn width(self) -> f32 {
        if self.stacked {
            self.offset + self.square
        } else {
            BOX.min(self.offset + self.square)
        }
    }

    pub(crate) fn height(self) -> f32 {
        if self.stacked {
            self.offset + self.square + 2.0 + self.small * 2.0 + 2.0
        } else {
            BOX
        }
    }
}

/// Where everything in the color box goes, from the box's top-left corner.
pub(crate) struct WellRects {
    pub(crate) fill: Rect,
    pub(crate) stroke: Rect,
    pub(crate) swap: Rect,
    pub(crate) defaults: Rect,
}

pub(crate) fn rects_with(origin: Pos2, m: Metrics) -> WellRects {
    let at = |x: f32, y: f32, side: f32| Rect::from_min_size(origin + vec2(x, y), vec2(side, side));
    let below = m.offset + m.square + 2.0;
    let (swap, defaults) = if m.stacked {
        let x = (m.width() - m.small) / 2.0;
        (at(x, below + m.small + 2.0, m.small), at(x, below, m.small))
    } else {
        (
            at(m.square + 2.0, 0.0, m.small),
            at(0.0, m.square + 2.0, m.small),
        )
    };
    WellRects {
        fill: at(0.0, 0.0, m.square),
        stroke: at(m.offset, m.offset, m.square),
        swap,
        defaults,
    }
}

impl Workspace {
    /// The color box and its popups, centered in the strip.
    pub(crate) fn color_wells(&mut self, ui: &mut Ui) {
        let width = ui.available_width();
        let m = Metrics::for_width(width);
        let (area, _) = ui.allocate_exact_size(vec2(width, m.height()), Sense::hover());
        let rects = rects_with(pos2_centered(area, m), m);
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
            rects.stroke.shrink(m.frame),
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
        if small_button(ui, rects.swap, "swap-colors", "Swap fill and stroke", m) {
            self.swap_colors();
        }
        if small_button(ui, rects.defaults, "default-colors", "Black and white", m) {
            self.default_colors();
        }
    }
}

/// The top-left corner that centers the squares across `area`.
fn pos2_centered(area: Rect, m: Metrics) -> Pos2 {
    Pos2::new(area.center().x - m.width() / 2.0, area.top())
}

fn small_button(ui: &mut Ui, rect: Rect, icon: &str, hint: &str, m: Metrics) -> bool {
    ui.put(rect, |ui: &mut Ui| {
        chrome::icon_button(ui, icon, m.small - 4.0, false, true, hint)
    })
    .clicked()
}
