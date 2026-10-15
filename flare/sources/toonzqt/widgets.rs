//! Shared panel grammar: measured tabs, semantic surfaces and locale-aware fields.
use crate::theme::{self, Palette};
use eframe::egui::{self, vec2, Color32, FontId, Rect, Response, Sense, Stroke};

pub fn button(ui: &mut egui::Ui, text: impl Into<egui::WidgetText>) -> Response {
    let p = if ui.visuals().dark_mode {
        theme::Theme::Dark
    } else {
        theme::Theme::Light
    }
    .palette();
    let background = ui.painter().add(egui::Shape::Noop);
    let response = ui
        .scope(|ui| {
            let visuals = ui.visuals_mut();
            for visuals in [
                &mut visuals.widgets.inactive,
                &mut visuals.widgets.hovered,
                &mut visuals.widgets.active,
                &mut visuals.widgets.open,
            ] {
                visuals.weak_bg_fill = Color32::TRANSPARENT;
                visuals.bg_fill = Color32::TRANSPARENT;
                visuals.bg_stroke.color = Color32::TRANSPARENT;
            }
            ui.add(egui::Button::new(text).corner_radius(0))
        })
        .inner;
    let down = response.is_pointer_button_down_on();
    let [top, bottom] = if down {
        p.chrome.pressed
    } else if response.hovered() {
        p.chrome.hover
    } else {
        p.chrome.button
    };
    ui.painter().set(
        background,
        theme::gradient(
            response.rect,
            Color32::from_gray(top),
            Color32::from_gray(bottom),
        ),
    );
    theme::bevel_edges(ui.painter(), response.rect, p, down);
    if response.has_focus() {
        ui.painter().rect_stroke(
            response.rect,
            0.0,
            Stroke::new(1.0, p.accent),
            egui::StrokeKind::Inside,
        );
    }
    response
}

