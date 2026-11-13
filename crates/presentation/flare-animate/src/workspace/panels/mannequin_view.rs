//! The Mannequin panel's view of the figure, shaded as a 3D drawing figure, and posing it there
//! as one poses such a figure: a drag on any part pulls it and the body follows, the pinned
//! hands and feet staying put (a right click pins or lets go of one); a click picks a part and
//! shows rings round its joint, and a drag along a ring turns it about that axis alone; with Alt
//! a drag turns a part about the line of sight. A drag anywhere else turns the view round the
//! figure, a right or middle drag moves it across, and scrolling zooms. A grip under the view
//! makes it taller or shorter.

use egui::{Color32, Pos2, Rect, Sense, Stroke, Ui, vec2};
use flare_core::Rgba8;
use flare_mannequin::{Axis, Look, Placement};
use flare_ui::{icons, theme};

use crate::workspace::Workspace;
use crate::workspace::mannequin::{MannequinDrag, VIEW_HEIGHT_RANGE, ZOOM_RANGE, pixels, show};

/// Degrees the view turns for a point of drag.
const ORBIT_DEGREES: f64 = 0.5;
/// How far a joint's rings reach from it, and how near one a press takes it, in points.
pub(crate) const RING_RADIUS: f64 = 30.0;
const RING_REACH: f64 = 6.0;
/// The grip under the view.
const GRIP: f32 = 8.0;

fn at(p: Pos2) -> [f64; 2] {
    [f64::from(p.x), f64::from(p.y)]
}

fn pos(p: [f64; 2]) -> Pos2 {
    Pos2::new(p[0] as f32, p[1] as f32)
}

/// The figure's placement in the view `rect`, in points: its middle at the middle (moved by
/// `pan`), four fifths as tall at a zoom of one.
pub(crate) fn view_placement(rect: Rect, zoom: f32, pan: egui::Vec2) -> Option<Placement> {
    let c = rect.center() + pan;
    Placement::new(
        [f64::from(c.x), f64::from(c.y)],
        f64::from(rect.height() * 0.8 * zoom),
    )
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
        let width = ui.available_width().max(120.0);
        let size = vec2(width, self.mannequin_view_height(ui, width));
        let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
        self.mannequin.view_rect = Some(rect);
        let label = tr!(self, "pane-mannequin");
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Canvas, true, &label));
        if let Some(placement) = view_placement(rect, self.mannequin.zoom, self.mannequin.pan) {
            self.mannequin_input(ui, &response, rect, placement);
        }
        if let Some(placement) = view_placement(rect, self.mannequin.zoom, self.mannequin.pan) {
            self.paint_mannequin_view(ui, rect, placement);
        }
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
        ) else {
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

    fn mannequin_input(
        &mut self,
        ui: &Ui,
        response: &egui::Response,
        rect: Rect,
        placement: Placement,
    ) {
        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                let zoom = self.mannequin.zoom * (scroll * 0.002).exp();
                self.mannequin.zoom = zoom.clamp(ZOOM_RANGE.0, ZOOM_RANGE.1);
            }
        }
        // What a press picks is what was drawn there last.
        self.mannequin_picture(ui, rect, placement);
        let (alt, shift) = ui.input(|i| (i.modifiers.alt, i.modifiers.shift));
        let across = response.dragged_by(egui::PointerButton::Secondary)
            || response.dragged_by(egui::PointerButton::Middle);
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
            let figure = &mut self.mannequin.figure;
            let p = at(p);
            match drag {
                MannequinDrag::Orbit => {
                    let d = response.drag_delta();
                    figure.orbit(
                        f64::from(d.x) * ORBIT_DEGREES,
                        f64::from(d.y) * ORBIT_DEGREES,
                    );
                }
                MannequinDrag::Pan => self.mannequin.pan += response.drag_delta(),
                MannequinDrag::Pose(grab) => figure.drag(&grab, p),
                MannequinDrag::Turn { joint, last } => {
                    figure.turn(joint, placement, last, p);
                    self.mannequin.drag = Some(MannequinDrag::Turn { joint, last: p });
                }
                MannequinDrag::Ring { grab, last } => {
                    figure.turn_ring(&grab, [p[0] - last[0], p[1] - last[1]]);
                    self.mannequin.drag = Some(MannequinDrag::Ring { grab, last: p });
                }
            }
        }
        if response.drag_stopped() {
            self.mannequin.drag = None;
        }
        if let Some(p) = response.interact_pointer_pos() {
            if response.clicked() {
                self.mannequin.selected = self.mannequin_part_at(ui, rect, p);
            } else if response.secondary_clicked()
                && let Some(joint) = self.mannequin_part_at(ui, rect, p)
            {
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

    /// What a drag from `p` does: a ring of the picked joint, then a part, then the view.
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
        if self.mannequin_picture(ui, rect, placement)
            && let Some(shown) = &self.mannequin.shown
        {
            let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
            painter.image(shown.texture.id(), rect, uv, Color32::WHITE);
        }
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
