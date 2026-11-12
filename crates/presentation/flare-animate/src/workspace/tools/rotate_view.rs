//! The Rotation tool: it turns the view of the stage, never the document (Animate's Rotation
//! tool). A drag on the stage turns it about the panel's center; the dial, shown when Alt (Option
//! on a Mac) is tapped on its own, turns it by its needle, in 15 degree steps, or back upright.
//! The geometry is `flare_ui::view`; the tap is `flare_ui::tap`.

use egui::emath::Rot2;
use egui::{Align2, Color32, Key, Pos2, Response, Sense, Stroke, Ui, Vec2};
use flare_ui::chrome;
use flare_ui::tap::ModifierTap;
use flare_ui::view::{ROTATION_STEP_DEGREES, snap_rotation, turn_between};

use crate::workspace::*;

/// What the Rotation tool and its dial are doing.
#[derive(Default)]
pub(in crate::workspace) struct RotateView {
    /// A turning drag on the stage: where it started on screen, and the turn then.
    drag: Option<(Pos2, f32)>,
    /// The dial is showing, with the tool in hand before it, to go back to.
    dial: Option<Tool>,
    tap: ModifierTap,
}

/// The size of the dial, in points.
const DIAL: f32 = 76.0;

/// The key a person taps for the dial, as their keyboard names it.
pub(in crate::workspace) fn tap_key(ctx: &egui::Context) -> &'static str {
    if ctx.os() == egui::os::OperatingSystem::Mac {
        "\u{2325} Option"
    } else {
        "Alt"
    }
}

impl Workspace {
    /// How far the stage view is turned, in whole degrees clockwise, in `(-180, 180]`.
    pub fn stage_rotation_degrees(&self) -> f32 {
        self.stage_view.rotation_degrees()
    }

    /// Turns the stage view to `degrees` clockwise. Only the view turns; the document does not.
    pub fn set_stage_rotation_degrees(&mut self, degrees: f32) {
        self.stage_view.set_rotation_degrees(degrees);
    }

    /// Sets the stage view upright again.
    pub fn reset_stage_rotation(&mut self) {
        self.stage_view.reset_rotation();
    }

    pub fn rotation_dial_open(&self) -> bool {
        self.rotate.dial.is_some()
    }

    /// Shows the dial with the Rotation tool in hand, or hides it and goes back to the tool held
    /// before. What a tap of Alt (Option) does.
    pub fn toggle_rotation_dial(&mut self) {
        match self.rotate.dial.take() {
            Some(before) => {
                if self.tool == Tool::RotateView {
                    self.set_tool(before);
                }
            }
            None => {
                let before = self.tool;
                self.set_tool(Tool::RotateView);
                self.rotate.dial = Some(before);
            }
        }
    }

    /// Called by [`Self::set_tool`]: a tool other than this one closes the dial.
    pub(in crate::workspace) fn rotate_view_tool_changed(&mut self, tool: Tool) {
        self.rotate.drag = None;
        if tool != Tool::RotateView {
            self.rotate.dial = None;
        }
    }

    /// Watches for Alt (Option) tapped on its own. Alt with a click, a drag, a key or another
    /// modifier is still that chord, and nothing here sees it; neither does a tap while a field
    /// takes the typing or the shortcut editor waits for keys.
    pub(in crate::workspace) fn watch_rotation_tap(&mut self, ctx: &egui::Context) {
        let busy = ctx.egui_wants_keyboard_input() || self.shortcut_editor.capturing.is_some();
        let (down, other, now) = ctx.input(|i| {
            let m = i.modifiers;
            let other = m.shift
                || m.ctrl
                || m.mac_cmd
                || m.command
                || i.pointer.any_down()
                || i.pointer.any_pressed()
                || i.events.iter().any(|e| {
                    matches!(
                        e,
                        egui::Event::Key { .. } | egui::Event::Text(_) | egui::Event::Copy
                    )
                });
            (m.alt, other || busy, i.time)
        });
        if self.rotate.tap.update(down, other, now) {
            self.toggle_rotation_dial();
        }
    }

    /// The Rotation tool's drag on the stage: the view turns by the angle the pointer sweeps
    /// about the panel's center, in 15 degree steps with Shift. A double-click sets it upright.
    pub(in crate::workspace) fn rotate_view_stage(&mut self, ui: &Ui, response: &Response) {
        if self.tool != Tool::RotateView {
            return;
        }
        let primary = egui::PointerButton::Primary;
        let center = response.rect.center();
        if response.double_clicked() {
            self.rotate.drag = None;
            self.stage_view.reset_rotation();
            return;
        }
        if response.drag_started_by(primary)
            && let Some(from) = ui.input(|i| i.pointer.press_origin())
        {
            self.rotate.drag = Some((from, self.stage_view.rotation()));
        }
        if response.dragged_by(primary)
            && let (Some((from, start)), Some(to)) =
                (self.rotate.drag, response.interact_pointer_pos())
            && let Some(swept) = turn_between(center, from, to)
        {
            let snap = ui.input(|i| i.modifiers.shift);
            self.turn_view_to(start + swept, snap);
        }
        if response.drag_stopped_by(primary) {
            self.rotate.drag = None;
        }
    }

    fn turn_view_to(&mut self, angle: f32, snap: bool) {
        let angle = if snap {
            snap_rotation(angle, ROTATION_STEP_DEGREES)
        } else {
            angle
        };
        self.stage_view.set_rotation(angle);
    }