/// Native combo interaction with a compact gradient face and separate arrow well.
/// The underlying egui widget retains keyboard navigation and popup semantics.
pub fn combo_box<R>(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    selected_text: impl Into<egui::WidgetText>,
    width: f32,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<Option<R>> {
    combo_box_face(ui, id, selected_text, width, false, add)
}

/// Zoom and editable-value selectors have a pale value well next to the arrow.
pub fn combo_value_box<R>(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    selected_text: impl Into<egui::WidgetText>,
    width: f32,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<Option<R>> {
    combo_box_face(ui, id, selected_text, width, true, add)
}

fn combo_box_face<R>(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    selected_text: impl Into<egui::WidgetText>,
    width: f32,
    value_well: bool,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<Option<R>> {
    let p = if ui.visuals().dark_mode {
        theme::Theme::Dark
    } else {
        theme::Theme::Light
    }
    .palette();
    let background = ui.painter().add(egui::Shape::Noop);
    let popup_visuals = ui.visuals().clone();
    let response = ui
        .scope(|ui| {
            ui.spacing_mut().icon_width = 12.0;
            ui.spacing_mut().icon_spacing = 5.0;
            let visuals = ui.visuals_mut();
            if value_well {
                visuals.override_text_color =
                    Some(Color32::from_gray(if p.back.r() > 128 { 50 } else { 35 }));
            }
            for state in [&mut visuals.widgets.inactive, &mut visuals.widgets.hovered] {
                state.bg_fill = Color32::TRANSPARENT;
                state.weak_bg_fill = Color32::TRANSPARENT;
                state.bg_stroke.color = Color32::TRANSPARENT;
            }
            // Borrow the visual states together so all response states reserve an identical frame.
            for state in [&mut visuals.widgets.active, &mut visuals.widgets.open] {
                state.bg_fill = Color32::TRANSPARENT;
                state.weak_bg_fill = Color32::TRANSPARENT;
                state.bg_stroke.color = Color32::TRANSPARENT;
            }
            egui::ComboBox::from_id_salt(id)
                .width(width)
                .height(300.0)
                .selected_text(selected_text)
                .icon(move |ui, rect, _, _| {
                    let c = rect.center();
                    ui.painter().add(egui::Shape::convex_polygon(
                        vec![
                            c + vec2(-3.0, -1.6),
                            c + vec2(3.0, -1.6),
                            c + vec2(0.0, 2.0),
                        ],
                        p.text,
                        Stroke::NONE,
                    ));
                })
                .show_ui(ui, |ui| {
                    *ui.visuals_mut() = popup_visuals;
                    add(ui)
                })
        })
        .inner;
    let [top, bottom] = if response.response.is_pointer_button_down_on() {
        p.chrome.pressed
    } else if response.response.hovered() {
        p.chrome.hover
    } else if value_well {
        if p.back.r() > 128 {
            [217, 201]
        } else {
            [85, 69]
        }
    } else {
        p.chrome.button
    };
    let mut shapes = vec![theme::gradient(
        response.response.rect,
        Color32::from_gray(top),
        Color32::from_gray(bottom),
    )];
    if value_well {
        let mut face = response.response.rect;
        face.max.x -= 18.0;
        shapes.push(egui::Shape::rect_filled(face, 0.0, p.field));
    }
    ui.painter().set(background, egui::Shape::Vec(shapes));
    theme::field_edges(ui.painter(), response.response.rect, p);
    if value_well {
        for vertical in [false, true] {
            theme::edge_bands(
                ui.painter(),
                response.response.rect,
                vertical,
                false,
                if p.back.r() > 128 {
                    &[126, 151, 221]
                } else {
                    &[36, 63, 121]
                },
            );
            theme::edge_bands(
                ui.painter(),
                response.response.rect,
                vertical,
                true,
                if p.back.r() > 128 {
                    &[126, 151]
                } else {
                    &[36, 63]
                },
            );
        }
    }
    let pixel = 1.0 / ui.ctx().pixels_per_point();
    let mut well = response.response.rect.shrink(3.0 * pixel);
    well.min.x = well.right() - 16.0;
    theme::edge_bands(
        ui.painter(),
        well,
        true,
        false,
        if p.back.r() > 128 {
            &[148, 220]
        } else {
            &[42, 91]
        },
    );
    if response.response.has_focus() {
        ui.painter().rect_stroke(
            response.response.rect,
            0.0,
            Stroke::new(pixel, p.accent),
            egui::StrokeKind::Inside,
        );
    }
    response
}

/// An entry's light face uses dark text in both themes; the surrounding UI keeps its palette.
pub fn text_edit(ui: &mut egui::Ui, edit: egui::TextEdit<'_>) -> Response {
    let p = if ui.visuals().dark_mode {
        theme::Theme::Dark
    } else {
        theme::Theme::Light
    }
    .palette();
    let response = ui
        .scope(|ui| {
            ui.visuals_mut().override_text_color =
                Some(Color32::from_gray(if p.back.r() > 128 { 50 } else { 35 }));
            ui.add(edit)
        })
        .inner;
    theme::field_edges(ui.painter(), response.rect, p);
    if response.has_focus() {
        let pixel = 1.0 / ui.ctx().pixels_per_point();
        ui.painter().rect_stroke(
            response.rect,
            0.0,
            Stroke::new(pixel, p.accent),
            egui::StrokeKind::Inside,
        );
    }
    response
}

/// Selection rows always reserve their border, even when it is invisible.
/// egui 0.36's selectable_label removes that frame while inactive, which changes
/// its allocated size when hover adds the border. Interaction changes paint only.
pub fn selectable(ui: &mut egui::Ui, selected: bool, label: String) -> Response {
    ui.scope(|ui| {
        ui.visuals_mut().widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
        ui.visuals_mut().widgets.inactive.bg_stroke.color = Color32::TRANSPARENT;
        ui.add(
            egui::Button::new(label)
                .selected(selected)
                .frame(true)
                .frame_when_inactive(true),
        )
    })
    .inner
}

pub fn tab(ui: &mut egui::Ui, id: &str, label: &str, selected: bool, p: Palette) -> Response {
    tab_with_flat_selected_face(ui, id, label, selected, p, false)
}

/// Inspector dock tabs use a flat selected face that joins the panel surface.
pub fn panel_tab(ui: &mut egui::Ui, id: &str, label: &str, selected: bool, p: Palette) -> Response {
    tab_with_flat_selected_face(ui, id, label, selected, p, true)
}

/// Thin dock header band, separate from the taller tab and control faces.
pub fn dock_cap(
    ui: &mut egui::Ui,
    p: Palette,
    side_margin: f32,
    right: bool,
    label: &str,
) -> Response {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 12.0), Sense::hover());
    let rect = rect.expand2(vec2(side_margin, 0.0));
    let light = p.back.r() > 128;
    let [top, bottom] = if light { [147, 123] } else { [48, 37] };
    ui.painter().add(theme::gradient(
        rect,
        Color32::from_gray(top),
        Color32::from_gray(bottom),
    ));
    let edge: &[u8] = if light {
        &[104, 166, 152]
    } else {
        &[35, 60, 52]
    };
    theme::edge_bands(ui.painter(), rect, false, false, edge);
    theme::edge_bands(
        ui.painter(),
        rect,
        false,
        true,
        &[if light { 116 } else { 37 }],
    );
    let hit = Rect::from_min_size(
        egui::pos2(
            if right {
                rect.right() - 22.0
            } else {
                rect.left()
            },
            rect.top(),
        ),
        vec2(22.0, rect.height()),
    );
    let response = ui.interact(hit, ui.id().with("dock.collapse"), Sense::click());
    let color = if response.hovered() {
        p.text
    } else if light {
        Color32::from_gray(67)
    } else {
        Color32::from_gray(161)
    };
    let center = hit.center();
    let pixel = 1.0 / ui.ctx().pixels_per_point();
    for offset in [-2.5, 2.5] {
        let x = center.x + offset;
        let direction = if right { 1.0 } else { -1.0 };
        let points = vec![
            egui::pos2(x - direction * 2.0, center.y - 2.5),
            egui::pos2(x + direction * 2.0, center.y),
            egui::pos2(x - direction * 2.0, center.y + 2.5),
        ];
        ui.painter().add(egui::Shape::convex_polygon(
            points.iter().map(|p| *p + vec2(0.0, pixel)).collect(),
            Color32::from_gray(if light { 178 } else { 76 }),
            Stroke::NONE,
        ));
        ui.painter()
            .add(egui::Shape::convex_polygon(points, color, Stroke::NONE));
    }
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    response.on_hover_text(label)
}

/// Compact panel menu mark, independently drawn and bounded by its tab rail.
pub fn panel_menu(ui: &mut egui::Ui, p: Palette, label: &str) -> Response {
    let (rect, response) =
        ui.allocate_exact_size(vec2(18.0, theme::PANEL_TAB_HEIGHT), Sense::click());
    let painter = ui.painter_at(rect);
    let color = if response.hovered() {
        p.text
    } else {
        Color32::from_gray(if p.back.r() > 128 { 69 } else { 168 })
    };
    let c = rect.center();
    let pixel = 1.0 / ui.ctx().pixels_per_point();
    for y in [-3.0, 0.0, 3.0] {
        painter.line_segment(
            [c + vec2(0.0, y), c + vec2(7.0, y)],
            Stroke::new(pixel, color),
        );
    }
    painter.add(egui::Shape::convex_polygon(
        vec![
            c + vec2(-6.0, -1.5),
            c + vec2(-2.0, -1.5),
            c + vec2(-4.0, 1.5),
        ],
        color,
        Stroke::NONE,
    ));
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    response.on_hover_text(label)
}

/// The dock rail is its own surface, spanning the unused space after the tabs.
pub fn tab_strip<R>(
    ui: &mut egui::Ui,
    p: Palette,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    let background = ui.painter().add(egui::Shape::Noop);
    let response = egui::Frame::NONE.show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        add(ui)
    });
    let shade = p.tab_rail.r();
    ui.painter().set(
        background,
        theme::gradient(
            response.response.rect,
            Color32::from_gray(shade.saturating_add(4)),
            Color32::from_gray(shade.saturating_sub(8)),
        ),
    );
    response
}

