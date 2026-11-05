//! The Properties panel and its tabs.

use crate::workspace::Workspace;
use crate::workspace::{PropsContext, PropsKey, Tool};
use egui::{RichText, Ui};
use flare_menubar::MenuAction;
use flare_ui::chrome;
use flare_ui::icons;
use flare_ui::theme;

impl Workspace {
    // ---- properties --------------------------------------------------------------------

    pub(crate) fn properties_panel(&mut self, ui: &mut Ui) {
        let context = self.props_context();
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| match context {
                PropsContext::Object => self.object_tab(ui),
                PropsContext::Frame => self.frame_tab(ui),
                PropsContext::Tool => self.tool_tab(ui),
                PropsContext::Document => self.doc_tab(ui),
            });
    }

    /// What the panel is about now: it follows the selection (see [`PropsContext`]).
    pub(in crate::workspace) fn props_context(&mut self) -> PropsContext {
        if let Some((context, key)) = self.props_forced {
            if key == self.props_key() {
                return context;
            }
            self.props_forced = None;
        }
        if self.selection_bounds().is_some() {
            PropsContext::Object
        } else if self.frame_range.is_some() {
            PropsContext::Frame
        } else if matches!(
            self.tool,
            Tool::Rectangle
                | Tool::Ellipse
                | Tool::Line
                | Tool::Pen
                | Tool::Text
                | Tool::PolyStar
                | Tool::Pencil
                | Tool::Brush
                | Tool::Bucket
                | Tool::InkBottle
                | Tool::Camera
                | Tool::Ruler
                | Tool::Mirror
        ) {
            PropsContext::Tool
        } else {
            PropsContext::Document
        }
    }

    fn props_key(&self) -> PropsKey {
        PropsKey {
            tool: self.tool,
            selected: self.selected,
            others: self.also_selected.len(),
            frames: self.frame_range,
        }
    }

    /// Asks the panel for `context` until the selection or the tool changes.
    pub(in crate::workspace) fn show_props_context(&mut self, context: PropsContext) {
        self.props_forced = Some((context, self.props_key()));
    }

    pub(crate) fn tool_tab(&mut self, ui: &mut Ui) {
        self.tool_options(ui);
        chrome::section(ui, "fill-color", &tr!(self, "opt-fill-stroke"));
        let (gradient, vertical, outline) = (
            tr!(self, "opt-gradient-check"),
            tr!(self, "opt-vertical"),
            tr!(self, "opt-outline"),
        );
        let (none, round, square) = (
            tr!(self, "opt-cap-none"),
            tr!(self, "opt-round"),
            tr!(self, "opt-square"),
        );
        let (bevel, miter) = (tr!(self, "opt-bevel"), tr!(self, "opt-miter"));
        chrome::row(ui, &tr!(self, "opt-fill"), |ui| {
            ui.color_edit_button_srgba(&mut self.paint.fill);
            ui.checkbox(&mut self.paint.gradient_enabled, &gradient);
        });
        if self.paint.gradient_enabled {
            chrome::row(ui, &tr!(self, "opt-to"), |ui| {
                ui.color_edit_button_srgba(&mut self.paint.gradient_end);
                ui.checkbox(&mut self.paint.gradient_vertical, &vertical);
            });
        }
        chrome::row(ui, &tr!(self, "opt-stroke"), |ui| {
            ui.color_edit_button_srgba(&mut self.paint.stroke_color);
            ui.checkbox(&mut self.paint.stroke_enabled, &outline);
        });
        chrome::row(ui, &tr!(self, "opt-width"), |ui| {
            ui.add_enabled(
                self.paint.stroke_enabled,
                egui::DragValue::new(&mut self.paint.stroke_width)
                    .range(0.5..=200.0)
                    .suffix(" px"),
            );
        });
        chrome::row(ui, &tr!(self, "opt-cap"), |ui| {
            use flare_document::Cap;
            egui::ComboBox::from_id_salt("cap_menu")
                .width(80.0)
                .selected_text(match self.paint.stroke_cap {
                    Cap::Butt => none.as_str(),
                    Cap::Round => round.as_str(),
                    Cap::Square => square.as_str(),
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.paint.stroke_cap, Cap::Butt, &none);
                    ui.selectable_value(&mut self.paint.stroke_cap, Cap::Round, &round);
                    ui.selectable_value(&mut self.paint.stroke_cap, Cap::Square, &square);
                });
        });
        chrome::row(ui, &tr!(self, "opt-join"), |ui| {
            use flare_document::Join;
            egui::ComboBox::from_id_salt("join_menu")
                .width(80.0)
                .selected_text(match self.paint.stroke_join {
                    Join::Round => round.as_str(),
                    Join::Bevel => bevel.as_str(),
                    Join::Miter => miter.as_str(),
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.paint.stroke_join, Join::Round, &round);
                    ui.selectable_value(&mut self.paint.stroke_join, Join::Bevel, &bevel);
                    ui.selectable_value(&mut self.paint.stroke_join, Join::Miter, &miter);
                });
        });
        chrome::row(ui, &tr!(self, "opt-stroke-style"), |ui| {
            use flare_document::StrokeStyle;
            let current = self.style_label(self.paint.stroke_style);
            egui::ComboBox::from_id_salt("stroke_style_menu")
                .width(80.0)
                .selected_text(current)
                .show_ui(ui, |ui| {
                    for value in StrokeStyle::ALL {
                        let label = self.style_label(value);
                        ui.selectable_value(&mut self.paint.stroke_style, value, label);
                    }
                });
        });
    }

    /// The current frame and layer.
    pub(crate) fn frame_tab(&mut self, ui: &mut Ui) {
        let frame_label = tr!(self, "prop-frame");
        chrome::section(ui, "playhead", &frame_label);
        chrome::row(ui, &frame_label, |ui| {
            ui.label(format!("{} / {}", self.frame + 1, self.scene_frames()));
        });
        let info = self.active_layer().map(|layer| {
            let span = layer
                .span_at(self.frame)
                .map(|s| (s.start(), s.duration().get()));
            (layer.name().to_owned(), span)
        });
        match info {
            Some((name, span)) => {
                let (layer_label, exposure) = (tr!(self, "prop-layer"), tr!(self, "prop-exposure"));
                let span_text = span.map_or_else(
                    || tr!(self, "prop-empty"),
                    |(start, len)| {
                        tr!(
                            self,
                            "prop-exposure-span",
                            &[
                                ("start", &(start + 1).to_string()),
                                ("len", &len.to_string()),
                            ],
                        )
                    },
                );
                chrome::row(ui, &layer_label, |ui| {
                    ui.label(name);
                });
                chrome::row(ui, &exposure, |ui| {
                    if span.is_some() {
                        ui.label(&span_text);
                    } else {
                        ui.weak(&span_text);
                    }
                });
            }
            None => {
                ui.weak(tr!(self, "prop-no-layer"));
            }
        }
        chrome::section(ui, "frame-tween", &tr!(self, "prop-tween"));
        self.tween_controls(ui);
    }

    /// Whether the selected span is tweened and by how much it eases, with the controls to change
    /// both. A motion tween moves an instance; a shape tween changes a shape.
    pub(super) fn tween_controls(&mut self, ui: &mut Ui) {
        let action = if self.selection_holds_shape() {
            MenuAction::ToggleShapeTween
        } else {
            MenuAction::ToggleMotionTween
        };
        let name = self.tr(if action == MenuAction::ToggleShapeTween {
            "prop-shape-tween"
        } else {
            "prop-motion-tween"
        });
        let tweened = matches!(
            self.selection_tween(),
            Some(flare_document::Tween::Linear | flare_document::Tween::Shape)
        );
        let mut on = tweened;
        let usable = self.is_enabled(action);
        if ui
            .add_enabled(usable, egui::Checkbox::new(&mut on, name))
            .changed()
        {
            self.perform(action);
        }
        let mut ease = self.selection_ease().unwrap_or(0);
        let before = ease;
        let ease_label = tr!(self, "prop-ease");
        let ease_hint = tr!(self, "tween-ease-hint");
        chrome::row(ui, &ease_label, |ui| {
            ui.add_enabled(
                tweened,
                egui::Slider::new(
                    &mut ease,
                    -flare_document::MAX_EASE..=flare_document::MAX_EASE,
                ),
            )
            .on_hover_text(ease_hint);
        });
        if ease != before {
            self.set_selection_ease(ease);
        }
        self.interpolation_picker(ui, tweened);
    }

    pub(crate) fn doc_tab(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.add(icons::image("document", 18.0, theme::accent()));
            ui.label(RichText::new(tr!(self, "doc-title")).heading());
        });
        self.publish_section(ui);
        self.document_section(ui);
    }

    /// The SWF publish choices, with the button for the rest.
    fn publish_section(&mut self, ui: &mut Ui) {
        let title = tr!(self, "doc-publish-settings");
        chrome::section(ui, "file-publish", &title);
        self.publish_settings_ui(ui);
    }

    /// Stage size, background and the facts about the document.
    pub(crate) fn document_section(&mut self, ui: &mut Ui) {
        let title = tr!(self, "doc-title");
        chrome::section(ui, "document", &title);
        self.stage_settings(ui);
        let rate = self.document().frame_rate();
        let label = tr!(self, "doc-frame-rate");
        chrome::row(ui, &label, |ui| {
            let current = rate.per_second();
            let mut fps = self.fps_edit.unwrap_or(current);
            let response = ui.add_enabled(
                !self.in_symbol(),
                egui::DragValue::new(&mut fps)
                    .range(0.01..=240.0)
                    .speed(0.1)
                    .suffix(" fps"),
            );
            if response.changed() {
                self.fps_edit = Some(fps);
            }
            // Applied on release or Enter, so scrubbing the value is one undo step.
            if (response.drag_stopped() || response.lost_focus())
                && let Some(fps) = self.fps_edit.take()
            {
                self.set_frame_rate(fps);
            }
        });
        let label = tr!(self, "doc-scenes");
        chrome::row(ui, &label, |ui| {
            ui.label(self.document().scenes().len().to_string());
        });
        let label = tr!(self, "doc-sounds");
        chrome::row(ui, &label, |ui| {
            ui.label(self.document().sounds().len().to_string());
        });
    }
}
