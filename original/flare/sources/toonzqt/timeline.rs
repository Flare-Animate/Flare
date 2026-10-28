//! Timeline chrome shares the document editor's layer/frame authority.
use super::*;

#[derive(Clone, Copy, Default)]
struct TimelineScroll {
    offset: egui::Vec2,
    view: egui::Vec2,
    content: egui::Vec2,
}

/// Small fixed transport slots keep hover/focus independent from layout.
fn timeline_button(
    ui: &egui::Ui,
    rect: Rect,
    icon: Icon,
    label: &str,
    enabled: bool,
    selected: bool,
    p: theme::Palette,
) -> egui::Response {
    let response = ui.interact(
        rect,
        ui.id().with(label),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    if enabled && (response.hovered() || selected) {
        theme::bevel(
            ui.painter(),
            rect.shrink(0.8),
            p,
            response.hovered(),
            selected || response.is_pointer_button_down_on(),
        );
    }
    let icon_rect = Rect::from_center_size(rect.center(), vec2(12.0, 12.0));
    let color = if enabled {
        p.text
    } else {
        p.muted.gamma_multiply(0.4)
    };
    theme::paint_icon(
        ui,
        icon_rect.translate(vec2(0.0, 0.8)),
        icon,
        Color32::from_gray(if p.panel.r() < 128 { 39 } else { 239 }).gamma_multiply(if enabled {
            1.0
        } else {
            0.4
        }),
    );
    theme::paint_icon(ui, icon_rect, icon, color);
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect.shrink(0.8),
            0.0,
            Stroke::new(0.8, p.accent),
            egui::StrokeKind::Inside,
        );
    }
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label));
    response.on_hover_text(label)
}

fn paint_layer_page(ui: &egui::Ui, rect: Rect, p: theme::Palette) {
    let r = rect.shrink(1.2);
    let pixel = 1.0 / ui.ctx().pixels_per_point();
    let ink = if p.panel.r() < 128 {
        Color32::from_gray(233)
    } else {
        Color32::from_gray(61)
    };
    ui.painter().rect_filled(r, 0.0, ink.gamma_multiply(0.18));
    ui.painter()
        .rect_stroke(r, 0.0, Stroke::new(pixel, ink), egui::StrokeKind::Inside);
    let a = r.left_bottom() + vec2(0.0, -3.2);
    let b = r.left_bottom() + vec2(3.2, 0.0);
    ui.painter().add(egui::Shape::convex_polygon(
        vec![a, b, a + vec2(3.2, 0.0)],
        ink,
        Stroke::NONE,
    ));
}

fn paint_header_flag(ui: &egui::Ui, rect: Rect, lock: bool, p: theme::Palette) {
    let c = rect.center();
    let ink = if p.panel.r() < 128 {
        Color32::from_gray(179)
    } else {
        Color32::from_gray(76)
    };
    let highlight = if p.panel.r() < 128 {
        Color32::from_gray(97)
    } else {
        Color32::from_gray(237)
    };
    if lock {
        let body = Rect::from_center_size(c + vec2(0.0, 2.0), vec2(8.0, 7.2));
        ui.painter()
            .rect_filled(body.translate(vec2(0.0, 0.8)), 0.8, highlight);
        ui.painter().rect_filled(body, 0.8, ink);
        ui.painter().rect_stroke(
            Rect::from_center_size(c + vec2(0.0, -2.4), vec2(4.8, 7.2)),
            2.4,
            Stroke::new(1.6, ink),
            egui::StrokeKind::Inside,
        );
        ui.painter().rect_filled(
            Rect::from_center_size(c + vec2(0.0, 2.0), vec2(1.6, 3.2)),
            0.0,
            p.panel,
        );
    } else {
        for (offset, color) in [(vec2(0.0, 0.8), highlight), (vec2(0.0, 0.0), ink)] {
            ui.painter().add(egui::epaint::EllipseShape::filled(
                c + offset,
                vec2(6.0, 3.6),
                color,
            ));
        }
        ui.painter().circle_filled(c, 2.4, p.panel);
        ui.painter().circle_filled(c, 1.3, ink);
        ui.painter()
            .circle_filled(c + vec2(-0.8, -0.8), 0.8, highlight);
    }
}

fn paint_current_pencil(ui: &egui::Ui, rect: Rect, p: theme::Palette) {
    let a = rect.left_bottom() + vec2(2.0, -2.0);
    let b = rect.right_top() + vec2(-2.0, 2.0);
    ui.painter().line_segment([a, b], Stroke::new(3.2, p.text));
    ui.painter().line_segment(
        [a + vec2(0.8, -0.8), b + vec2(-1.6, 1.6)],
        Stroke::new(1.2, Color32::from_gray(205)),
    );
    ui.painter().add(egui::Shape::convex_polygon(
        vec![
            a + vec2(-1.6, 1.6),
            a + vec2(-0.8, -0.8),
            a + vec2(0.8, 0.8),
        ],
        p.text,
        Stroke::NONE,
    ));
}

/// Dedicated rails control the same ScrollArea offset as wheel and drag scrolling.
fn timeline_scrollbar(
    ui: &egui::Ui,
    rect: Rect,
    vertical: bool,
    offset: &mut f32,
    view: f32,
    content: f32,
    step: f32,
    p: theme::Palette,
    i18n: &Localizer,
) {
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }
    let breadth = if vertical {
        rect.width()
    } else {
        rect.height()
    };
    let before = Rect::from_min_size(rect.min, vec2(breadth, breadth));
    let after = Rect::from_min_size(rect.max - vec2(breadth, breadth), vec2(breadth, breadth));
    let track = if vertical {
        Rect::from_min_max(before.left_bottom(), after.right_top())
    } else {
        Rect::from_min_max(before.right_top(), after.left_bottom())
    };
    let max_offset = (content - view).max(0.0);
    *offset = offset.clamp(0.0, max_offset);
    let coord = |v: egui::Pos2| if vertical { v.y } else { v.x };
    let length = if vertical {
        track.height()
    } else {
        track.width()
    };
    if length <= 0.0 {
        return;
    }
    theme::scroll_groove(ui.painter(), track, vertical, p);
    let thumb_len =
        (length * (view / content.max(view).max(1.0))).clamp(12.0_f32.min(length), length);
    let travel = (length - thumb_len).max(0.0);
    let start = coord(track.min)
        + if max_offset > 0.0 {
            *offset / max_offset * travel
        } else {
            0.0
        };
    let thumb = if vertical {
        Rect::from_min_size(pos2(track.left(), start), vec2(track.width(), thumb_len))
    } else {
        Rect::from_min_size(pos2(start, track.top()), vec2(thumb_len, track.height()))
    };
    let id = ui.id().with(("timeline.rail", vertical));
    let response = ui.interact(track, id, Sense::click_and_drag());
    let label = i18n.text(if vertical {
        "timeline-scroll-vertical"
    } else {
        "timeline-scroll-horizontal"
    });
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Slider, max_offset > 0.0, &label)
    });
    for (r, negative, key) in [
        (
            before,
            true,
            if vertical {
                "timeline-scroll-up"
            } else {
                "timeline-scroll-left"
            },
        ),
        (
            after,
            false,
            if vertical {
                "timeline-scroll-down"
            } else {
                "timeline-scroll-right"
            },
        ),
    ] {
        let response = ui.interact(r, id.with(("arrow", negative)), Sense::click());
        theme::scroll_arrow_face(
            ui.painter(),
            r,
            p,
            response.hovered(),
            response.is_pointer_button_down_on(),
        );
        let c = r.center();
        let sign = if negative { -1.0 } else { 1.0 };
        let points = if vertical {
            vec![
                c + vec2(0.0, sign * 2.4),
                c + vec2(-2.4, -sign * 1.6),
                c + vec2(2.4, -sign * 1.6),
            ]
        } else {
            vec![
                c + vec2(sign * 2.4, 0.0),
                c + vec2(-sign * 1.6, -2.4),
                c + vec2(-sign * 1.6, 2.4),
            ]
        };
        ui.painter().add(egui::Shape::convex_polygon(
            points,
            p.text
                .gamma_multiply(if max_offset > 0.0 { 1.0 } else { 0.45 }),
            Stroke::NONE,
        ));
        let tip = i18n.text(key);
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, max_offset > 0.0, &tip)
        });
        if response.on_hover_text(tip).clicked() {
            *offset = (*offset + sign * step).clamp(0.0, max_offset);
            ui.ctx().request_repaint();
        }
    }
    if response.drag_started() {
        let anchor = ui.input(|i| i.pointer.press_origin()).map_or(0.0, |pos| {
            if thumb.contains(pos) {
                coord(pos) - coord(thumb.center())
            } else {
                0.0
            }
        });
        ui.ctx()
            .data_mut(|d| d.insert_temp(id.with("grab"), anchor));
    }
    if (response.dragged() || response.clicked()) && travel > 0.0 {
        if let Some(pos) = response.interact_pointer_pos() {
            let anchor = if response.dragged() {
                ui.ctx()
                    .data_mut(|d| d.get_temp::<f32>(id.with("grab")))
                    .unwrap_or(0.0)
            } else {
                0.0
            };
            *offset = ((coord(pos) - coord(track.min) - thumb_len * 0.5 - anchor) / travel)
                .clamp(0.0, 1.0)
                * max_offset;
            ui.ctx().request_repaint();
        }
    }
    if max_offset > 0.0 {
        theme::scroll_thumb(
            ui.painter(),
            thumb,
            p,
            response.hovered(),
            response.dragged(),
        );
    }
    response.on_hover_text(label);
}