/// Dock rails own their lower compound seam; selected tabs remain open to the pane.
pub fn panel_tab_strip<R>(
    ui: &mut egui::Ui,
    p: Palette,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    let background = ui.painter().add(egui::Shape::Noop);
    let response = egui::Frame::NONE.show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        ui.spacing_mut().interact_size.y = theme::PANEL_TAB_HEIGHT;
        add(ui)
    });
    let shades: &[u8] = if p.back.r() > 128 {
        &[221, 161, 108]
    } else {
        &[84, 60, 35]
    };
    let pixel = 1.0 / ui.ctx().pixels_per_point();
    let r = response.response.rect;
    let shade = p.tab_rail.r();
    let mut shapes = vec![theme::gradient(
        r,
        Color32::from_gray(shade.saturating_add(4)),
        Color32::from_gray(shade.saturating_sub(8)),
    )];
    shapes.extend(shades.iter().enumerate().map(|(i, shade)| {
        let y = (r.bottom() / pixel).round() * pixel - (i + 1) as f32 * pixel;
        egui::Shape::rect_filled(
            Rect::from_min_max(egui::pos2(r.left(), y), egui::pos2(r.right(), y + pixel)),
            0.0,
            Color32::from_gray(*shade),
        )
    }));
    // Insert the seam behind the faces. The selected tab's lower edge stays open.
    ui.painter().set(background, egui::Shape::Vec(shapes));
    response
}

