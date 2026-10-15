//! Stage: rendering, tool input, selection overlays and the stage context menu.

use crate::toolbar::{FtMode, PencilMode, Tool};
use crate::{from32, App, ACCENT};
use ac_core::deform::{self, Transform3D, Warp};
use ac_core::{pt, Color, Editor, Element, ElementKind, Gradient, Matrix, Path, PointRef, Pt, Seg, Selection, Shape, Stroke};
use eframe::egui::{self, pos2, vec2, Color32, CursorIcon, Key, Pos2, Rect, Sense, Stroke as EStroke, Vec2};
use tiny_skia as sk;

pub enum Drag {
    None,
    Move { last: Pt, moved: bool, raw: Pt, applied: Pt, start: Option<(Pt, Pt)>, targets: (Vec<f32>, Vec<f32>) },
    Transform { mode: TMode, applied: Matrix, started: bool },
    /// Free Transform › Distort / Envelope: one handle moves, shapes are re-mapped.
    FtWarp { envelope: bool, handle: usize, lo: Pt, hi: Pt, handles: Vec<Pt>, base: Vec<(Selection, Path)>, started: bool },
    /// Free Transform: move the transformation point.
    Pivot,
    Marquee { start: Pt, cur: Pt, add: bool },
    /// Text tool: dragging out a fixed-width text box.
    TextBox { sel: Selection, start: Pt },
    Draw { start: Pt, cur: Pt },
    Freehand { pts: Vec<Pt> },
    /// Fluid Brush: points with timestamps (speed → width).
    Fluid { pts: Vec<(Pt, f64)> },
    Erase { mark: usize, removed: usize, last: Pt, start: Pt },
    /// Selection tool bending an edge (or dragging a corner) of a merge shape.
    Bend { layer: usize, key: u32, ei: usize, bend: Box<ac_core::merge::Bend>, inv: Matrix, from: Pt, part: Option<ac_core::merge::Part>, started: bool },
    Point { sel: Selection, r: PointRef, started: bool },
    ZoomBox { start: Pos2, cur: Pos2 },
    Pan { last: Pos2 },
    Gizmo3D { kind: Grab3D, last: Pos2, center: Pos2, started: bool },
    Grad { sel: Selection, handle: usize, started: bool },
    BoneDraw { layer: usize, parent: usize, head: Pt, cur: Pt },
    Ik { layer: usize, element: usize, grab: Pt, started: bool },
    WidthPt { sel: Selection, t: f32, started: bool },
    /// Width tool on a merge-shape line: (layer, line, position along it).
    MergeWidth { layer: usize, line: usize, t: f32, started: bool },
    WarpPin { sel: Selection, pin: usize, started: bool },
    CameraPan { last: Pos2, rotate: bool, started: bool },
    /// Free camera (VCam): move the frame, zoom by a corner, rotate by the knob.
    VCam { mode: VCamGrab, from: Pt, cam0: Matrix, started: bool },
    RotView { start: f32, base: f32 },
    /// A guide being dragged out of a ruler.
    GuideNew { vertical: bool, pos: f32 },
    /// An existing guide being moved (drop it on a ruler to remove it).
    Guide { vertical: bool, index: usize, pos: f32, started: bool },
    /// Dragging a shape hint.
    Hint { key: u32, index: usize, started: bool },
    /// Posing a shape armature with the Selection tool.
    RigIk { sel: Selection, bone: usize, started: bool },
    /// Editing a motion tween's path on the stage.
    MotionPath { grab: crate::motion_ui::PathGrab, started: bool },
    /// Magnet / Pinch / Pump / Bender / Iron (OpenToonz deformation tools).
    Sculpt(Box<ac_core::editor::Sculpt>),
    /// Bender: drawing the segment across the lines.
    BenderLine { a: Pt, cur: Pt },
    /// Particle tool: aiming the emitter just placed.
    ParticleAim { origin: Pt, cur: Pt },
    /// Asset Warp pin on a bitmap, symbol or group.
    MeshPin { sel: Selection, pin: usize, started: bool },
    /// Raster Brush stroke (time of the last event, for the brush's speed).
    Paint(Box<ac_core::editor::PaintSession>, f64),
    /// Assistants tool: dragging handle `k` of assistant `i` (`k = None`: the whole assistant).
    AssistPt { i: usize, k: Option<usize>, last: Pt, started: bool },
}

#[derive(Clone, Copy)]
pub enum TMode {
    /// Scale about `anchor`; `ax`/`ay` say which axes follow the mouse.
    Scale { anchor: Pt, start: Pt, ax: bool, ay: bool },
    Rotate { center: Pt, start_angle: f32 },
    /// Drag an edge: horizontal skew (top/bottom edge) or vertical (left/right).
    Skew { horizontal: bool, anchor: Pt, start: Pt },
}

/// What the pointer grabbed on the transform box.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Grab {
    Pivot,
    /// Handle index: [TL, TR, BL, BR, T, B, L, R].
    Scale(usize),
    /// Edge index: 0 top, 1 bottom, 2 left, 3 right.
    Skew(usize),
    Rotate(usize),
}

/// 3D tool gizmo parts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Grab3D {
    X,
    Y,
    Z,
    Free,
    Center,
    TX,
    TY,
    TZ,
}

/// Pen tool path in progress: anchors with their outgoing handle offsets.
#[derive(Default)]
pub struct PenState {
    pub pts: Vec<(Pt, Pt)>,
    pub dragging: bool,
}

/// Inline text editing (Text tool).
pub struct TextEdit {
    pub sel: Selection,
    pub buffer: String,
    pub mark: usize,
    pub focus_requested: bool,
}

const GIZMO_R: f32 = 70.0;

fn handles8(lo: Pt, hi: Pt) -> [Pt; 8] {
    let (mx, my) = ((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0);
    [pt(lo.x, lo.y), pt(hi.x, lo.y), pt(lo.x, hi.y), pt(hi.x, hi.y), pt(mx, lo.y), pt(mx, hi.y), pt(lo.x, my), pt(hi.x, my)]
}

fn seg_dist(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_sq().max(1e-6)).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

/// What a Camera-tool press grabs on the free camera frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VCamGrab {
    Move,
    Zoom,
    Rotate,
}

/// Free camera (VCam): the camera frame's corners in stage space (TL, TR, BR, BL)
/// at `frame`, with follow and shake.
pub(crate) fn vcam_quad(app: &App, frame: u32) -> Option<[Pt; 4]> {
    let cam = app.ed.camera_final(frame)?;
    let inv = ac_core::camera_matrix_to_view(&app.ed.doc, &cam.matrix).invert()?;
    let (w, h) = (app.ed.doc.width, app.ed.doc.height);
    Some([inv.apply(pt(0.0, 0.0)), inv.apply(pt(w, 0.0)), inv.apply(pt(w, h)), inv.apply(pt(0.0, h))])
}

/// Is the stage showing the scene as authored with a free camera frame on it?
pub(crate) fn vcam_frame_shown(app: &App) -> bool {
    app.ed.doc.camera_mode == ac_core::CameraMode::Free && app.ed.editing.is_none() && app.ed.camera_layer().is_some() && !app.ed.camera_view
}

/// Edit in place: one level of symbol/group editing opened from the stage,
/// drawn where it sits with the rest of the scene dimmed behind it.
#[derive(Clone, Copy, Debug)]
pub struct InPlace {
    /// The symbol being edited at this level (the group's temporary symbol for groups).
    pub symbol: ac_core::SymbolId,
    /// Object → stage transform of this level (parents included).
    pub matrix: Matrix,
    /// Scene frame and the scene object opened (outermost level).
    pub scene_frame: u32,
    pub scene_obj: Option<Selection>,
}

impl App {
    /// The in-place transform, if the current editing level was opened in place.
    pub(crate) fn in_place(&self) -> Option<Matrix> {
        let last = self.place.last()?;
        (self.ed.editing == Some(last.symbol)).then_some(last.matrix)
    }

    /// Double-click on the stage: edit a symbol instance or a group in place.
    pub(crate) fn edit_in_place(&mut self, h: Selection) -> bool {
        let Some(el) = self.ed.element_at(h) else { return false };
        let parent = self.in_place().unwrap_or(Matrix::IDENTITY);
        if self.ed.editing.is_some() && self.in_place().is_none() {
            self.place.clear(); // isolated editing: nested levels stay isolated
        }
        let (scene_frame, scene_obj) = match self.place.first() {
            Some(p) => (p.scene_frame, p.scene_obj),
            None if self.ed.editing.is_none() => (self.ed.frame, Some(h)),
            None => (0, None),
        };
        let m = parent.concat(&el.matrix);
        match el.kind {
            ElementKind::Instance(i) => {
                if self.ed.doc.symbol(i.symbol).is_none() {
                    return false;
                }
                self.ed.enter_symbol(i.symbol);
                self.place.push(InPlace { symbol: i.symbol, matrix: m, scene_frame, scene_obj });
                self.status = format!("Editing {} in place. Esc or double-click empty space to go back.", self.ed.doc.symbol(i.symbol).map(|s| s.name.clone()).unwrap_or_default());
            }
            ElementKind::Group(_) => {
                if !self.ed.enter_group(h) {
                    return false;
                }
                let Some(id) = self.ed.editing else { return false };
                self.place.push(InPlace { symbol: id, matrix: m, scene_frame, scene_obj });
                self.status = "Editing group. Esc or double-click empty space to go back.".into();
            }
            _ => return false,
        }
        true
    }

    /// Esc / breadcrumb: leave the innermost group, else go back to the scene.
    pub(crate) fn exit_editing(&mut self, all: bool) {
        if !all && !self.ed.group_edit.is_empty() {
            self.ed.exit_group();
            self.place.pop();
            if self.in_place().is_none() && !self.place.is_empty() {
                self.place.clear();
            }
            return;
        }
        let back = self.place.first().copied();
        self.ed.exit_to_scene();
        self.place.clear();
        match back {
            Some(p) => {
                self.ed.set_frame(p.scene_frame);
                if let Some(s) = p.scene_obj {
                    self.ed.layer = s.layer;
                    self.ed.selection = vec![s];
                }
            }
            None => self.fit_pending = true,
        }
    }

    fn content_offset(&self) -> Vec2 {
        if self.in_place().is_some() {
            Vec2::ZERO
        } else if self.ed.editing.is_some() {
            vec2(self.ed.doc.width / 2.0, self.ed.doc.height / 2.0)
        } else {
            Vec2::ZERO
        }
    }
    /// Stage coordinates → screen points relative to the stage rect (pan, view rotation, zoom).
    fn view_base(&self) -> Matrix {
        Matrix::translate(self.pan.x, self.pan.y).concat(&Matrix::rotate(self.view_rot)).concat(&Matrix::scale(self.zoom, self.zoom))
    }
    fn view_content(&self) -> Matrix {
        self.view_base().concat(&self.content_matrix())
    }
    /// Content (the timeline being edited) → stage coordinates.
    pub(crate) fn content_matrix(&self) -> Matrix {
        let o = self.content_offset();
        let v = Matrix::translate(o.x, o.y);
        match self.in_place() {
            Some(m) => v.concat(&m),
            None => v,
        }
    }
    /// Screen point → stage coordinates (Navigator).
    pub(crate) fn screen_to_stage(&self, rect: Rect, s: Pos2) -> Pt {
        let v = s - rect.min;
        self.view_base().invert().map(|m| m.apply(pt(v.x, v.y))).unwrap_or(pt(v.x, v.y))
    }
    /// Scroll so a stage point sits in the middle of the stage area.
    pub(crate) fn center_on(&mut self, rect: Rect, p: Pt) {
        let q = self.view_base().apply(p);
        self.pan += rect.center() - (rect.min + vec2(q.x, q.y));
    }
    pub(crate) fn to_screen(&self, rect: Rect, p: Pt) -> Pos2 {
        let q = self.view_content().apply(p);
        rect.min + vec2(q.x, q.y)
    }
    pub(crate) fn to_doc(&self, rect: Rect, s: Pos2) -> Pt {
        let v = s - rect.min;
        self.view_content().invert().map(|m| m.apply(pt(v.x, v.y))).unwrap_or(pt(v.x, v.y))
    }
    /// A doc-space box as a screen quad (TL, TR, BR, BL) — it is rotated with the view.
    fn quad(&self, rect: Rect, lo: Pt, hi: Pt) -> Vec<Pos2> {
        [pt(lo.x, lo.y), pt(hi.x, lo.y), pt(hi.x, hi.y), pt(lo.x, hi.y)].iter().map(|p| self.to_screen(rect, *p)).collect()
    }
    pub(crate) fn fit(&mut self, rect: Rect) {
        let d = &self.ed.doc;
        // the stage, or (View › Show All) a given box
        let (lo, hi) = self.fit_bounds.take().unwrap_or((pt(0.0, 0.0), pt(d.width, d.height)));
        let (w, h) = ((hi.x - lo.x).max(1.0), (hi.y - lo.y).max(1.0));
        let (c, s) = (self.view_rot.cos().abs(), self.view_rot.sin().abs());
        let (bw, bh) = (w * c + h * s, w * s + h * c);
        self.zoom = ((rect.width() - 60.0) / bw).min((rect.height() - 60.0) / bh).clamp(0.05, 8.0);
        self.pan = Vec2::ZERO;
        let o = self.content_offset();
        let center = pt(lo.x + w / 2.0 - o.x, lo.y + h / 2.0 - o.y);
        let now = self.to_screen(rect, center);
        self.pan += rect.center() - now;
    }
    pub(crate) fn zoom_at(&mut self, rect: Rect, anchor: Pos2, factor: f32) {
        let new = (self.zoom * factor).clamp(0.05, 32.0);
        let a = anchor - rect.min;
        self.pan = a - (a - self.pan) * (new / self.zoom);
        self.zoom = new;
    }
    /// Rotate the view about the centre of the stage area (Rotation tool).
    pub(crate) fn set_view_rot(&mut self, rect: Rect, rad: f32) {
        let c = self.to_doc(rect, rect.center());
        self.view_rot = rad;
        let now = self.to_screen(rect, c);
        self.pan += rect.center() - now;
    }
    fn cur_fill(&self) -> Option<Color> {
        self.fill_on.then(|| from32(self.fill))
    }
    fn cur_stroke(&self) -> Option<Stroke> {
        self.stroke_on.then(|| Stroke { color: from32(self.stroke), width: self.stroke_w })
    }
    /// The current gradient's stops: a gradient swatch's own when its ends are
    /// still the fill and second colour, else fill → second colour.
    pub(crate) fn fill_stops(&self) -> Vec<(f32, Color)> {
        let (a, b) = (from32(self.fill), from32(self.topts.fill2));
        match &self.topts.grad_stops {
            Some(s) if s.len() >= 2 && s[0].1 == a && s[s.len() - 1].1 == b => s.clone(),
            _ => vec![(0.0, a), (1.0, b)],
        }
    }
    /// A new shape with the current fill type (solid, or a gradient from fill → second colour).
    pub(crate) fn make_shape(&self, path: Path, fill: Option<Color>, stroke: Option<Stroke>) -> Shape {
        let mut s = Shape::new(path, fill, stroke);
        if let (Some(k), Some(f)) = (self.topts.fill_gradient, fill) {
            // Lock Fill: one gradient across the stage
            let b = if self.topts.lock_fill { Some((pt(0.0, 0.0), pt(self.ed.doc.width, self.ed.doc.height))) } else { s.path.bounds() };
            if let Some((lo, hi)) = b {
                let mut stops = self.fill_stops();
                stops[0].1 = f;
                s.gradient = Some(Gradient::fit(k, stops, lo, hi));
            }
        }
        s
    }
    /// Put a newly drawn shape on the stage: a drawing object (Object
    /// Drawing on) or into the layer's merge shape.
    pub(crate) fn place_shape(&mut self, name: &str, shape: Shape) -> bool {
        if self.topts.object_draw || shape.width_profile.is_some() {
            return self.ed.add_element(Element::shape(name, shape)).is_some();
        }
        self.ed.merge_draw(&shape.path, shape.fill, shape.gradient, shape.stroke)
    }
    fn locked_msg(&mut self) {
        self.status = "The current layer is locked or hidden. Pick another layer in the timeline.".into();
    }