impl Studio {
    pub(super) fn select_timeline_cell(&mut self, layer: usize, frame: u32) {
        self.timeline_state.ranges.clear();
        self.clear_layer_selection();
        self.editor.layer = layer.min(self.editor.timeline().layers.len().saturating_sub(1));
        self.editor.frame = frame.min(document::MAX_FRAMES - 1);
        self.editor.frame_sel = None;
        self.editor.selection.clear();
        self.timeline_state.anchor = Some((self.editor.layer, self.editor.frame));
        self.timeline_state.focused = true;
        self.playing = false;
        self.sync_forms();
    }
    fn timeline_footer(
        &mut self,
        ui: &mut egui::Ui,
        p: theme::Palette,
        label_width: f32,
        scroll: &mut TimelineScroll,
    ) {
        egui::Panel::bottom("studio.transport")
            .show_separator_line(false)
            .exact_size(22.4)
            .frame(egui::Frame::NONE.fill(p.panel))
            .show(ui, |ui| {
                let r = ui.max_rect();
                theme::edge_bands(
                    ui.painter(),
                    r,
                    false,
                    false,
                    if p.panel.r() < 128 {
                        &[62, 97, 80]
                    } else {
                        &[196, 170, 194]
                    },
                );
                let slot = |x: f32| {
                    Rect::from_center_size(
                        pos2(r.left() + x + 9.0, r.center().y + 1.2),
                        vec2(18.0, 18.0),
                    )
                };
                for (x, icon, command) in [
                    (4.0, Icon::File, Command::AddLayer),
                    (24.0, Icon::Layers, Command::AddFolder),
                    (44.0, Icon::Trash, Command::DeleteLayer),
                ] {
                    let label = self.i18n.text(if command == Command::DeleteLayer {
                        "action-trash-layers"
                    } else if command == Command::AddLayer {
                        "action-add-layer"
                    } else {
                        command.label()
                    });
                    let button =
                        timeline_button(ui, slot(x), icon, &label, self.enabled(command), false, p);
                    if command == Command::AddFolder {
                        let folder = slot(x).shrink(4.4);
                        ui.painter().rect_filled(folder.expand(0.8), 0.0, p.panel);
                        ui.painter()
                            .rect_filled(folder, 0.0, p.text.gamma_multiply(0.7));
                        ui.painter().rect_filled(
                            Rect::from_min_size(folder.min + vec2(0.0, -1.6), vec2(4.8, 2.4)),
                            0.0,
                            p.text.gamma_multiply(0.7),
                        );
                    }
                    let drop = command == Command::DeleteLayer
                        && self.timeline_state.layer_drag.is_some()
                        && ui.input(|i| {
                            i.pointer.button_released(egui::PointerButton::Primary)
                                && i.pointer
                                    .interact_pos()
                                    .is_some_and(|pos| button.rect.contains(pos))
                        });
                    if button.clicked() || drop {
                        self.timeline_state.layer_drag = None;
                        self.dispatch(command, ui.ctx());
                    }
                }
                let split = r.left() + label_width;
                for i in 0..4 {
                    let x = split - 8.0 + i as f32 * 1.6;
                    ui.painter().rect_filled(
                        Rect::from_min_size(pos2(x, r.center().y - 4.0), vec2(0.8, 9.6)),
                        0.0,
                        p.muted,
                    );
                }
                theme::edge_bands(
                    ui.painter(),
                    Rect::from_min_size(
                        pos2(split - 0.8, r.top() + 3.2),
                        vec2(1.6, r.height() - 4.8),
                    ),
                    true,
                    false,
                    if p.panel.r() < 128 {
                        &[44, 92]
                    } else {
                        &[158, 229]
                    },
                );
                let last = self.editor.timeline().frame_count().saturating_sub(1);
                for (i, icon, key, target) in [
                    (0, Icon::First, "action-first", 0),
                    (
                        1,
                        Icon::Previous,
                        "action-previous",
                        self.editor.frame.saturating_sub(1),
                    ),
                    (
                        3,
                        Icon::Next,
                        "action-next",
                        (self.editor.frame + 1).min(last),
                    ),
                    (4, Icon::Last, "action-last", last),
                ] {
                    if timeline_button(
                        ui,
                        slot(label_width + 2.4 + i as f32 * 18.0),
                        icon,
                        &self.i18n.text(key),
                        true,
                        false,
                        p,
                    )
                    .clicked()
                    {
                        self.editor.frame = target;
                        self.playing = false;
                        self.sync_forms();
                    }
                }
                if timeline_button(
                    ui,
                    slot(label_width + 38.4),
                    if self.playing {
                        Icon::Pause
                    } else {
                        Icon::Play
                    },
                    &self.i18n.text(if self.playing {
                        "action-pause"
                    } else {
                        "action-play"
                    }),
                    true,
                    self.playing,
                    p,
                )
                .clicked()
                {
                    self.dispatch(Command::Play, ui.ctx());
                }
                let wide = r.width() >= 900.0;
                for (i, icon, command) in [
                    (0, Icon::Plus, Command::InsertFrame),
                    (1, Icon::Key, Command::Keyframe),
                    (2, Icon::File, Command::BlankKeyframe),
                    (3, Icon::Layers, Command::CopyFrames),
                    (4, Icon::File, Command::PasteFrames),
                    (5, Icon::Close, Command::RemoveFrame),
                ] {
                    if !wide && i > 1 {
                        continue;
                    }
                    if timeline_button(
                        ui,
                        slot(label_width + 100.0 + i as f32 * 18.0),
                        icon,
                        &self.i18n.text(if command == Command::Keyframe {
                            "action-insert-keyframe"
                        } else {
                            command.label()
                        }),
                        self.enabled(command),
                        false,
                        p,
                    )
                    .clicked()
                    {
                        self.dispatch(command, ui.ctx());
                    }
                }
                let display = timeline_button(
                    ui,
                    slot(label_width + if wide { 212.0 } else { 136.0 }),
                    Icon::Monitor,
                    &self.i18n.text("field-frame-display"),
                    true,
                    false,
                    p,
                );
                egui::Popup::menu(&display).show(|ui| {
                    for (mode, key) in [
                        (FrameDisplay::Cells, "frames-cells"),
                        (FrameDisplay::Preview, "frames-preview"),
                        (FrameDisplay::Context, "frames-context"),
                    ] {
                        if widgets::selectable(
                            ui,
                            self.prefs.frame_display == mode,
                            self.i18n.text(key),
                        )
                        .clicked()
                        {
                            self.prefs.frame_display = mode;
                            self.reset_timeline_gesture();
                            ui.close();
                        }
                    }
                });
                let counter_x = r.left() + label_width + 244.0;
                let text_y = r.center().y + 0.8;
                let number = |painter: &egui::Painter, x: f32, text: String, color: Color32| {
                    painter.text(
                        pos2(x, text_y),
                        egui::Align2::LEFT_CENTER,
                        text,
                        FontId::proportional(10.0),
                        color,
                    );
                };
                if wide {
                    number(
                        ui.painter(),
                        counter_x,
                        (self.editor.frame + 1).to_string(),
                        p.accent,
                    );
                    number(
                        ui.painter(),
                        counter_x + 30.0,
                        self.i18n.format(
                            "frame-rate",
                            &[("rate", &self.i18n.number(self.editor.doc.fps, 2))],
                        ),
                        p.muted,
                    );
                    let time = self.i18n.format(
                        "timeline-elapsed-time",
                        &[(
                            "time",
                            &self.i18n.number(
                                self.editor.frame as f32 / self.editor.doc.fps.max(0.01),
                                1,
                            ),
                        )],
                    );
                    number(ui.painter(), counter_x + 83.0, time, p.accent);
                }
                let rail_left = (r.left() + label_width + if wide { 355.2 } else { 160.0 })
                    .min(r.right() - 200.0);
                let rail_right = (r.right() - 181.6).max(rail_left + 18.0);
                let hscroll = Rect::from_min_max(
                    pos2(rail_left, r.center().y - 4.8),
                    pos2(rail_right, r.center().y + 7.2),
                );
                timeline_scrollbar(
                    ui,
                    hscroll,
                    false,
                    &mut scroll.offset.x,
                    (scroll.view.x - label_width).max(1.0),
                    (scroll.content.x - label_width).max(1.0),
                    self.prefs.frame_width * 5.0,
                    p,
                    &self.i18n,
                );
                let center_slot = Rect::from_center_size(
                    pos2(rail_right + 18.0, r.center().y + 1.2),
                    vec2(18.0, 18.0),
                );
                if timeline_button(
                    ui,
                    center_slot,
                    Icon::Undo,
                    &self.i18n.text("action-center-playhead"),
                    true,
                    false,
                    p,
                )
                .clicked()
                {
                    let cell = if self.prefs.frame_display == FrameDisplay::Cells {
                        self.prefs.frame_width
                    } else {
                        72.0
                    };
                    scroll.offset.x = ((self.editor.frame as f32 + 0.5) * cell
                        - (scroll.view.x - label_width) * 0.5)
                        .clamp(0.0, (scroll.content.x - scroll.view.x).max(0.0));
                }
                let zoom = Rect::from_min_max(
                    pos2(rail_right + 51.2, r.center().y - 4.0),
                    pos2(r.right() - 25.6, r.center().y + 5.6),
                );
                let zoom_response = ui.interact(
                    zoom.expand2(vec2(0.0, 3.2)),
                    ui.id().with("timeline.zoom"),
                    Sense::click_and_drag(),
                );
                let label = self.i18n.text("field-frame-size");
                zoom_response.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Slider, true, &label)
                });
                if let Some(pos) = zoom_response.interact_pointer_pos() {
                    if zoom_response.clicked() || zoom_response.dragged() {
                        self.prefs.frame_width =
                            6.0 + ((pos.x - zoom.left()) / zoom.width()).clamp(0.0, 1.0) * 14.0;
                    }
                }
                let groove = Rect::from_center_size(zoom.center(), vec2(zoom.width(), 4.0));
                theme::scroll_groove(ui.painter(), groove, false, p);
                let thumb_x = zoom.left() + (self.prefs.frame_width - 6.0) / 14.0 * zoom.width();
                let thumb =
                    Rect::from_center_size(pos2(thumb_x, zoom.center().y - 1.6), vec2(6.4, 9.6));
                theme::scroll_thumb(
                    ui.painter(),
                    thumb,
                    p,
                    zoom_response.hovered(),
                    zoom_response.dragged(),
                );
                for (x, scale) in [(zoom.left() - 16.0, 0.65), (zoom.right() + 14.0, 1.0)] {
                    let c = pos2(x, zoom.center().y + 0.8);
                    ui.painter().add(egui::Shape::convex_polygon(
                        vec![
                            c + vec2(-6.4, 2.4) * scale,
                            c + vec2(-2.4, -1.6) * scale,
                            c + vec2(0.0, 0.8) * scale,
                            c + vec2(2.4, -3.2) * scale,
                            c + vec2(6.4, 2.4) * scale,
                        ],
                        p.muted,
                        Stroke::NONE,
                    ));
                }
                zoom_response.on_hover_text(label);
            });
    }
    pub(super) fn timeline(&mut self, ui: &mut egui::Ui) {
        if !self.prefs.timeline {
            return;
        }
        let p = self.prefs.theme.palette();
        let panel = if self.prefs.timeline_top {
            egui::Panel::top("studio.timeline")
        } else {
            egui::Panel::bottom("studio.timeline")
        };
        panel
            .show_separator_line(false)
            .resizable(true)
            .default_size(200.0)
            .min_size(150.0)
            .max_size(420.0)
            .frame(egui::Frame::NONE.fill(p.timeline))
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                let cap = Rect::from_min_size(ui.cursor().min, vec2(ui.available_width(), 1.6));
                theme::edge_bands(
                    ui.painter(),
                    cap,
                    false,
                    false,
                    if p.panel.r() < 128 {
                        &[34, 54]
                    } else {
                        &[98, 153]
                    },
                );
                ui.add_space(1.6);
                widgets::panel_tab_strip(ui, p, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        if widgets::panel_tab(
                            ui,
                            "timeline",
                            &self.i18n.text("panel-timeline"),
                            !self.output_tab,
                            p,
                        )
                        .clicked()
                        {
                            self.output_tab = false;
                        }
                        if widgets::panel_tab(
                            ui,
                            "output",
                            &self.i18n.text("panel-output"),
                            self.output_tab,
                            p,
                        )
                        .clicked()
                        {
                            self.output_tab = true;
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let menu =
                                widgets::panel_menu(ui, p, &self.i18n.text("timeline-panel-menu"));
                            egui::Popup::menu(&menu).show(|ui| {
                                self.frame_display_menu(ui);
                                for command in [
                                    Command::AddLayer,
                                    Command::AddFolder,
                                    Command::Keyframe,
                                    Command::BlankKeyframe,
                                    Command::InsertFrame,
                                    Command::RemoveFrame,
                                ] {
                                    self.menu_command(ui, command);
                                }
                            });
                            self.workspace_grip(
                                ui,
                                workspace::DockModule::Timeline,
                                vec2(18.0, theme::PANEL_TAB_HEIGHT),
                            );
                        });
                    });
                });
                if self.output_tab {
                    ui.add_space(8.0);
                    ui.label(if self.status.is_empty() {
                        self.i18n.text("status-ready")
                    } else {
                        self.status.clone()
                    });
                    return;
                }
                let scroll_id = egui::Id::new("studio.timeline.scroll.metrics");
                let mut scroll = ui
                    .ctx()
                    .data_mut(|d| d.get_temp::<TimelineScroll>(scroll_id))
                    .unwrap_or_default();
                let label_width = if self.prefs.locale == Locale::Expanded {
                    230.0
                } else {
                    176.0
                };
                self.timeline_footer(ui, p, label_width, &mut scroll);
                let layers = self.editor.timeline().layers.clone();
                let rows: Vec<usize> = (0..layers.len())
                    .filter(|&i| !self.editor.layer_collapsed_away(i))
                    .collect();
                // Keep unoccupied cells available to extend an animation past its current end.
                let frames = self
                    .editor
                    .timeline()
                    .frame_count()
                    .saturating_add(600)
                    .clamp(120, document::MAX_FRAMES);
                let previews = self.prefs.frame_display != FrameDisplay::Cells;
                let cell = if previews {
                    72.0
                } else {
                    self.prefs.frame_width.clamp(6.0, 20.0)
                };
                let frames = frames
                    .max(((ui.available_width() - label_width) / cell).ceil() as u32 + 1)
                    .min(document::MAX_FRAMES);
                let ruler_height = 24.0;
                let row_height = 20.0;
                let mut row_offsets = vec![ruler_height];
                for &i in &rows {
                    row_offsets.push(
                        row_offsets.last().unwrap()
                            + (row_height * layers[i].row_height as f32 / 100.0).max(if previews {
                                56.0
                            } else {
                                row_height
                            }),
                    );
                }
                let viewport = ui.available_rect_before_wrap();
                let rail_width = 13.6;
                let vscroll = Rect::from_min_max(
                    pos2(viewport.right() - rail_width, viewport.top() + ruler_height),
                    viewport.max,
                );
                let mut grid_ui = ui.new_child(egui::UiBuilder::new().max_rect(
                    Rect::from_min_max(viewport.min, pos2(vscroll.left(), viewport.bottom())),
                ));
                let output = egui::ScrollArea::both()
                    .id_salt("studio.timeline.scroll")
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                    .scroll_offset(scroll.offset)
                    .auto_shrink([false, false])
                    .show(&mut grid_ui, |ui| {
                        let (rect, _) = ui.allocate_exact_size(
                            vec2(
                                label_width + frames as f32 * cell,
                                *row_offsets.last().unwrap(),
                            ),
                            Sense::hover(),
                        );
                        let visible = ui.clip_rect();
                        self.timeline_state.viewport = Some(visible);
                        let label_x = rect.left().max(visible.left());
                        let grid_clip = Rect::from_min_max(
                            pos2(label_x + label_width, visible.top()),
                            visible.max,
                        );
                        let ruler_painter = ui.painter_at(grid_clip);
                        ruler_painter.rect_filled(
                            Rect::from_min_size(
                                grid_clip.min,
                                vec2(grid_clip.width(), ruler_height),
                            ),
                            0.0,
                            p.panel,
                        );
                        let body_clip = Rect::from_min_max(
                            pos2(visible.left(), visible.top() + ruler_height),
                            visible.max,
                        );
                        let painter = ui.painter_at(Rect::from_min_max(
                            pos2(grid_clip.left(), visible.top() + ruler_height),
                            grid_clip.max,
                        ));
                        let first = (((grid_clip.left() - rect.left() - label_width) / cell)
                            .floor()
                            .max(0.0) as u32)
                            .min(frames);
                        let end = (((visible.right() - rect.left() - label_width) / cell)
                            .ceil()
                            .max(0.0) as u32)
                            .min(frames);
                        for frame in first..end {
                            let x = rect.left() + label_width + frame as f32 * cell;
                            let ruler = Rect::from_min_size(
                                pos2(x, visible.top()),
                                vec2(cell, ruler_height),
                            )
                            .intersect(grid_clip);
                            let response = ui.interact(
                                ruler,
                                ui.id().with(("ruler", frame)),
                                Sense::click_and_drag(),
                            );
                            if response.clicked() || response.dragged() {
                                if let Some(pos) = response.interact_pointer_pos() {
                                    let frame = (((pos.x - rect.left() - label_width) / cell)
                                        .floor()
                                        .max(0.0)
                                        as u32)
                                        .min(frames - 1);
                                    self.select_timeline_cell(self.editor.layer, frame);
                                }
                            }
                            let major = frame == 0 || (frame + 1) % 5 == 0;
                            if major {
                                ruler_painter.text(
                                    pos2(x + cell / 2.0, visible.top() + 10.4),
                                    egui::Align2::CENTER_CENTER,
                                    (frame + 1).to_string(),
                                    theme::medium(10.4),
                                    p.text,
                                );
                            }
                            let pixel = 1.0 / ui.ctx().pixels_per_point();
                            let tick_x = ((x + cell / 2.0) / pixel).floor() * pixel;
                            ruler_painter.rect_filled(
                                Rect::from_min_size(
                                    pos2(
                                        tick_x,
                                        visible.top() + ruler_height
                                            - if major { 4.0 } else { 2.4 },
                                    ),
                                    vec2(pixel, if major { 4.0 } else { 2.4 }),
                                ),
                                0.0,
                                Color32::from_gray(if p.panel.r() < 128 { 127 } else { 111 }),
                            );
                        }
                        for (row_index, &index) in rows.iter().enumerate() {
                            let layer = &layers[index];
                            let row_height = row_offsets[row_index + 1] - row_offsets[row_index];
                            let y = rect.top() + row_offsets[row_index];
                            let row = Rect::from_min_size(
                                pos2(rect.left(), y),
                                vec2(rect.width(), row_height),
                            );
                            if !row.intersects(body_clip) {
                                continue;
                            }
                            let selected = self.layer_row_selected(index);
                            let dark = p.panel.r() < 128;
                            painter.rect_filled(row, 0.0, p.panel);
                            for frame in first..end {
                                let r = Rect::from_min_size(
                                    pos2(rect.left() + label_width + frame as f32 * cell, y),
                                    vec2(cell, row_height),
                                );
                                let active = frame < layer.end;
                                let tween = layer.span_at(frame).is_some_and(|(_, k, _)| {
                                    matches!(k.tween, Tween::Classic { .. })
                                });
                                painter.rect_filled(
                                    r,
                                    0.0,
                                    if self.frame_selected(index, frame) {
                                        p.select
                                    } else if tween && active {
                                        p.tween
                                    } else if active {
                                        p.frame
                                    } else {
                                        Color32::from_gray(match (dark, (frame + 1) % 5 == 0) {
                                            (false, false) => 215,
                                            (false, true) => 206,
                                            (true, false) => 74,
                                            (true, true) => 69,
                                        })
                                    },
                                );
                                theme::edge_bands(
                                    &painter,
                                    r,
                                    true,
                                    false,
                                    if dark {
                                        &[68, 80, 80]
                                    } else {
                                        &[200, 222, 223]
                                    },
                                );
                                theme::edge_bands(
                                    &painter,
                                    r,
                                    false,
                                    false,
                                    if dark {
                                        &[100, 85, 77]
                                    } else {
                                        &[174, 225, 220]
                                    },
                                );
                                theme::edge_bands(
                                    &painter,
                                    r,
                                    false,
                                    true,
                                    if dark {
                                        &[75, 75, 69]
                                    } else {
                                        &[222, 215, 203]
                                    },
                                );
                                if previews && active {
                                    if let Some(texture) =
                                        self.frame_thumbnail(ui.ctx(), index, frame)
                                    {
                                        let image =
                                            Rect::from_center_size(r.center(), vec2(64.0, 44.0));
                                        painter.image(
                                            texture,
                                            image,
                                            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                                            Color32::WHITE,
                                        );
                                    }
                                }
                                if let Some(k) = layer.keyframes.get(&frame) {
                                    if k.elements.is_empty() {
                                        painter.circle_stroke(
                                            pos2(r.center().x, r.bottom() - 5.6),
                                            1.8,
                                            Stroke::new(
                                                1.0 / ui.ctx().pixels_per_point(),
                                                Color32::from_gray(38),
                                            ),
                                        );
                                    } else {
                                        painter.circle_filled(
                                            pos2(r.center().x, r.bottom() - 5.6),
                                            1.8,
                                            Color32::from_gray(38),
                                        );
                                    }
                                }
                                let response = ui.interact(
                                    r.intersect(painter.clip_rect()),
                                    ui.id().with(("cell", index, frame)),
                                    Sense::click_and_drag(),
                                );
                                response.widget_info(|| {
                                    egui::WidgetInfo::labeled(
                                        egui::WidgetType::Button,
                                        true,
                                        self.i18n.format(
                                            "timeline-cell",
                                            &[
                                                ("layer", &layer.name),
                                                ("frame", &(frame + 1).to_string()),
                                            ],
                                        ),
                                    )
                                });
                                if response.double_clicked() {
                                    self.clear_layer_selection();
                                    self.timeline_state.ranges.clear();
                                    self.editor.select_span(index, frame);
                                    self.timeline_state.focused = true;
                                    self.timeline_state.anchor = Some((index, self.editor.frame));
                                    self.playing = false;
                                } else if response.clicked() {
                                    let mods = ui.input(|i| i.modifiers);
                                    if mods.command || mods.ctrl {
                                        self.toggle_frame_cell(index, frame);
                                    } else if mods.shift && self.prefs.span_selection {
                                        self.add_frame_span(index, frame);
                                    } else if mods.shift {
                                        let anchor = self
                                            .timeline_state
                                            .anchor
                                            .unwrap_or((self.editor.layer, self.editor.frame));
                                        self.select_frame_range(anchor, (index, frame));
                                    } else {
                                        self.select_timeline_cell(index, frame);
                                        if self.prefs.span_selection {
                                            self.editor.select_span(index, frame);
                                        }
                                    }
                                }
                                if response.secondary_clicked() {
                                    if !self.frame_selected(index, frame) {
                                        self.select_timeline_cell(index, frame);
                                    }
                                    self.timeline_state.focused = true;
                                }
                                if response.drag_started_by(egui::PointerButton::Primary) {
                                    let mut source = self.editor.frame_target();
                                    let mut resize_left = None;
                                    if ui.input(|i| i.modifiers.ctrl || i.modifiers.command) {
                                        if let Some((start, key, _)) = layer.span_at(frame) {
                                            let last = layer.span_end(start) - 1;
                                            if key.tween == Tween::None
                                                && key.motion.is_none()
                                                && (frame == start || frame == last)
                                            {
                                                resize_left = Some(frame == start && start != last);
                                                source = ac_core::FrameSel {
                                                    layer0: index,
                                                    layer1: index,
                                                    f0: start,
                                                    f1: last,
                                                };
                                            }
                                        }
                                    }
                                    self.timeline_state.gesture =
                                        Some(timeline_ops::FrameGesture {
                                            source,
                                            origin: (index, frame),
                                            target: (index, frame),
                                            moving: self.editor.frame_sel.is_some()
                                                && self.frame_selected(index, frame)
                                                && !ui.input(|i| i.modifiers.shift),
                                            copy: ui.input(|i| i.modifiers.alt),
                                            resize_left,
                                        });
                                    self.timeline_state.focused = true;
                                    self.playing = false;
                                }
                                response.context_menu(|ui| {
                                    for command in [
                                        Command::InsertFrame,
                                        Command::Keyframe,
                                        Command::BlankKeyframe,
                                        Command::RemoveFrame,
                                        Command::ClearKeyframe,
                                        Command::Tween,
                                        Command::CopyFrames,
                                        Command::CutFrames,
                                        Command::PasteFrames,
                                        Command::PasteOverwriteFrames,
                                        Command::ClearFrames,
                                        Command::ReverseFrames,
                                        Command::FrameByFrame,
                                    ] {
                                        self.menu_command(ui, command);
                                    }
                                });
                            }
                            if let Some((start, key, _)) = layer.span_at(self.editor.frame) {
                                if let Some(label) = &key.label {
                                    painter.text(
                                        pos2(
                                            rect.left() + label_width + start as f32 * cell + 5.0,
                                            y + 3.0,
                                        ),
                                        egui::Align2::LEFT_TOP,
                                        label,
                                        theme::medium(10.0),
                                        p.accent,
                                    );
                                }
                            }
                            if self.timeline_editable(index) {
                                let x = rect.left() + label_width + layer.end as f32 * cell;
                                let handle = Rect::from_min_size(
                                    pos2(x - 3.0, y + 2.0),
                                    vec2(6.0, row_height - 4.0),
                                )
                                .intersect(grid_clip);
                                let response = ui
                                    .interact(
                                        handle,
                                        ui.id().with(("duration", index)),
                                        Sense::drag(),
                                    )
                                    .on_hover_cursor(egui::CursorIcon::ResizeHorizontal)
                                    .on_hover_text(self.i18n.text("timeline-duration-hint"));
                                painter.line_segment(
                                    [pos2(x, y + 3.0), pos2(x, y + row_height - 3.0)],
                                    Stroke::new(1.5, p.muted),
                                );
                                if response.drag_started() {
                                    self.timeline_state.gesture = None;
                                    self.timeline_state.duration_drag = Some((index, layer.end));
                                }
                            }
                            let name_area = Rect::from_min_size(
                                pos2(label_x, y),
                                vec2(label_width, row_height),
                            );
                            ui.painter().rect_filled(
                                name_area,
                                0.0,
                                if selected { p.select } else { p.panel },
                            );
                            paint_layer_page(
                                ui,
                                Rect::from_min_size(
                                    name_area.min + vec2(25.0, (row_height - 12.0) * 0.5),
                                    vec2(12.0, 12.0),
                                ),
                                p,
                            );
                            let indent = self.editor.layer_depth(index) as f32 * 12.0;
                            let name_rect = Rect::from_min_size(
                                name_area.min + vec2(42.0 + indent, 0.0),
                                vec2((label_width - 111.0 - indent).max(20.0), row_height),
                            )
                            .intersect(body_clip);
                            if !self.layer_name_editor(ui, index, name_rect) {
                                ui.painter().with_clip_rect(name_rect).text(
                                    name_rect.left_center(),
                                    egui::Align2::LEFT_CENTER,
                                    &layer.name,
                                    theme::medium(11.0),
                                    p.text,
                                );
                                let response = ui.interact(
                                    name_rect,
                                    ui.id().with(("layer", index)),
                                    Sense::click_and_drag(),
                                );
                                response.widget_info(|| {
                                    egui::WidgetInfo::labeled(
                                        egui::WidgetType::SelectableLabel,
                                        true,
                                        &layer.name,
                                    )
                                });
                                if response.double_clicked() {
                                    self.begin_layer_rename(index);
                                } else if response.clicked() {
                                    self.select_layer_rows(
                                        index,
                                        ui.input(|i| i.modifiers.shift),
                                        ui.input(|i| i.modifiers.command || i.modifiers.ctrl),
                                    );
                                }
                                if response.secondary_clicked() {
                                    if !self.layer_selection_active()
                                        || !self.layer_row_selected(index)
                                    {
                                        self.select_layer_rows(index, false, false);
                                    }
                                }
                                if response.drag_started() {
                                    if !self.layer_selection_active()
                                        || !self.layer_row_selected(index)
                                    {
                                        self.select_layer_rows(index, false, false);
                                    }
                                    self.timeline_state.layer_drag = Some(index);
                                }
                                response.context_menu(|ui| {
                                    for command in [
                                        Command::AddLayer,
                                        Command::AddFolder,
                                        Command::CopyLayers,
                                        Command::CutLayers,
                                        Command::PasteLayers,
                                        Command::SelectAllLayers,
                                        Command::DuplicateLayer,
                                        Command::DeleteLayer,
                                        Command::LayerUp,
                                        Command::LayerDown,
                                        Command::IndentLayer,
                                        Command::OutdentLayer,
                                        Command::ExpandFolders,
                                        Command::CollapseFolders,
                                        Command::ShowAllLayers,
                                        Command::HideAllLayers,
                                        Command::LockAllLayers,
                                        Command::UnlockAllLayers,
                                    ] {
                                        self.menu_command(ui, command);
                                    }
                                });
                            }
                            if layer.kind == ac_core::LayerKind::Folder {
                                let r = Rect::from_min_size(
                                    name_area.min + vec2(8.0, (row_height - 16.0) * 0.5),
                                    vec2(20.0, 16.0),
                                );
                                ui.painter().rect_filled(
                                    r,
                                    0.0,
                                    if selected { p.select } else { p.panel },
                                );
                                ui.painter().text(
                                    r.center(),
                                    egui::Align2::CENTER_CENTER,
                                    if layer.collapsed { ">" } else { "v" },
                                    theme::medium(12.0),
                                    p.text,
                                );
                                let response = ui
                                    .interact(r, ui.id().with(("folder", index)), Sense::click())
                                    .on_hover_text(self.i18n.text("action-toggle-folder"));
                                if response.clicked() {
                                    self.editor.begin_edit("layer.collapse");
                                    self.editor.toggle_folder(index);
                                }
                            }
                            self.outline_control(
                                ui,
                                Rect::from_min_size(
                                    pos2(
                                        label_x + label_width - 17.6,
                                        y + (row_height - 17.0) * 0.5,
                                    ),
                                    vec2(11.2, 17.0),
                                ),
                                Some(index),
                            );
                            for (offset, flag, key, value, icon) in [
                                (
                                    56.0,
                                    ac_core::LayerFlag::Visible,
                                    "action-show-layer",
                                    layer.visible,
                                    if layer.visible {
                                        Icon::Eye
                                    } else {
                                        Icon::EyeOff
                                    },
                                ),
                                (
                                    36.0,
                                    ac_core::LayerFlag::Locked,
                                    "action-lock-layer",
                                    layer.locked,
                                    Icon::Lock,
                                ),
                            ] {
                                let lock = matches!(flag, ac_core::LayerFlag::Locked);
                                let painted_value = self.flag_preview(index, lock, value);
                                let r = Rect::from_min_size(
                                    pos2(
                                        label_x + label_width - offset,
                                        y + (row_height - 17.0) * 0.5,
                                    ),
                                    vec2(17.0, 17.0),
                                );
                                if (!lock && painted_value) || (lock && !painted_value) {
                                    ui.painter().circle_filled(
                                        r.center(),
                                        1.5,
                                        if selected {
                                            Color32::from_gray(243)
                                        } else {
                                            p.muted
                                        },
                                    );
                                } else {
                                    theme::paint_icon(
                                        ui,
                                        r.shrink(2.4),
                                        if lock { icon } else { Icon::EyeOff },
                                        p.text,
                                    );
                                }
                                let tip = self.i18n.text(key);
                                let response = ui
                                    .interact(
                                        r,
                                        ui.id().with(("flag", index, offset as u32)),
                                        Sense::click_and_drag(),
                                    )
                                    .on_hover_text(&tip);
                                response.widget_info(|| {
                                    egui::WidgetInfo::labeled(
                                        egui::WidgetType::Checkbox,
                                        true,
                                        &tip,
                                    )
                                });
                                if response.clicked() {
                                    if ui.input(|i| i.modifiers.alt) {
                                        self.solo_layer_flag(index, lock);
                                    } else {
                                        self.set_layer_flags(&[index], lock, !value);
                                    }
                                }
                                if response.drag_started() {
                                    self.begin_flag_drag(index, lock, !value);
                                }
                            }
                            if index == self.editor.layer {
                                paint_current_pencil(
                                    ui,
                                    Rect::from_min_size(
                                        pos2(
                                            label_x + label_width - 75.0,
                                            y + (row_height - 13.0) * 0.5,
                                        ),
                                        vec2(13.0, 13.0),
                                    ),
                                    p,
                                );
                            }
                            theme::edge_bands(
                                ui.painter(),
                                name_area,
                                false,
                                true,
                                if p.panel.r() < 128 { &[107] } else { &[230] },
                            );
                        }
                        // Gestures preview geometry only; mutation happens once on release.
                        if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                            let frame_drag = self.timeline_state.gesture.is_some()
                                || self.timeline_state.duration_drag.is_some();
                            if ui.input(|i| i.pointer.primary_down())
                                && (frame_drag
                                    || self.timeline_state.layer_drag.is_some()
                                    || self.flag_drag_active())
                            {
                                let edge = |p: f32, min: f32, max: f32| {
                                    if p < min + 20.0 {
                                        (min + 20.0 - p).clamp(0.0, 20.0)
                                    } else if p > max - 20.0 {
                                        -(p - max + 20.0).clamp(0.0, 20.0)
                                    } else {
                                        0.0
                                    }
                                };
                                let delta = vec2(
                                    if frame_drag {
                                        edge(pos.x, grid_clip.left(), grid_clip.right())
                                    } else {
                                        0.0
                                    },
                                    edge(pos.y, visible.top(), visible.bottom()),
                                );
                                if visible.contains(pos) && delta != egui::Vec2::ZERO {
                                    ui.scroll_with_delta_animation(
                                        delta * ui.input(|i| i.stable_dt.min(0.05)) * 24.0,
                                        egui::style::ScrollAnimation::none(),
                                    );
                                    ui.ctx().request_repaint();
                                }
                            }
                            let target_frame = (((pos.x - rect.left() - label_width) / cell)
                                .floor()
                                .max(0.0) as u32)
                                .min(frames - 1);
                            let row = row_offsets
                                .partition_point(|&y| y <= pos.y - rect.top())
                                .saturating_sub(1)
                                .min(rows.len() - 1);
                            let target_layer = rows[row];
                            let row_top = rect.top() + row_offsets[row];
                            let row_bottom = rect.top() + row_offsets[row + 1];
                            let drop_position =
                                ((pos.y - row_top) / (row_bottom - row_top)).clamp(0.0, 1.0);
                            if self.timeline_state.layer_drag.is_some() && visible.contains(pos) {
                                let into = layers[target_layer].kind == ac_core::LayerKind::Folder
                                    && (0.25..=0.75).contains(&drop_position);
                                let marker = Rect::from_min_max(
                                    pos2(label_x, row_top),
                                    pos2(label_x + label_width, row_bottom),
                                );
                                if into {
                                    painter.rect_stroke(
                                        marker,
                                        0.0,
                                        Stroke::new(2.0, p.accent),
                                        egui::StrokeKind::Inside,
                                    );
                                } else {
                                    let y = if drop_position >= 0.5 {
                                        row_bottom
                                    } else {
                                        row_top
                                    };
                                    painter.line_segment(
                                        [pos2(marker.left(), y), pos2(marker.right(), y)],
                                        Stroke::new(2.0, p.accent),
                                    );
                                }
                            }
                            self.update_flag_drag(
                                target_layer,
                                &rows,
                                ui.input(|i| {
                                    i.pointer.button_released(egui::PointerButton::Primary)
                                }),
                                visible.contains(pos)
                                    && pos.x >= label_x
                                    && pos.x < label_x + label_width,
                            );
                            if let Some(mut gesture) = self.timeline_state.gesture {
                                gesture.target = (target_layer, target_frame);
                                self.timeline_state.gesture = Some(gesture);
                                if let Some(left) = gesture.resize_left {
                                    let delta = target_frame as i64 - gesture.origin.1 as i64;
                                    let valid = target_layer == gesture.source.layer0
                                        && self
                                            .resized_static_span(gesture.source, left, delta)
                                            .is_some();
                                    ui.ctx().set_cursor_icon(if valid {
                                        egui::CursorIcon::ResizeHorizontal
                                    } else {
                                        egui::CursorIcon::NotAllowed
                                    });
                                    if valid {
                                        let x = rect.left()
                                            + label_width
                                            + (target_frame + u32::from(!left)) as f32 * cell;
                                        painter.line_segment(
                                            [pos2(x, row_top), pos2(x, row_bottom)],
                                            Stroke::new(2.0, p.accent),
                                        );
                                    }
                                } else if !gesture.moving {
                                    self.select_frame_range(gesture.origin, gesture.target);
                                } else {
                                    let delta = target_frame as i64 - gesture.origin.1 as i64;
                                    let dl = target_layer as i64 - gesture.origin.0 as i64;
                                    let valid = self
                                        .frame_ranges()
                                        .iter()
                                        .all(|s| self.can_retime(*s, delta, dl));
                                    ui.ctx().set_cursor_icon(if valid {
                                        egui::CursorIcon::Grabbing
                                    } else {
                                        egui::CursorIcon::NotAllowed
                                    });
                                    if valid {
                                        let x = rect.left()
                                            + label_width
                                            + (gesture.source.f0 as i64 + delta) as f32 * cell;
                                        painter.rect_stroke(
                                            Rect::from_min_size(
                                                pos2(x, rect.top() + row_offsets[row]),
                                                vec2(
                                                    gesture.source.frames() as f32 * cell,
                                                    row_height,
                                                ),
                                            ),
                                            0.0,
                                            Stroke::new(2.0, p.accent),
                                            egui::StrokeKind::Inside,
                                        );
                                    }
                                }
                            }
                            if let Some((layer, _)) = self.timeline_state.duration_drag {
                                let end = (target_frame + 1).max(
                                    layers[layer]
                                        .keyframes
                                        .keys()
                                        .next_back()
                                        .copied()
                                        .unwrap_or(0)
                                        + 1,
                                );
                                self.timeline_state.duration_drag = Some((layer, end));
                                let x = rect.left() + label_width + end as f32 * cell;
                                painter.line_segment(
                                    [pos2(x, rect.top()), pos2(x, visible.bottom())],
                                    Stroke::new(2.0, p.accent),
                                );
                            }
                            if ui.input(|i| i.pointer.button_released(egui::PointerButton::Primary))
                            {
                                if let Some(gesture) = self.timeline_state.gesture.take() {
                                    if let Some(left) = gesture.resize_left {
                                        if grid_clip.contains(pos)
                                            && target_layer == gesture.source.layer0
                                        {
                                            self.resize_static_span(
                                                gesture.source,
                                                left,
                                                gesture.target.1 as i64 - gesture.origin.1 as i64,
                                            );
                                        }
                                    } else if gesture.moving && grid_clip.contains(pos) {
                                        self.retime_frames(
                                            gesture.source,
                                            gesture.target.1 as i64 - gesture.origin.1 as i64,
                                            gesture.target.0 as i64 - gesture.origin.0 as i64,
                                            gesture.copy,
                                        );
                                    }
                                }
                                if let Some((layer, end)) = self.timeline_state.duration_drag.take()
                                {
                                    if grid_clip.contains(pos) {
                                        self.set_timeline_duration(layer, end);
                                    }
                                }
                                if let Some(from) = self.timeline_state.layer_drag.take() {
                                    if pos.x >= label_x
                                        && pos.x < label_x + label_width
                                        && visible.contains(pos)
                                    {
                                        self.drop_layers_at(from, target_layer, drop_position);
                                        self.sync_forms();
                                    }
                                }
                            }
                        }
                        let pixel = 1.0 / ui.ctx().pixels_per_point();
                        let x =
                            ((rect.left() + label_width + (self.editor.frame as f32 + 0.5) * cell)
                                / pixel)
                                .floor()
                                * pixel;
                        painter.rect_filled(
                            Rect::from_min_max(
                                pos2(x, visible.top() + ruler_height),
                                pos2(x + pixel, visible.bottom()),
                            ),
                            0.0,
                            p.playhead,
                        );
                        let cap = Rect::from_min_size(
                            pos2(x - cell / 2.0, visible.top() + 2.4),
                            vec2(cell, 20.8),
                        );
                        ruler_painter.rect_filled(cap, 0.0, p.playhead.gamma_multiply(0.22));
                        ruler_painter.rect_stroke(
                            cap.shrink(pixel * 0.5),
                            0.0,
                            Stroke::new(pixel, p.playhead),
                            egui::StrokeKind::Inside,
                        );
                        let ruler_bottom = Rect::from_min_max(
                            pos2(visible.left(), visible.top()),
                            pos2(visible.right(), visible.top() + ruler_height),
                        );
                        theme::edge_bands(
                            &ruler_painter,
                            ruler_bottom,
                            false,
                            true,
                            if p.panel.r() < 128 { &[80] } else { &[194] },
                        );
                        // Pinned label header stays legible when the frame grid scrolls horizontally.
                        let header = Rect::from_min_size(
                            pos2(label_x, visible.top()),
                            vec2(label_width, ruler_height),
                        );
                        ui.painter().rect_filled(header, 0.0, p.panel);
                        self.outline_control(
                            ui,
                            Rect::from_min_size(
                                pos2(label_x + label_width - 17.6, visible.top() + 4.0),
                                vec2(11.2, 17.0),
                            ),
                            None,
                        );
                        let outline = Rect::from_min_size(
                            pos2(label_x + label_width - 14.6, visible.top() + 7.0),
                            vec2(5.2, 11.0),
                        );
                        ui.painter().rect_filled(outline, 0.0, p.panel);
                        ui.painter().rect_stroke(
                            outline,
                            0.0,
                            Stroke::new(1.0 / ui.ctx().pixels_per_point(), p.text),
                            egui::StrokeKind::Inside,
                        );
                        for (offset, icon) in [(56.0, Icon::Eye), (36.0, Icon::Lock)] {
                            paint_header_flag(
                                ui,
                                Rect::from_min_size(
                                    pos2(label_x + label_width - offset, visible.top() + 4.0),
                                    vec2(16.0, 16.0),
                                ),
                                matches!(icon, Icon::Lock),
                                p,
                            );
                            let lock = matches!(icon, Icon::Lock);
                            let all = layers
                                .iter()
                                .all(|l| if lock { l.locked } else { l.visible });
                            let command = match (lock, all) {
                                (true, true) => Command::UnlockAllLayers,
                                (true, false) => Command::LockAllLayers,
                                (false, true) => Command::HideAllLayers,
                                (false, false) => Command::ShowAllLayers,
                            };
                            let r = Rect::from_min_size(
                                pos2(label_x + label_width - offset, visible.top() + 4.0),
                                vec2(16.0, 16.0),
                            );
                            let response = ui
                                .interact(r, ui.id().with(("all.flags", lock)), Sense::click())
                                .on_hover_text(self.i18n.text(command.label()));
                            response.widget_info(|| {
                                egui::WidgetInfo::labeled(
                                    egui::WidgetType::Button,
                                    true,
                                    self.i18n.text(command.label()),
                                )
                            });
                            if response.clicked() {
                                self.dispatch(command, ui.ctx());
                            }
                        }
                        // The pinned label divider extends through the unoccupied body.
                        let divider = Rect::from_min_max(
                            pos2(label_x + label_width - 2.4, visible.top() + ruler_height),
                            pos2(label_x + label_width + 1.6, visible.bottom()),
                        );
                        theme::edge_bands(
                            ui.painter(),
                            divider,
                            true,
                            false,
                            if p.panel.r() < 128 {
                                &[60, 83, 66]
                            } else {
                                &[202, 229, 209]
                            },
                        );
                    });
                scroll.offset = output.state.offset;
                scroll.content = output.content_size;
                scroll.view = output.inner_rect.size();
                timeline_scrollbar(
                    ui,
                    vscroll,
                    true,
                    &mut scroll.offset.y,
                    scroll.view.y - ruler_height,
                    scroll.content.y - ruler_height,
                    18.0,
                    p,
                    &self.i18n,
                );
                ui.painter().rect_filled(
                    Rect::from_min_max(pos2(vscroll.left(), viewport.top()), vscroll.right_top()),
                    0.0,
                    p.panel,
                );
                ui.ctx().data_mut(|d| d.insert_temp(scroll_id, scroll));
                ui.allocate_rect(viewport, Sense::hover());
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::{kittest::{NodeT, Queryable}, Harness};

    #[test]
    fn compact_timeline_rails_zoom_and_controls_preserve_history_and_geometry() {
        for locale in [Locale::English, Locale::French, Locale::Expanded] {
            for theme in [theme::Theme::Light, theme::Theme::Dark] {
                let mut h = Harness::builder()
                    .with_size(vec2(1536.0, 824.0))
                    .with_pixels_per_point(1.25)
                    .build_eframe(|cc| {
                        let mut app = Studio::with_preferences(
                            &cc.egui_ctx,
                            Preferences {
                                locale,
                                theme,
                                ..Default::default()
                            },
                        );
                        let mut doc = document::blank("Scene", "Layer 1");
                        for i in 2..=40 {
                            doc.scene
                                .layers
                                .push(ac_core::Layer::new(&format!("Layer {i}"), 1));
                        }
                        app.replace(doc, None, false);
                        app
                    });
                h.run();
                let before = h.state().editor.doc.clone();
                let history = h.state().editor.undo_depth();
                let label = |h: &Harness<Studio>, key: &str| h.state().i18n.text(key);
                let horizontal = h
                    .get_by_label(&label(&h, "timeline-scroll-horizontal"))
                    .rect();
                let vertical = h
                    .get_by_label(&label(&h, "timeline-scroll-vertical"))
                    .rect();
                let zoom = h.get_by_label(&label(&h, "field-frame-size")).rect();
                assert!(horizontal.width() > 100.0 && horizontal.height() < 14.0);
                assert!(vertical.height() > 50.0 && vertical.width() < 15.0);
                h.get_by_label(&label(&h, "timeline-scroll-horizontal"))
                    .hover();
                for _ in 0..3 {
                    h.step();
                    assert_eq!(
                        h.query_all_by_label(&label(&h, "timeline-scroll-horizontal"))
                            .find(|n| n.accesskit_node().role() == egui::accesskit::Role::Slider).unwrap().rect(),
                        horizontal
                    );
                }
                h.get_by_label(&label(&h, "timeline-scroll-right")).click();
                h.run();
                let state = || {
                    h.ctx
                        .data_mut(|d| {
                            d.get_temp::<TimelineScroll>(egui::Id::new(
                                "studio.timeline.scroll.metrics",
                            ))
                        })
                        .unwrap()
                };
                assert!(state().offset.x > 0.0);
                h.get_by_label(&label(&h, "timeline-scroll-down")).click();
                h.run();
                let state = h
                    .ctx
                    .data_mut(|d| {
                        d.get_temp::<TimelineScroll>(egui::Id::new(
                            "studio.timeline.scroll.metrics",
                        ))
                    })
                    .unwrap();
                assert!(state.offset.y > 0.0);
                super::super::timeline_tests::drag(
                    &mut h,
                    horizontal.center(),
                    horizontal.right_center() - vec2(8.0, 0.0),
                    false,
                    egui::Modifiers::NONE,
                );
                let state = h
                    .ctx
                    .data_mut(|d| {
                        d.get_temp::<TimelineScroll>(egui::Id::new(
                            "studio.timeline.scroll.metrics",
                        ))
                    })
                    .unwrap();
                assert!(state.offset.x > 500.0);
                h.get_by_label(&label(&h, "action-center-playhead")).click();
                h.run();
                let state = h
                    .ctx
                    .data_mut(|d| {
                        d.get_temp::<TimelineScroll>(egui::Id::new(
                            "studio.timeline.scroll.metrics",
                        ))
                    })
                    .unwrap();
                assert_eq!(state.offset.x, 0.0);
                super::super::timeline_tests::click(
                    &mut h,
                    zoom.right_center() - vec2(2.0, 0.0),
                    egui::Modifiers::NONE,
                );
                assert!(h.state().prefs.frame_width > 18.0);
                assert_eq!(h.get_by_label(&label(&h, "field-frame-size")).rect(), zoom);
                h.get_by_label(&label(&h, "field-frame-display")).click();
                h.run();
                h.get_by_label(&label(&h, "frames-preview")).click();
                h.run();
                assert!(h.state().prefs.frame_display == FrameDisplay::Preview);
                assert_eq!(h.state().editor.doc, before);
                assert_eq!(h.state().editor.undo_depth(), history);
                h.get_by_label(&label(&h, "cmd-folder")).click();
                h.run();
                assert_eq!(h.state().editor.timeline().layers.len(), 41);
                h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
                h.run();
                assert_eq!(h.state().editor.doc, before);
            }
        }
    }
}