fn tab_with_flat_selected_face(
    ui: &mut egui::Ui,
    id: &str,
    label: &str,
    selected: bool,
    p: Palette,
    flat_selected_face: bool,
) -> Response {
    // Ghidra's DrawTabItemParameters separates rollover and frontmost tab paint.
    let text = ui.painter().layout_no_wrap(
        label.into(),
        FontId::new(
            if flat_selected_face {
                11.2
            } else {
                theme::UI_TEXT_SIZE
            },
            egui::FontFamily::Name("Semibold".into()),
        ),
        p.text,
    );
    let (rect, _) = ui.allocate_exact_size(
        vec2(
            text.size().x + if flat_selected_face { 12.0 } else { 16.0 },
            if flat_selected_face {
                theme::PANEL_TAB_HEIGHT
            } else {
                theme::TAB_HEIGHT
            },
        ),
        Sense::hover(),
    );
    let response = ui.interact(rect, ui.id().with(id), Sense::click());
    // A dock's separator is immediately outside this allocation. Clip the face
    // and inset centered strokes so neither can erase it at fractional DPI.
    let painter = ui.painter_at(rect);
    let (mut fill, mut stroke) = if selected {
        (p.chrome.tab, p.chrome.tab_normal_stroke)
    } else if response.hovered() {
        (p.chrome.tab_hover, p.chrome.tab_over_stroke)
    } else {
        (p.chrome.tab_inactive, p.chrome.tab_inactive_stroke)
    };
    // The supplied dark dock crop has a recessed inactive face, distinct from
    // generic raised document tabs. These are screenshot-calibrated values.
    if flat_selected_face && ui.visuals().dark_mode {
        (fill, stroke) = if selected {
            (fill, 93)
        } else if response.hovered() {
            ([68, 57], 81)
        } else {
            ([49, 38], 61)
        };
    }
    if selected && flat_selected_face {
        painter.rect_filled(rect, 0.0, p.panel);
    } else {
        painter.add(theme::gradient(
            rect,
            Color32::from_gray(fill[0]),
            Color32::from_gray(fill[1]),
        ));
    }
    // Flash tabs use a state-specific gray stroke; frontmost tabs leave their
    // lower edge open against the pane, instead of adding a focus-blue stripe.
    if flat_selected_face {
        let light = p.back.r() > 128;
        let top: &[u8] = match (light, selected) {
            (true, true) => &[109, 214, 212],
            (false, true) => &[38, 84, 76],
            (true, false) => &[104, 169, 156],
            (false, false) => &[35, 61, 53],
        };
        theme::edge_bands(&painter, rect, false, false, top);
        // The adjacent dock groove owns the left edge of the frontmost tab.
        if !selected {
            // The inactive tab owns its left junction; the selected neighbor
            // has no right frame, avoiding an extra boxed edge on Properties.
            theme::edge_bands(
                &painter,
                rect,
                true,
                false,
                if light { &[100, 182] } else { &[32, 66] },
            );
            theme::edge_bands(&painter, rect, true, true, &[if light { 100 } else { 32 }]);
            theme::edge_bands(&painter, rect, false, true, &[if light { 100 } else { 32 }]);
        }
    } else {
        let pixel = 1.0 / ui.painter().ctx().pixels_per_point();
        let edge = Stroke::new(pixel, Color32::from_gray(stroke));
        let outline = rect.shrink(pixel * 0.5);
        for side in [
            [outline.left_top(), outline.right_top()],
            [outline.left_top(), outline.left_bottom()],
            [outline.right_top(), outline.right_bottom()],
        ] {
            painter.line_segment(side, edge);
        }
        if !selected {
            painter.line_segment([outline.left_bottom(), outline.right_bottom()], edge);
        }
    }
    ui.painter().galley(
        rect.center() - text.size() / 2.0,
        text,
        // Unselected tabs remain enabled static text, not disabled/muted text.
        p.text,
    );
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::SelectableLabel, true, label));
    response
}