    /// The dial over the foot of the stage, while it is open. Escape closes it, as a second tap
    /// does.
    pub(in crate::workspace) fn rotation_dial(&mut self, ui: &mut Ui, stage: &Response) {
        if self.rotate.dial.is_none() {
            return;
        }
        if !ui.ctx().egui_wants_keyboard_input() && ui.input(|i| i.key_pressed(Key::Escape)) {
            self.toggle_rotation_dial();
            return;
        }
        let (title, left, right, upright, angle) = (
            tr!(self, "view-turn-title"),
            tr!(self, "view-turn-left"),
            tr!(self, "view-turn-right"),
            tr!(self, "view-turn-reset"),
            tr!(self, "view-turn-angle"),
        );
        let hint = tr!(self, "view-turn-hint", &[("key", tap_key(ui.ctx()))]);
        let at = stage.rect.center_bottom() - Vec2::new(0.0, 12.0);
        egui::Area::new(ui.make_persistent_id("view-turn-dial"))
            .order(egui::Order::Foreground)
            .pivot(Align2::CENTER_BOTTOM)
            .fixed_pos(at)
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        self.dial_face(ui);
                        ui.vertical(|ui| {
                            ui.strong(title);
                            ui.horizontal(|ui| {
                                let step = ROTATION_STEP_DEGREES.to_radians();
                                if chrome::icon_button(ui, "rotate-ccw", 16.0, false, true, &left)
                                    .clicked()
                                {
                                    self.stage_view.rotate_by(-step);
                                }
                                let mut degrees = self.stage_view.rotation_degrees();
                                let field = ui.add(
                                    egui::DragValue::new(&mut degrees)
                                        .range(-180.0..=180.0)
                                        .speed(1.0)
                                        .suffix("\u{b0}")
                                        .fixed_decimals(0),
                                );
                                if field.changed() {
                                    self.stage_view.set_rotation_degrees(degrees);
                                }
                                field.on_hover_text(&angle);
                                if chrome::icon_button(ui, "rotate-cw", 16.0, false, true, &right)
                                    .clicked()
                                {
                                    self.stage_view.rotate_by(step);
                                }
                                let turned = self.stage_view.is_turned();
                                if ui
                                    .add_enabled(turned, egui::Button::new(&upright).small())
                                    .clicked()
                                {
                                    self.stage_view.reset_rotation();
                                }
                            });
                            ui.weak(hint);
                        });
                    });
                });
            });
    }

    /// The dial's face: its needle points where the top of the stage is; a press or a drag on it
    /// points the needle there, in 15 degree steps with Shift.
    fn dial_face(&mut self, ui: &mut Ui) {
        let (rect, response) = ui.allocate_exact_size(Vec2::splat(DIAL), Sense::click_and_drag());
        let center = rect.center();
        let radius = DIAL / 2.0 - 4.0;
        let up = Vec2::new(0.0, -radius);
        if (response.dragged() || response.clicked())
            && let Some(p) = response.interact_pointer_pos()
            && let Some(angle) = turn_between(center, center + up, p)
        {
            let snap = ui.input(|i| i.modifiers.shift);
            self.turn_view_to(angle, snap);
        }
        let painter = ui.painter_at(rect);
        let rim = Stroke::new(1.0, flare_ui::theme::text_dim());
        painter.circle(center, radius, flare_ui::theme::panel_raised(), rim);
        // A mark every 45 degrees, longer at the four quarters.
        for i in 0..8 {
            let dir = Rot2::from_angle(i as f32 * std::f32::consts::FRAC_PI_4) * up.normalized();
            let inner = if i % 2 == 0 {
                radius - 8.0
            } else {
                radius - 4.0
            };
            painter.line_segment([center + dir * inner, center + dir * radius], rim);
        }
        let needle = Rot2::from_angle(self.stage_view.rotation()) * up.normalized();
        let ink = flare_ui::theme::accent();
        painter.line_segment(
            [center, center + needle * (radius - 6.0)],
            Stroke::new(2.0, ink),
        );
        painter.circle_filled(center + needle * (radius - 6.0), 4.0, ink);
        painter.circle_filled(center, 2.5, Color32::WHITE);
        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }
    }

    /// The Rotation tool's controls in Properties: the angle and a way back upright.
    pub(in crate::workspace) fn rotate_view_options(&mut self, ui: &mut Ui) {
        let label = tr!(self, "view-turn-angle");
        let mut degrees = self.stage_view.rotation_degrees();
        if chrome::slider(ui, &label, &mut degrees, -180.0..=180.0, "\u{b0}", false).changed() {
            self.stage_view.set_rotation_degrees(degrees);
        }
        let upright = tr!(self, "view-turn-reset");
        if ui
            .add_enabled(self.stage_view.is_turned(), egui::Button::new(upright))
            .clicked()
        {
            self.stage_view.reset_rotation();
        }
        let key = tap_key(ui.ctx());
        ui.weak(tr!(self, "tools-help-rotate-view", &[("key", key)]));
    }

    /// In the bar over the stage while the view is turned: the angle, and a click sets it upright.
    pub(in crate::workspace) fn rotation_readout(&mut self, ui: &mut Ui) {
        if !self.stage_view.is_turned() {
            return;
        }
        let degrees = format!("{}", self.stage_view.rotation_degrees());
        let hint = tr!(self, "view-turn-turned", &[("degrees", degrees.as_str())]);
        let text = format!("{degrees}\u{b0}");
        if ui
            .add(egui::Button::image_and_text(
                flare_ui::icons::image("tool-rotate-view", 14.0, Color32::WHITE),
                text,
            ))
            .on_hover_text(hint)
            .clicked()
        {
            self.stage_view.reset_rotation();
        }
    }
}