    /// Free Transform's transformation point (defaults to the selection centre).
    pub(crate) fn pivot(&self) -> Option<Pt> {
        if let Some((sel, p)) = &self.ft_pivot {
            if *sel == self.ed.selection {
                return Some(*p);
            }
        }
        let (lo, hi) = self.ed.selection_bounds()?;
        Some(pt((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0))
    }

    fn grab_at(&self, rect: Rect, pos: Pos2, tool: Tool) -> Option<Grab> {
        let (lo, hi) = self.ed.selection_bounds()?;
        let ft = tool == Tool::FreeTransform;
        if ft {
            if let Some(pv) = self.pivot() {
                if self.to_screen(rect, pv).distance(pos) < 7.0 {
                    return Some(Grab::Pivot);
                }
            }
        }
        let hs = handles8(lo, hi);
        let n = if ft { 8 } else { 4 };
        let mut best: Option<(usize, f32)> = None;
        for (i, h) in hs.iter().enumerate().take(n) {
            let d = self.to_screen(rect, *h).distance(pos);
            if d < 7.0 && best.map(|b| d < b.1).unwrap_or(true) {
                best = Some((i, d));
            }
        }
        if let Some((i, _)) = best {
            return Some(Grab::Scale(i));
        }
        if !ft {
            return None;
        }
        let q = self.quad(rect, lo, hi); // TL TR BR BL
        for (e, (a, b)) in [(q[0], q[1]), (q[3], q[2]), (q[0], q[3]), (q[1], q[2])].into_iter().enumerate() {
            if seg_dist(pos, a, b) < 5.0 {
                return Some(Grab::Skew(e));
            }
        }
        let inside = {
            let p = self.to_doc(rect, pos);
            p.x >= lo.x && p.x <= hi.x && p.y >= lo.y && p.y <= hi.y
        };
        if !inside {
            for (i, h) in hs.iter().enumerate().take(4) {
                if self.to_screen(rect, *h).distance(pos) < 24.0 {
                    return Some(Grab::Rotate(i));
                }
            }
        }
        None
    }

    /// Distort/Envelope handles of the current selection.
    fn warp_handles(&self) -> Option<(Pt, Pt, Vec<Pt>)> {
        let (lo, hi) = self.ed.selection_bounds()?;
        let hs = match self.topts.ft_mode {
            FtMode::Distort => vec![pt(lo.x, lo.y), pt(hi.x, lo.y), pt(hi.x, hi.y), pt(lo.x, hi.y)],
            FtMode::Envelope => deform::envelope_grid(lo, hi).to_vec(),
            FtMode::Normal => return None,
        };
        Some((lo, hi, hs))
    }

    /// 3D gizmo centre on screen for the first selected object.
    fn gizmo_center(&self, rect: Rect) -> Option<Pos2> {
        let s = *self.ed.selection.first()?;
        let el = self.ed.element_at(s)?;
        let c = match el.three_d {
            Some(t) => {
                let cw = el.matrix.apply(t.center);
                deform::project_3d(&t, cw, &self.ed.doc.perspective(), cw)
            }
            None => {
                let (lo, hi) = self.ed.bounds_of(&[s])?;
                pt((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0)
            }
        };
        Some(self.to_screen(rect, c))
    }

    fn grab3d_at(&self, rect: Rect, pos: Pos2) -> Option<(Grab3D, Pos2)> {
        let c = self.gizmo_center(rect)?;
        let d = pos - c;
        let dc = d.length();
        let r = GIZMO_R;
        let g = if self.tool == Tool::Rotate3D {
            if dc < 7.0 {
                Grab3D::Center
            } else if d.x.abs() < 5.0 && d.y.abs() <= r {
                Grab3D::X
            } else if d.y.abs() < 5.0 && d.x.abs() <= r {
                Grab3D::Y
            } else if (dc - r).abs() < 6.0 {
                Grab3D::Z
            } else if (dc - r - 14.0).abs() < 6.0 {
                Grab3D::Free
            } else {
                return None;
            }
        } else if dc < 8.0 {
            Grab3D::TZ
        } else if d.x > 0.0 && d.x < r + 10.0 && d.y.abs() < 6.0 {
            Grab3D::TX
        } else if d.y < 0.0 && -d.y < r + 10.0 && d.x.abs() < 6.0 {
            Grab3D::TY
        } else {
            return None;
        };
        Some((g, c))
    }

    /// Give every selected object a 3D centre (its local bounds centre) before a 3D edit.
    pub(crate) fn ensure_3d(&mut self) {
        let sels = self.ed.selection.clone();
        for s in &sels {
            let Some(el) = self.ed.element_at(*s) else { continue };
            if el.three_d.is_some() {
                continue;
            }
            let Some((lo, hi)) = self.ed.bounds_of(&[*s]) else { continue };
            let Some(inv) = el.matrix.invert() else { continue };
            let c = inv.apply(pt((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0));
            self.ed.selection = vec![*s];
            self.ed.modify_selected_continuous(|e| e.three_d = Some(Transform3D { center: c, ..Default::default() }));
        }
        self.ed.selection = sels;
    }

    /// A selected (or clicked) shape and the doc→local transform for point editing.
    fn shape_for_points(&mut self, p: Pt, select_hit: bool) -> Option<(Selection, Matrix, Path)> {
        let candidates: Vec<Selection> = self.ed.selection.clone();
        for s in candidates {
            if let Some(el) = self.ed.element_at(s) {
                if let ElementKind::Shape(sh) = &el.kind {
                    let inv = el.matrix.invert()?;
                    let tol = 8.0 / self.zoom;
                    let lp = inv.apply(p);
                    if sh.path.pick_point(lp, tol / el.matrix.decompose().scale_x.abs().max(0.01), true).is_some() || sh.path.hit(lp, true, tol) {
                        return Some((s, inv, sh.path.clone()));
                    }
                }
            }
        }
        if !select_hit {
            return None;
        }
        let hit = self.ed.hit_test(p)?;
        let el = self.ed.element_at(hit)?;
        match &el.kind {
            ElementKind::Shape(sh) => {
                self.ed.select(Some(hit), false);
                Some((hit, el.matrix.invert()?, sh.path.clone()))
            }
            _ => {
                self.ed.select(Some(hit), false);
                self.status = "This tool works on shapes. Break apart symbols/text first (Ctrl+B).".into();
                None
            }
        }
    }

    pub(crate) fn finish_pen(&mut self, close: bool) {
        let Some(pen) = self.pen.take() else { return };
        if pen.pts.len() < 2 {
            return;
        }
        let mut segs = vec![Seg::M(pen.pts[0].0)];
        let seg = |(a, ha): (Pt, Pt), (b, hb): (Pt, Pt)| {
            if ha.x.abs() + ha.y.abs() + hb.x.abs() + hb.y.abs() < 0.01 {
                Seg::L(b)
            } else {
                Seg::C(pt(a.x + ha.x, a.y + ha.y), pt(b.x - hb.x, b.y - hb.y), b)
            }
        };
        for w in pen.pts.windows(2) {
            segs.push(seg(w[0], w[1]));
        }
        if close {
            segs.push(seg(*pen.pts.last().unwrap(), pen.pts[0]));
            segs.push(Seg::Z);
        }
        let fill = if close { self.cur_fill() } else { None };
        let stroke = self.cur_stroke().or(if fill.is_none() { Some(Stroke { color: from32(self.stroke), width: self.stroke_w }) } else { None });
        let shape = self.make_shape(Path { segs }, fill, stroke);
        if !self.place_shape("Pen Path", shape) {
            self.locked_msg();
        }
    }

    pub(crate) fn finish_text(&mut self) {
        let Some(te) = self.text_edit.take() else { return };
        if te.buffer.trim().is_empty() {
            self.ed.selection = vec![te.sel];
            self.ed.delete_selected();
        }
        self.ed.squash_undo(te.mark, "Text");
    }

    fn snap_targets(&self) -> (Vec<f32>, Vec<f32>) {
        let mut xs = Vec::new();
        let mut ys = Vec::new();
        if self.ed.editing.is_none() {
            let (w, h) = (self.ed.doc.width, self.ed.doc.height);
            xs.extend([0.0, w / 2.0, w]);
            ys.extend([0.0, h / 2.0, h]);
        } else {
            xs.push(0.0);
            ys.push(0.0);
        }
        for it in self.ed.evaluate_current(self.ed.frame) {
            let Some((l, e)) = it.src else { continue };
            if self.ed.is_selected(Selection { layer: l, element: e }) {
                continue;
            }
            if let Some((lo, hi)) = it.path.transformed(&it.matrix).bounds() {
                xs.extend([lo.x, (lo.x + hi.x) / 2.0, hi.x]);
                ys.extend([lo.y, (lo.y + hi.y) / 2.0, hi.y]);
            }
        }
        (xs, ys)
    }

    /// World-space joints of every bone on visible layers: (layer, bone index, head, tail).
    pub(crate) fn bone_segments(&self) -> Vec<(usize, usize, Pt, Pt)> {
        let mut out = Vec::new();
        let f = self.ed.frame;
        for (li, l) in self.ed.timeline().layers.iter().enumerate() {
            if l.bones.is_empty() || !l.visible {
                continue;
            }
            let Some((els, _)) = ac_core::layer_elements_at(l, f) else { continue };
            for (bi, b) in l.bones.iter().enumerate() {
                if let Some(pe) = els.get(b.parent) {
                    out.push((li, bi, pe.matrix.apply(b.head), pe.matrix.apply(b.tail_parent)));
                }
            }
        }
        out
    }
}

fn snap_axis(vals: [f32; 3], targets: &[f32], thr: f32) -> (f32, Option<f32>) {
    let mut best: Option<(f32, f32)> = None; // (adjust, target)
    for v in vals {
        for t in targets {
            let d = t - v;
            if d.abs() <= thr && best.map(|b| d.abs() < b.0.abs()).unwrap_or(true) {
                best = Some((d, *t));
            }
        }
    }
    best.map(|(d, t)| (d, Some(t))).unwrap_or((0.0, None))
}

fn point_in_poly(poly: &[Pt], p: Pt) -> bool {
    let mut inside = false;
    let n = poly.len();
    if n < 3 {
        return false;
    }
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (poly[i], poly[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Select editable objects whose bounds lie entirely inside a lasso polygon.
fn lasso_select(app: &mut App, poly: &[Pt], add: bool) {
    let mut found = Vec::new();
    for it in app.ed.evaluate_current(app.ed.frame) {
        let Some((l, e)) = it.src else { continue };
        if !app.ed.layer_editable(l) {
            continue;
        }
        let s = Selection { layer: l, element: e };
        if found.contains(&s) {
            continue;
        }
        if let Some((lo, hi)) = app.ed.bounds_of(&[s]) {
            if [pt(lo.x, lo.y), pt(hi.x, lo.y), pt(lo.x, hi.y), pt(hi.x, hi.y)].iter().all(|c| point_in_poly(poly, *c)) {
                found.push(s);
            }
        }
    }
    if !add {
        app.ed.deselect();
    }
    found.retain(|s| !matches!(app.ed.element_at(*s).map(|e| e.kind), Some(ElementKind::Canvas(_))));
    for s in found {
        if !app.ed.is_selected(s) {
            app.ed.selection.push(s);
        }
    }
    app.ed.merge_select_area(&[poly.to_vec()], true);
    app.ed.selection.sort();
    app.ed.frame_sel = None;
    app.status = format!("{} object(s) selected", app.ed.selection.len());
}

/// Paint Brush: turn a smoothed centre line into a filled outline.
fn brush_outline(pts: &[Pt], size: f32, smooth: f32, shape: ac_core::merge::BrushShape) -> Option<Path> {
    if shape != ac_core::merge::BrushShape::Round {
        let centre: Vec<Pt> = if pts.len() == 1 {
            pts.to_vec()
        } else {
            Path::smooth_polyline(pts, 0.3 + smooth / 100.0 * 3.0).flatten(8).into_iter().flat_map(|(c, _)| c).collect()
        };
        let polys = ac_core::merge::nib_outline(&centre, &shape.nib(size.max(0.5)));
        return (!polys.is_empty()).then(|| ac_core::merge::polys_path(&polys));
    }
    if pts.len() == 1 {
        return Some(Path::ellipse(pts[0].x, pts[0].y, size / 2.0, size / 2.0));
    }
    let center = Path::smooth_polyline(pts, 0.3 + smooth / 100.0 * 3.0);
    let mut b = sk::PathBuilder::new();
    for s in &center.segs {
        match *s {
            Seg::M(p) => b.move_to(p.x, p.y),
            Seg::L(p) => b.line_to(p.x, p.y),
            Seg::Q(c, p) => b.quad_to(c.x, c.y, p.x, p.y),
            Seg::C(c1, c2, p) => b.cubic_to(c1.x, c1.y, c2.x, c2.y, p.x, p.y),
            Seg::Z => b.close(),
        }
    }
    let path = b.finish()?;
    let st = sk::Stroke { width: size.max(0.5), line_cap: sk::LineCap::Round, line_join: sk::LineJoin::Round, ..Default::default() };
    let outline = path.stroke(&st, 1.0)?;
    let mut segs = Vec::new();
    for seg in outline.segments() {
        segs.push(match seg {
            sk::PathSegment::MoveTo(p) => Seg::M(pt(p.x, p.y)),
            sk::PathSegment::LineTo(p) => Seg::L(pt(p.x, p.y)),
            sk::PathSegment::QuadTo(c, p) => Seg::Q(pt(c.x, c.y), pt(p.x, p.y)),
            sk::PathSegment::CubicTo(c1, c2, p) => Seg::C(pt(c1.x, c1.y), pt(c2.x, c2.y), pt(p.x, p.y)),
            sk::PathSegment::Close => Seg::Z,
        });
    }
    Some(Path { segs })
}

/// Fluid Brush widths: slow strokes are thick, fast strokes thin.
fn fluid_widths(pts: &[(Pt, f64)], size: f32, sensitivity: f32, zoom: f32) -> Vec<f32> {
    let n = pts.len();
    let raw: Vec<f32> = (0..n)
        .map(|i| {
            let (a, b) = (pts[i.saturating_sub(1)], pts[(i + 1).min(n - 1)]);
            let dt = (b.1 - a.1).max(1e-3) as f32;
            let speed = a.0.dist(b.0) * zoom / dt; // screen px / s
            let k = (speed / 1800.0).min(1.0) * sensitivity / 100.0;
            size * (1.0 - 0.85 * k)
        })
        .collect();
    // smooth, then taper the ends a little like a real brush
    (0..n)
        .map(|i| {
            let lo = i.saturating_sub(3);
            let hi = (i + 3).min(n - 1);
            let avg = raw[lo..=hi].iter().sum::<f32>() / (hi - lo + 1) as f32;
            let end = (i.min(n - 1 - i) as f32 / 4.0).min(1.0);
            avg * (0.45 + 0.55 * end)
        })
        .collect()
}

/// Oval with Animate's options: start/end angle (degrees, clockwise from 3 o'clock) and inner radius (0–99%).
pub(crate) fn oval_path(cx: f32, cy: f32, rx: f32, ry: f32, start: f32, end: f32, inner: f32) -> Path {
    let mut sweep = (end - start).rem_euclid(360.0);
    let full = sweep < 0.01;
    if full {
        sweep = 360.0;
    }
    let inner = (inner / 100.0).clamp(0.0, 0.99);
    if full && inner <= 0.0 {
        return Path::ellipse(cx, cy, rx, ry);
    }
    let arc = |k: f32, a0: f32, sweep: f32| -> Vec<Pt> {
        let n = ((sweep / 4.0).ceil() as usize).max(2);
        (0..=n).map(|i| { let a = (a0 + sweep * i as f32 / n as f32).to_radians(); pt(cx + rx * k * a.cos(), cy + ry * k * a.sin()) }).collect()
    };
    if full {
        let mut outer = arc(1.0, start, 360.0);
        outer.pop();
        let mut inn = arc(inner, start, 360.0);
        inn.pop();
        inn.reverse();
        let mut p = Path::polygon(&outer);
        p.segs.extend(Path::polygon(&inn).segs);
        return p;
    }
    let mut pts = arc(1.0, start, sweep);
    if inner > 0.0 {
        let mut i = arc(inner, start, sweep);
        i.reverse();
        pts.extend(i);
    } else {
        pts.push(pt(cx, cy));
    }
    Path::polygon(&pts)
}

pub fn stage(app: &mut App, ctx: &egui::Context) {
    let free = app.ed.doc.camera_mode == ac_core::CameraMode::Free;
    // Stage camera: look through it with the Camera tool and while playing.
    // Free camera (VCam): the scene as authored with the camera frame on it;
    // look through it while playing or with "Preview through camera".
    let through = if free { app.playing || app.vcam_preview } else { app.playing || app.tool == Tool::Camera };
    app.ed.camera_view = app.ed.editing.is_none() && through && app.ed.camera_layer().is_some();
    egui::CentralPanel::default().frame(egui::Frame::none().fill(Color32::from_gray(0x3C))).show(ctx, |ui| {
        egui::Frame::none().fill(Color32::from_gray(0x2E)).inner_margin(egui::Margin::symmetric(8.0, 4.0)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let crumbs = app.ed.breadcrumb();
                // Edit Scene: switch scenes from the stage header (as Animate's clapper menu)
                if app.ed.doc.scene_count() > 1 {
                    ui.menu_button("Scenes", |ui| {
                        let names: Vec<String> = app.ed.doc.scenes().map(|t| t.name.clone()).collect();
                        for (i, n) in names.iter().enumerate() {
                            if ui.selectable_label(i == app.ed.scene, n).clicked() {
                                app.ed.goto_scene(i);
                                app.fit_pending = true;
                                ui.close_menu();
                            }
                        }
                        ui.separator();
                        if ui.button("Scene panel…").clicked() {
                            app.scene_open = true;
                            ui.close_menu();
                        }
                    });
                }
                for (i, c) in crumbs.iter().enumerate() {
                    if i > 0 {
                        ui.label("›");
                    }
                    let icon = if i == 0 { "🎬" } else { "🔷" };
                    if ui.link(format!("{icon} {c}")).clicked() && i == 0 && app.ed.editing.is_some() {
                        app.exit_editing(true);
                    }
                }
                if app.ed.camera_view {
                    ui.label(egui::RichText::new("🎥 Camera view").color(Color32::from_rgb(0xFF, 0xA0, 0x30)));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.toggle_value(&mut app.view.clip, "Clip").on_hover_text("Clip content outside the stage (Ctrl+Shift+W)");
                    ui.toggle_value(&mut app.view.rulers, "Rulers").on_hover_text("Rulers (Ctrl+Alt+Shift+R)");
                    ui.separator();
                    zoom_menu(app, ui);
                    if app.view_rot.abs() > 1e-4
                        && ui.small_button(format!("⟲ {:.0}°", app.view_rot.to_degrees())).on_hover_text("View rotated (Rotation tool). Click to reset").clicked() {
                            let r = app.stage_rect;
                            app.set_view_rot(r, 0.0);
                        }
                });
            });
        });
        let (full, _) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
        let (rul, rect) = crate::rulers::layout(app, full);
        let resp = ui.interact(rect, ui.id().with("stage-area"), Sense::click_and_drag());
        app.stage_rect = rect;
        if app.fit_pending {
            app.fit(rect);
            app.fit_pending = false;
        }
        if resp.clicked() || resp.drag_started() || resp.secondary_clicked() {
            if app.text_edit.is_some() && !matches!(app.tool, Tool::Text) {
                app.finish_text();
            }
            if let Some(id) = ui.ctx().memory(|m| m.focused()) {
                if app.text_edit.is_none() {
                    ui.ctx().memory_mut(|m| m.surrender_focus(id));
                }
            }
        }
        if resp.hovered() {
            let space = ui.input(|i| i.key_down(Key::Space));
            let hover = resp.hover_pos();
            let grab_tool = app.tool == Tool::Select || (app.tool == Tool::FreeTransform && app.topts.ft_mode == FtMode::Normal);
            let grab = hover.filter(|_| grab_tool).and_then(|h| app.grab_at(rect, h, app.tool));
            let icon = match if space { Tool::Hand } else { app.tool } {
                Tool::Hand | Tool::Camera => if resp.dragged() { CursorIcon::Grabbing } else { CursorIcon::Grab },
                Tool::Zoom => if ui.input(|i| i.modifiers.alt) { CursorIcon::ZoomOut } else { CursorIcon::ZoomIn },
                Tool::Text => CursorIcon::Text,
                Tool::Select | Tool::FreeTransform => match grab {
                    Some(Grab::Scale(i)) => match i { 0 | 3 => CursorIcon::ResizeNwSe, 1 | 2 => CursorIcon::ResizeNeSw, 4 | 5 => CursorIcon::ResizeVertical, _ => CursorIcon::ResizeHorizontal },
                    Some(Grab::Skew(e)) => if e < 2 { CursorIcon::ResizeColumn } else { CursorIcon::ResizeRow },
                    Some(Grab::Rotate(_)) => CursorIcon::Alias,
                    Some(Grab::Pivot) => CursorIcon::Move,
                    None => CursorIcon::Default,
                },
                Tool::Rotate3D | Tool::Translate3D | Tool::GradientTransform | Tool::RotateView => CursorIcon::Default,
                Tool::Bone | Tool::AssetWarp | Tool::Width | Tool::Pinch | Tool::Pump => CursorIcon::PointingHand,
                t if !t.usable() => CursorIcon::NotAllowed,
                _ => CursorIcon::Crosshair,
            };
            ui.ctx().set_cursor_icon(icon);
        }
        if let Some((top, left)) = rul {
            crate::rulers::rulers(app, ui, top, left, rect);
        }
        if resp.hovered() && matches!(app.tool, Tool::Select | Tool::FreeTransform) {
            if let Some((v, _)) = resp.hover_pos().and_then(|h| app.guide_at(rect, h)).filter(|_| matches!(app.drag, Drag::None)) {
                ui.ctx().set_cursor_icon(if v { CursorIcon::ResizeHorizontal } else { CursorIcon::ResizeVertical });
            }
        }
        stage_input(app, ui, rect, &resp);
        resp.context_menu(|ui| app.object_menu(ui));
        render_stage(app, ui, rect);
        overlays(app, ui, rect);
        text_editor(app, ui, rect);
    });
}

fn stage_input(app: &mut App, ui: &egui::Ui, rect: Rect, resp: &egui::Response) {
    if let Some(hover) = resp.hover_pos() {
        let scroll = ui.input(|i| i.raw_scroll_delta.y);
        if scroll != 0.0 {
            if app.tool == Tool::Camera && app.ed.camera_layer().is_some() && !app.playing && (app.ed.doc.camera_mode == ac_core::CameraMode::Stage || app.ed.camera_view || vcam_quad(app, app.ed.frame).is_some_and(|q| ac_core::geom::point_in_polygon(app.to_doc(rect, hover), &q))) {
                // Camera tool: scroll zooms the camera, not the view (free camera: over its frame).
                let cam = app.ed.camera_at(app.ed.frame).unwrap_or_else(|| ac_core::default_camera(&app.ed.doc).matrix);
                let (c, rot, z) = Editor::camera_params(&cam);
                let nz = (z * if scroll > 0.0 { 1.1 } else { 1.0 / 1.1 }).clamp(0.05, 50.0);
                app.ed.begin_edit("Camera Zoom");
                app.ed.set_camera(Editor::camera_from(c, rot, nz));
                app.status = format!("Camera zoom {:.0}%", nz * 100.0);
            } else {
                app.zoom_at(rect, hover, if scroll > 0.0 { 1.1 } else { 1.0 / 1.1 });
            }
        }
    }
    let (space, shift, alt) = ui.input(|i| (i.key_down(Key::Space), i.modifiers.shift, i.modifiers.alt));
    app.cmd_down = ui.input(|i| i.modifiers.command);
    let tool = if space { Tool::Hand } else { app.tool };

    // Pen: Enter finishes an open path, Escape too.
    if app.pen.is_some() && ui.input(|i| i.key_pressed(Key::Enter) || i.key_pressed(Key::Escape)) {
        app.finish_pen(false);
    }
    if !app.poly_lasso.is_empty() && ui.input(|i| i.key_pressed(Key::Escape)) {
        app.poly_lasso.clear();
    }

    // Enable Simple Buttons: buttons show Over/Down and take the clicks.
    if app.simple_buttons && app.ed.editing.is_none() && matches!(app.drag, Drag::None) {
        let down = ui.input(|i| i.pointer.primary_down());
        let hit = resp.hover_pos().and_then(|h| app.ed.button_at(app.to_doc(rect, h)));
        app.ed.button_state = hit.map(|s| (s, if down { 2 } else { 1 }));
        if hit.is_some() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            return;
        }
    } else if !app.simple_buttons {
        app.ed.button_state = None;
    }
    let Some(pos) = resp.interact_pointer_pos() else { return };
    let p = app.to_doc(rect, pos);
    let origin = if resp.drag_started() { ui.input(|i| i.pointer.press_origin()).unwrap_or(pos) } else { pos };
    let p0 = app.to_doc(rect, origin);

    if resp.secondary_clicked() {
        let hit = app.ed.hit_test(p);
        if hit.map(|h| !app.ed.is_selected(h)).unwrap_or(false) {
            app.ed.select(hit, false);
        }
        return;
    }
    if resp.double_clicked() {
        match tool {
            Tool::Select | Tool::FreeTransform => {
                // merge shape: a fill with its outline, or a line with the lines joined to it
                if let Some((li, Some(part), _)) = app.ed.merge_part_at(p, 1.0 / app.zoom) {
                    app.ed.merge_select(li, part, true, shift);
                    app.drag = Drag::None;
                    return;
                }
                if let Some(h) = app.ed.hit_test(p) {
                    match app.ed.element_at(h).map(|e| e.kind) {
                        Some(ElementKind::Instance(_) | ElementKind::Group(_)) => {
                            if app.edit_in_place(h) {
                                return;
                            }
                        }
                        Some(ElementKind::Text(t)) => {
                            app.set_tool(Tool::Text);
                            app.ed.select(Some(h), false);
                            app.text_edit = Some(TextEdit { sel: h, buffer: t.text, mark: app.ed.undo_depth(), focus_requested: false });
                            return;
                        }
                        _ => {}
                    }
                } else if app.in_place().is_some() || !app.ed.group_edit.is_empty() {
                    // double-click outside the edited object: back one level
                    app.exit_editing(false);
                    app.drag = Drag::None;
                    return;
                }
            }
            Tool::Pen => {
                // the double-click's second press added a duplicate point
                if let Some(pen) = &mut app.pen {
                    if pen.pts.len() >= 2 && pen.pts[pen.pts.len() - 1].0.dist(pen.pts[pen.pts.len() - 2].0) < 3.0 / app.zoom {
                        pen.pts.pop();
                    }
                }
                app.finish_pen(false);
                return;
            }
            Tool::PolyLasso => {
                let poly = std::mem::take(&mut app.poly_lasso);
                lasso_select(app, &poly, shift);
                return;
            }
            Tool::RotateView => {
                app.set_view_rot(rect, 0.0);
                return;
            }
            _ => {}
        }
    }

    // Shift and Trace › Edit Shift: drags move/turn a ghost instead of using the tool
    if app.onion_opts.shift_trace && app.onion_opts.shift_edit {
        if resp.drag_started() {
            app.onion_opts.shift_pivot = Some(p0);
        }
        if resp.dragged() {
            let k = usize::from(shift);
            let prev = app.to_doc(rect, pos - resp.drag_delta());
            let pivot = app.onion_opts.shift_pivot.unwrap_or(p0);
            let m = &mut app.onion_opts.shift[k];
            if alt {
                // turn about where the drag started
                let c = pivot;
                let a = (p.y - c.y).atan2(p.x - c.x) - (prev.y - c.y).atan2(prev.x - c.x);
                *m = Matrix::translate(c.x, c.y).concat(&Matrix::rotate(a)).concat(&Matrix::translate(-c.x, -c.y)).concat(m);
            } else {
                *m = Matrix::translate(p.x - prev.x, p.y - prev.y).concat(m);
            }
        }
        return;
    }
    let pressed = resp.drag_started() || (resp.clicked() && matches!(app.drag, Drag::None));
    if pressed {
        app.playing = false;
        app.ed.frame_sel = None;
        if !tool.usable() {
            app.status = format!("{} is planned: {}", tool.info().name, tool.info().planned.unwrap_or(""));
            return;
        }
        press(app, rect, tool, origin, p0, shift, alt);
    }

    if resp.dragged() {
        drag(app, ui, rect, pos, p, shift, alt);
    }

    let released = resp.drag_stopped() || (resp.clicked() && !matches!(app.drag, Drag::Pan { .. }));
    if released {
        release(app, rect, p, shift, alt);
    }
}

fn press(app: &mut App, rect: Rect, tool: Tool, origin: Pos2, p0: Pt, shift: bool, alt: bool) {
    match tool {
        Tool::Hand => app.drag = Drag::Pan { last: origin },
        Tool::Zoom => app.drag = Drag::ZoomBox { start: origin, cur: origin },
        Tool::RotateView => {
            let c = rect.center();
            app.drag = Drag::RotView { start: (origin.y - c.y).atan2(origin.x - c.x), base: app.view_rot };
        }
        Tool::FreeTransform if app.topts.ft_mode != FtMode::Normal && !app.ed.selection.is_empty() => {
            if let Some((lo, hi, hs)) = app.warp_handles() {
                if let Some(i) = hs.iter().position(|h| app.to_screen(rect, *h).distance(origin) < 8.0) {
                    let mut base = Vec::new();
                    let mut skipped = 0;
                    for s in app.ed.selection.clone() {
                        match app.ed.element_at(s) {
                            Some(Element { kind: ElementKind::Shape(sh), matrix, .. }) if app.ed.layer_editable(s.layer) => base.push((s, sh.path.transformed(&matrix))),
                            _ => skipped += 1,
                        }
                    }
                    if skipped > 0 {
                        app.status = format!("{} works on shapes: {skipped} symbol/text object(s) skipped (Break Apart first).", if app.topts.ft_mode == FtMode::Distort { "Distort" } else { "Envelope" });
                    }
                    if !base.is_empty() {
                        app.drag = Drag::FtWarp { envelope: app.topts.ft_mode == FtMode::Envelope, handle: i, lo, hi, handles: hs, base, started: false };
                    }
                    return;
                }
            }
            select_press(app, p0, shift);
        }
        Tool::Select | Tool::FreeTransform => {
            // Shape bones first (they sit inside the selection box): drag one to pose the shape.
            if app.tool == Tool::Select && app.topts.show_bones {
                if let Some((sel, bone, _, _)) = app.ed.rig_bones().into_iter().rev().find(|(s, _, h, t)| app.ed.layer_editable(s.layer) && seg_dist(origin, app.to_screen(rect, *h), app.to_screen(rect, *t)) < 5.0) {
                    app.drag = Drag::RigIk { sel, bone, started: false };
                    return;
                }
            }
            if let Some(g) = app.grab_at(rect, origin, tool) {
                let (lo, hi) = app.ed.selection_bounds().unwrap();
                let pv = app.pivot().unwrap_or(pt((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0));
                let hs = handles8(lo, hi);
                let mode = match g {
                    Grab::Pivot => {
                        app.drag = Drag::Pivot;
                        return;
                    }
                    Grab::Rotate(_) => TMode::Rotate { center: pv, start_angle: (p0.y - pv.y).atan2(p0.x - pv.x) },
                    Grab::Scale(i) => {
                        let opposite = [3, 2, 1, 0, 5, 4, 7, 6][i];
                        let anchor = if alt { pv } else { hs[opposite] };
                        TMode::Scale { anchor, start: hs[i], ax: !(4..6).contains(&i), ay: i < 6 }
                    }
                    Grab::Skew(e) => {
                        let horizontal = e < 2;
                        let anchor = if alt { pv } else { [pt(lo.x, hi.y), pt(lo.x, lo.y), pt(hi.x, lo.y), pt(lo.x, lo.y)][e] };
                        TMode::Skew { horizontal, anchor, start: p0 }
                    }
                };
                app.drag = Drag::Transform { mode, applied: Matrix::IDENTITY, started: false };
                return;
            }
            if let Some((vertical, index)) = app.guide_at(rect, origin) {
                let pos = if vertical { app.ed.doc.guides.x[index] } else { app.ed.doc.guides.y[index] };
                app.drag = Drag::Guide { vertical, index, pos, started: false };
                return;
            }
            if let Some((key, index)) = crate::motion_ui::hint_at(app, rect, origin) {
                app.drag = Drag::Hint { key, index, started: false };
                return;
            }
            // Merge shapes: drag a selected piece, bend an edge, or pick a fill/line and drag it.
            if app.tool == Tool::Select && merge_press(app, p0, shift) {
                return;
            }
            // Motion path: drag a position point, or bend the path where no object is in the way.
            if let Some(g) = crate::motion_ui::path_grab(app, rect, origin, false) {
                if matches!(g, crate::motion_ui::PathGrab::Anchor(_)) || app.ed.hit_test(p0).is_none() {
                    app.drag = Drag::MotionPath { grab: g, started: false };
                    return;
                }
            }
            select_press(app, p0, shift);
        }
        Tool::Subselect if crate::motion_ui::path_grab(app, rect, origin, true).is_some() => {
            let g = crate::motion_ui::path_grab(app, rect, origin, true).unwrap();
            app.drag = Drag::MotionPath { grab: g, started: false };
        }
        Tool::Subselect => {
            if let Some((s, inv, path)) = app.shape_for_points(p0, true) {
                let scale = app.ed.element_at(s).map(|e| e.matrix.decompose().scale_x.abs().max(0.01)).unwrap_or(1.0);
                if let Some(r) = path.pick_point(inv.apply(p0), 8.0 / app.zoom / scale, true) {
                    app.drag = Drag::Point { sel: s, r, started: false };
                } else {
                    app.drag = Drag::Move { last: p0, moved: false, raw: pt(0.0, 0.0), applied: pt(0.0, 0.0), start: None, targets: (vec![], vec![]) };
                }
            } else if app.ed.hit_test(p0).is_none() {
                app.ed.select(None, false);
            }
        }
        Tool::AddAnchor | Tool::DeleteAnchor | Tool::ConvertAnchor => {
            if let Some((s, inv, path)) = app.shape_for_points(p0, true) {
                let scale = app.ed.element_at(s).map(|e| e.matrix.decompose().scale_x.abs().max(0.01)).unwrap_or(1.0);
                let lp = inv.apply(p0);
                let tol = 8.0 / app.zoom / scale;
                let mut path2 = path.clone();
                let changed = match tool {
                    Tool::AddAnchor => path2.insert_anchor(lp, tol).is_some(),
                    Tool::DeleteAnchor => match path.pick_point(lp, tol, false) {
                        Some(PointRef::Anchor(i)) => path2.delete_anchor(i),
                        _ => false,
                    },
                    _ => match path.pick_point(lp, tol, false) {
                        Some(PointRef::Anchor(i)) => {
                            path2.convert_anchor(i);
                            true
                        }
                        _ => false,
                    },
                };
                if changed {
                    app.ed.selection = vec![s];
                    let label = tool.info().name;
                    app.ed.modify_selected(label, move |e| {
                        if let ElementKind::Shape(sh) = &mut e.kind {
                            sh.path = path2.clone();
                            sh.warp = None;
                        }
                    });
                } else {
                    app.status = match tool {
                        Tool::AddAnchor => "Click on the shape's outline to add a point.".into(),
                        _ => "Click on an anchor point.".into(),
                    };
                }
            }
        }
        Tool::Pen => {
            if !app.ed.layer_editable(app.ed.layer) {
                app.locked_msg();
                return;
            }
            let first = app.pen.as_ref().and_then(|pn| pn.pts.first().map(|f| f.0));
            let n = app.pen.as_ref().map(|pn| pn.pts.len()).unwrap_or(0);
            if let Some(f) = first {
                if n >= 2 && app.to_screen(rect, f).distance(origin) < 8.0 {
                    app.finish_pen(true);
                    return;
                }
            }
            let sp = app.snap_pt(p0);
            let pen = app.pen.get_or_insert_with(Default::default);
            pen.pts.push((sp, pt(0.0, 0.0)));
            pen.dragging = true;
        }
        Tool::Lasso => app.drag = Drag::Freehand { pts: vec![p0] },
        Tool::MagicWand => {
            if app.ed.magic_wand(p0, app.topts.wand_threshold, shift) {
                app.status = "Magic Wand: area selected (Delete removes it)".into();
            } else {
                app.status = "Magic Wand works on broken-apart bitmaps (select a bitmap, Ctrl+B)".into();
            }
        }
        Tool::PolyLasso => {
            if app.poly_lasso.len() >= 3 && app.to_screen(rect, app.poly_lasso[0]).distance(origin) < 8.0 {
                let poly = std::mem::take(&mut app.poly_lasso);
                lasso_select(app, &poly, shift);
            } else {
                app.poly_lasso.push(p0);
            }
        }
        Tool::Text => {
            if app.text_edit.is_some() {
                app.finish_text();
            }
            if let Some(h) = app.ed.hit_test(p0) {
                if let Some(ElementKind::Text(t)) = app.ed.element_at(h).map(|e| e.kind) {
                    app.ed.select(Some(h), false);
                    app.text_edit = Some(TextEdit { sel: h, buffer: t.text, mark: app.ed.undo_depth(), focus_requested: false });
                    return;
                }
            }
            let mark = app.ed.undo_depth();
            let mut el = Element::text("", app.topts.text_size, from32(app.fill), p0);
            if let ElementKind::Text(t) = &mut el.kind {
                t.font = app.topts.text_font;
                t.align = app.topts.text_align;
                t.weight = if app.topts.text_bold { 700 } else { 400 };
                t.italic = app.topts.text_italic;
                t.field.kind = app.topts.text_kind;
                t.field.selectable = app.topts.text_kind != ac_core::text::TextKind::Static;
            }
            match app.ed.add_element(el) {
                Some(s) => {
                    app.text_edit = Some(TextEdit { sel: s, buffer: String::new(), mark, focus_requested: false });
                    // drag out to give the new text a fixed width (words wrap)
                    app.drag = Drag::TextBox { sel: s, start: p0 };
                }
                None => app.locked_msg(),
            }
        }
        Tool::Rect | Tool::Oval | Tool::Line | Tool::PolyStar | Tool::RectPrimitive | Tool::OvalPrimitive => {
            let s = app.snap_pt(p0);
            app.drag = Drag::Draw { start: s, cur: s }
        }
        Tool::Pencil | Tool::Brush => app.drag = Drag::Freehand { pts: vec![p0] },
        Tool::FluidBrush => app.drag = Drag::Fluid { pts: vec![(p0, app.ctx.input(|i| i.time))] },
        Tool::RasterBrush => raster_press(app, p0),
        Tool::Eraser => {
            let mark = app.ed.undo_depth();
            if app.ed.canvas_at(app.ed.layer).is_some() {
                if app.topts.faucet {
                    if !app.ed.merge_faucet(p0) {
                        app.status = "Faucet: click a fill or a line.".into();
                    }
                    return;
                }
                app.ed.begin_edit("Erase");
                merge_erase_at(app, p0, p0, p0);
                app.drag = Drag::Erase { mark, removed: 1, last: p0, start: p0 };
                return;
            }
            let removed = erase_at(app, p0);
            app.drag = Drag::Erase { mark, removed, last: p0, start: p0 };
        }
        Tool::Bucket if app.ed.canvas_at(app.ed.layer).is_some() && app.topts.fill_gradient.is_none() && app.ed.hit_test(p0).map(|h| matches!(app.ed.element_at(h).map(|e| e.kind), Some(ElementKind::Canvas(_)))).unwrap_or(true) => {
            let gap = [0.0, 3.0, 8.0, 16.0][app.topts.bucket_gap as usize] / app.zoom;
            if !app.ed.merge_bucket(p0, from32(app.fill), gap) {
                app.status = "Paint Bucket: click a fill, or inside closed lines (try a larger gap size).".into();
            }
        }
        Tool::Bucket => match app.ed.hit_test(p0) {
            Some(s) => {
                let shape = matches!(app.ed.element_at(s).map(|e| e.kind), Some(ElementKind::Shape(_)));
                match (app.topts.fill_gradient, shape) {
                    (Some(kind), true) => {
                        let stops = app.fill_stops();
                        app.ed.selection = vec![s];
                        app.ed.modify_selected("Paint Bucket", move |e| {
                            if let ElementKind::Shape(sh) = &mut e.kind {
                                sh.fill = Some(stops[0].1);
                                sh.gradient = sh.path.bounds().map(|(lo, hi)| Gradient::fit(kind, stops.clone(), lo, hi));
                            }
                        });
                    }
                    _ => {
                        let mark = app.ed.undo_depth();
                        let had_gradient = matches!(app.ed.element_at(s).map(|e| e.kind), Some(ElementKind::Shape(sh)) if sh.gradient.is_some());
                        app.ed.paint_bucket(s, from32(app.fill));
                        if had_gradient {
                            app.ed.selection = vec![s];
                            app.ed.modify_selected("Paint Bucket", |e| {
                                if let ElementKind::Shape(sh) = &mut e.kind {
                                    sh.gradient = None;
                                }
                            });
                            app.ed.squash_undo(mark, "Paint Bucket");
                        }
                    }
                }
            }
            None => app.status = "Paint Bucket: click inside a shape.".into(),
        },
        Tool::InkBottle if app.ed.merge_ink(p0, Stroke { color: from32(app.stroke), width: app.stroke_w }) => {}
        Tool::InkBottle => match app.ed.hit_test(p0) {
            Some(s) => {
                app.ed.selection = vec![s];
                let st = Stroke { color: from32(app.stroke), width: app.stroke_w };
                app.ed.modify_selected("Ink Bottle", move |e| {
                    if let ElementKind::Shape(sh) = &mut e.kind {
                        sh.stroke = Some(st);
                    }
                });
            }
            None => app.status = "Ink Bottle: click a shape to give it an outline.".into(),
        },
        Tool::Eyedropper => eyedrop(app, p0),
        Tool::Rotate3D | Tool::Translate3D => {
            if let Some((kind, center)) = app.grab3d_at(rect, origin) {
                let editable = app.ed.selection.iter().all(|s| app.ed.layer_editable(s.layer));
                if editable {
                    app.drag = Drag::Gizmo3D { kind, last: origin, center, started: false };
                } else {
                    app.locked_msg();
                }
            } else if let Some(h) = app.ed.hit_test(p0) {
                app.ed.select(Some(h), shift);
            } else if !shift {
                app.ed.select(None, false);
            }
        }
        Tool::GradientTransform => gradient_press(app, rect, origin, p0),
        Tool::Bone if app.ed.rig_bones().iter().any(|(s, _, _, t)| app.ed.layer_editable(s.layer) && app.to_screen(rect, *t).distance(origin) < 8.0) => {
            // continue a shape armature from a bone's tail
            let (s, _, _, t) = app.ed.rig_bones().into_iter().find(|(s, _, _, t)| app.ed.layer_editable(s.layer) && app.to_screen(rect, *t).distance(origin) < 8.0).unwrap();
            app.drag = Drag::BoneDraw { layer: s.layer, parent: s.element, head: t, cur: p0 };
        }
        Tool::Bind => {
            if let Some((s, b, _, _)) = app.ed.rig_bones().into_iter().find(|(s, _, h, t)| app.ed.layer_editable(s.layer) && seg_dist(origin, app.to_screen(rect, *h), app.to_screen(rect, *t)) < 6.0) {
                app.bind_bone = Some((s, b));
                app.status = format!("Bone {} selected: click points of the shape to bind them to it (Alt+click unbinds)", b + 1);
            } else if let Some((s, b)) = app.bind_bone {
                match bind_point_at(app, rect, s, origin) {
                    Some(idx) => app.ed.rig_bind(s, idx, if alt { None } else { Some(b) }),
                    None => app.status = "Bind: click a point of the shape (square) or a bone.".into(),
                }
            } else {
                app.status = "Bind: click a shape bone first (add bones by dragging inside a shape with the Bone tool).".into();
            }
        }
        Tool::Bone => {
            // Start from an existing joint (chains bones), or from the object under the mouse.
            let joint = app.bone_segments().into_iter().find(|(l, _, _, tail)| app.ed.layer_editable(*l) && app.to_screen(rect, *tail).distance(origin) < 8.0);
            if let Some((l, bi, _, tail)) = joint {
                let child = app.ed.bones(l)[bi].child;
                app.drag = Drag::BoneDraw { layer: l, parent: child, head: tail, cur: p0 };
            } else if let Some(h) = app.ed.hit_test(p0) {
                if app.ed.layer_editable(h.layer) {
                    app.ed.select(Some(h), false);
                    app.drag = Drag::BoneDraw { layer: h.layer, parent: h.element, head: p0, cur: p0 };
                } else {
                    app.locked_msg();
                }
            } else {
                app.status = "Bone: press on an object, then drag to another object on the same layer.".into();
            }
        }
        Tool::Width => {
            if alt {
                if let Some((sel, i)) = width_point_at(app, rect, origin) {
                    app.ed.selection = vec![sel];
                    app.ed.modify_selected("Delete Width Point", move |e| {
                        if let ElementKind::Shape(sh) = &mut e.kind {
                            if let Some(prof) = &mut sh.width_profile {
                                prof.remove(i);
                                if prof.iter().all(|w| (w.1 - 1.0).abs() < 1e-3) {
                                    sh.width_profile = None;
                                }
                            }
                        }
                    });
                    app.status = "Width point deleted".into();
                } else if let Some((_, li, line, i)) = app.ed.merge_width_points().into_iter().find(|w| app.to_screen(rect, w.0).distance(origin) < 8.0) {
                    app.ed.merge_remove_width_point(li, line, i);
                    app.status = "Width point deleted".into();
                } else {
                    app.status = "Width: Alt+click a width point (pink dot) to delete it.".into();
                }
                return;
            }
            let found = stroked_shape_at(app, p0);
            match found {
                Some((s, t)) => {
                    app.ed.select(Some(s), false);
                    app.drag = Drag::WidthPt { sel: s, t, started: false };
                }
                None => {
                    // lines of a merge shape
                    app.ed.commit_floats();
                    match app.ed.merge_line_at(p0, 8.0 / app.zoom) {
                        Some((layer, line, t)) => {
                            app.ed.selection.clear();
                            app.drag = Drag::MergeWidth { layer, line, t, started: false };
                        }
                        None => app.status = "Width: press on a line or the stroke (outline) of a shape and drag outwards or inwards.".into(),
                    }
                }
            }
        }
        Tool::AssetWarp => warp_press(app, p0, alt),
        Tool::Magnet | Tool::Pinch | Tool::Pump | Tool::Iron | Tool::Bender => sculpt_press(app, tool, p0),
        Tool::Assistant => assist_press(app, rect, origin, p0, alt),
        Tool::Particles => {
            let el = Element { name: String::new(), matrix: Matrix::translate(p0.x, p0.y), ..Element::particles(ac_core::particles::Emitter::default()) };
            if app.ed.add_element(el).is_some() {
                app.drag = Drag::ParticleAim { origin: p0, cur: p0 };
                app.status = "Particle emitter placed: drag to aim it (length = speed); press Enter/Ctrl+Enter to play. Settings in Properties.".into();
            } else {
                app.status = "The current layer is locked or hidden".into();
            }
        }
        Tool::Camera => {
            if app.ed.editing.is_some() {
                app.status = "The camera belongs to the scene: leave symbol editing first (Esc).".into();
                return;
            }
            if app.ed.camera_layer().is_none() {
                app.ed.add_camera();
                app.ed.camera_view = true;
                app.status = "Added a Camera layer. Drag to pan, scroll to zoom, Shift+drag to rotate; keyframe it like any layer.".into();
            }
            if vcam_frame_shown(app) {
                // free camera: grab the frame like an object
                let Some(q) = vcam_quad(app, app.ed.frame) else { return };
                let s: Vec<Pos2> = q.iter().map(|p| app.to_screen(rect, *p)).collect();
                let knob = vcam_knob(&s);
                let cam0 = app.ed.camera_at(app.ed.frame).unwrap_or_else(|| ac_core::default_camera(&app.ed.doc).matrix);
                let mode = if knob.distance(origin) < 9.0 {
                    Some(VCamGrab::Rotate)
                } else if s.iter().any(|c| c.distance(origin) < 9.0) {
                    Some(VCamGrab::Zoom)
                } else if ac_core::geom::point_in_polygon(p0, &q) {
                    Some(VCamGrab::Move)
                } else {
                    None
                };
                match mode {
                    Some(mode) => app.drag = Drag::VCam { mode, from: p0, cam0, started: false },
                    None => app.status = "Free camera: drag inside the camera frame to move it, a corner to zoom, the round knob to rotate.".into(),
                }
                return;
            }
            app.drag = Drag::CameraPan { last: origin, rotate: shift, started: false };
        }
    }
}

/// Selection-tool press on an object (or empty stage).
/// Bind tool: the point of shape `s` under the pointer (index in path order).
fn bind_point_at(app: &App, rect: Rect, s: Selection, at: Pos2) -> Option<usize> {
    let el = app.ed.element_at(s)?;
    let ElementKind::Shape(sh) = &el.kind else { return None };
    let posed = sh.rig.as_ref().map(|r| r.deform(&sh.path)).unwrap_or_else(|| sh.path.clone());
    let pts = ac_core::rig::path_points(&posed);
    ac_core::rig::anchor_indices(&posed).into_iter().filter(|&i| app.to_screen(rect, el.matrix.apply(pts[i])).distance(at) < 7.0).min_by(|&a, &b| {
        let da = app.to_screen(rect, el.matrix.apply(pts[a])).distance(at);
        let db = app.to_screen(rect, el.matrix.apply(pts[b])).distance(at);
        da.partial_cmp(&db).unwrap()
    })
}

/// Selection tool on a merge shape. True if handled.
fn merge_press(app: &mut App, p0: Pt, shift: bool) -> bool {
    let tol = 4.0 / app.zoom;
    let Some((li, part, in_float)) = app.ed.merge_part_at(p0, 1.0 / app.zoom) else {
        // not over a fill: still maybe near an edge (strokes are thin)
        if let Some((layer, key, ei, bend, inv)) = (!shift).then(|| app.ed.merge_begin_bend(p0, tol, app.cmd_down)).flatten() {
            app.drag = Drag::Bend { layer, key, ei, bend: Box::new(bend), inv, from: p0, part: None, started: false };
            return true;
        }
        return false;
    };
    if in_float {
        let sel = app.ed.canvas_at(li).map(|(_, ei)| Selection { layer: li, element: ei });
        if let Some(s) = sel.filter(|s| !app.ed.is_selected(*s)) {
            app.ed.selection.push(s);
            app.ed.selection.sort();
        }
        start_move(app, p0);
        return true;
    }
    if !shift {
        if let Some((layer, key, ei, bend, inv)) = app.ed.merge_begin_bend(p0, tol, app.cmd_down) {
            app.drag = Drag::Bend { layer, key, ei, bend: Box::new(bend), inv, from: p0, part, started: false };
            return true;
        }
    }
    let Some(part) = part else { return false };
    app.ed.merge_select(li, part, false, shift);
    start_move(app, p0);
    true
}

fn start_move(app: &mut App, p0: Pt) {
    let mut targets = if app.topts.snap { app.snap_targets() } else { (vec![], vec![]) };
    let b = app.ed.selection_bounds();
    app.guide_grid_targets(&mut targets.0, &mut targets.1, b);
    app.drag = Drag::Move { last: p0, moved: false, raw: pt(0.0, 0.0), applied: pt(0.0, 0.0), start: b, targets };
}

fn select_press(app: &mut App, p0: Pt, shift: bool) {
    if let Some(hit) = app.ed.hit_test(p0) {
        // Preferences › Shift select off: clicks add to the selection
        let adds = !shift && !app.settings.prefs.shift_select;
        if adds {
            if !app.ed.is_selected(hit) {
                app.ed.select(Some(hit), true);
            }
        } else if shift {
            app.ed.select(Some(hit), true);
        } else if !app.ed.is_selected(hit) {
            app.ed.select(Some(hit), false);
        }
        // Objects joined by bones are posed with inverse kinematics.
        let boned = app.ed.bones(hit.layer).iter().any(|b| b.parent == hit.element || b.child == hit.element);
        if boned && !shift && app.tool == Tool::Select {
            if let Some(inv) = app.ed.element_at(hit).and_then(|e| e.matrix.invert()) {
                app.ed.select(Some(hit), false);
                app.drag = Drag::Ik { layer: hit.layer, element: hit.element, grab: inv.apply(p0), started: false };
                return;
            }
        }
        if app.ed.is_selected(hit) {
            let mut targets = if app.topts.snap { app.snap_targets() } else { (vec![], vec![]) };
            let b = app.ed.selection_bounds();
            app.guide_grid_targets(&mut targets.0, &mut targets.1, b);
            app.drag = Drag::Move { last: p0, moved: false, raw: pt(0.0, 0.0), applied: pt(0.0, 0.0), start: b, targets };
        }
    } else {
        if !shift {
            app.ed.select(None, false);
        }
        app.drag = Drag::Marquee { start: p0, cur: p0, add: shift };
    }
}

/// An interior width point of a selected shape near the screen position `at`: (shape, index).
fn width_point_at(app: &App, rect: Rect, at: Pos2) -> Option<(Selection, usize)> {
    let mut cands = app.ed.selection.clone();
    if let Some((s, _)) = stroked_shape_at(app, app.to_doc(rect, at)) {
        cands.push(s);
    }
    for s in cands {
        let Some(el) = app.ed.element_at(s) else { continue };
        let ElementKind::Shape(sh) = &el.kind else { continue };
        let Some(prof) = &sh.width_profile else { continue };
        for (i, (t, _)) in prof.iter().enumerate() {
            if *t <= 1e-4 || *t >= 1.0 - 1e-4 {
                continue;
            }
            if let Some(q) = deform::point_at(&sh.path, *t) {
                if app.to_screen(rect, el.matrix.apply(q)).distance(at) < 8.0 {
                    return Some((s, i));
                }
            }
        }
    }
    None
}

/// Width tool target: a stroked shape whose outline is near `p` → (shape, position along it).
fn stroked_shape_at(app: &App, p: Pt) -> Option<(Selection, f32)> {
    let mut cands: Vec<Selection> = app.ed.selection.clone();
    if let Some(h) = app.ed.hit_test(p) {
        cands.push(h);
    }
    // also any shape whose stroke passes near p
    for it in app.ed.evaluate_current(app.ed.frame).iter().rev() {
        if let Some((l, e)) = it.src {
            cands.push(Selection { layer: l, element: e });
        }
    }
    for s in cands {
        if !app.ed.layer_editable(s.layer) {
            continue;
        }
        let Some(el) = app.ed.element_at(s) else { continue };
        let ElementKind::Shape(sh) = &el.kind else { continue };
        let Some(st) = sh.stroke else { continue };
        let Some(inv) = el.matrix.invert() else { continue };
        let lp = inv.apply(p);
        let scale = el.matrix.decompose().scale_x.abs().max(0.01);
        if let Some((t, d)) = deform::nearest_t(&sh.path, lp) {
            let max_w = sh.width_profile.as_ref().map(|w| w.iter().map(|x| x.1).fold(1.0f32, f32::max)).unwrap_or(1.0);
            if d < st.width * max_w / 2.0 + 8.0 / app.zoom / scale {
                return Some((s, t));
            }
        }
    }
    None
}

impl App {
    /// Are the drawing assistants shaping the current freehand stroke?
    pub(crate) fn assisting(&self) -> bool {
        self.topts.assist_on && !self.assist_bypass && self.ed.doc.assistants.iter().any(|a| a.enabled)
    }
}

fn raster_opts(app: &App) -> ac_core::paint::PaintOpts {
    ac_core::paint::PaintOpts {
        color: from32(app.fill),
        radius: (app.topts.raster_size > 0.0).then_some(app.topts.raster_size),
        opacity: app.topts.raster_opacity,
        pressure: app.topts.raster_pressure,
        eraser: app.topts.raster_eraser,
    }
}

/// Raster Brush press: start painting into the bitmap under the pointer (or a new one).
fn raster_press(app: &mut App, p0: Pt) {
    let Some(preset) = ac_core::paint::PRESETS.get(app.topts.raster_brush) else { return };
    match app.ed.paint_begin(p0, preset, &raster_opts(app)) {
        Ok(mut s) => {
            app.ed.paint_to(&mut s, p0, None, 0.01);
            app.drag = Drag::Paint(Box::new(s), app.ctx.input(|i| i.time));
        }
        Err(e) => app.status = format!("Raster Brush: {e}."),
    }
}

/// A pinned joint: a red pin head with a ring.
fn draw_pin(painter: &egui::Painter, c: Pos2) {
    painter.circle_stroke(c, 7.0, EStroke::new(1.5_f32, Color32::from_rgb(0xE0, 0x30, 0x30)));
    painter.circle_filled(c + vec2(0.0, -9.0), 3.0, Color32::from_rgb(0xE0, 0x30, 0x30));
    painter.line_segment([c + vec2(0.0, -6.0), c], EStroke::new(1.5_f32, Color32::from_rgb(0xE0, 0x30, 0x30)));
}

/// Assistants tool: grab a handle or an assistant's line; Alt+click deletes it.
fn assist_press(app: &mut App, rect: Rect, origin: Pos2, p0: Pt, alt: bool) {
    let mut hit: Option<(usize, Option<usize>)> = None;
    for (i, a) in app.ed.doc.assistants.iter().enumerate().rev() {
        if let Some(k) = a.points().iter().position(|q| app.to_screen(rect, *q).distance(origin) < 9.0) {
            hit = Some((i, Some(k)));
            break;
        }
    }
    if hit.is_none() {
        // near an assistant's line: move the whole assistant
        for (i, a) in app.ed.doc.assistants.iter().enumerate().rev() {
            let pts = a.points();
            let near = pts.windows(2).any(|w| {
                let (x, y) = (app.to_screen(rect, w[0]), app.to_screen(rect, w[1]));
                seg_dist(origin, x, y) < 6.0
            });
            if near {
                hit = Some((i, None));
                break;
            }
        }
    }
    match hit {
        Some((i, _)) if alt => {
            app.ed.remove_assistant(i);
            app.assist_sel = None;
            app.status = "Assistant deleted".into();
        }
        Some((i, k)) => {
            app.assist_sel = Some(i);
            app.drag = Drag::AssistPt { i, k, last: p0, started: false };
        }
        None => {
            app.assist_sel = None;
            app.status = "Assistants: add one in Properties ▸ Tool, then drag its handles. Alt+click a handle deletes the assistant.".into();
        }
    }
}

/// Guides as thin lines across the view, replicators as their axes; handles
/// when the Assistants tool is active.
fn draw_assistants(app: &App, painter: &egui::Painter, rect: Rect, handles: bool) {
    use ac_core::assist::AssistantKind as K;
    let view_lo = app.to_doc(rect, rect.left_top());
    let view_hi = app.to_doc(rect, rect.right_bottom());
    let far = view_lo.dist(view_hi).max(100.0) * 2.0;
    let guide_col = Color32::from_rgba_unmultiplied(0x40, 0xC0, 0xFF, 150);
    let rep_col = Color32::from_rgba_unmultiplied(0xFF, 0x70, 0xD0, 170);
    for (i, a) in app.ed.doc.assistants.iter().enumerate() {
        let k = if a.enabled { 1.0 } else { 0.35 };
        let sel = handles && app.assist_sel == Some(i);
        let g = EStroke::new(if sel { 1.6_f32 } else { 1.0 }, guide_col.gamma_multiply(k));
        let r = EStroke::new(if sel { 1.8_f32 } else { 1.2 }, rep_col.gamma_multiply(k));
        let line = |a: Pt, b: Pt, st: EStroke| {
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            let l = (dx * dx + dy * dy).sqrt().max(1e-6);
            let (ux, uy) = (dx / l * far, dy / l * far);
            painter.line_segment([app.to_screen(rect, pt(a.x - ux, a.y - uy)), app.to_screen(rect, pt(a.x + ux, a.y + uy))], st);
        };
        let ellipse = |m: ac_core::Matrix, st: EStroke| {
            let pts: Vec<Pos2> = (0..=64).map(|j| {
                let t = std::f32::consts::TAU * j as f32 / 64.0;
                app.to_screen(rect, m.apply(pt(t.cos(), t.sin())))
            }).collect();
            painter.add(egui::Shape::line(pts, st));
        };
        match &a.kind {
            K::Line { a: p, b: q, parallel } => {
                line(*p, *q, g);
                if *parallel {
                    // a few parallels to show the direction
                    let (dx, dy) = (q.x - p.x, q.y - p.y);
                    let l = (dx * dx + dy * dy).sqrt().max(1e-6);
                    let n = pt(-dy / l * 40.0, dx / l * 40.0);
                    for j in [-2.0, -1.0, 1.0, 2.0] {
                        line(pt(p.x + n.x * j, p.y + n.y * j), pt(q.x + n.x * j, q.y + n.y * j), EStroke::new(0.6_f32, guide_col.gamma_multiply(k * 0.5)));
                    }
                }
            }
            K::VanishingPoint { c } => {
                for j in 0..16 {
                    let t = std::f32::consts::TAU * j as f32 / 16.0;
                    let e = pt(c.x + t.cos() * far, c.y + t.sin() * far);
                    painter.line_segment([app.to_screen(rect, *c), app.to_screen(rect, e)], EStroke::new(0.7_f32, guide_col.gamma_multiply(k * 0.6)));
                }
                painter.circle_stroke(app.to_screen(rect, *c), 5.0, g);
            }
            K::Ellipse { c, a: ax, b, concentric } => {
                if let Some(m) = ac_core::assist::ellipse_matrix(*c, *ax, *b) {
                    ellipse(m, g);
                    if *concentric {
                        for s in [0.5f32, 1.5, 2.0] {
                            ellipse(ac_core::Matrix { a: m.a * s, b: m.b * s, c: m.c * s, d: m.d * s, ..m }, EStroke::new(0.6_f32, guide_col.gamma_multiply(k * 0.5)));
                        }
                    }
                }
            }
            K::Mirror { a: p, b: q } => line(*p, *q, r),
            K::Star { c, a: p, count, mirror } => {
                let n = (*count).max(1);
                let (dx, dy) = (p.x - c.x, p.y - c.y);
                for j in 0..n {
                    let t = std::f32::consts::TAU * j as f32 / n as f32;
                    let (s_, c_) = t.sin_cos();
                    let e = pt(c.x + (dx * c_ - dy * s_) * 3.0, c.y + (dx * s_ + dy * c_) * 3.0);
                    painter.line_segment([app.to_screen(rect, *c), app.to_screen(rect, e)], if j == 0 { r } else { EStroke::new(0.8_f32, rep_col.gamma_multiply(k * 0.6)) });
                    if *mirror {
                        let t2 = t + std::f32::consts::PI / n as f32;
                        let (s2, c2) = t2.sin_cos();
                        let e2 = pt(c.x + (dx * c2 - dy * s2) * 3.0, c.y + (dx * s2 + dy * c2) * 3.0);
                        painter.line_segment([app.to_screen(rect, *c), app.to_screen(rect, e2)], EStroke::new(0.5_f32, rep_col.gamma_multiply(k * 0.4)));
                    }
                }
            }
            K::Grid { c, a: p, b: q, count_a, count_a_inv, count_b, count_b_inv } => {
                let (da, db) = (pt(p.x - c.x, p.y - c.y), pt(q.x - c.x, q.y - c.y));
                let (a0, a1, b0, b1) = (-(*count_a_inv as i32), (*count_a).max(1) as i32, -(*count_b_inv as i32), (*count_b).max(1) as i32);
                for ib in b0..b1 {
                    for ia in a0..a1 {
                        let o = pt(c.x + da.x * ia as f32 + db.x * ib as f32, c.y + da.y * ia as f32 + db.y * ib as f32);
                        let quad = [o, pt(o.x + da.x, o.y + da.y), pt(o.x + da.x + db.x, o.y + da.y + db.y), pt(o.x + db.x, o.y + db.y)];
                        let st = if ia == 0 && ib == 0 { r } else { EStroke::new(0.7_f32, rep_col.gamma_multiply(k * 0.55)) };
                        painter.add(egui::Shape::closed_line(quad.iter().map(|x| app.to_screen(rect, *x)).collect(), st));
                    }
                }
            }
            K::Perspective { vx, vy, vz, three_point } => {
                // horizon, and a fan of rays from each vanishing point
                line(*vx, *vy, g);
                let faint = EStroke::new(0.6_f32, guide_col.gamma_multiply(k * 0.5));
                let mut vps = vec![*vx, *vy];
                if *three_point {
                    vps.push(*vz);
                }
                for v in &vps {
                    for j in 0..12 {
                        let t = std::f32::consts::TAU * j as f32 / 12.0 + 0.13;
                        painter.line_segment([app.to_screen(rect, *v), app.to_screen(rect, pt(v.x + t.cos() * far, v.y + t.sin() * far))], faint);
                    }
                    painter.circle_stroke(app.to_screen(rect, *v), 5.0, g);
                }
                if !*three_point {
                    let mid = vx.lerp(*vy, 0.5);
                    for j in [-2.0f32, -1.0, 0.0, 1.0, 2.0] {
                        let x = mid.x + j * vx.dist(*vy) / 6.0;
                        line(pt(x, mid.y), pt(x, mid.y + 1.0), faint);
                    }
                }
            }
            K::Fisheye { c, a: ax, b } => {
                if let Some(m) = ac_core::assist::ellipse_matrix(*c, *ax, *b) {
                    ellipse(m, g);
                    let faint = EStroke::new(0.6_f32, guide_col.gamma_multiply(k * 0.5));
                    for s in [0.25f32, 0.5, 0.75] {
                        ellipse(m.concat(&ac_core::Matrix { a: s, b: 0.0, c: 0.0, d: 1.0, tx: 0.0, ty: 0.0 }), faint);
                        ellipse(m.concat(&ac_core::Matrix { a: 1.0, b: 0.0, c: 0.0, d: s, tx: 0.0, ty: 0.0 }), faint);
                    }
                    for j in 0..8 {
                        let t = std::f32::consts::TAU * j as f32 / 8.0;
                        painter.line_segment([app.to_screen(rect, *c), app.to_screen(rect, m.apply(pt(t.cos(), t.sin())))], faint);
                    }
                }
            }
            K::Affine { c0, a0, b0, c1, a1, b1, count, count_inv } => {
                let frame = |c: Pt, a: Pt, b: Pt, st: EStroke| {
                    let d = pt(a.x + b.x - c.x, a.y + b.y - c.y);
                    painter.add(egui::Shape::closed_line([c, a, d, b].iter().map(|x| app.to_screen(rect, *x)).collect(), st));
                };
                frame(*c0, *a0, *b0, EStroke::new(1.4_f32, guide_col.gamma_multiply(k)));
                frame(*c1, *a1, *b1, EStroke::new(1.4_f32, Color32::from_rgb(0xFF, 0xA0, 0x30).gamma_multiply(k)));
                // the copies of the first frame
                let reps = ac_core::assist::replicas(std::slice::from_ref(a));
                for m in reps.iter().skip(1).take((*count as usize + *count_inv as usize).min(60)) {
                    frame(m.apply(*c0), m.apply(*a0), m.apply(*b0), EStroke::new(0.6_f32, rep_col.gamma_multiply(k * 0.5)));
                }
            }
            K::Jitter { c, period, amplitude, .. } => {
                let (h, q) = (period / 2.0, period / 4.0);
                let zig = [pt(c.x - h, c.y), pt(c.x - q, c.y + amplitude), pt(c.x + q, c.y - amplitude), pt(c.x + h, c.y)];
                painter.add(egui::Shape::line(zig.iter().map(|x| app.to_screen(rect, *x)).collect(), r));
            }
        }
        if handles {
            for (j, q) in a.points().iter().enumerate() {
                let s = app.to_screen(rect, *q);
                let col = if a.is_guide() { guide_col } else { rep_col };
                let centre = j == 0 || (j == 3 && matches!(a.kind, K::Affine { .. }));
                if centre && !matches!(a.kind, K::Perspective { .. }) {
                    painter.circle_filled(s, 5.0, if sel { Color32::WHITE } else { col });
                    painter.circle_stroke(s, 5.0, EStroke::new(1.0_f32, Color32::BLACK));
                } else {
                    painter.circle_filled(s, 4.0, Color32::WHITE);
                    painter.circle_stroke(s, 4.0, EStroke::new(1.5_f32, col));
                }
            }
        }
    }
}

/// Emitter markers: dashed area and an arrow for direction and speed (one second of travel / 2).
fn draw_emitters(app: &App, painter: &egui::Painter, rect: Rect) {
    let tool = app.tool == Tool::Particles;
    let tl = app.ed.timeline();
    for (li, l) in tl.layers.iter().enumerate() {
        if !l.visible {
            continue;
        }
        let Some((els, _)) = ac_core::model::layer_elements_at(l, app.ed.frame) else { continue };
        let parent = app.ed.layer_parent_at(li).unwrap_or(Matrix::IDENTITY);
        for (ei, el) in els.iter().enumerate() {
            let ElementKind::Particles(e) = &el.kind else { continue };
            let sel = app.ed.is_selected(ac_core::Selection { layer: li, element: ei });
            if !tool && !sel {
                continue;
            }
            let m = parent.concat(&el.matrix);
            let col = Color32::from_rgb(0xFF, 0xA0, 0x30).gamma_multiply(if sel { 1.0 } else { 0.6 });
            let st = EStroke::new(1.0_f32, col);
            let (w, h) = (e.area.x.max(0.0) / 2.0, e.area.y.max(0.0) / 2.0);
            let quad: Vec<Pos2> = [pt(-w, -h), pt(w, -h), pt(w, h), pt(-w, h)].iter().map(|q| app.to_screen(rect, m.apply(*q))).collect();
            if w > 0.0 || h > 0.0 {
                painter.add(egui::Shape::dashed_line(&[quad[0], quad[1], quad[2], quad[3], quad[0]], st, 4.0, 3.0));
            }
            let o = app.to_screen(rect, m.apply(pt(0.0, 0.0)));
            painter.circle_stroke(o, 5.0, st);
            let d = e.direction.to_radians();
            let len = e.speed * 0.5;
            let tip = app.to_screen(rect, m.apply(pt(d.cos() * len, d.sin() * len)));
            painter.arrow(o, tip - o, st);
            for side in [-1.0f32, 1.0] {
                let a = d + side * e.spread.to_radians() / 2.0;
                let q = app.to_screen(rect, m.apply(pt(a.cos() * len * 0.6, a.sin() * len * 0.6)));
                painter.line_segment([o, q], EStroke::new(0.6_f32, col.gamma_multiply(0.6)));
            }
        }
    }
}

/// Magnet / Pinch / Pump / Iron / Bender press.
fn sculpt_press(app: &mut App, tool: Tool, p0: Pt) {
    use ac_core::sculpt::Kind;
    let kind = match tool {
        Tool::Magnet => Kind::Magnet,
        Tool::Pinch => Kind::Pinch,
        Tool::Pump => Kind::Pump,
        Tool::Iron => Kind::Iron,
        _ => Kind::Bender,
    };
    let opts = ac_core::sculpt::Opts { size: app.topts.sculpt_size, sharp: app.topts.sculpt_sharp, strength: app.topts.sculpt_strength };
    let tol = 8.0 / app.zoom;
    if kind == Kind::Bender {
        // near the segment: start a new one; elsewhere: bend with the current one
        let near_seg = app.bender_seg.map(|(a, b)| {
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            let t = (((p0.x - a.x) * dx + (p0.y - a.y) * dy) / (dx * dx + dy * dy).max(1e-6)).clamp(0.0, 1.0);
            p0.dist(a.lerp(b, t)) < tol
        });
        if let (Some(seg), Some(false)) = (app.bender_seg, near_seg) {
            match app.ed.sculpt_begin(kind, p0, opts, tol, Some(seg)) {
                Some(sc) => app.drag = Drag::Sculpt(Box::new(sc)),
                None => app.status = "Bender: the segment doesn't cross any line on an unlocked layer. Draw it again across the lines.".into(),
            }
            return;
        }
        app.bender_seg = None;
        app.drag = Drag::BenderLine { a: p0, cur: p0 };
        return;
    }
    match app.ed.sculpt_begin(kind, p0, opts, tol, None) {
        Some(sc) => {
            app.ed.selection.clear();
            app.drag = Drag::Sculpt(Box::new(sc));
        }
        None => {
            app.status = match kind {
                Kind::Magnet | Kind::Iron => format!("{}: no lines or shapes within the Size radius on an unlocked layer.", kind.name()),
                Kind::Pump => "Pump: press on a line (or a drawing object's stroke) and drag up or down.".into(),
                _ => "Pinch: press on a line or a shape's edge and drag.".into(),
            }
        }
    }
}

fn warp_press(app: &mut App, p0: Pt, alt: bool) {
    // pins on a selected bitmap / symbol / group come first (they may lie outside its pixels)
    let tol = 9.0 / app.zoom;
    for s in app.ed.selection.clone() {
        if let Some(i) = app.ed.mesh_pins(s).iter().position(|q| q.dist(p0) < tol) {
            if alt {
                app.ed.mesh_pin_remove(s, i);
            } else {
                app.drag = Drag::MeshPin { sel: s, pin: i, started: false };
            }
            return;
        }
    }
    let Some((s, inv, _)) = app.shape_for_points(p0, true) else {
        match app.ed.hit_test(p0) {
            Some(h) if !alt && matches!(app.ed.element_at(h).map(|e| e.kind), Some(ElementKind::Bitmap(_) | ElementKind::Instance(_) | ElementKind::Group(_))) => {
                if !app.ed.layer_editable(h.layer) {
                    app.locked_msg();
                    return;
                }
                if let Some(i) = app.ed.mesh_pin_add(h, p0) {
                    app.drag = Drag::MeshPin { sel: h, pin: i, started: false };
                    app.status = "Asset Warp: pins hold the picture; drag one to bend it around the others. Alt+click a pin removes it.".into();
                }
            }
            Some(_) => {}
            None => app.status = "Asset Warp: click a shape, bitmap or symbol to drop a pin, then drag pins to bend it.".into(),
        }
        return;
    };
    if !app.ed.layer_editable(s.layer) {
        app.locked_msg();
        return;
    }
    let Some(el) = app.ed.element_at(s) else { return };
    let ElementKind::Shape(sh) = &el.kind else { return };
    let lp = inv.apply(p0);
    let scale = el.matrix.decompose().scale_x.abs().max(0.01);
    let tol = 9.0 / app.zoom / scale;
    let mut warp = sh.warp.clone().unwrap_or_else(|| Warp::new(&sh.path));
    if let Some(i) = warp.pins.iter().position(|(_, q)| q.dist(lp) < tol) {
        if alt {
            warp.pins.remove(i);
            let path = warp.apply();
            app.ed.selection = vec![s];
            app.ed.modify_selected("Remove Pin", move |e| {
                if let ElementKind::Shape(sh) = &mut e.kind {
                    sh.path = path.clone();
                    sh.warp = Some(warp.clone());
                }
            });
        } else {
            app.drag = Drag::WarpPin { sel: s, pin: i, started: false };
        }
        return;
    }
    if alt {
        return;
    }
    warp.pins.push((lp, lp));
    let pin = warp.pins.len() - 1;
    let path = warp.apply();
    app.ed.selection = vec![s];
    app.ed.modify_selected("Add Pin", move |e| {
        if let ElementKind::Shape(sh) = &mut e.kind {
            sh.path = path.clone();
            sh.warp = Some(warp.clone());
        }
    });
    app.drag = Drag::WarpPin { sel: s, pin, started: false };
    app.status = "Pin added. Drag a pin to bend the shape; Alt+click a pin to remove it.".into();
}

/// Bitmap-fill handles sit this many bitmap pixels from the tile origin.
const BMP_UNIT: f32 = 48.0;

/// Gradient (or bitmap fill) handles (world): centre, x-handle, y-handle.
fn gradient_handles(el: &Element) -> Option<[Pt; 3]> {
    let ElementKind::Shape(sh) = &el.kind else { return None };
    let (m, u) = match (&sh.gradient, &sh.bitmap_fill) {
        (Some(g), _) => (g.matrix, 1.0),
        (None, Some(b)) => (b.matrix, BMP_UNIT),
        _ => return None,
    };
    let w = el.matrix.concat(&m);
    Some([w.apply(pt(0.0, 0.0)), w.apply(pt(u, 0.0)), w.apply(pt(0.0, u))])
}

/// Gradient Transform shows the third (height) handle for radial gradients
/// and bitmap fills.
fn two_axis(el: &Element) -> bool {
    matches!(&el.kind, ElementKind::Shape(sh) if sh.gradient.as_ref().map(|g| g.kind == ac_core::GradientKind::Radial).unwrap_or(sh.bitmap_fill.is_some()))
}

fn gradient_press(app: &mut App, rect: Rect, origin: Pos2, p0: Pt) {
    for s in app.ed.selection.clone() {
        let Some(el) = app.ed.element_at(s) else { continue };
        let radial = two_axis(&el);
        if let Some(hs) = gradient_handles(&el) {
            for (i, h) in hs.iter().enumerate() {
                if i == 2 && !radial {
                    continue;
                }
                if app.to_screen(rect, *h).distance(origin) < 8.0 {
                    if app.ed.layer_editable(s.layer) {
                        app.drag = Drag::Grad { sel: s, handle: i, started: false };
                    } else {
                        app.locked_msg();
                    }
                    return;
                }
            }
        }
    }
    match app.ed.hit_test(p0) {
        Some(h) => {
            app.ed.select(Some(h), false);
            let has = app.ed.element_at(h).map(|e| gradient_handles(&e).is_some()).unwrap_or(false);
            app.status = if has { "Drag the centre to move the gradient or bitmap fill, the square handle to rotate/stretch it, the round one to change its height.".into() } else { "This object has no gradient or bitmap fill. Set one in Properties first.".into() };
        }
        None => app.ed.select(None, false),
    }
}

fn drag(app: &mut App, ui: &egui::Ui, rect: Rect, pos: Pos2, p: Pt, shift: bool, alt: bool) {
    let zoom = app.zoom;
    let tool_now = app.tool;
    let snap_on = !alt;
    if let Some(pen) = app.pen.as_mut().filter(|pn| pn.dragging) {
        if let Some(last) = pen.pts.last_mut() {
            last.1 = pt(p.x - last.0.x, p.y - last.0.y);
        }
    }
    if let Drag::RigIk { sel, bone, started } = app.drag {
        if !started {
            app.ed.begin_edit("Pose Shape Bones");
            app.drag = Drag::RigIk { sel, bone, started: true };
        }
        app.ed.rig_ik(sel, bone, p);
        return;
    }
    if let Drag::Bend { layer, key, ei, ref bend, inv, from, part, started } = app.drag {
        let a = inv.apply(from);
        let b = inv.apply(p);
        let shape = bend.apply(pt(b.x - a.x, b.y - a.y));
        if started || p.dist(from) * app.zoom > 2.0 {
            app.ed.merge_apply_bend(layer, key, ei, shape, !started);
            if let Drag::Bend { started, .. } = &mut app.drag {
                *started = true;
            }
        }
        let _ = part;
        return;
    }
    if matches!(app.drag, Drag::Paint(..)) {
        if let Drag::Paint(mut s, t0) = std::mem::replace(&mut app.drag, Drag::None) {
            let now = ui.input(|i| i.time);
            app.ed.paint_to(&mut s, p, None, (now - t0).max(0.001));
            app.drag = Drag::Paint(s, now);
        }
        return;
    }
    if matches!(app.drag, Drag::Sculpt(_)) {
        if let Drag::Sculpt(mut sc) = std::mem::replace(&mut app.drag, Drag::None) {
            app.ed.sculpt_drag(&mut sc, p);
            app.drag = Drag::Sculpt(sc);
        }
        return;
    }
    if let Drag::BenderLine { cur, .. } = &mut app.drag {
        *cur = p;
        return;
    }
    if let Drag::ParticleAim { origin, cur } = &mut app.drag {
        *cur = p;
        let d = pt(p.x - origin.x, p.y - origin.y);
        if (d.x * d.x + d.y * d.y).sqrt() * app.zoom > 5.0 {
            let (dir, speed) = (d.y.atan2(d.x).to_degrees(), (d.x * d.x + d.y * d.y).sqrt() * 2.0);
            app.ed.modify_selected_continuous(move |e| {
                if let ElementKind::Particles(pe) = &mut e.kind {
                    pe.direction = dir;
                    pe.speed = speed;
                }
            });
        }
        return;
    }
    if let Drag::AssistPt { i, k, last, started } = app.drag {
        if let Some(mut a) = app.ed.doc.assistants.get(i).cloned() {
            match k {
                Some(k) => a.set_point(k, p),
                None => a.translate(pt(p.x - last.x, p.y - last.y)),
            }
            app.ed.set_assistant(i, a, !started);
        }
        app.drag = Drag::AssistPt { i, k, last: p, started: true };
        return;
    }
    if let Drag::Hint { key, index, mut started } = app.drag {
        crate::motion_ui::hint_drag(app, key, index, p, &mut started);
        app.drag = Drag::Hint { key, index, started };
        return;
    }
    if let Drag::MotionPath { grab, mut started } = app.drag {
        crate::motion_ui::path_drag(app, grab, p, &mut started, alt);
        app.drag = Drag::MotionPath { grab, started };
        return;
    }
    match &mut app.drag {
        Drag::Pan { last } => {
            app.pan += pos - *last;
            *last = pos;
        }
        Drag::ZoomBox { cur, .. } => *cur = pos,
        Drag::RotView { start, base } => {
            let c = rect.center();
            let mut a = *base + (pos.y - c.y).atan2(pos.x - c.x) - *start;
            if shift {
                let step = 15f32.to_radians();
                a = (a / step).round() * step;
            }
            app.set_view_rot(rect, a);
        }
        Drag::Move { last, moved, raw, applied, start, targets } => {
            let d = pt(p.x - last.x, p.y - last.y);
            *last = p;
            if !*moved {
                // Alt-drag with the Selection tool drags a copy (one undo step)
                if !(alt && app.tool == Tool::Select && app.ed.alt_drag_copy()) {
                    app.ed.begin_drag();
                } else {
                    app.status = "Copy by dragging".into();
                }
                *moved = true;
            }
            raw.x += d.x;
            raw.y += d.y;
            let mut want = *raw;
            app.snap_guides = (None, None);
            if let (true, Some((lo, hi))) = (snap_on, *start) {
                let thr = app.view.snap_px / zoom;
                let (ax, gx) = snap_axis([lo.x + raw.x, (lo.x + hi.x) / 2.0 + raw.x, hi.x + raw.x], &targets.0, thr);
                let (ay, gy) = snap_axis([lo.y + raw.y, (lo.y + hi.y) / 2.0 + raw.y, hi.y + raw.y], &targets.1, thr);
                want = pt(raw.x + ax, raw.y + ay);
                app.snap_guides = (gx, gy);
            }
            if let (true, Some((lo, _))) = (app.view.snap_pixels, *start) {
                // Snap to Pixels: the selection's top-left lands on a whole pixel
                if app.snap_guides.0.is_none() {
                    want.x = (lo.x + want.x).round() - lo.x;
                }
                if app.snap_guides.1.is_none() {
                    want.y = (lo.y + want.y).round() - lo.y;
                }
            }
            let inc = pt(want.x - applied.x, want.y - applied.y);
            *applied = want;
            app.ed.modify_selected_continuous(|e| {
                e.matrix.tx += inc.x;
                e.matrix.ty += inc.y;
            });
            if let Some((_, pv)) = &mut app.ft_pivot {
                pv.x += inc.x;
                pv.y += inc.y;
            }
        }
        Drag::Pivot => {
            let sel = app.ed.selection.clone();
            app.ft_pivot = Some((sel, p));
        }
        Drag::Transform { mode, applied, started } => {
            if !*started {
                app.ed.begin_edit(match mode { TMode::Scale { .. } => "Scale", TMode::Rotate { .. } => "Rotate", TMode::Skew { .. } => "Skew" });
                *started = true;
            }
            let total = match *mode {
                TMode::Scale { anchor: a, start: s, ax, ay } => {
                    let mut sx = if ax && (s.x - a.x).abs() > 0.5 { (p.x - a.x) / (s.x - a.x) } else { 1.0 };
                    let mut sy = if ay && (s.y - a.y).abs() > 0.5 { (p.y - a.y) / (s.y - a.y) } else { 1.0 };
                    if shift && ax && ay {
                        let u = sx.abs().max(sy.abs());
                        sx = u * sx.signum();
                        sy = u * sy.signum();
                    }
                    let clamp = |v: f32| if v.abs() < 0.01 { 0.01_f32.copysign(if v == 0.0 { 1.0 } else { v }) } else { v };
                    Matrix::translate(a.x, a.y).concat(&Matrix::scale(clamp(sx), clamp(sy))).concat(&Matrix::translate(-a.x, -a.y))
                }
                TMode::Rotate { center: c, start_angle } => {
                    let mut ang = (p.y - c.y).atan2(p.x - c.x) - start_angle;
                    if shift {
                        let step = std::f32::consts::PI / 4.0;
                        ang = (ang / step).round() * step;
                    }
                    Matrix::translate(c.x, c.y).concat(&Matrix::rotate(ang)).concat(&Matrix::translate(-c.x, -c.y))
                }
                TMode::Skew { horizontal, anchor: a, start: s } => {
                    if horizontal {
                        let k = if (s.y - a.y).abs() > 0.5 { (p.x - s.x) / (s.y - a.y) } else { 0.0 };
                        Matrix { a: 1.0, b: 0.0, c: k, d: 1.0, tx: -k * a.y, ty: 0.0 }
                    } else {
                        let k = if (s.x - a.x).abs() > 0.5 { (p.y - s.y) / (s.x - a.x) } else { 0.0 };
                        Matrix { a: 1.0, b: k, c: 0.0, d: 1.0, tx: 0.0, ty: -k * a.x }
                    }
                }
            };
            if let Some(inv) = applied.invert() {
                let inc = total.concat(&inv);
                app.ed.modify_selected_continuous(|e| e.matrix = inc.concat(&e.matrix));
                if let Some((_, pv)) = &mut app.ft_pivot {
                    *pv = inc.apply(*pv);
                }
                *applied = total;
            }
        }
        Drag::FtWarp { envelope, handle, lo, hi, handles, base, started } => {
            if !*started {
                app.ed.begin_edit(if *envelope { "Envelope" } else { "Distort" });
                *started = true;
            }
            handles[*handle] = p;
            let (lo, hi, envelope) = (*lo, *hi, *envelope);
            let hs = handles.clone();
            let base = base.clone();
            let keep = app.ed.selection.clone();
            for (s, world) in base {
                let out = if envelope {
                    let g: [Pt; 9] = hs.clone().try_into().unwrap_or(deform::envelope_grid(lo, hi));
                    deform::envelope(&world, lo, hi, &g)
                } else {
                    let q: [Pt; 4] = hs.clone().try_into().unwrap_or([lo, pt(hi.x, lo.y), hi, pt(lo.x, hi.y)]);
                    deform::distort(&world, lo, hi, &q)
                };
                app.ed.selection = vec![s];
                app.ed.modify_selected_continuous(|e| {
                    let m = e.matrix;
                    if let (ElementKind::Shape(sh), Some(inv)) = (&mut e.kind, m.invert()) {
                        sh.path = out.transformed(&inv);
                        sh.warp = None;
                    }
                });
            }
            app.ed.selection = keep;
        }
        Drag::Gizmo3D { kind, last, center, started } => {
            let (kind, center, last0, first) = (*kind, *center, *last, !*started);
            *last = pos;
            *started = true;
            let d = pos - last0;
            let dw = {
                let a = app.to_doc(rect, last0);
                pt(p.x - a.x, p.y - a.y)
            };
            let a0 = (last0.y - center.y).atan2(last0.x - center.x);
            let a1 = (pos.y - center.y).atan2(pos.x - center.x);
            if first {
                app.ed.begin_edit(match kind { Grab3D::TX | Grab3D::TY | Grab3D::TZ => "3D Translation", Grab3D::Center => "3D Center", _ => "3D Rotation" });
                app.ensure_3d();
            }
            let mut dz = (a1 - a0).to_degrees();
            if dz > 180.0 {
                dz -= 360.0;
            } else if dz < -180.0 {
                dz += 360.0;
            }
            app.ed.modify_selected_continuous(|e| {
                let mut t = e.three_d.unwrap_or_default();
                match kind {
                    Grab3D::X => t.rx -= d.y * 0.5,
                    Grab3D::Y => t.ry += d.x * 0.5,
                    Grab3D::Z => t.rz += dz,
                    Grab3D::Free => {
                        t.rx -= d.y * 0.5;
                        t.ry += d.x * 0.5;
                    }
                    Grab3D::Center => {
                        if let Some(inv) = e.matrix.invert() {
                            let cw = e.matrix.apply(t.center);
                            t.center = inv.apply(pt(cw.x + dw.x, cw.y + dw.y));
                        }
                    }
                    Grab3D::TX => e.matrix.tx += dw.x,
                    Grab3D::TY => e.matrix.ty += dw.y,
                    Grab3D::TZ => t.z += d.y * 2.0 / zoom,
                }
                e.three_d = Some(t);
            });
        }
        Drag::Grad { sel, handle, started } => {
            let (sel, handle) = (*sel, *handle);
            if !*started {
                *started = true;
                app.ed.begin_edit("Gradient Transform");
            }
            let Some(el) = app.ed.element_at(sel) else { return };
            let Some(inv) = el.matrix.invert() else { return };
            let lp = inv.apply(p);
            app.ed.selection = vec![sel];
            app.ed.modify_selected_continuous(|e| {
                let ElementKind::Shape(sh) = &mut e.kind else { return };
                let linear = sh.gradient.as_ref().map(|g| g.kind == ac_core::GradientKind::Linear).unwrap_or(false);
                // bitmap fills: the handles sit BMP_UNIT pixels out, so scale the vectors back
                let (m, unit) = match (&mut sh.gradient, &mut sh.bitmap_fill) {
                    (Some(g), _) => (&mut g.matrix, 1.0),
                    (None, Some(b)) => (&mut b.matrix, BMP_UNIT),
                    _ => return,
                };
                let lp = if handle == 0 { lp } else { pt(m.tx + (lp.x - m.tx) / unit, m.ty + (lp.y - m.ty) / unit) };
                match handle {
                    0 => {
                        m.tx = lp.x;
                        m.ty = lp.y;
                    }
                    1 => {
                        let u = pt(lp.x - m.tx, lp.y - m.ty);
                        let len = (u.x * u.x + u.y * u.y).sqrt().max(0.5);
                        let len2 = (m.c * m.c + m.d * m.d).sqrt();
                        let len2 = if linear { len } else { len2 };
                        m.a = u.x;
                        m.b = u.y;
                        m.c = -u.y / len * len2;
                        m.d = u.x / len * len2;
                    }
                    _ => {
                        let u = pt(m.a, m.b);
                        let len = (u.x * u.x + u.y * u.y).sqrt().max(1e-3);
                        let n = pt(-u.y / len, u.x / len);
                        let h = ((lp.x - m.tx) * n.x + (lp.y - m.ty) * n.y).abs().max(0.5);
                        m.c = n.x * h;
                        m.d = n.y * h;
                    }
                }
            });
        }
        Drag::BoneDraw { cur, .. } => *cur = p,
        Drag::Ik { layer, element, grab, started } => {
            if !*started {
                *started = true;
                app.ed.begin_edit("Pose Bones");
            }
            let (l, e, g) = (*layer, *element, *grab);
            app.ed.ik_drag(l, e, g, p);
        }
        Drag::MergeWidth { layer, line, t, started } => {
            let (li, ln, t) = (*layer, *line, *t);
            let first = !*started;
            *started = true;
            let Some((_, ei)) = app.ed.canvas_at(li) else { return };
            let Some(m) = app.ed.element_at(Selection { layer: li, element: ei }).map(|e| e.matrix) else { return };
            let Some(s) = app.ed.canvas(li).and_then(|c| c.strokes.get(ln)).cloned() else { return };
            let Some(c) = deform::point_at(&s.path(), t) else { return };
            let lp = m.invert().map(|i| i.apply(p)).unwrap_or(p);
            let mult = (2.0 * lp.dist(c) / s.stroke.width.max(0.1)).clamp(0.05, 60.0);
            app.ed.merge_set_width(li, ln, t, mult, first);
            app.status = format!("Width {:.1} px at {:.0}% along the line", mult * s.stroke.width, t * 100.0);
        }
        Drag::WidthPt { sel, t, started } => {
            let (sel, t) = (*sel, *t);
            if !*started {
                *started = true;
                app.ed.begin_edit("Width");
            }
            let Some(el) = app.ed.element_at(sel) else { return };
            let Some(inv) = el.matrix.invert() else { return };
            let ElementKind::Shape(sh) = &el.kind else { return };
            let (Some(st), Some(c)) = (sh.stroke, deform::point_at(&sh.path, t)) else { return };
            let lp = inv.apply(p);
            let mult = (2.0 * lp.dist(c) / st.width.max(0.1)).clamp(0.05, 60.0);
            app.ed.selection = vec![sel];
            app.ed.modify_selected_continuous(|e| {
                if let ElementKind::Shape(sh) = &mut e.kind {
                    let prof = sh.width_profile.get_or_insert_with(|| vec![(0.0, 1.0), (1.0, 1.0)]);
                    match prof.iter_mut().find(|(pt_, _)| (pt_ - t).abs() < 0.02) {
                        Some(w) => w.1 = mult,
                        None => {
                            prof.push((t, mult));
                            prof.sort_by(|a, b| a.0.total_cmp(&b.0));
                        }
                    }
                }
            });
            app.status = format!("Width {:.1} px at {:.0}% along the stroke", mult * st.width, t * 100.0);
        }
        Drag::MeshPin { sel, pin, started } => {
            let (sel, pin, first) = (*sel, *pin, !*started);
            *started = true;
            app.ed.mesh_pin_move(sel, pin, p, first);
        }
        Drag::WarpPin { sel, pin, started } => {
            let (sel, pin) = (*sel, *pin);
            if !*started {
                *started = true;
                app.ed.begin_edit("Asset Warp");
            }
            let Some(inv) = app.ed.element_at(sel).and_then(|e| e.matrix.invert()) else { return };
            let lp = inv.apply(p);
            app.ed.selection = vec![sel];
            app.ed.modify_selected_continuous(|e| {
                if let ElementKind::Shape(sh) = &mut e.kind {
                    if let Some(w) = &mut sh.warp {
                        if let Some(pp) = w.pins.get_mut(pin) {
                            pp.1 = lp;
                        }
                        sh.path = w.apply();
                    }
                }
            });
        }
        Drag::VCam { mode, from, cam0, started } => {
            let (mode, from, cam0, first) = (*mode, *from, *cam0, !*started);
            *started = true;
            if first {
                app.ed.begin_edit("Camera");
            }
            let (c, rot, z) = Editor::camera_params(&cam0);
            // the frame's centre as shown (follow/shake move it)
            let shown = vcam_quad(app, app.ed.frame).map(|q| pt((q[0].x + q[2].x) / 2.0, (q[0].y + q[2].y) / 2.0)).unwrap_or(c);
            let following = app.ed.camera_opts().is_some_and(|o| o.follow.as_deref().is_some_and(|f| !f.is_empty()));
            match mode {
                VCamGrab::Move if following => {
                    // following an object: the drag sets the offset from it
                    let mut o = app.ed.camera_opts().unwrap_or_default();
                    o.follow_offset = pt(o.follow_offset.x + p.x - from.x, o.follow_offset.y + p.y - from.y);
                    if let Drag::VCam { from, .. } = &mut app.drag {
                        *from = p;
                    }
                    app.ed.set_camera_opts(o);
                    app.status = "Camera offset from the followed object".into();
                }
                VCamGrab::Move => {
                    app.ed.set_camera(Editor::camera_from(pt(c.x + p.x - from.x, c.y + p.y - from.y), rot, z));
                }
                VCamGrab::Zoom => {
                    let (d0, d1) = (from.dist(shown).max(1.0), p.dist(shown).max(1.0));
                    let nz = (z * d0 / d1).clamp(0.05, 50.0);
                    app.ed.set_camera(Editor::camera_from(c, rot, nz));
                    app.status = format!("Camera zoom {:.0}%", nz * 100.0);
                }
                VCamGrab::Rotate => {
                    let a0 = (from.y - shown.y).atan2(from.x - shown.x);
                    let a1 = (p.y - shown.y).atan2(p.x - shown.x);
                    let mut nr = rot + (a1 - a0).to_degrees();
                    if shift {
                        nr = (nr / 15.0).round() * 15.0;
                    }
                    app.ed.set_camera(Editor::camera_from(c, nr, z));
                    app.status = format!("Camera rotation {nr:.0}°");
                }
            }
        }
        Drag::CameraPan { last, rotate, started } => {
            let (rotate, l, first) = (*rotate, *last, !*started);
            *last = pos;
            *started = true;
            if first {
                app.ed.begin_edit("Camera");
            }
            let a = app.to_doc(rect, l);
            let cam = app.ed.camera_at(app.ed.frame).unwrap_or_else(|| ac_core::default_camera(&app.ed.doc).matrix);
            let (c, rot, z) = Editor::camera_params(&cam);
            let new = if rotate {
                let sc = pt(app.ed.doc.width / 2.0, app.ed.doc.height / 2.0);
                let da = (p.y - sc.y).atan2(p.x - sc.x) - (a.y - sc.y).atan2(a.x - sc.x);
                Editor::camera_from(c, rot - da.to_degrees(), z)
            } else {
                // a view-space drag moves the scene with the mouse: the camera goes the other way
                let d = pt(p.x - a.x, p.y - a.y);
                let r = rot.to_radians();
                let w = pt((d.x * r.cos() - d.y * r.sin()) / z, (d.x * r.sin() + d.y * r.cos()) / z);
                Editor::camera_from(pt(c.x - w.x, c.y - w.y), rot, z)
            };
            app.ed.set_camera(new);
        }
        Drag::Guide { vertical, index, pos, started } => {
            let (v, i) = (*vertical, *index);
            let first = !*started;
            *started = true;
            let np = app_snap(&app.view, &Default::default(), if v { p.x } else { p.y }, v, zoom).round();
            *pos = np;
            if first {
                app.ed.begin_edit("Move Guide");
            }
            app.ed.move_guide(v, i, np);
        }
        Drag::GuideNew { .. } => {}
        Drag::Marquee { cur, .. } => *cur = p,
        Drag::TextBox { sel, start } => {
            let w = p.x - start.x;
            if w.abs() > 6.0 / zoom {
                let (s, x0, w) = (*sel, start.x.min(p.x), w.abs().max(10.0));
                app.ed.selection = vec![s];
                app.ed.modify_selected_continuous(|e| {
                    if let ElementKind::Text(t) = &mut e.kind {
                        t.para.width = Some(w);
                        e.matrix.tx = x0;
                    }
                });
            }
        }
        Drag::Draw { start, cur } => {
            let sp = pt(app_snap(&app.view, &app.ed.doc.guides, p.x, true, zoom), app_snap(&app.view, &app.ed.doc.guides, p.y, false, zoom));
            *cur = sp;
            let p = sp;
            if shift {
                if tool_now == Tool::Line {
                    let (dx, dy) = (p.x - start.x, p.y - start.y);
                    let ang = (dy.atan2(dx) / (std::f32::consts::PI / 4.0)).round() * (std::f32::consts::PI / 4.0);
                    let len = (dx * dx + dy * dy).sqrt();
                    *cur = pt(start.x + len * ang.cos(), start.y + len * ang.sin());
                } else {
                    let s = (p.x - start.x).abs().max((p.y - start.y).abs());
                    *cur = pt(start.x + s * (p.x - start.x).signum(), start.y + s * (p.y - start.y).signum());
                }
            }
        }
        Drag::Freehand { pts } => {
            if pts.last().map(|l| l.dist(p) > 1.5 / zoom).unwrap_or(true) {
                pts.push(p);
            }
        }
        Drag::Fluid { pts } => {
            if pts.last().map(|l| l.0.dist(p) > 2.0 / zoom).unwrap_or(true) {
                pts.push((p, ui.input(|i| i.time)));
            }
        }
        Drag::Point { sel, r, started } => {
            if !*started {
                app.ed.begin_edit("Edit Points");
                *started = true;
            }
            let (sel, r) = (*sel, *r);
            if let Some(inv) = app.ed.element_at(sel).and_then(|e| e.matrix.invert()) {
                let lp = inv.apply(p);
                app.ed.selection = vec![sel];
                app.ed.modify_selected_continuous(|e| {
                    if let ElementKind::Shape(sh) = &mut e.kind {
                        sh.path.set_point(r, lp);
                        sh.warp = None;
                    }
                });
            }
        }
        Drag::Erase { last, start, .. } if app.ed.canvas_at(app.ed.layer).is_some() && !app.topts.faucet => {
            let (l, st) = (*last, *start);
            if l.dist(p) * app.zoom > 1.5 {
                merge_erase_at(app, l, p, st);
                if let Drag::Erase { last, .. } = &mut app.drag {
                    *last = p;
                }
            }
        }
        Drag::Erase { .. } => {
            let removed = erase_at(app, p);
            if let Drag::Erase { removed: r, last, .. } = &mut app.drag {
                *r += removed;
                *last = p;
            }
        }
        Drag::None | Drag::MotionPath { .. } | Drag::Hint { .. } | Drag::Bend { .. } | Drag::RigIk { .. } | Drag::Sculpt(_) | Drag::BenderLine { .. } | Drag::ParticleAim { .. } | Drag::AssistPt { .. } | Drag::Paint(..) => {}
    }
}

fn release(app: &mut App, rect: Rect, p: Pt, shift: bool, alt: bool) {
    if let Some(pen) = &mut app.pen {
        pen.dragging = false;
    }
    app.snap_guides = (None, None);
    let drag = std::mem::replace(&mut app.drag, Drag::None);
    match drag {
        Drag::Guide { vertical, index, started: true, .. } => {
            let off = app.ctx.input(|i| i.pointer.latest_pos()).map(|h| !rect.contains(h)).unwrap_or(false);
            if off {
                app.ed.remove_guide(vertical, index);
                app.status = "Guide removed".into();
            }
        }
        Drag::Move { moved: false, .. } => {
            if !shift && app.ed.selection.len() > 1 {
                if let Some(hit) = app.ed.hit_test(p) {
                    app.ed.select(Some(hit), false);
                }
            }
        }
        Drag::Paint(s, _) => {
            let id = s.bitmap;
            app.ed.paint_end(*s);
            if app.ed.doc.bitmap(id).is_some() {
                app.status = "Raster Brush: painted into a bitmap on this keyframe (F6 then paint again for the next drawing)".into();
            }
        }
        Drag::Sculpt(sc) => {
            app.ed.sculpt_end(&sc);
            if sc.started {
                app.status = format!("{}: {} object(s) changed", sc.kind.name(), sc.targets.len());
            }
        }
        Drag::BenderLine { a, cur } => {
            if a.dist(cur) * app.zoom > 4.0 {
                app.bender_seg = Some((a, cur));
                app.status = "Bender: now drag on one side of the segment to bend the lines it crosses (press away from it to draw a new one).".into();
            }
        }
        Drag::Bend { layer, key, ei, started, part, .. } => {
            if started {
                app.ed.merge_finish_bend(layer, key, ei);
            } else if let Some(part) = part {
                app.ed.merge_select(layer, part, false, shift);
            }
        }
        Drag::Marquee { start, cur, add } => {
            if start.dist(cur) > 2.0 / app.zoom {
                app.ed.select_in_rect(start, cur, add);
                let (lo, hi) = (pt(start.x.min(cur.x), start.y.min(cur.y)), pt(start.x.max(cur.x), start.y.max(cur.y)));
                app.ed.merge_select_area(&[vec![lo, pt(hi.x, lo.y), hi, pt(lo.x, hi.y)]], true);
                let n = app.ed.selection.len();
                app.status = if n == 0 { "No objects fully inside the box (locked layers are skipped).".into() } else { format!("{n} object(s) selected") };
            }
        }
        Drag::ZoomBox { start, cur } => {
            if start.distance(cur) < 4.0 {
                app.zoom_at(rect, cur, if alt { 0.5 } else { 2.0 });
            } else {
                let r = Rect::from_two_pos(start, cur);
                let factor = (rect.width() / r.width()).min(rect.height() / r.height()).clamp(0.1, 16.0);
                let c = r.center();
                app.zoom_at(rect, c, factor);
                app.pan += rect.center() - c;
            }
        }
        Drag::Draw { start, cur } if start.dist(cur) > 2.0 && matches!(app.tool, Tool::RectPrimitive | Tool::OvalPrimitive) => {
            let (x0, y0, x1, y1) = (start.x.min(cur.x), start.y.min(cur.y), start.x.max(cur.x), start.y.max(cur.y));
            let o = &app.topts;
            let prim = if app.tool == Tool::RectPrimitive {
                ac_core::Primitive::Rect { x: x0, y: y0, w: x1 - x0, h: y1 - y0, radius: [o.rect_radius; 4] }
            } else {
                ac_core::Primitive::Oval { cx: (x0 + x1) / 2.0, cy: (y0 + y1) / 2.0, rx: (x1 - x0) / 2.0, ry: (y1 - y0) / 2.0, start: o.oval_start, end: o.oval_end, inner: o.oval_inner, close: true }
            };
            let mut shape = app.make_shape(prim.path(), app.cur_fill(), app.cur_stroke());
            shape.primitive = Some(prim);
            let name = if app.tool == Tool::RectPrimitive { "Rectangle Primitive" } else { "Oval Primitive" };
            if app.ed.add_element(Element::shape(name, shape)).is_none() {
                app.locked_msg();
            }
        }
        Drag::Draw { start, cur } if start.dist(cur) > 2.0 => {
            let (x0, y0, x1, y1) = (start.x.min(cur.x), start.y.min(cur.y), start.x.max(cur.x), start.y.max(cur.y));
            let o = &app.topts;
            let (name, path, fill) = match app.tool {
                Tool::Rect => ("Rectangle", Path::rounded_rect(x0, y0, x1 - x0, y1 - y0, o.rect_radius), app.cur_fill()),
                Tool::Oval => ("Oval", oval_path((x0 + x1) / 2.0, (y0 + y1) / 2.0, (x1 - x0) / 2.0, (y1 - y0) / 2.0, o.oval_start, o.oval_end, o.oval_inner), app.cur_fill()),
                Tool::PolyStar => ("PolyStar", Path::polystar((x0 + x1) / 2.0, (y0 + y1) / 2.0, (x1 - x0) / 2.0, (y1 - y0) / 2.0, o.poly_sides, o.poly_star, o.poly_inner), app.cur_fill()),
                _ => ("Line", Path { segs: vec![Seg::M(start), Seg::L(cur)] }, None),
            };
            let stroke = if app.tool == Tool::Line { Some(Stroke { color: from32(app.stroke), width: app.stroke_w }) } else { app.cur_stroke() };
            if fill.is_none() && stroke.is_none() {
                app.status = "Both fill and stroke are set to none.".into();
            } else {
                let shape = app.make_shape(path, fill, stroke);
                if !app.place_shape(name, shape) {
                    app.locked_msg();
                }
            }
        }
        Drag::Freehand { pts } => match app.tool {
            Tool::Lasso => {
                if pts.len() > 2 {
                    lasso_select(app, &pts, shift);
                }
            }
            Tool::Pencil | Tool::Brush if app.assisting() && pts.len() > 1 => {
                // drawing assistants: snapped and replicated strokes, one undo step
                let mark = app.ed.undo_depth();
                let tool = app.tool;
                for poly in ac_core::assist::apply(&app.ed.doc.assistants, &pts, 1.0 / app.zoom) {
                    app.drag = Drag::Freehand { pts: poly };
                    app.assist_bypass = true;
                    release(app, rect, p, shift, alt);
                    app.assist_bypass = false;
                }
                app.ed.squash_undo(mark, if tool == Tool::Pencil { "Pencil" } else { "Brush" });
            }
            Tool::Pencil if pts.len() > 1 => {
                let path = match app.topts.pencil_mode {
                    PencilMode::Straighten => {
                        let simple = ac_core::geom::simplify(&pts, 4.0 / app.zoom);
                        let mut p = Path::polygon(&simple);
                        p.segs.pop();
                        p
                    }
                    PencilMode::Smooth => Path::smooth_polyline(&pts, 1.5 / app.zoom),
                    PencilMode::Ink => Path::smooth_polyline(&pts, 0.3 / app.zoom),
                };
                let stroke = Some(Stroke { color: from32(app.stroke), width: app.stroke_w });
                if !app.place_shape("Pencil", Shape::new(path, None, stroke)) {
                    app.locked_msg();
                }
            }
            Tool::Brush => {
                if let Some(path) = brush_outline(&pts, app.topts.brush_size, app.topts.brush_smooth, app.topts.brush_shape) {
                    let shape = app.make_shape(path, Some(from32(app.fill)), None);
                    let mode = app.topts.brush_mode;
                    let ok = if app.topts.object_draw || shape.gradient.is_some() || mode == ac_core::merge::PaintMode::Normal {
                        app.place_shape("Brush", shape)
                    } else {
                        let polys = ac_core::merge::path_polygons(&shape.path);
                        app.ed.merge_paint(&polys, from32(app.fill), mode, pts[0])
                    };
                    if !ok {
                        app.locked_msg();
                    }
                }
            }
            _ => {}
        },
        Drag::Fluid { pts } if app.assisting() && pts.len() > 1 => {
            let mark = app.ed.undo_depth();
            let raw: Vec<Pt> = pts.iter().map(|x| x.0).collect();
            for poly in ac_core::assist::apply(&app.ed.doc.assistants, &raw, 1.0 / app.zoom) {
                app.drag = Drag::Fluid { pts: poly.into_iter().zip(pts.iter().map(|x| x.1)).collect() };
                app.assist_bypass = true;
                release(app, rect, p, shift, alt);
                app.assist_bypass = false;
            }
            app.ed.squash_undo(mark, "Fluid Brush");
        }
        Drag::Fluid { pts } => {
            let widths = fluid_widths(&pts, app.topts.fluid_size, app.topts.fluid_sensitivity, app.zoom);
            let centre: Vec<Pt> = pts.iter().map(|x| x.0).collect();
            let path = if centre.len() == 1 { Path::ellipse(centre[0].x, centre[0].y, app.topts.fluid_size / 2.0, app.topts.fluid_size / 2.0) } else { deform::outline_polyline(&centre, &widths, false) };
            let shape = app.make_shape(path, Some(from32(app.fill)), None);
            if !app.place_shape("Fluid Brush", shape) {
                app.locked_msg();
            }
        }
        Drag::BoneDraw { layer, parent, head, cur } if app.ed.hit_test_on_layer(cur, layer, None) == Some(parent) || app.ed.rig_bones().iter().any(|(s, _, _, t)| s.layer == layer && s.element == parent && t.dist(head) < 0.01) => {
            if head.dist(cur) < 3.0 / app.zoom {
                app.status = "Bone: drag inside the shape to give it a bone.".into();
                return;
            }
            let s = Selection { layer, element: parent };
            match app.ed.rig_add_bone(s, head, cur) {
                Some(b) => app.status = format!("Shape bone {} added. Drag a bone with the Selection tool to bend the shape; the Bind tool chooses which points follow which bone.", b + 1),
                None => app.status = "Shape bones go inside a drawing object (a shape, not a symbol or group).".into(),
            }
        }
        Drag::BoneDraw { layer, parent, head, cur } => {
            if head.dist(cur) < 3.0 / app.zoom {
                app.status = "Bone: drag from one object to another.".into();
                return;
            }
            match app.ed.hit_test_on_layer(cur, layer, Some(parent)) {
                Some(child) => match app.ed.add_bone(layer, parent, child, head, cur) {
                    Ok(()) => {
                        let n = app.ed.bones(layer).len();
                        app.status = format!("Bone added ({n} on this layer). Drag objects with the Selection tool (V) to pose them.");
                    }
                    Err(e) => app.status = format!("Can't add bone: {e}."),
                },
                None => app.status = "Bone: end the drag on another object on the same layer.".into(),
            }
        }
        Drag::Erase { .. } if app.ed.canvas_at(app.ed.layer).is_some() => {}
        Drag::Erase { mark, removed, .. }
            if removed > 0 => {
                app.ed.squash_undo(mark, "Erase");
                app.status = format!("Erased {removed} object(s)");
            }
        _ => {}
    }
}

/// Eraser (object mode): delete the topmost editable object under `p`.
/// Eraser on a merge shape: the swept capsule from `last` to `p`.
fn merge_erase_at(app: &mut App, last: Pt, p: Pt, start: Pt) {
    let r = app.topts.eraser_size / 2.0;
    let d = pt(p.x - last.x, p.y - last.y);
    let len = (d.x * d.x + d.y * d.y).sqrt();
    let a0 = if len > 1e-3 { d.y.atan2(d.x) } else { 0.0 };
    let mut poly = vec![];
    for k in 0..=12 {
        let a = a0 + std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * k as f32 / 12.0;
        poly.push(pt(last.x + r * a.cos(), last.y + r * a.sin()));
    }
    for k in 0..=12 {
        let a = a0 - std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * k as f32 / 12.0;
        poly.push(pt(p.x + r * a.cos(), p.y + r * a.sin()));
    }
    app.ed.merge_erase(&[poly], app.topts.eraser_mode, start, None);
}

fn erase_at(app: &mut App, p: Pt) -> usize {
    let r = app.topts.eraser_size / 2.0;
    let probes = [p, pt(p.x + r, p.y), pt(p.x - r, p.y), pt(p.x, p.y + r), pt(p.x, p.y - r)];
    for q in probes {
        if let Some(h) = app.ed.hit_test(q) {
            if app.ed.layer_editable(h.layer) {
                app.ed.selection = vec![h];
                return app.ed.delete_selected();
            }
        }
    }
    0
}

/// Eyedropper: pick fill (incl. gradients) or stroke under the cursor and
/// switch to the tool that applies it (Animate behaviour).
fn eyedrop(app: &mut App, p: Pt) {
    let items = app.ed.evaluate_current(app.ed.frame);
    let Some(it) = items.iter().rev().find(|it| it.hit(p)) else {
        app.status = "Eyedropper: click a coloured object.".into();
        return;
    };
    let on_edge = it.stroke.is_some() && it.matrix.invert().map(|inv| it.path.hit(inv.apply(p), false, it.stroke.unwrap().width / 2.0 + 2.0 / app.zoom)).unwrap_or(false);
    if let (true, Some(st)) = (on_edge || (it.fill.is_none() && it.gradient.is_none()), it.stroke) {
        app.stroke = crate::c32(st.color);
        app.stroke_w = st.width;
        app.stroke_on = true;
        app.set_tool(Tool::InkBottle);
        app.status = format!("Picked stroke {} — Ink Bottle selected", st.color.to_hex());
    } else if let Some(g) = &it.gradient {
        if let (Some(a), Some(b)) = (g.stops.first(), g.stops.last()) {
            app.fill = crate::c32(a.1);
            app.topts.fill2 = crate::c32(b.1);
            app.topts.fill_gradient = Some(g.kind);
            app.fill_on = true;
            app.set_tool(Tool::Bucket);
            app.status = format!("Picked {} gradient {} → {} — Paint Bucket selected", if g.kind == ac_core::GradientKind::Linear { "linear" } else { "radial" }, a.1.to_hex(), b.1.to_hex());
        }
    } else if let Some(f) = it.fill {
        app.fill = crate::c32(f);
        app.fill_on = true;
        app.topts.fill_gradient = None;
        app.set_tool(Tool::Bucket);
        app.status = format!("Picked fill {} — Paint Bucket selected", f.to_hex());
    }
}

/// Clip Content Outside Stage: paint the pasteboard back over everything
/// off the stage (in `view`, which may be rotated), then its drop shadow.
fn clip_outside_stage(pix: &mut sk::Pixmap, view: &Matrix, w: f32, h: f32) {
    let quad = |x: f32, y: f32| [view.apply(Pt { x, y }), view.apply(Pt { x: x + w, y }), view.apply(Pt { x: x + w, y: y + h }), view.apply(Pt { x, y: y + h })];
    let ring = |pb: &mut sk::PathBuilder, q: [Pt; 4]| {
        pb.move_to(q[0].x, q[0].y);
        for p in &q[1..] {
            pb.line_to(p.x, p.y);
        }
        pb.close();
    };
    let stage = quad(0.0, 0.0);
    let (pw, ph) = (pix.width() as f32, pix.height() as f32);
    let mut pb = sk::PathBuilder::new();
    if let Some(r) = sk::Rect::from_xywh(0.0, 0.0, pw, ph) {
        pb.push_rect(r);
    }
    ring(&mut pb, stage);
    let mut paint = sk::Paint::default();
    paint.set_color(sk::Color::from_rgba8(0x3C, 0x3C, 0x3C, 255));
    if let Some(p) = pb.finish() {
        pix.fill_path(&p, &paint, sk::FillRule::EvenOdd, sk::Transform::identity(), None);
    }
    for (off, a) in [(6.0, 40u8), (3.0, 60)] {
        let mut pb = sk::PathBuilder::new();
        ring(&mut pb, quad(off, off));
        ring(&mut pb, stage);
        paint.set_color(sk::Color::from_rgba8(0, 0, 0, a));
        if let Some(p) = pb.finish() {
            pix.fill_path(&p, &paint, sk::FillRule::EvenOdd, sk::Transform::identity(), None);
        }
    }
}

/// Zoom presets in the edit bar.
pub(crate) const ZOOM_PRESETS: [f32; 9] = [0.25, 0.5, 1.0, 2.0, 4.0, 8.0, 0.0, -1.0, -2.0];

fn zoom_menu(app: &mut App, ui: &mut egui::Ui) {
    egui::ComboBox::from_id_salt("edit-bar.zoom").width(76.0).selected_text(format!("{:.0}%", app.zoom * 100.0)).show_ui(ui, |ui| {
        for z in ZOOM_PRESETS {
            let label = match z {
                0.0 => "Fit in Window".to_string(),
                -1.0 => "Show Frame".to_string(),
                -2.0 => "Show All".to_string(),
                _ => format!("{:.0}%", z * 100.0),
            };
            if ui.selectable_label(z > 0.0 && (app.zoom - z).abs() < 1e-3, label).clicked() {
                match z {
                    0.0 | -1.0 => app.fit_pending = true,
                    -2.0 => show_all(app),
                    _ => app.zoom = z,
                }
            }
        }
    });
}

/// Zoom and scroll so every item on the current frame (and the stage) is in view.
fn show_all(app: &mut App) {
    let items = app.ed.evaluate_current(app.ed.frame);
    let d = &app.ed.doc;
    let (mut lo, mut hi) = (Pt { x: 0.0, y: 0.0 }, Pt { x: d.width, y: d.height });
    for it in &items {
        if let Some((a, b)) = it.path.transformed(&it.matrix).bounds() {
            lo = Pt { x: lo.x.min(a.x), y: lo.y.min(a.y) };
            hi = Pt { x: hi.x.max(b.x), y: hi.y.max(b.y) };
        }
    }
    app.fit_bounds = Some((lo, hi));
    app.fit_pending = true;
}

fn to_ts(m: &Matrix) -> sk::Transform {
    sk::Transform::from_row(m.a, m.b, m.c, m.d, m.tx, m.ty)
}

fn render_stage(app: &mut App, ui: &egui::Ui, rect: Rect) {
    let ppp = ui.ctx().pixels_per_point();
    let (pw, ph) = ((rect.width() * ppp).round() as u32, (rect.height() * ppp).round() as u32);
    let Some(mut pix) = sk::Pixmap::new(pw.max(1), ph.max(1)) else { return };
    pix.fill(sk::Color::from_rgba8(0x3C, 0x3C, 0x3C, 255));
    let view = Matrix::scale(ppp, ppp).concat(&app.view_base());
    let d = &app.ed.doc;
    let in_place = app.in_place().is_some();
    let stage_dim = app.ed.editing.is_some() && !in_place;
    for (off, a) in [(6.0, 40u8), (3.0, 60)] {
        if let Some(r) = sk::Rect::from_xywh(off, off, d.width, d.height) {
            let mut paint = sk::Paint::default();
            paint.set_color(sk::Color::from_rgba8(0, 0, 0, a));
            pix.fill_rect(r, &paint, to_ts(&view), None);
        }
    }
    if let Some(r) = sk::Rect::from_xywh(0.0, 0.0, d.width, d.height) {
        let mut paint = sk::Paint::default();
        let bg = d.background;
        paint.set_color(sk::Color::from_rgba8(bg.r, bg.g, bg.b, if stage_dim { 90 } else { 255 }));
        pix.fill_rect(r, &paint, to_ts(&view), None);
    }
    if in_place {
        // the scene behind, dimmed, without the object being edited
        if let Some(first) = app.place.first().copied() {
            let ctx = ac_core::EvalCtx::new(&app.ed.doc, first.scene_frame);
            let mut items = Vec::new();
            ac_core::evaluate(&ctx, app.ed.scene_tl(), first.scene_frame, &Matrix::IDENTITY, 1.0, 0, &mut items);
            if let Some(o) = first.scene_obj {
                items.retain(|it| it.src != Some((o.layer, o.element)));
            }
            ac_render::draw_items(&mut pix.as_mut(), &items, &ac_render::RenderOpts { view, alpha: 0.35, tint: None });
        }
    }
    let content_view = Matrix::scale(ppp, ppp).concat(&app.view_content());
    let n = app.ed.frame_count();
    if (app.onion || app.onion_opts.multi) && !app.playing {
        let (a, b) = app.onion_range();
        let cur = app.ed.frame;
        let span = (cur - a.min(cur)).max(b.saturating_sub(cur)).max(1) as f32;
        let keys: std::collections::BTreeSet<u32> = app.ed.timeline().layers.iter().filter(|l| l.visible).flat_map(|l| l.keyframes.keys().copied().collect::<Vec<_>>()).collect();
        // far frames first so nearer ones draw on top
        let mut frames: Vec<u32> = (a..=b.min(n.saturating_sub(1))).filter(|&f| f != cur).collect();
        frames.sort_by_key(|&f| std::cmp::Reverse((f as i64 - cur as i64).abs()));
        for f in frames {
            let multi = app.onion_opts.multi;
            if (multi || app.onion_opts.keys_only) && !keys.contains(&f) {
                continue; // Edit Multiple Frames / Keyframes Only show the keyframes
            }
            let dist = (f as i64 - cur as i64).unsigned_abs() as f32;
            let mut items = app.ed.evaluate_current(f);
            let col = if f < cur { Color::rgb(0x30, 0x60, 0xFF) } else { Color::rgb(0x20, 0xC0, 0x40) };
            let opts = if multi {
                ac_render::RenderOpts { view: content_view, alpha: 1.0, tint: None }
            } else if app.onion_opts.outline {
                for it in &mut items {
                    it.outline = Some(col);
                }
                ac_render::RenderOpts { view: content_view, alpha: (1.0 - (dist - 1.0) / (span + 1.0)).clamp(0.25, 1.0), tint: None }
            } else {
                ac_render::RenderOpts { view: content_view, alpha: 0.5 * (1.0 - (dist - 1.0) / (span + 1.0)).clamp(0.15, 1.0), tint: Some((col, 0.85)) }
            };
            ac_render::draw_items(&mut pix.as_mut(), &items, &opts);
        }
    }
    // Shift and Trace: previous / next keyframe ghosts with their own offsets
    if app.onion_opts.shift_trace && !app.playing {
        for (k, f) in shift_trace_frames(app).into_iter().enumerate() {
            let Some(f) = f else { continue };
            // only the current layer's drawing, as in OpenToonz
            let mut items = app.ed.evaluate_current(f);
            items.retain(|it| it.src.is_some_and(|s| s.0 == app.ed.layer));
            let col = if k == 0 { Color::rgb(0xE0, 0x40, 0x40) } else { Color::rgb(0x30, 0xA0, 0xE0) };
            let view = content_view.concat(&app.onion_opts.shift[k]);
            ac_render::draw_items(&mut pix.as_mut(), &items, &ac_render::RenderOpts { view, alpha: 0.45, tint: Some((col, 0.8)) });
        }
    }
    let mut items = app.ed.evaluate_current(app.ed.frame);
    crate::scripting::apply_overrides(app, &mut items);
    ac_render::draw_items(&mut pix.as_mut(), &items, &ac_render::RenderOpts { view: content_view, ..Default::default() });
    if app.view.clip {
        clip_outside_stage(&mut pix, &view, d.width, d.height);
    }
    // free camera: dim what the camera doesn't see
    if app.vcam_dim && vcam_frame_shown(app) {
        if let Some(q) = vcam_quad(app, app.ed.frame) {
            let mut pb = sk::PathBuilder::new();
            let (w, h) = (pw as f32, ph as f32);
            pb.push_rect(sk::Rect::from_xywh(0.0, 0.0, w, h).unwrap());
            let v = content_view;
            let s: Vec<Pt> = q.iter().map(|p| v.apply(*p)).collect();
            pb.move_to(s[0].x, s[0].y);
            for p in &s[1..] {
                pb.line_to(p.x, p.y);
            }
            pb.close();
            if let Some(path) = pb.finish() {
                let mut paint = sk::Paint::default();
                paint.set_color(sk::Color::from_rgba8(0, 0, 0, 80));
                pix.fill_path(&path, &paint, sk::FillRule::EvenOdd, sk::Transform::identity(), None);
            }
        }
    }

    let img = egui::ColorImage::from_rgba_premultiplied([pw as usize, ph as usize], pix.data());
    let tex = match &mut app.stage_tex {
        Some(t) => {
            t.set(img, egui::TextureOptions::NEAREST);
            t.clone()
        }
        None => {
            let t = ui.ctx().load_texture("stage", img, egui::TextureOptions::NEAREST);
            app.stage_tex = Some(t.clone());
            t
        }
    };
    ui.painter_at(rect).image(tex.id(), rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
}

/// The free camera's rotation knob: above the middle of the frame's top edge.
fn vcam_knob(s: &[Pos2]) -> Pos2 {
    let top = pos2((s[0].x + s[1].x) / 2.0, (s[0].y + s[1].y) / 2.0);
    let c = pos2((s[0].x + s[2].x) / 2.0, (s[0].y + s[2].y) / 2.0);
    let d = (top - c).normalized();
    top + d * 22.0
}

fn handle_box(painter: &egui::Painter, c: Pos2, size: f32, fill: Color32) {
    let r = Rect::from_center_size(c, vec2(size, size));
    painter.rect_filled(r, 0.0, fill);
    painter.rect_stroke(r, 0.0, EStroke::new(1.0_f32, ACCENT));
}

/// A selected merge-shape piece: Flash's dot screen over it.
fn draw_merge_selection(app: &App, painter: &egui::Painter, rect: Rect) {
    for s in &app.ed.selection {
        let Some(el) = app.ed.element_at(*s) else { continue };
        let ElementKind::Canvas(c) = &el.kind else { continue };
        let Some(f) = &c.float else { continue };
        let m = el.matrix;
        let dot = Color32::from_rgba_unmultiplied(255, 255, 255, 170);
        let dark = Color32::from_rgba_unmultiplied(0, 0, 0, 120);
        for r in &f.fills {
            let Some((lo, hi)) = r.bounds() else { continue };
            let step = (4.0 / app.zoom).max(0.5);
            let mut y = lo.y + step / 2.0;
            let mut row = 0;
            while y < hi.y && row < 400 {
                let mut x = lo.x + if row % 2 == 0 { 0.0 } else { step / 2.0 };
                while x < hi.x {
                    if r.contains(pt(x, y)) {
                        let sp = app.to_screen(rect, m.apply(pt(x, y)));
                        painter.circle_filled(sp, 0.9, if (row / 2) % 2 == 0 { dot } else { dark });
                    }
                    x += step;
                }
                y += step / 2.0;
                row += 1;
            }
        }
        for st in &f.strokes {
            let pts: Vec<Pos2> = st.path().flatten(8).into_iter().flat_map(|(v, _)| v).map(|q| app.to_screen(rect, m.apply(q))).collect();
            painter.add(egui::Shape::dashed_line(&pts, EStroke::new(1.0_f32, Color32::WHITE), 3.0, 3.0));
        }
    }
}

fn overlays(app: &App, ui: &egui::Ui, rect: Rect) {
    let painter = ui.painter_at(rect);
    let preview = EStroke::new(1.0_f32, ACCENT);
    let hover = ui.input(|i| i.pointer.hover_pos());
    draw_merge_selection(app, &painter, rect);
    if app.ed.editing.is_some() {
        let c = app.to_screen(rect, pt(0.0, 0.0));
        let s = EStroke::new(1.0_f32, Color32::from_gray(20));
        painter.line_segment([c - vec2(9.0, 0.0), c + vec2(9.0, 0.0)], s);
        painter.line_segment([c - vec2(0.0, 9.0), c + vec2(0.0, 9.0)], s);
    }
    crate::rulers::draw(app, &painter, rect);
    if matches!(app.tool, Tool::Select | Tool::Subselect | Tool::FreeTransform) && !app.playing {
        crate::motion_ui::draw_path(app, &painter, rect, app.tool == Tool::Subselect);
        crate::motion_ui::draw_hints(app, &painter, rect);
    }
    // Free camera (VCam) frame on the scene
    if vcam_frame_shown(app) {
        if let Some(q) = vcam_quad(app, app.ed.frame) {
            let cam_col = Color32::from_rgb(0xFF, 0xA0, 0x30);
            let s: Vec<Pos2> = q.iter().map(|p| app.to_screen(rect, *p)).collect();
            painter.add(egui::Shape::closed_line(s.clone(), EStroke::new(2.0_f32, cam_col)));
            let c = pos2((s[0].x + s[2].x) / 2.0, (s[0].y + s[2].y) / 2.0);
            painter.line_segment([c - vec2(8.0, 0.0), c + vec2(8.0, 0.0)], EStroke::new(1.0_f32, cam_col));
            painter.line_segment([c - vec2(0.0, 8.0), c + vec2(0.0, 8.0)], EStroke::new(1.0_f32, cam_col));
            if app.tool == Tool::Camera {
                for k in &s {
                    painter.rect_filled(Rect::from_center_size(*k, vec2(8.0, 8.0)), 0.0, cam_col);
                }
                let knob = vcam_knob(&s);
                let top = pos2((s[0].x + s[1].x) / 2.0, (s[0].y + s[1].y) / 2.0);
                painter.line_segment([top, knob], EStroke::new(1.0_f32, cam_col));
                painter.circle_filled(knob, 5.0, cam_col);
            }
            if let Some(cam) = app.ed.camera_final(app.ed.frame) {
                let (cc, rot, z) = Editor::camera_params(&cam.matrix);
                let follow = app.ed.camera_opts().and_then(|o| o.follow).map(|f| format!("  following {f}")).unwrap_or_default();
                painter.text(s[0] + vec2(6.0, 6.0), egui::Align2::LEFT_TOP, format!("🎥 VCam  x {:.0} y {:.0}  zoom {:.0}%  rot {:.0}°{follow}", cc.x, cc.y, z * 100.0, rot), egui::FontId::proportional(12.0), cam_col);
            }
        }
    }
    // Camera frame
    if app.ed.camera_view {
        let cam_col = Color32::from_rgb(0xFF, 0xA0, 0x30);
        let q = app.quad(rect, pt(0.0, 0.0), pt(app.ed.doc.width, app.ed.doc.height));
        painter.add(egui::Shape::closed_line(q.clone(), EStroke::new(2.0_f32, cam_col)));
        if let Some(cam) = app.ed.camera_at(app.ed.frame) {
            let (c, rot, z) = Editor::camera_params(&cam);
            painter.text(q[0] + vec2(6.0, 6.0), egui::Align2::LEFT_TOP, format!("🎥 Camera  x {:.0} y {:.0}  zoom {:.0}%  rot {:.0}°", c.x, c.y, z * 100.0, rot), egui::FontId::proportional(12.0), cam_col);
        }
        let c = app.to_screen(rect, pt(app.ed.doc.width / 2.0, app.ed.doc.height / 2.0));
        painter.line_segment([c - vec2(8.0, 0.0), c + vec2(8.0, 0.0)], EStroke::new(1.0_f32, cam_col));
        painter.line_segment([c - vec2(0.0, 8.0), c + vec2(0.0, 8.0)], EStroke::new(1.0_f32, cam_col));
    }
    let point_tools = matches!(app.tool, Tool::Subselect | Tool::AddAnchor | Tool::DeleteAnchor | Tool::ConvertAnchor);
    if point_tools {
        // Anchors (squares) and handles (dots) of selected shapes.
        for s in &app.ed.selection {
            let Some(el) = app.ed.element_at(*s) else { continue };
            let ElementKind::Shape(sh) = &el.kind else { continue };
            let m = el.matrix;
            let outline = sh.path.transformed(&m);
            for (poly, closed) in outline.flatten(12) {
                let mut pts: Vec<Pos2> = poly.iter().map(|p| app.to_screen(rect, *p)).collect();
                if closed {
                    pts.push(pts[0]);
                }
                painter.add(egui::Shape::line(pts, EStroke::new(1.0_f32, ACCENT)));
            }
            if app.tool == Tool::Subselect {
                for (_, _, h, a) in sh.path.handles() {
                    let (hs, as_) = (app.to_screen(rect, m.apply(h)), app.to_screen(rect, m.apply(a)));
                    painter.line_segment([as_, hs], EStroke::new(1.0_f32, ACCENT.gamma_multiply(0.7)));
                    painter.circle_filled(hs, 3.0, ACCENT);
                }
            }
            for (_, a) in sh.path.anchors() {
                handle_box(&painter, app.to_screen(rect, m.apply(a)), 6.0, Color32::WHITE);
            }
        }
    } else if !app.playing {
        if app.ed.selection.len() > 1 {
            for s in &app.ed.selection {
                if let Some((lo, hi)) = app.ed.bounds_of(&[*s]) {
                    painter.add(egui::Shape::closed_line(app.quad(rect, lo, hi), EStroke::new(1.0_f32, ACCENT.gamma_multiply(0.7))));
                }
            }
        }
        if let Some((lo, hi)) = app.ed.selection_bounds() {
            painter.add(egui::Shape::closed_line(app.quad(rect, lo, hi), EStroke::new(1.0_f32, ACCENT)));
            let ft = app.tool == Tool::FreeTransform;
            if ft && app.topts.ft_mode != FtMode::Normal {
                if let Some((_, _, mut hs)) = app.warp_handles() {
                    if let Drag::FtWarp { handles, .. } = &app.drag {
                        hs = handles.clone();
                    }
                    let sp: Vec<Pos2> = hs.iter().map(|h| app.to_screen(rect, *h)).collect();
                    let grid = EStroke::new(1.0_f32, Color32::from_rgb(0x40, 0xE0, 0x90));
                    if sp.len() == 4 {
                        painter.add(egui::Shape::closed_line(sp.clone(), grid));
                    } else {
                        for r in 0..3 {
                            painter.add(egui::Shape::line(vec![sp[r * 3], sp[r * 3 + 1], sp[r * 3 + 2]], grid));
                            painter.add(egui::Shape::line(vec![sp[r], sp[r + 3], sp[r + 6]], grid));
                        }
                    }
                    for c in sp {
                        let hot = hover.map(|h| h.distance(c) < 8.0).unwrap_or(false);
                        handle_box(&painter, c, 8.0, if hot { Color32::YELLOW } else { Color32::from_rgb(0x40, 0xE0, 0x90) });
                    }
                }
            } else if matches!(app.tool, Tool::Select | Tool::FreeTransform) {
                let hs = handles8(lo, hi);
                let n = if ft { 8 } else { 4 };
                let hot = hover.and_then(|h| app.grab_at(rect, h, app.tool));
                for (i, h) in hs.iter().enumerate().take(n) {
                    let c = app.to_screen(rect, *h);
                    handle_box(&painter, c, 7.0, if hot == Some(Grab::Scale(i)) { Color32::YELLOW } else { Color32::WHITE });
                }
                if ft {
                    if let Some(Grab::Skew(e)) = hot {
                        let q = app.quad(rect, lo, hi);
                        let (a, b) = [(q[0], q[1]), (q[3], q[2]), (q[0], q[3]), (q[1], q[2])][e];
                        painter.line_segment([a, b], EStroke::new(3.0_f32, Color32::YELLOW));
                    }
                    if let Some(Grab::Rotate(i)) = hot {
                        let c = app.to_screen(rect, hs[i]);
                        painter.circle_stroke(c, 12.0, EStroke::new(1.5_f32, Color32::YELLOW));
                    }
                    if let Some(pv) = app.pivot() {
                        let c = app.to_screen(rect, pv);
                        painter.circle_filled(c, 4.5, Color32::WHITE);
                        painter.circle_stroke(c, 4.5, EStroke::new(1.0_f32, Color32::BLACK));
                    }
                }
            }
            if app.ed.selection.len() == 1 && !ft {
                if let Some(el) = app.ed.selected_element() {
                    if matches!(el.kind, ElementKind::Instance(_)) {
                        let reg = app.to_screen(rect, pt(el.matrix.tx, el.matrix.ty));
                        painter.circle_filled(reg, 3.5, Color32::WHITE);
                        painter.circle_stroke(reg, 3.5, EStroke::new(1.0_f32, Color32::BLACK));
                    }
                }
            }
        }
    }
    // Bones
    if app.topts.show_bones && !app.playing {
        let hot_bone = Color32::from_rgb(0xFF, 0xD0, 0x40);
        for (_, _, head, tail) in app.bone_segments() {
            let (h, t) = (app.to_screen(rect, head), app.to_screen(rect, tail));
            let d = t - h;
            let len = d.length();
            if len > 1.0 {
                let n = vec2(-d.y, d.x) / len * (len * 0.12).clamp(3.0, 9.0);
                let knee = h + d * 0.2;
                let kite = vec![h, knee + n, t, knee - n];
                painter.add(egui::Shape::convex_polygon(kite.clone(), Color32::from_rgba_unmultiplied(0x30, 0xA0, 0xFF, 110), EStroke::NONE));
                painter.add(egui::Shape::closed_line(kite, EStroke::new(1.2_f32, Color32::WHITE)));
            }
            painter.circle_filled(h, 4.0, Color32::WHITE);
            painter.circle_stroke(h, 4.0, EStroke::new(1.0_f32, ACCENT));
            let hot = app.tool == Tool::Bone && hover.map(|p| p.distance(t) < 8.0).unwrap_or(false);
            painter.circle_filled(t, if hot { 5.5 } else { 4.0 }, if hot { hot_bone } else { Color32::WHITE });
            painter.circle_stroke(t, 4.0, EStroke::new(1.0_f32, ACCENT));
        }
        // pinned joints
        let l = app.ed.layer;
        for b in app.ed.bones(l).iter().filter_map(|b| b.pin) {
            draw_pin(&painter, app.to_screen(rect, b));
        }
    }
    // Shape armatures
    if app.topts.show_bones && !app.playing {
        let bind = if app.tool == Tool::Bind { app.bind_bone } else { None };
        for (s, b, head, tail) in app.ed.rig_bones() {
            let (h, t) = (app.to_screen(rect, head), app.to_screen(rect, tail));
            let d = t - h;
            let len = d.length();
            let chosen = bind == Some((s, b));
            if len > 1.0 {
                let n = vec2(-d.y, d.x) / len * (len * 0.1).clamp(2.5, 7.0);
                let knee = h + d * 0.2;
                let kite = vec![h, knee + n, t, knee - n];
                let fill = if chosen { Color32::from_rgba_unmultiplied(0xFF, 0xD0, 0x40, 150) } else { Color32::from_rgba_unmultiplied(0x60, 0xD0, 0x90, 110) };
                painter.add(egui::Shape::convex_polygon(kite.clone(), fill, EStroke::NONE));
                painter.add(egui::Shape::closed_line(kite, EStroke::new(1.0_f32, Color32::WHITE)));
            }
            painter.circle_filled(h, 3.0, Color32::WHITE);
            painter.circle_filled(t, 3.0, Color32::WHITE);
            if let Some(ElementKind::Shape(Shape { rig: Some(r), .. })) = app.ed.element_at(s).map(|e| e.kind) {
                if r.bones.get(b).and_then(|x| x.pin).is_some() {
                    draw_pin(&painter, t);
                }
            }
        }
        if let Some((s, b)) = bind {
            if let Some(el) = app.ed.element_at(s) {
                if let ElementKind::Shape(Shape { rig: Some(r), path, .. }) = &el.kind {
                    let posed = r.deform(path);
                    let (rest, now) = (ac_core::rig::path_points(path), ac_core::rig::path_points(&posed));
                    for i in ac_core::rig::anchor_indices(path) {
                        let mine = r.main_bone(i, rest[i]) == Some(b);
                        let pinned = r.bind.iter().any(|(k, _)| *k == i);
                        let c = app.to_screen(rect, el.matrix.apply(now[i]));
                        let col = if mine { Color32::from_rgb(0xFF, 0xD0, 0x40) } else { Color32::from_rgb(0x40, 0x90, 0xFF) };
                        let r = Rect::from_center_size(c, vec2(7.0, 7.0));
                        painter.rect_filled(r, 0.0, col);
                        if pinned {
                            painter.rect_stroke(r, 0.0, EStroke::new(1.5_f32, Color32::BLACK));
                        }
                    }
                }
            }
        }
    }
    // Tool-specific overlays
    match app.tool {
        Tool::Rotate3D | Tool::Translate3D if !app.ed.selection.is_empty() && !app.playing => {
            if let Some(c) = app.gizmo_center(rect) {
                let r = GIZMO_R;
                let hot = match &app.drag {
                    Drag::Gizmo3D { kind, .. } => Some(*kind),
                    _ => hover.and_then(|h| app.grab3d_at(rect, h).map(|g| g.0)),
                };
                let col = |g: Grab3D, base: Color32| if hot == Some(g) { Color32::YELLOW } else { base };
                let red = Color32::from_rgb(0xE8, 0x40, 0x40);
                let green = Color32::from_rgb(0x40, 0xC8, 0x50);
                let blue = Color32::from_rgb(0x40, 0x90, 0xFF);
                if app.tool == Tool::Rotate3D {
                    painter.circle_stroke(c, r + 14.0, EStroke::new(2.0_f32, col(Grab3D::Free, Color32::from_rgb(0xFF, 0x90, 0x20))));
                    painter.circle_stroke(c, r, EStroke::new(2.5_f32, col(Grab3D::Z, blue)));
                    painter.line_segment([c - vec2(0.0, r), c + vec2(0.0, r)], EStroke::new(3.0_f32, col(Grab3D::X, red)));
                    painter.line_segment([c - vec2(r, 0.0), c + vec2(r, 0.0)], EStroke::new(3.0_f32, col(Grab3D::Y, green)));
                    painter.circle_filled(c, 5.0, col(Grab3D::Center, Color32::WHITE));
                    painter.circle_stroke(c, 5.0, EStroke::new(1.0_f32, Color32::BLACK));
                } else {
                    painter.arrow(c, vec2(r, 0.0), EStroke::new(3.0_f32, col(Grab3D::TX, red)));
                    painter.arrow(c, vec2(0.0, -r), EStroke::new(3.0_f32, col(Grab3D::TY, green)));
                    painter.circle_filled(c, 6.0, col(Grab3D::TZ, Color32::from_gray(20)));
                    painter.circle_stroke(c, 6.0, EStroke::new(1.5_f32, Color32::WHITE));
                }
                if let Some(t) = app.ed.selected_element().and_then(|e| e.three_d) {
                    painter.text(c + vec2(r + 20.0, -r), egui::Align2::LEFT_TOP, format!("X {:.0}°  Y {:.0}°  Z {:.0}°\nz {:.0}", t.rx, t.ry, t.rz, t.z), egui::FontId::proportional(11.0), Color32::WHITE);
                }
            }
        }
        Tool::GradientTransform => {
            for s in &app.ed.selection {
                let Some(el) = app.ed.element_at(*s) else { continue };
                let Some([c, hx, hy]) = gradient_handles(&el) else { continue };
                let radial = two_axis(&el);
                let (sc, sx, sy) = (app.to_screen(rect, c), app.to_screen(rect, hx), app.to_screen(rect, hy));
                let thin = EStroke::new(1.0_f32, Color32::WHITE);
                if radial {
                    // ellipse outline of the gradient's unit circle
                    let w = |a: f32| app.to_screen(rect, pt(c.x + (hx.x - c.x) * a.cos() + (hy.x - c.x) * a.sin(), c.y + (hx.y - c.y) * a.cos() + (hy.y - c.y) * a.sin()));
                    painter.add(egui::Shape::closed_line((0..48).map(|i| w(i as f32 / 48.0 * std::f32::consts::TAU)).collect(), thin));
                    painter.line_segment([sc, sy], thin);
                    painter.circle_filled(sy, 5.0, Color32::WHITE);
                    painter.circle_stroke(sy, 5.0, EStroke::new(1.0_f32, ACCENT));
                } else {
                    // bounding lines at both ends of the ramp
                    let perp = (sy - sc).normalized() * 400.0;
                    let back = sc - (sx - sc);
                    painter.line_segment([sx - perp, sx + perp], thin);
                    painter.line_segment([back - perp, back + perp], thin);
                }
                painter.line_segment([sc, sx], thin);
                handle_box(&painter, sx, 8.0, Color32::WHITE);
                painter.circle_filled(sc, 5.0, Color32::WHITE);
                painter.circle_stroke(sc, 5.0, EStroke::new(1.0_f32, ACCENT));
            }
        }
        Tool::Width => {
            let warn = Color32::from_rgb(0xFF, 0x60, 0xC0);
            for s in &app.ed.selection {
                let Some(el) = app.ed.element_at(*s) else { continue };
                let ElementKind::Shape(sh) = &el.kind else { continue };
                if let Some(prof) = &sh.width_profile {
                    for (t, _) in prof {
                        if let Some(q) = deform::point_at(&sh.path, *t) {
                            painter.circle_filled(app.to_screen(rect, el.matrix.apply(q)), 4.0, warn);
                        }
                    }
                }
            }
            for (q, ..) in app.ed.merge_width_points() {
                painter.circle_filled(app.to_screen(rect, q), 4.0, warn);
            }
            if let Some(h) = hover.filter(|h| rect.contains(*h)) {
                let p = app.to_doc(rect, h);
                if stroked_shape_at(app, p).is_none() {
                    if let Some((li, line, t)) = app.ed.merge_line_at(p, 8.0 / app.zoom) {
                        let m = app.ed.canvas_at(li).and_then(|(_, ei)| app.ed.element_at(Selection { layer: li, element: ei })).map(|e| e.matrix).unwrap_or(Matrix::IDENTITY);
                        if let Some(q) = app.ed.canvas(li).and_then(|c| c.strokes.get(line)).and_then(|s| deform::point_at(&s.path(), t)) {
                            painter.circle_stroke(app.to_screen(rect, m.apply(q)), 5.0, EStroke::new(1.5_f32, warn));
                        }
                    }
                }
                if let Some((s, t)) = stroked_shape_at(app, p) {
                    if let Some(el) = app.ed.element_at(s) {
                        if let ElementKind::Shape(sh) = &el.kind {
                            if let Some(q) = deform::point_at(&sh.path, t) {
                                painter.circle_stroke(app.to_screen(rect, el.matrix.apply(q)), 5.0, EStroke::new(1.5_f32, warn));
                            }
                        }
                    }
                }
            }
        }
        Tool::AssetWarp => {
            for s in &app.ed.selection {
                let Some(el) = app.ed.element_at(*s) else { continue };
                let ElementKind::Shape(sh) = &el.kind else { continue };
                for (poly, closed) in sh.path.transformed(&el.matrix).flatten(8) {
                    let mut pts: Vec<Pos2> = poly.iter().map(|p| app.to_screen(rect, *p)).collect();
                    if closed {
                        pts.push(pts[0]);
                    }
                    painter.add(egui::Shape::line(pts, EStroke::new(1.0_f32, ACCENT.gamma_multiply(0.7))));
                }
                if let Some(w) = &sh.warp {
                    for (_, q) in &w.pins {
                        let c = app.to_screen(rect, el.matrix.apply(*q));
                        painter.circle_filled(c, 5.5, Color32::from_rgb(0xFF, 0xC8, 0x30));
                        painter.circle_stroke(c, 5.5, EStroke::new(1.5_f32, Color32::BLACK));
                    }
                }
            }
            // bitmaps / symbols / groups: the deformed mesh and its pins
            for s in &app.ed.selection {
                let Some(el) = app.ed.element_at(*s) else { continue };
                let Some(w) = &el.mesh else { continue };
                let (Some((lo, hi)), Some(inv)) = (app.ed.bounds_of(&[*s]), el.matrix.invert()) else { continue };
                let corners = [pt(lo.x, lo.y), pt(hi.x, lo.y), pt(hi.x, hi.y), pt(lo.x, hi.y)].map(|c| inv.apply(c));
                let (llo, lhi) = corners.iter().fold((corners[0], corners[0]), |(a, b), c| (pt(a.x.min(c.x), a.y.min(c.y)), pt(b.x.max(c.x), b.y.max(c.y))));
                let m = w.map(llo, lhi, &el.matrix);
                let line = EStroke::new(0.7_f32, ACCENT.gamma_multiply(0.45));
                let step = (m.cols / 8).max(1);
                for r in (0..=m.rows).step_by(step) {
                    let pts: Vec<Pos2> = (0..=m.cols).map(|c| app.to_screen(rect, m.dst[r * (m.cols + 1) + c])).collect();
                    painter.add(egui::Shape::line(pts, line));
                }
                for c in (0..=m.cols).step_by(step) {
                    let pts: Vec<Pos2> = (0..=m.rows).map(|r| app.to_screen(rect, m.dst[r * (m.cols + 1) + c])).collect();
                    painter.add(egui::Shape::line(pts, line));
                }
                for q in app.ed.mesh_pins(*s) {
                    let c = app.to_screen(rect, q);
                    painter.circle_filled(c, 5.5, Color32::from_rgb(0xFF, 0xC8, 0x30));
                    painter.circle_stroke(c, 5.5, EStroke::new(1.5_f32, Color32::BLACK));
                }
            }
        }
        _ => {}
    }
    // Snap guides
    let guide = EStroke::new(1.0_f32, Color32::from_rgb(0xFF, 0x3D, 0xB5));
    if let Some(x) = app.snap_guides.0 {
        painter.line_segment([app.to_screen(rect, pt(x, -1e5)), app.to_screen(rect, pt(x, 1e5))], guide);
    }
    if let Some(y) = app.snap_guides.1 {
        painter.line_segment([app.to_screen(rect, pt(-1e5, y)), app.to_screen(rect, pt(1e5, y))], guide);
    }
    match &app.drag {
        Drag::Marquee { start, cur, .. } => {
            let (lo, hi) = (pt(start.x.min(cur.x), start.y.min(cur.y)), pt(start.x.max(cur.x), start.y.max(cur.y)));
            let q = app.quad(rect, lo, hi);
            painter.add(egui::Shape::convex_polygon(q.clone(), ACCENT.gamma_multiply(0.12), EStroke::NONE));
            painter.add(egui::Shape::closed_line(q, preview));
        }
        Drag::ZoomBox { start, cur } => {
            painter.rect_stroke(Rect::from_two_pos(*start, *cur), 0.0, EStroke::new(1.0_f32, Color32::WHITE));
        }
        Drag::Draw { start, cur } => {
            let (a, b) = (*start, *cur);
            let o = &app.topts;
            let path = match app.tool {
                Tool::Rect | Tool::RectPrimitive => Some(Path::rounded_rect(a.x.min(b.x), a.y.min(b.y), (b.x - a.x).abs(), (b.y - a.y).abs(), o.rect_radius)),
                Tool::OvalPrimitive => Some(oval_path((a.x + b.x) / 2.0, (a.y + b.y) / 2.0, (b.x - a.x).abs() / 2.0, (b.y - a.y).abs() / 2.0, o.oval_start, o.oval_end, o.oval_inner)),
                Tool::Oval => Some(oval_path((a.x + b.x) / 2.0, (a.y + b.y) / 2.0, (b.x - a.x).abs() / 2.0, (b.y - a.y).abs() / 2.0, o.oval_start, o.oval_end, o.oval_inner)),
                Tool::PolyStar => Some(Path::polystar((a.x + b.x) / 2.0, (a.y + b.y) / 2.0, (b.x - a.x).abs() / 2.0, (b.y - a.y).abs() / 2.0, o.poly_sides, o.poly_star, o.poly_inner)),
                _ => None,
            };
            match path {
                Some(path) => {
                    for (poly, closed) in path.flatten(16) {
                        let mut pts: Vec<Pos2> = poly.iter().map(|p| app.to_screen(rect, *p)).collect();
                        if closed {
                            pts.push(pts[0]);
                        }
                        painter.add(egui::Shape::line(pts, preview));
                    }
                }
                None => {
                    painter.line_segment([app.to_screen(rect, a), app.to_screen(rect, b)], EStroke::new(app.stroke_w * app.zoom, app.stroke));
                }
            }
        }
        Drag::Freehand { pts } => {
            let st = match app.tool {
                Tool::Brush => EStroke::new(app.topts.brush_size * app.zoom, app.fill),
                Tool::Lasso => EStroke::new(1.0_f32, ACCENT),
                _ => EStroke::new(app.stroke_w * app.zoom, app.stroke),
            };
            let polys = if app.tool != Tool::Lasso && app.assisting() && pts.len() > 1 { ac_core::assist::apply(&app.ed.doc.assistants, pts, 1.0 / app.zoom) } else { vec![pts.clone()] };
            for poly in polys {
                let s: Vec<Pos2> = poly.iter().map(|p| app.to_screen(rect, *p)).collect();
                painter.add(egui::Shape::line(s, st));
            }
        }
        Drag::Fluid { pts } => {
            let w = fluid_widths(pts, app.topts.fluid_size, app.topts.fluid_sensitivity, app.zoom);
            let raw: Vec<Pt> = pts.iter().map(|x| x.0).collect();
            let polys = if app.assisting() && raw.len() > 1 { ac_core::assist::apply(&app.ed.doc.assistants, &raw, 1.0 / app.zoom) } else { vec![raw] };
            for poly in polys {
                for i in 1..poly.len() {
                    painter.line_segment([app.to_screen(rect, poly[i - 1]), app.to_screen(rect, poly[i])], EStroke::new(w[i] * app.zoom, app.fill));
                }
            }
            if let Some(f) = pts.first() {
                painter.circle_filled(app.to_screen(rect, f.0), w.first().copied().unwrap_or(1.0) * app.zoom / 2.0, app.fill);
            }
        }
        Drag::BoneDraw { head, cur, .. } => {
            let (h, t) = (app.to_screen(rect, *head), app.to_screen(rect, *cur));
            painter.line_segment([h, t], EStroke::new(4.0_f32, Color32::from_rgba_unmultiplied(0x30, 0xA0, 0xFF, 180)));
            painter.circle_filled(h, 4.0, Color32::WHITE);
            painter.circle_stroke(t, 5.0, EStroke::new(1.5_f32, Color32::WHITE));
        }
        _ => {}
    }
    // Pen path in progress
    if let Some(pen) = &app.pen {
        let mut prev: Option<(Pt, Pt)> = None;
        for &(a, h) in &pen.pts {
            if let Some((pa, ph)) = prev {
                let samples: Vec<Pos2> = (0..=16)
                    .map(|i| {
                        let t = i as f32 / 16.0;
                        let (c1, c2) = (pt(pa.x + ph.x, pa.y + ph.y), pt(a.x - h.x, a.y - h.y));
                        let u = 1.0 - t;
                        let q = pt(
                            u * u * u * pa.x + 3.0 * u * u * t * c1.x + 3.0 * u * t * t * c2.x + t * t * t * a.x,
                            u * u * u * pa.y + 3.0 * u * u * t * c1.y + 3.0 * u * t * t * c2.y + t * t * t * a.y,
                        );
                        app.to_screen(rect, q)
                    })
                    .collect();
                painter.add(egui::Shape::line(samples, EStroke::new(1.5_f32, ACCENT)));
            }
            let c = app.to_screen(rect, a);
            if h.x.abs() + h.y.abs() > 0.01 {
                let h1 = app.to_screen(rect, pt(a.x + h.x, a.y + h.y));
                let h2 = app.to_screen(rect, pt(a.x - h.x, a.y - h.y));
                painter.line_segment([h1, h2], EStroke::new(1.0_f32, ACCENT.gamma_multiply(0.7)));
                painter.circle_filled(h1, 2.5, ACCENT);
                painter.circle_filled(h2, 2.5, ACCENT);
            }
            handle_box(&painter, c, 6.0, Color32::WHITE);
            prev = Some((a, h));
        }
        if let (Some(&(last, _)), Some(hp), false) = (pen.pts.last(), hover, pen.dragging) {
            painter.line_segment([app.to_screen(rect, last), hp], EStroke::new(1.0_f32, ACCENT.gamma_multiply(0.5)));
            if pen.pts.len() >= 2 && app.to_screen(rect, pen.pts[0].0).distance(hp) < 8.0 {
                painter.circle_stroke(app.to_screen(rect, pen.pts[0].0), 6.0, EStroke::new(1.5_f32, Color32::WHITE));
            }
        }
    }
    // Polygon lasso in progress
    if !app.poly_lasso.is_empty() {
        let mut pts: Vec<Pos2> = app.poly_lasso.iter().map(|p| app.to_screen(rect, *p)).collect();
        if let Some(h) = hover {
            pts.push(h);
        }
        painter.add(egui::Shape::line(pts, EStroke::new(1.0_f32, ACCENT)));
    }
    // Drawing assistants
    if !app.ed.doc.assistants.is_empty() && (app.tool == Tool::Assistant || (app.topts.assist_on && matches!(app.tool, Tool::Pencil | Tool::Brush | Tool::FluidBrush))) {
        draw_assistants(app, &painter, rect, app.tool == Tool::Assistant);
    }
    // Particle emitters: their area and direction while the tool is active or one is selected
    draw_emitters(app, &painter, rect);
    // Bender segment
    if app.tool == Tool::Bender {
        let seg = match &app.drag {
            Drag::BenderLine { a, cur } => Some((*a, *cur)),
            _ => app.bender_seg,
        };
        if let Some((a, b)) = seg {
            let orange = Color32::from_rgb(0xFF, 0xA0, 0x30);
            let (sa, sb) = (app.to_screen(rect, a), app.to_screen(rect, b));
            painter.line_segment([sa, sb], EStroke::new(1.5_f32, orange));
            painter.circle_filled(sa, 4.0, orange);
            painter.circle_stroke(sb, 3.5, EStroke::new(1.5_f32, orange));
        }
    }
    // Eraser / brush cursors
    if let Some(h) = hover.filter(|h| rect.contains(*h)) {
        match app.tool {
            Tool::Eraser => {
                painter.circle_stroke(h, app.topts.eraser_size / 2.0 * app.zoom, EStroke::new(1.0_f32, Color32::WHITE));
            }
            Tool::RasterBrush => {
                let r = if app.topts.raster_size > 0.0 { app.topts.raster_size } else { ac_core::paint::PRESETS.get(app.topts.raster_brush).map(|p| p.base_radius()).unwrap_or(2.0) };
                painter.circle_stroke(h, (r * app.zoom).max(1.5), EStroke::new(1.0_f32, Color32::from_gray(200)));
            }
            Tool::Magnet | Tool::Iron => {
                let c = match &app.drag {
                    Drag::Sculpt(sc) if app.tool == Tool::Magnet => app.to_screen(rect, sc.from),
                    _ => h,
                };
                painter.circle_stroke(c, app.topts.sculpt_size * app.zoom, EStroke::new(1.0_f32, Color32::from_rgb(0xFF, 0xA0, 0x30)));
                painter.circle_filled(h, 2.0, Color32::from_rgb(0xFF, 0xA0, 0x30));
            }
            Tool::Brush | Tool::FluidBrush if matches!(app.drag, Drag::None) => {
                let size = if app.tool == Tool::Brush { app.topts.brush_size } else { app.topts.fluid_size };
                let st = EStroke::new(1.0_f32, Color32::from_gray(200));
                if app.tool == Tool::Brush && app.topts.brush_shape != ac_core::merge::BrushShape::Round {
                    // the tip as drawn (stage space, so it follows zoom and view rotation)
                    let c = app.to_doc(rect, h);
                    let pts: Vec<Pos2> = app.topts.brush_shape.nib(size).iter().map(|n| app.to_screen(rect, pt(c.x + n.x, c.y + n.y))).collect();
                    painter.add(egui::Shape::closed_line(pts, st));
                } else {
                    painter.circle_stroke(h, size / 2.0 * app.zoom, st);
                }
            }
            _ => {}
        }
    }
    if app.lib_drag.is_some() && ui.rect_contains_pointer(rect) {
        painter.rect_stroke(rect.shrink(2.0), 0.0, EStroke::new(2.0_f32, ACCENT));
    }
}

/// Inline editor for the Text tool. The vector text updates live as you type.
fn text_editor(app: &mut App, ui: &mut egui::Ui, rect: Rect) {
    let Some(te) = &app.text_edit else { return };
    let sel = te.sel;
    let Some(el) = app.ed.element_at(sel) else {
        app.text_edit = None;
        return;
    };
    let anchor = app.to_screen(rect, pt(el.matrix.tx, el.matrix.ty));
    let (lo, hi) = app.ed.bounds_of(&[sel]).unwrap_or((pt(el.matrix.tx, el.matrix.ty), pt(el.matrix.tx + 10.0, el.matrix.ty + 10.0)));
    let q = app.quad(rect, lo, hi);
    let box_r = Rect::from_points(&q).expand(3.0);
    ui.painter_at(rect).add(egui::Shape::closed_line(q, EStroke::new(1.0_f32, ACCENT)));
    let mut buffer = te.buffer.clone();
    let focus_requested = te.focus_requested;
    let mark = te.mark;
    let mut done = false;
    let mut got_focus = false;
    egui::Area::new(egui::Id::new("text-tool-editor")).order(egui::Order::Foreground).fixed_pos(pos2(anchor.x.min(box_r.left()), box_r.bottom() + 6.0)).show(ui.ctx(), |ui| {
        egui::Frame::popup(ui.style()).show(ui, |ui| {
            ui.label(egui::RichText::new("Type text · Esc or click away to finish").small().color(Color32::from_gray(160)));
            let r = ui.add(egui::TextEdit::multiline(&mut buffer).desired_rows(2).desired_width(260.0));
            // Keep asking until the box really has focus (the click that
            // created it can steal focus back in the same frame).
            if !focus_requested && !r.has_focus() {
                r.request_focus();
            }
            got_focus = r.has_focus();
            if r.changed() {
                let text = buffer.clone();
                if app.ed.undo_depth() == mark {
                    app.ed.begin_drag(); // first keystroke when editing existing text
                }
                app.ed.selection = vec![sel];
                app.ed.modify_selected_continuous(|e| {
                    if let ElementKind::Text(t) = &mut e.kind {
                        t.text = text.clone();
                    }
                });
            }
            if ui.input(|i| i.key_pressed(Key::Escape)) || ui.button("Done").clicked() {
                done = true;
            }
        });
    });
    if let Some(te) = &mut app.text_edit {
        te.buffer = buffer;
        te.focus_requested |= got_focus;
    }
    if done {
        app.finish_text();
    }
}

/// Guide/grid snap of one coordinate without borrowing the whole app (used mid-drag).
fn app_snap(view: &crate::rulers::ViewOpts, guides: &ac_core::Guides, v: f32, vertical: bool, zoom: f32) -> f32 {
    let thr = 6.0 / zoom;
    let mut best: Option<f32> = None;
    let mut consider = |t: f32| {
        if (t - v).abs() <= thr && best.map(|b| (t - v).abs() < (b - v).abs()).unwrap_or(true) {
            best = Some(t);
        }
    };
    if view.snap_guides && view.guides {
        for t in if vertical { &guides.x } else { &guides.y } {
            consider(*t);
        }
    }
    if view.snap_grid {
        let g = view.grid_size.max(1.0);
        consider((v / g).round() * g);
    }
    best.unwrap_or(v)
}

/// Shift and Trace ghosts: the current layer's previous and next keyframes.
fn shift_trace_frames(app: &App) -> [Option<u32>; 2] {
    let Some(l) = app.ed.timeline().layers.get(app.ed.layer) else { return [None, None] };
    let cur = app.ed.frame;
    let start = l.span_at(cur).map(|(s, _, _)| s).unwrap_or(cur);
    let prev = l.keyframes.range(..start).next_back().map(|(&k, _)| k);
    let next = l.keyframes.range(cur + 1..).next().map(|(&k, _)| k).filter(|&k| k < l.end);
    [prev, next]
}