pub fn field(
    ui: &mut egui::Ui,
    value: &mut f32,
    french: bool,
    suffix: &str,
    width: f32,
) -> Response {
    let p = if ui.visuals().dark_mode {
        theme::Theme::Dark
    } else {
        theme::Theme::Light
    }
    .palette();
    let unit = suffix.trim();
    let unit_width = if unit.is_empty() { 0.0 } else { 22.0 };
    let response = ui
        .scope(|ui| {
            let visuals = ui.visuals_mut();
            visuals.override_text_color = Some(p.accent);
            for visuals in [
                &mut visuals.widgets.inactive,
                &mut visuals.widgets.hovered,
                &mut visuals.widgets.active,
                &mut visuals.widgets.open,
            ] {
                visuals.weak_bg_fill = Color32::TRANSPARENT;
                visuals.bg_fill = Color32::TRANSPARENT;
                visuals.bg_stroke.color = Color32::TRANSPARENT;
            }
            ui.style_mut().override_font_id = Some(FontId::proportional(theme::UI_TEXT_SIZE));
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                let response = ui.add_sized(
                    [width - unit_width, theme::CONTROL_HEIGHT],
                    egui::DragValue::new(value)
                        .speed(0.5)
                        .custom_formatter(move |n, _| {
                            let s = if n.fract().abs() < 0.001 {
                                format!("{n:.0}")
                            } else {
                                format!("{n:.1}")
                            };
                            if french {
                                s.replace('.', ",")
                            } else {
                                s
                            }
                        })
                        .custom_parser(|s| s.replace(',', ".").parse().ok()),
                );
                if !unit.is_empty() {
                    ui.add_sized(
                        [unit_width, theme::CONTROL_HEIGHT],
                        egui::Label::new(egui::RichText::new(unit).color(p.text)),
                    );
                }
                response
            })
            .inner
        })
        .inner;
    let pixel = 1.0 / ui.painter().ctx().pixels_per_point();
    let rect = response.rect;
    let line_y = rect.bottom() - 3.0 * pixel;
    let value_text = if value.fract().abs() < 0.001 {
        format!("{value:.0}")
    } else {
        format!("{value:.1}")
    };
    let text_width = ui
        .painter()
        .layout_no_wrap(
            value_text,
            FontId::proportional(theme::UI_TEXT_SIZE),
            p.accent,
        )
        .size()
        .x;
    let start = rect.center().x - text_width * 0.5;
    // Hot-text underline follows the number itself, not the entire input box.
    let mut x = start;
    while x < start + text_width {
        ui.painter().line_segment(
            [
                egui::pos2(x, line_y),
                egui::pos2((x + pixel).min(start + text_width), line_y),
            ],
            Stroke::new(pixel, p.accent),
        );
        x += 2.0 * pixel;
    }
    response
}

