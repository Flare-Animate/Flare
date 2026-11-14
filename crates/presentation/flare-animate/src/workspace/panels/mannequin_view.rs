//! The Mannequin panel's view of the figure, shaded as a 3D drawing figure, and posing it there
//! as one poses such a figure: a drag on any part pulls it and the body follows, the pinned
//! hands and feet staying put (a right click pins or lets go of one); a click picks a part and
//! shows rings round its joint, and a drag along a ring turns it about that axis alone; with Alt
//! a drag turns a part about the line of sight.
//!
//! Moving round it is a 3D program's way (`flare_mannequin::Camera`): a drag anywhere else
//! turns the view round what it looks at, a right, middle or Shift drag slides it across,
//! scrolling or pinching zooms toward the pointer and Shift-scrolling slides; a double click on
//! a part makes the view turn round that part, and on nothing frames the whole figure. Buttons
//! in the view's corner zoom, frame and turn it square to a side. A picture to pose from stands
//! behind the figure with its marks (see `mannequin_marks`). A grip under the view makes it
//! taller or shorter.

use egui::{Color32, Pos2, Rect, Sense, Stroke, Ui, vec2};
use flare_core::Rgba8;
use flare_mannequin::{Axis, Camera, Facing, Look, Placement};
use flare_ui::{chrome, icons, theme};

use crate::workspace::Workspace;
use crate::workspace::mannequin::{MannequinDrag, VIEW_HEIGHT_RANGE, pixels, show};

/// Degrees the view turns for a point of drag.
const ORBIT_DEGREES: f64 = 0.5;
/// How far a joint's rings reach from it, and how near one a press takes it, in points.
pub(crate) const RING_RADIUS: f64 = 30.0;
const RING_REACH: f64 = 6.0;
/// The grip under the view.
const GRIP: f32 = 8.0;
/// How much a point of scrolling zooms.
const SCROLL_ZOOM: f64 = 0.002;
/// How much a press of a zoom button zooms.
pub(crate) const BUTTON_ZOOM: f64 = 1.25;

fn at(p: Pos2) -> [f64; 2] {
    [f64::from(p.x), f64::from(p.y)]
}

pub(crate) fn pos(p: [f64; 2]) -> Pos2 {
    Pos2::new(p[0] as f32, p[1] as f32)
}

/// The view's middle, and how tall the figure is there at a zoom of one: four fifths of it.
fn view_frame(rect: Rect) -> ([f64; 2], f64) {
    let c = rect.center();
    (
        [f64::from(c.x), f64::from(c.y)],
        f64::from(rect.height() * 0.8),
    )
}

/// The figure's placement in the view `rect`, in points, as `camera` sees it.
pub(crate) fn view_placement(rect: Rect, camera: &Camera) -> Option<Placement> {
    let (center, height) = view_frame(rect);
    camera.placement(center, height)
}

fn axis_color(axis: Axis) -> Color32 {
    match axis {
        Axis::Bend => Color32::from_rgb(232, 84, 84),
        Axis::Twist => Color32::from_rgb(92, 196, 104),
        Axis::Side => Color32::from_rgb(80, 140, 240),
    }
}

impl Workspace {
    /// How tall the view is: as the person made it, as tall as the window allows when the panel
    /// is made large, else a little taller than wide.
    fn mannequin_view_height(&self, ui: &Ui, width: f32) -> f32 {
        let (low, high) = VIEW_HEIGHT_RANGE;
        let screen = ui.ctx().content_rect().height();
        let own = self
            .mannequin
            .view_height
            .unwrap_or((width * 1.25).clamp(260.0, 520.0));
        let height = if self.mannequin.expanded {
            own.max(screen * 0.62)
        } else {
            own
        };
        height.clamp(low, high)
    }

    pub(crate) fn mannequin_view(&mut self, ui: &mut Ui) {
        if self.poll_mannequin_reference() {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(100));
        }
        let width = ui.available_width().max(120.0);
        let size = vec2(width, self.mannequin_view_height(ui, width));
        let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
        self.mannequin.view_rect = Some(rect);
        if std::mem::take(&mut self.mannequin.frame_next) {
            self.frame_mannequin();
        }
        let label = tr!(self, "pane-mannequin");
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Canvas, true, &label));
        if let Some(placement) = view_placement(rect, &self.mannequin.camera) {
            self.mannequin_input(ui, &response, rect, placement);
        }
        if let Some(placement) = view_placement(rect, &self.mannequin.camera) {
            self.paint_mannequin_view(ui, rect, placement);
        }
        self.mannequin_view_buttons(ui, rect);
        self.mannequin_view_grip(ui, rect);
    }

    /// The picture of the figure for the view, made again only when something it shows changed.
    fn mannequin_picture(&mut self, ui: &Ui, rect: Rect, placement: Placement) -> bool {
        let k = ui.ctx().pixels_per_point();
        let size = [pixels(rect.width(), k), pixels(rect.height(), k)];
        let Some(inside) = Placement::new(
            [
                (placement.center[0] - f64::from(rect.min.x)) * f64::from(k),
                (placement.center[1] - f64::from(rect.min.y)) * f64::from(k),
            ],
            placement.height * f64::from(k),
        )
        .and_then(|p| p.with_focus(placement.focus())) else {
            return false;
        };
        let accent = theme::accent();
        let look = Look {
            picked: self
                .mannequin
                .selected
                .map(|j| (j, Rgba8::opaque(accent.r(), accent.g(), accent.b()))),
            guides: self.mannequin.guides,
            shadow: true,
        };
        let figure = self.mannequin.figure;
        let ctx = ui.ctx().clone();
        show(
            &ctx,
            &mut self.mannequin.shown,
            "mannequin-view",
            &figure,
            inside,
            size,
            look,
        )
        .is_some()
    }

    /// The joint whose part is seen at `p` in the view.
    fn mannequin_part_at(&self, ui: &Ui, rect: Rect, p: Pos2) -> Option<flare_mannequin::Joint> {
        let k = ui.ctx().pixels_per_point();
        let local = (p - rect.min) * k;
        self.mannequin
            .shown
            .as_ref()?
            .picture
            .joint_at([f64::from(local.x), f64::from(local.y)])
    }

    /// Scrolling and pinching over the view zoom toward the pointer, Shift-scrolling slides,
    /// and none of it scrolls the panel too.
    fn mannequin_wheel(&mut self, ui: &Ui, rect: Rect) {
        let (pinch, scroll, shift, pointer) = ui.input(|i| {
            (
                i.zoom_delta(),
                i.smooth_scroll_delta,
                i.modifiers.shift,
                i.pointer.hover_pos(),
            )
        });
        ui.ctx()
            .input_mut(|i| i.smooth_scroll_delta = egui::Vec2::ZERO);
        let (center, height) = view_frame(rect);
        let view = self.mannequin.figure.view;
        let toward = pointer.map_or(center, at);
        let camera = &mut self.mannequin.camera;
        if pinch != 1.0 {
            camera.zoom_about(&view, center, height, toward, f64::from(pinch));
        } else if shift {
            camera.pan(
                &view,
                center,
                height,
                [f64::from(scroll.x), f64::from(scroll.y)],
            );
        } else if scroll.y != 0.0 {
            let factor = (f64::from(scroll.y) * SCROLL_ZOOM).exp();
            camera.zoom_about(&view, center, height, toward, factor);
        }
    }

    fn mannequin_input(
        &mut self,
        ui: &Ui,
        response: &egui::Response,
        rect: Rect,
        placement: Placement,
    ) {
        if response.hovered() {
            self.mannequin_wheel(ui, rect);
        }
        // What a press picks is what was drawn there last.
        self.mannequin_picture(ui, rect, placement);
        let (alt, shift) = ui.input(|i| (i.modifiers.alt, i.modifiers.shift));
        let across = ui.input(|i| i.pointer.secondary_down() || i.pointer.middle_down());
        if response.drag_started()
            && let Some(p) = ui
                .input(|i| i.pointer.press_origin())
                .or_else(|| response.interact_pointer_pos())
        {
            self.mannequin.drag =
                Some(self.mannequin_drag_for(ui, rect, placement, p, alt, shift || across));
        }
        if response.dragged()
            && let (Some(drag), Some(p)) =
                (self.mannequin.drag.clone(), response.interact_pointer_pos())
        {
            self.mannequin_dragged(response, rect, placement, drag, at(p));
        }
        if response.drag_stopped() {
            self.mannequin.drag = None;
        }
        let Some(p) = response.interact_pointer_pos() else {
            return;
        };
        // A double click is two clicks too: the part is picked, then looked at.
        if response.double_clicked() {
            match self.mannequin_part_at(ui, rect, p) {
                Some(joint) => self.look_at_mannequin_part(joint),
                None => self.frame_mannequin(),
            }
        }
        if response.clicked() {
            self.mannequin.selected = self.mannequin_part_at(ui, rect, p);
        } else if response.secondary_clicked() {
            if let Some(joint) = self.mannequin_mark_at(placement, p) {
                self.cycle_mannequin_mark_depth(joint);
            } else if let Some(joint) = self.mannequin_part_at(ui, rect, p) {
                // A right click on a hand, a forearm, a foot or a shin pins its end.
                let end = joint.reach_chain().map_or_else(
                    || {
                        flare_mannequin::Joint::ALL
                            .into_iter()
                            .find(|e| e.reach_chain().is_some_and(|c| c[0] == joint))
                    },
                    |_| Some(joint),
                );
                if let Some(end) = end {
                    self.toggle_mannequin_pin(end);
                }
            }
        }
    }

    fn mannequin_dragged(
        &mut self,
        response: &egui::Response,
        rect: Rect,
        placement: Placement,
        drag: MannequinDrag,
        p: [f64; 2],
    ) {
        let figure = &mut self.mannequin.figure;
        match drag {
            MannequinDrag::Orbit => {
                let d = response.drag_delta();
                figure.orbit(
                    f64::from(d.x) * ORBIT_DEGREES,
                    f64::from(d.y) * ORBIT_DEGREES,
                );
            }
            MannequinDrag::Pan => {
                let d = response.drag_delta();
                let (center, height) = view_frame(rect);
                let view = figure.view;
                self.mannequin
                    .camera
                    .pan(&view, center, height, [f64::from(d.x), f64::from(d.y)]);
            }
            MannequinDrag::Pose(grab) => figure.drag(&grab, p),
            MannequinDrag::Turn { joint, last } => {
                figure.turn(joint, placement, last, p);
                self.mannequin.drag = Some(MannequinDrag::Turn { joint, last: p });
            }
            MannequinDrag::Ring { grab, last } => {
                figure.turn_ring(&grab, [p[0] - last[0], p[1] - last[1]]);
                self.mannequin.drag = Some(MannequinDrag::Ring { grab, last: p });
            }
            MannequinDrag::Mark(joint) => self.drag_mannequin_mark(placement, joint, p),
        }
    }

    /// What a drag from `p` does: a mark on the picture, a ring of the picked joint, then a
    /// part, then the view.
    fn mannequin_drag_for(
        &mut self,
        ui: &Ui,
        rect: Rect,
        placement: Placement,
        p: Pos2,
        alt: bool,
        across: bool,
    ) -> MannequinDrag {
        if across {
            return MannequinDrag::Pan;
        }
        if let Some(joint) = self.mannequin_mark_at(placement, p) {
            return MannequinDrag::Mark(joint);
        }
        let press = at(p);
        let figure = &self.mannequin.figure;
        if let Some(joint) = self.mannequin.selected
            && let Some(grab) = figure.grab_ring(joint, placement, RING_RADIUS, press, RING_REACH)
        {
            return MannequinDrag::Ring { grab, last: press };
        }
        match self.mannequin_part_at(ui, rect, p) {
            Some(joint) => {
                self.mannequin.selected = Some(joint);
                if alt {
                    MannequinDrag::Turn { joint, last: press }
                } else {
                    let grab = figure.grab(joint, placement, press, &self.mannequin.held);
                    MannequinDrag::Pose(Box::new(grab))
                }
            }
            None => MannequinDrag::Orbit,
        }
    }

    fn paint_mannequin_view(&mut self, ui: &Ui, rect: Rect, placement: Placement) {
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 4.0, theme::field());
        let floor = Stroke::new(1.0, theme::text_dim().gamma_multiply(0.35));
        for [a, b] in self.mannequin.figure.floor(placement) {
            painter.line_segment([pos(a), pos(b)], floor);
        }
        self.paint_mannequin_reference(ui, &painter, placement);
        if self.mannequin_picture(ui, rect, placement)
            && let Some(shown) = &self.mannequin.shown
        {
            let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
            painter.image(shown.texture.id(), rect, uv, Color32::WHITE);
        }
        self.paint_mannequin_marks(&painter, placement);
        let figure = &self.mannequin.figure;
        let marks = figure.marks(placement);
        // The pinned hands and feet.
        for joint in &self.mannequin.held {
            let at = pos(marks[joint.index()].at);
            let size = 14.0;
            let spot = Rect::from_center_size(at + vec2(0.0, -size * 0.5), vec2(size, size));
            painter.circle_filled(at, 3.0, theme::accent());
            icons::image("pin", size, theme::accent()).paint_at(ui, spot);
        }
        // The rings of the picked joint, the half toward the viewer bold.
        let Some(joint) = self.mannequin.selected else {
            return;
        };
        for ring in figure.rings(joint, placement, RING_RADIUS) {
            let color = axis_color(ring.axis);
            let n = ring.points.len();
            for i in 0..n {
                let (a, b) = (ring.points[i], ring.points[(i + 1) % n]);
                let stroke = if ring.front[i] {
                    Stroke::new(2.0, color)
                } else {
                    Stroke::new(1.0, color.gamma_multiply(0.35))
                };
                painter.line_segment([pos(a), pos(b)], stroke);
            }
        }
        painter.circle_filled(pos(marks[joint.index()].at), 3.5, theme::accent());
    }

    /// In the view's top right corner: zoom out and in, frame the figure, and a side to see it
    /// square from.
    fn mannequin_view_buttons(&mut self, ui: &mut Ui, view: Rect) {
        let hints = [
            tr!(self, "mannequin-zoom-out"),
            tr!(self, "mannequin-zoom-in"),
            tr!(self, "mannequin-frame"),
            tr!(self, "mannequin-sides"),
        ];
        let sides: Vec<(Facing, String)> = Facing::ALL
            .into_iter()
            .map(|f| (f, self.tr(&format!("mannequin-facing-{}", f.id()))))
            .collect();
        let corner = Rect::from_min_size(
            Pos2::new(view.max.x - 4.0 * 24.0 - 6.0, view.min.y + 4.0),
            vec2(4.0 * 24.0, 22.0),
        );
        let mut chosen = None;
        // Over the view, taking no room from what comes after it.
        let mut ui = ui.new_child(egui::UiBuilder::new().max_rect(corner));
        {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                if chrome::icon_button(ui, "zoom-out", 16.0, false, true, &hints[0]).clicked() {
                    self.zoom_mannequin(1.0 / BUTTON_ZOOM);
                }
                if chrome::icon_button(ui, "zoom-in", 16.0, false, true, &hints[1]).clicked() {
                    self.zoom_mannequin(BUTTON_ZOOM);
                }
                if chrome::icon_button(ui, "zoom-fit", 16.0, false, true, &hints[2]).clicked() {
                    self.frame_mannequin();
                }
                let sides_button = chrome::icon_button(ui, "camera", 16.0, false, true, &hints[3]);
                egui::Popup::menu(&sides_button).show(|ui| {
                    for (facing, name) in &sides {
                        if ui.button(name).clicked() {
                            chosen = Some(*facing);
                            ui.close();
                        }
                    }
                });
            });
        }
        if let Some(facing) = chosen {
            self.face_mannequin(facing);
        }
    }

    /// The grip under the view: dragging it makes the view taller or shorter.
    fn mannequin_view_grip(&mut self, ui: &mut Ui, view: Rect) {
        let (rect, response) =
            ui.allocate_exact_size(vec2(view.width(), GRIP), Sense::click_and_drag());
        let hint = tr!(self, "mannequin-resize");
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Slider, true, &hint));
        let response = response.on_hover_cursor(egui::CursorIcon::ResizeVertical);
        let color = if response.hovered() || response.dragged() {
            theme::text()
        } else {
            theme::text_dim().gamma_multiply(0.6)
        };
        let c = rect.center();
        for dx in [-8.0, 0.0, 8.0] {
            ui.painter().circle_filled(c + vec2(dx, 0.0), 1.5, color);
        }
        if response.dragged() {
            let (low, high) = VIEW_HEIGHT_RANGE;
            let height = (view.height() + response.drag_delta().y).clamp(low, high);
            self.mannequin.view_height = Some(height);
        }
        if response.double_clicked() {
            self.mannequin.view_height = None;
        }
    }
}