pub fn section(ui: &mut egui::Ui, text: &str, p: Palette) {
    ui.add_space(2.0);
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 20.0), Sense::hover());
    // Extend the band through the inspector's reserved content margin.
    let rect = Rect::from_min_max(rect.min - vec2(6.0, 0.0), rect.max + vec2(6.0, 0.0));
    theme::panel_bar(ui.painter(), rect, p);
    let caret_x = rect.left() + 9.0;
    let caret_y = rect.center().y;
    ui.painter().add(egui::Shape::convex_polygon(
        vec![
            egui::pos2(caret_x, caret_y - 3.0),
            egui::pos2(caret_x + 6.0, caret_y - 3.0),
            egui::pos2(caret_x + 3.0, caret_y + 2.0),
        ],
        p.text,
        Stroke::NONE,
    ));
    ui.painter().text(
        rect.left_center() + vec2(22.0, 0.0),
        egui::Align2::LEFT_CENTER,
        text,
        theme::medium(theme::UI_TEXT_SIZE),
        p.text,
    );
}

pub fn swatches(ui: &mut egui::Ui, fill: &mut Color32, p: Palette) {
    let colors = [
        0xffffff, 0xd4d4d8, 0x71717a, 0x27272a, 0x18181b, 0xf87171, 0xfb923c, 0xfbbf24, 0xa3e635,
        0x34d399, 0x22d3ee, 0x60a5fa, 0x818cf8, 0xa78bfa, 0xe879f9, 0xfb7185,
    ];
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
        for (index, hex) in colors.into_iter().enumerate() {
            let color = Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8);
            let (r, _) = ui.allocate_exact_size(vec2(23.0, 23.0), Sense::hover());
            let response = ui.interact(r, ui.id().with(("swatch", index)), Sense::click());
            ui.painter().rect_filled(r.shrink(2.0), 3.0, color);
            ui.painter().rect_stroke(
                r.shrink(2.0),
                3.0,
                Stroke::new(1.0, p.field_border),
                egui::StrokeKind::Inside,
            );
            if *fill == color || response.hovered() {
                ui.painter().rect_stroke(
                    r,
                    4.0,
                    Stroke::new(1.0, p.accent),
                    egui::StrokeKind::Inside,
                );
            }
            let label = format!("#{hex:06X}");
            response
                .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &label));
            if response.on_hover_text(label).clicked() {
                *fill = color;
            }
        }
    });
}

/// Ruler values derive from the stage transform, so zoom/pan never desynchronize them.
pub fn rulers(ui: &egui::Ui, area: Rect, stage: Rect, zoom: f32, p: Palette) {
    let painter = ui.painter_at(area);
    let top = Rect::from_min_size(area.min, vec2(area.width(), 20.0));
    let left = Rect::from_min_size(area.min, vec2(20.0, area.height()));
    painter.rect_filled(top, 0.0, p.tab);
    painter.rect_filled(left, 0.0, p.tab);
    let step = if zoom >= 2.0 {
        10.0
    } else if zoom >= 0.6 {
        50.0
    } else {
        200.0
    };
    for (horizontal, r, origin) in [(true, top, stage.left()), (false, left, stage.top())] {
        let (start, end) = if horizontal {
            (r.left() + 20.0, r.right())
        } else {
            (r.top() + 20.0, r.bottom())
        };
        let first = ((start - origin) / zoom / step).floor() as i32;
        let last = ((end - origin) / zoom / step).ceil() as i32;
        for n in first..=last {
            let value = n as f32 * step;
            let pos = origin + value * zoom;
            if pos < start || pos > end {
                continue;
            }
            let a = if horizontal {
                egui::pos2(pos, r.bottom())
            } else {
                egui::pos2(r.right(), pos)
            };
            painter.line_segment(
                [
                    a,
                    a - if horizontal {
                        vec2(0.0, 5.0)
                    } else {
                        vec2(5.0, 0.0)
                    },
                ],
                Stroke::new(1.0, p.muted),
            );
            let label = format!("{value:.0}");
            if horizontal {
                painter.text(
                    egui::pos2(pos + 3.0, r.top() + 7.0),
                    egui::Align2::LEFT_CENTER,
                    label,
                    FontId::monospace(9.0),
                    p.muted,
                );
            } else {
                painter.text(
                    egui::pos2(r.left() + 9.0, pos - 7.0),
                    egui::Align2::CENTER_CENTER,
                    label,
                    FontId::monospace(8.0),
                    p.muted,
                );
            }
        }
    }
    painter.rect_filled(Rect::from_min_size(area.min, vec2(20.0, 20.0)), 0.0, p.tab);
}

/// A restrained primary action shared by document dialogs.
pub fn primary(ui: &mut egui::Ui, label: String, p: Palette) -> Response {
    ui.add(
        egui::Button::new(egui::RichText::new(label).color(Color32::WHITE))
            .fill(if ui.visuals().dark_mode {
                Color32::from_rgb(38, 115, 200)
            } else {
                p.accent
            })
            .min_size(vec2(92.0, 30.0)),
    )
}

pub fn document_preview(
    ui: &mut egui::Ui,
    width: f32,
    height: f32,
    background: Color32,
    p: Palette,
) {
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 115.0), Sense::hover());
    ui.painter().rect_filled(r, 4.0, p.back);
    if width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0 {
        let scale = ((r.width() - 28.0) / width).min((r.height() - 28.0) / height);
        let paper = Rect::from_center_size(r.center(), vec2(width, height) * scale);
        ui.painter().rect_filled(
            paper.translate(vec2(0.0, 2.0)).expand(1.0),
            0.0,
            Color32::from_black_alpha(25),
        );
        ui.painter().rect_filled(paper, 0.0, background);
    }
}

/// Etched separators retain egui's allocation, with a dark line and lower/right bevel.
pub fn dock_separator(ui: &mut egui::Ui, p: Palette, side_margin: f32) -> Response {
    let pixel = 1.0 / ui.ctx().pixels_per_point();
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), 4.0 * pixel), Sense::hover());
    let rect = rect.expand2(vec2(side_margin, 0.0));
    theme::edge_bands(
        ui.painter(),
        rect,
        false,
        false,
        if p.back.r() > 128 {
            &[195, 107, 188, 216]
        } else {
            &[66, 36, 73, 80]
        },
    );
    response
}

/// Etched separators retain egui's allocation, with a dark line and lower/right bevel.
pub fn separator(ui: &mut egui::Ui) -> Response {
    let p = if ui.visuals().dark_mode {
        theme::Theme::Dark
    } else {
        theme::Theme::Light
    }
    .palette();
    let vertical = ui.layout().main_dir().is_horizontal();
    let response = ui
        .scope(|ui| {
            ui.visuals_mut().widgets.noninteractive.bg_stroke.color = Color32::TRANSPARENT;
            ui.separator()
        })
        .inner;
    let pixel = 1.0 / ui.ctx().pixels_per_point();
    let mut rect = response.rect;
    if vertical {
        rect.min.x = rect.center().x - pixel;
        rect.max.x = rect.min.x + 2.0 * pixel;
    } else {
        rect.min.y = rect.center().y - pixel;
        rect.max.y = rect.min.y + 2.0 * pixel;
    }
    theme::edge_bands(
        ui.painter(),
        rect,
        vertical,
        false,
        &[p.line.r(), p.divider_bevel.r()],
    );
    response
}
