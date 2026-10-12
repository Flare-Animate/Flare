//! Merge Drawing: Flash's shape model, where everything drawn on a layer in
//! merge mode lives in one shape and new drawing **cuts and merges** with
//! what is already there.
//!
//! The shape is kept as a planar arrangement:
//! * **fill regions**: disjoint, each one connected area (outer contour + holes)
//!   with one paint. Drawing a fill cuts every region under it; regions with the
//!   same paint that touch become one.
//! * **stroke edges**: polylines with a stroke style, split wherever lines cross
//!   (so each piece is selectable, as in Flash). A line drawn across a fill
//!   splits the fill in two.
//!
//! Curves are flattened to polylines; boolean work is done by `i_overlay`.

use crate::geom::{pt, Matrix, Path, Pt};
use crate::model::{Color, Gradient, Stroke};
use i_overlay::core::fill_rule::FillRule;
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::float::clip::FloatClip;
use i_overlay::float::single::SingleFloatOverlay;
use i_overlay::float::slice::FloatSlice;
use i_overlay::string::clip::ClipRule;
use serde::{Deserialize, Serialize};

type P = [f64; 2];
type Contour = Vec<P>;
type Shp = Vec<Contour>;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FillRegion {
    pub fill: Option<Color>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gradient: Option<Gradient>,
    /// Outer contour first, then holes (even-odd).
    pub contours: Vec<Vec<Pt>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StrokeEdge {
    pub pts: Vec<Pt>,
    #[serde(default)]
    pub closed: bool,
    pub stroke: Stroke,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MergeShape {
    pub fills: Vec<FillRegion>,
    pub strokes: Vec<StrokeEdge>,
    /// The selected piece, lifted off the shape: it floats on top (and can be
    /// moved, transformed or recoloured) until it is deselected and merges back.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub float: Option<Box<MergeShape>>,
}

/// A selectable piece of a merge shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Part {
    Fill(usize),
    Stroke(usize),
}

/// What the eraser / brush touches (Animate's Eraser and Brush modes).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PaintMode {
    /// Everything: fills and strokes.
    #[default]
    Normal,
    /// Fills only (strokes untouched).
    Fills,
    /// Strokes only (eraser).
    Lines,
    /// Only empty areas (brush: Paint Behind).
    Behind,
    /// Only the selected fills.
    Selection,
    /// Only the fill where the gesture started.
    Inside,
}

impl PaintMode {
    pub const BRUSH: [PaintMode; 5] = [
        PaintMode::Normal,
        PaintMode::Fills,
        PaintMode::Behind,
        PaintMode::Selection,
        PaintMode::Inside,
    ];
    pub const ERASER: [PaintMode; 5] = [
        PaintMode::Normal,
        PaintMode::Fills,
        PaintMode::Lines,
        PaintMode::Selection,
        PaintMode::Inside,
    ];
    pub fn name(self, eraser: bool) -> &'static str {
        match (self, eraser) {
            (PaintMode::Normal, true) => "Erase Normal",
            (PaintMode::Normal, false) => "Paint Normal",
            (PaintMode::Fills, true) => "Erase Fills",
            (PaintMode::Fills, false) => "Paint Fills",
            (PaintMode::Lines, _) => "Erase Lines",
            (PaintMode::Behind, _) => "Paint Behind",
            (PaintMode::Selection, true) => "Erase Selected Fills",
            (PaintMode::Selection, false) => "Paint Selection",
            (PaintMode::Inside, true) => "Erase Inside",
            (PaintMode::Inside, false) => "Paint Inside",
        }
    }
}

// ------------------------------------------------------------ conversions

fn p(q: Pt) -> P {
    [q.x as f64, q.y as f64]
}
fn q(a: P) -> Pt {
    pt(a[0] as f32, a[1] as f32)
}
fn shp_of(contours: &[Vec<Pt>]) -> Shp {
    contours
        .iter()
        .map(|c| c.iter().map(|x| p(*x)).collect())
        .collect()
}
fn contours_of(s: &Shp) -> Vec<Vec<Pt>> {
    s.iter()
        .map(|c| c.iter().map(|x| q(*x)).collect())
        .collect()
}

/// Flatten a path into closed polygons (for fills) — curves become polylines.
pub fn path_polygons(path: &Path) -> Vec<Vec<Pt>> {
    path.flatten(16)
        .into_iter()
        .filter(|(c, _)| c.len() >= 3)
        .map(|(c, _)| c)
        .collect()
}

/// Flatten a path into polylines (for strokes): (points, closed).
pub fn path_polylines(path: &Path) -> Vec<(Vec<Pt>, bool)> {
    path.flatten(16)
        .into_iter()
        .filter(|(c, _)| c.len() >= 2)
        .collect()
}

/// Even-odd normalise loose polygons into disjoint shapes.
fn normalize(polys: &[Vec<Pt>]) -> Vec<Shp> {
    let subj: Vec<Contour> = polys
        .iter()
        .map(|c| c.iter().map(|x| p(*x)).collect())
        .collect();
    let empty: Vec<Contour> = vec![];
    subj.overlay(&empty, OverlayRule::Subject, FillRule::EvenOdd)
}

pub(crate) fn union_shapes(a: &[Shp], b: &[Shp]) -> Vec<Shp> {
    let a: Vec<Shp> = a.to_vec();
    let b: Vec<Shp> = b.to_vec();
    a.overlay(&b, OverlayRule::Union, FillRule::NonZero)
}
fn diff_shapes(a: &[Shp], b: &[Shp]) -> Vec<Shp> {
    if b.is_empty() {
        return a.to_vec();
    }
    let a: Vec<Shp> = a.to_vec();
    let b: Vec<Shp> = b.to_vec();
    a.overlay(&b, OverlayRule::Difference, FillRule::NonZero)
}
fn intersect_shapes(a: &[Shp], b: &[Shp]) -> Vec<Shp> {
    let a: Vec<Shp> = a.to_vec();
    let b: Vec<Shp> = b.to_vec();
    a.overlay(&b, OverlayRule::Intersect, FillRule::NonZero)
}

fn shape_area(s: &Shp) -> f64 {
    s.iter()
        .map(|c| {
            let n = c.len();
            (0..n)
                .map(|i| c[i][0] * c[(i + 1) % n][1] - c[(i + 1) % n][0] * c[i][1])
                .sum::<f64>()
                / 2.0
        })
        .map(|a: f64| a.abs())
        .enumerate()
        .map(|(i, a)| if i == 0 { a } else { -a })
        .sum()
}

fn bbox(pts: impl Iterator<Item = Pt>) -> Option<(Pt, Pt)> {
    pts.fold(None, |acc, x| match acc {
        None => Some((x, x)),
        Some((lo, hi)) => Some((
            pt(lo.x.min(x.x), lo.y.min(x.y)),
            pt(hi.x.max(x.x), hi.y.max(x.y)),
        )),
    })
}
fn boxes_touch(a: (Pt, Pt), b: (Pt, Pt), pad: f32) -> bool {
    a.0.x <= b.1.x + pad && b.0.x <= a.1.x + pad && a.0.y <= b.1.y + pad && b.0.y <= a.1.y + pad
}

fn point_in_contours(contours: &[Vec<Pt>], x: Pt) -> bool {
    let mut inside = false;
    for c in contours {
        let n = c.len();
        let mut j = n.wrapping_sub(1);
        for i in 0..n {
            let (a, b) = (c[i], c[j]);
            if (a.y > x.y) != (b.y > x.y) && x.x < (b.x - a.x) * (x.y - a.y) / (b.y - a.y) + a.x {
                inside = !inside;
            }
            j = i;
        }
    }
    inside
}

fn seg_dist(x: Pt, a: Pt, b: Pt) -> (f32, f32) {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let l2 = dx * dx + dy * dy;
    let t = if l2 < 1e-12 {
        0.0
    } else {
        (((x.x - a.x) * dx + (x.y - a.y) * dy) / l2).clamp(0.0, 1.0)
    };
    (pt(a.x + dx * t, a.y + dy * t).dist(x), t)
}

/// Intersection parameters (t on ab, u on cd) of two segments.
fn seg_hit(a: Pt, b: Pt, c: Pt, d: Pt) -> Option<(f32, f32)> {
    let r = pt(b.x - a.x, b.y - a.y);
    let s = pt(d.x - c.x, d.y - c.y);
    let den = r.x * s.y - r.y * s.x;
    if den.abs() < 1e-9 {
        return None;
    }
    let qp = pt(c.x - a.x, c.y - a.y);
    let t = (qp.x * s.y - qp.y * s.x) / den;
    let u = (qp.x * r.y - qp.y * r.x) / den;
    (t > 1e-4 && t < 1.0 - 1e-4 && u > -1e-4 && u < 1.0 + 1e-4).then_some((t, u))
}

/// Points where `cut` crosses `line` (as distances along `line`'s segments: (segment, t)).
fn crossings(line: &[Pt], cut: &[Pt]) -> Vec<(usize, f32)> {
    let mut out = vec![];
    let bb = bbox(cut.iter().copied());
    for i in 0..line.len().saturating_sub(1) {
        let (a, b) = (line[i], line[i + 1]);
        if let Some(bb) = bb {
            if !boxes_touch(bbox([a, b].into_iter()).unwrap(), bb, 0.01) {
                continue;
            }
        }
        for j in 0..cut.len().saturating_sub(1) {
            if let Some((t, _)) = seg_hit(a, b, cut[j], cut[j + 1]) {
                out.push((i, t));
            }
        }
    }
    out.sort_by(|x, y| (x.0, x.1).partial_cmp(&(y.0, y.1)).unwrap());
    out.dedup_by(|x, y| x.0 == y.0 && (x.1 - y.1).abs() < 1e-3);
    out
}

/// Split a polyline at the given (segment, t) points.
fn split_polyline(line: &[Pt], at: &[(usize, f32)]) -> Vec<Vec<Pt>> {
    if at.is_empty() {
        return vec![line.to_vec()];
    }
    let mut parts = vec![];
    let mut cur = vec![line[0]];
    let mut k = 0;
    for i in 0..line.len() - 1 {
        while k < at.len() && at[k].0 == i {
            let x = line[i].lerp(line[i + 1], at[k].1);
            cur.push(x);
            parts.push(std::mem::take(&mut cur));
            cur.push(x);
            k += 1;
        }
        cur.push(line[i + 1]);
    }
    parts.push(cur);
    parts
        .into_iter()
        .filter(|pp| pp.len() >= 2 && polyline_len(pp) > 0.05)
        .collect()
}

fn polyline_len(l: &[Pt]) -> f32 {
    l.windows(2).map(|w| w[0].dist(w[1])).sum()
}

impl FillRegion {
    fn shp(&self) -> Shp {
        shp_of(&self.contours)
    }
    fn same_paint(&self, o: &FillRegion) -> bool {
        self.fill == o.fill && self.gradient == o.gradient
    }
    pub fn bounds(&self) -> Option<(Pt, Pt)> {
        bbox(self.contours.first()?.iter().copied())
    }
    pub fn contains(&self, x: Pt) -> bool {
        point_in_contours(&self.contours, x)
    }
    pub fn path(&self) -> Path {
        let mut segs = vec![];
        for c in &self.contours {
            if c.len() < 3 {
                continue;
            }
            segs.push(crate::geom::Seg::M(c[0]));
            for x in &c[1..] {
                segs.push(crate::geom::Seg::L(*x));
            }
            segs.push(crate::geom::Seg::Z);
        }
        Path { segs }
    }
}

impl StrokeEdge {
    pub fn path(&self) -> Path {
        let mut segs = vec![crate::geom::Seg::M(self.pts[0])];
        for x in &self.pts[1..] {
            segs.push(crate::geom::Seg::L(*x));
        }
        if self.closed {
            segs.push(crate::geom::Seg::Z);
        }
        Path { segs }
    }
    fn closed_pts(&self) -> Vec<Pt> {
        let mut v = self.pts.clone();
        if self.closed {
            v.push(self.pts[0]);
        }
        v
    }
    pub fn dist(&self, x: Pt) -> f32 {
        self.closed_pts()
            .windows(2)
            .map(|w| seg_dist(x, w[0], w[1]).0)
            .fold(f32::MAX, f32::min)
    }
}

impl MergeShape {
    pub fn is_empty(&self) -> bool {
        self.fills.is_empty() && self.strokes.is_empty() && self.float.is_none()
    }
    /// Merge the floating selection back (deselect). True if there was one.
    pub fn commit_float(&mut self) -> bool {
        match self.float.take() {
            Some(f) => {
                self.merge_in(&f);
                true
            }
            None => false,
        }
    }
    /// Lift parts into the floating selection (committing any previous one first
    /// unless `add`).
    pub fn lift(&mut self, parts: &[Part], add: bool) {
        if !add {
            self.commit_float();
        }
        let piece = self.extract(parts);
        self.add_to_float(piece);
    }
    pub fn add_to_float(&mut self, piece: MergeShape) {
        if piece.fills.is_empty() && piece.strokes.is_empty() {
            return;
        }
        match &mut self.float {
            Some(f) => {
                f.fills.extend(piece.fills);
                f.strokes.extend(piece.strokes);
            }
            None => self.float = Some(Box::new(piece)),
        }
    }
    /// Hit only the floating selection.
    pub fn float_hit(&self, x: Pt, tol: f32) -> bool {
        self.float
            .as_ref()
            .map(|f| f.hit(x, tol).is_some())
            .unwrap_or(false)
    }

    /// Regions as shapes with paint, split into connected pieces.
    fn push_regions(&mut self, shapes: Vec<Shp>, fill: Option<Color>, gradient: Option<Gradient>) {
        for s in shapes {
            if s.is_empty() || shape_area(&s).abs() < 0.01 {
                continue;
            }
            self.fills.push(FillRegion {
                fill,
                gradient: gradient.clone(),
                contours: contours_of(&s),
            });
        }
    }

    /// Union regions of the same paint that overlap or touch.
    fn merge_same_paint(&mut self) {
        let mut out: Vec<FillRegion> = vec![];
        let mut taken = vec![false; self.fills.len()];
        for i in 0..self.fills.len() {
            if taken[i] {
                continue;
            }
            taken[i] = true;
            let mut group = vec![i];
            let mut bb = self.fills[i].bounds();
            // grow the group with touching regions of the same paint
            let mut changed = true;
            while changed {
                changed = false;
                for (j, other) in self.fills.iter().enumerate() {
                    if taken[j] || !other.same_paint(&self.fills[i]) {
                        continue;
                    }
                    let (Some(a), Some(b)) = (bb, other.bounds()) else {
                        continue;
                    };
                    if boxes_touch(a, b, 0.05) {
                        taken[j] = true;
                        group.push(j);
                        bb = Some((
                            pt(a.0.x.min(b.0.x), a.0.y.min(b.0.y)),
                            pt(a.1.x.max(b.1.x), a.1.y.max(b.1.y)),
                        ));
                        changed = true;
                    }
                }
            }
            if group.len() == 1 {
                out.push(self.fills[i].clone());
                continue;
            }
            let shapes: Vec<Shp> = group.iter().map(|&k| self.fills[k].shp()).collect();
            let empty: Vec<Shp> = vec![];
            let merged = shapes.overlay(&empty, OverlayRule::Subject, FillRule::NonZero);
            let (fill, gradient) = (self.fills[i].fill, self.fills[i].gradient.clone());
            for s in merged {
                if shape_area(&s).abs() >= 0.01 {
                    out.push(FillRegion {
                        fill,
                        gradient: gradient.clone(),
                        contours: contours_of(&s),
                    });
                }
            }
        }
        self.fills = out;
    }

    /// Draw a fill on top: it cuts every region (and the stroke parts) under it.
    /// `cut_strokes`: false for Paint Fills.
    pub fn add_fill(
        &mut self,
        polys: &[Vec<Pt>],
        fill: Option<Color>,
        gradient: Option<Gradient>,
        cut_strokes: bool,
    ) {
        let new = normalize(polys);
        if new.is_empty() {
            return;
        }
        self.cut(&new, true, cut_strokes);
        self.push_regions(new, fill, gradient);
        self.merge_same_paint();
    }

    /// Remove `area` from fills and/or strokes.
    fn cut(&mut self, area: &[Shp], fills: bool, strokes: bool) {
        let abb = bbox(
            area.iter()
                .flat_map(|s| s.iter().flat_map(|c| c.iter().map(|x| q(*x)))),
        );
        let Some(abb) = abb else { return };
        if fills {
            let old = std::mem::take(&mut self.fills);
            for r in old {
                match r.bounds() {
                    Some(b) if boxes_touch(b, abb, 0.01) => {
                        let rest = diff_shapes(&[r.shp()], area);
                        self.push_regions(rest, r.fill, r.gradient.clone());
                    }
                    _ => self.fills.push(r),
                }
            }
        }
        if strokes {
            let old = std::mem::take(&mut self.strokes);
            for s in old {
                let sb = bbox(s.pts.iter().copied()).unwrap();
                if !boxes_touch(sb, abb, s.stroke.width) {
                    self.strokes.push(s);
                    continue;
                }
                let line: Vec<P> = s.closed_pts().iter().map(|x| p(*x)).collect();
                let area_v: Vec<Shp> = area.to_vec();
                let kept = line.clip_by(
                    &area_v,
                    FillRule::NonZero,
                    ClipRule {
                        invert: true,
                        boundary_included: true,
                    },
                );
                for k in kept {
                    let pts: Vec<Pt> = k.iter().map(|x| q(*x)).collect();
                    if pts.len() >= 2 && polyline_len(&pts) > 0.05 {
                        self.strokes.push(StrokeEdge {
                            pts,
                            closed: false,
                            stroke: s.stroke,
                        });
                    }
                }
            }
        }
    }

    /// Draw a line: it splits the fills it crosses and the lines it crosses
    /// (and is split by them).
    pub fn add_stroke(&mut self, pts: &[Pt], closed: bool, stroke: Stroke) {
        if pts.len() < 2 {
            return;
        }
        let mut line = pts.to_vec();
        if closed {
            line.push(pts[0]);
        }
        // split fills along the line
        let lb = bbox(line.iter().copied()).unwrap();
        let old = std::mem::take(&mut self.fills);
        let lp: Vec<P> = line.iter().map(|x| p(*x)).collect();
        for r in old {
            match r.bounds() {
                Some(b) if boxes_touch(b, lb, 0.01) => {
                    let pieces = r.shp().slice_by(&lp, FillRule::EvenOdd);
                    self.push_regions(pieces, r.fill, r.gradient.clone());
                }
                _ => self.fills.push(r),
            }
        }
        // split existing strokes where the new line crosses them
        let old = std::mem::take(&mut self.strokes);
        for s in old {
            let sp = s.closed_pts();
            let at = crossings(&sp, &line);
            if at.is_empty() {
                self.strokes.push(s);
            } else {
                for piece in split_polyline(&sp, &at) {
                    self.strokes.push(StrokeEdge {
                        pts: piece,
                        closed: false,
                        stroke: s.stroke,
                    });
                }
            }
        }
        // and split the new line at existing strokes (and itself)
        let mut at = vec![];
        for s in &self.strokes {
            at.extend(crossings(&line, &s.closed_pts()));
        }
        at.sort_by(|x, y| (x.0, x.1).partial_cmp(&(y.0, y.1)).unwrap());
        at.dedup_by(|x, y| x.0 == y.0 && (x.1 - y.1).abs() < 1e-3);
        let pieces = split_polyline(&line, &at);
        let whole = pieces.len() == 1 && closed;
        for piece in pieces {
            let mut piece = piece;
            if whole {
                piece.pop();
            }
            self.strokes.push(StrokeEdge {
                pts: piece,
                closed: whole,
                stroke,
            });
        }
    }

    /// A filled and/or stroked path drawn in merge mode.
    pub fn add_path(
        &mut self,
        path: &Path,
        fill: Option<Color>,
        gradient: Option<Gradient>,
        stroke: Option<Stroke>,
    ) {
        let polys = path_polygons(path);
        if (fill.is_some() || gradient.is_some()) && !polys.is_empty() {
            self.add_fill(&polys, fill, gradient, true);
        }
        if let Some(st) = stroke {
            for (line, closed) in path_polylines(path) {
                // a closed fill outline is a closed stroke
                self.add_stroke(&line, closed, st);
            }
        }
    }

    /// Eraser over `area` (closed polygons) in `mode`.
    pub fn erase(
        &mut self,
        area: &[Vec<Pt>],
        mode: PaintMode,
        selected: &[Part],
        inside: Option<usize>,
    ) {
        let a = normalize(area);
        if a.is_empty() {
            return;
        }
        match mode {
            PaintMode::Normal => self.cut(&a, true, true),
            PaintMode::Fills => self.cut(&a, true, false),
            PaintMode::Lines => self.cut(&a, false, true),
            PaintMode::Selection | PaintMode::Inside | PaintMode::Behind => {
                let which: Vec<usize> = match mode {
                    PaintMode::Inside => inside.into_iter().collect(),
                    _ => selected
                        .iter()
                        .filter_map(|x| {
                            if let Part::Fill(i) = x {
                                Some(*i)
                            } else {
                                None
                            }
                        })
                        .collect(),
                };
                let mut out = vec![];
                for (i, r) in std::mem::take(&mut self.fills).into_iter().enumerate() {
                    if which.contains(&i) {
                        let rest = diff_shapes(&[r.shp()], &a);
                        for s in rest {
                            if shape_area(&s).abs() >= 0.01 {
                                out.push(FillRegion {
                                    fill: r.fill,
                                    gradient: r.gradient.clone(),
                                    contours: contours_of(&s),
                                });
                            }
                        }
                    } else {
                        out.push(r);
                    }
                }
                self.fills = out;
            }
        }
    }

    /// Brush paint `area` in `mode` (Paint Normal / Fills / Behind / Selection / Inside).
    pub fn paint(
        &mut self,
        area: &[Vec<Pt>],
        fill: Color,
        mode: PaintMode,
        selected: &[Part],
        inside: Option<usize>,
    ) {
        let a = normalize(area);
        if a.is_empty() {
            return;
        }
        let a = match mode {
            PaintMode::Normal | PaintMode::Fills | PaintMode::Lines => a,
            PaintMode::Behind => {
                let all: Vec<Shp> = self.fills.iter().map(|r| r.shp()).collect();
                diff_shapes(&a, &all)
            }
            PaintMode::Selection | PaintMode::Inside => {
                let which: Vec<usize> = match mode {
                    PaintMode::Inside => inside.into_iter().collect(),
                    _ => selected
                        .iter()
                        .filter_map(|x| {
                            if let Part::Fill(i) = x {
                                Some(*i)
                            } else {
                                None
                            }
                        })
                        .collect(),
                };
                let sel: Vec<Shp> = which
                    .iter()
                    .filter_map(|&i| self.fills.get(i))
                    .map(|r| r.shp())
                    .collect();
                if sel.is_empty() {
                    return;
                }
                intersect_shapes(&a, &sel)
            }
        };
        if a.is_empty() {
            return;
        }
        self.cut(&a, true, mode == PaintMode::Normal);
        self.push_regions(a, Some(fill), None);
        self.merge_same_paint();
    }

    /// Topmost part at `x` (strokes first, within `tol` of their edge).
    pub fn hit(&self, x: Pt, tol: f32) -> Option<Part> {
        for (i, s) in self.strokes.iter().enumerate().rev() {
            if s.dist(x) <= s.stroke.width / 2.0 + tol {
                return Some(Part::Stroke(i));
            }
        }
        self.fill_at(x).map(Part::Fill)
    }
    pub fn fill_at(&self, x: Pt) -> Option<usize> {
        self.fills.iter().rposition(|r| {
            r.bounds()
                .map(|b| boxes_touch(b, (x, x), 0.0))
                .unwrap_or(false)
                && r.contains(x)
        })
    }

    /// Double-click: a fill plus the strokes along its edge (or a stroke plus
    /// every stroke connected to it, same style).
    pub fn connected(&self, part: Part) -> Vec<Part> {
        match part {
            Part::Fill(i) => {
                let Some(r) = self.fills.get(i) else {
                    return vec![];
                };
                let mut v = vec![part];
                for (k, s) in self.strokes.iter().enumerate() {
                    let on = s
                        .pts
                        .iter()
                        .filter(|x| {
                            r.contours.iter().any(|c| {
                                c.windows(2)
                                    .chain(std::iter::once([c[c.len() - 1], c[0]].as_slice()))
                                    .any(|w| seg_dist(**x, w[0], w[1]).0 < s.stroke.width.max(1.0))
                            })
                        })
                        .count();
                    if on * 2 >= s.pts.len() {
                        v.push(Part::Stroke(k));
                    }
                }
                v
            }
            Part::Stroke(i) => {
                let mut seen = vec![i];
                let mut k = 0;
                while k < seen.len() {
                    let s = &self.strokes[seen[k]];
                    let ends = [s.pts[0], s.pts[s.pts.len() - 1]];
                    for (j, o) in self.strokes.iter().enumerate() {
                        if seen.contains(&j) || o.stroke != s.stroke {
                            continue;
                        }
                        let oe = [o.pts[0], o.pts[o.pts.len() - 1]];
                        if ends.iter().any(|a| oe.iter().any(|b| a.dist(*b) < 0.05)) {
                            seen.push(j);
                        }
                    }
                    k += 1;
                }
                seen.into_iter().map(Part::Stroke).collect()
            }
        }
    }

    /// Take parts out (they become a floating piece, e.g. while dragging).
    pub fn extract(&mut self, parts: &[Part]) -> MergeShape {
        let mut out = MergeShape::default();
        let mut fi: Vec<usize> = parts
            .iter()
            .filter_map(|x| {
                if let Part::Fill(i) = x {
                    Some(*i)
                } else {
                    None
                }
            })
            .collect();
        let mut si: Vec<usize> = parts
            .iter()
            .filter_map(|x| {
                if let Part::Stroke(i) = x {
                    Some(*i)
                } else {
                    None
                }
            })
            .collect();
        fi.sort();
        fi.dedup();
        si.sort();
        si.dedup();
        for &i in fi.iter().rev() {
            if i < self.fills.len() {
                out.fills.insert(0, self.fills.remove(i));
            }
        }
        for &i in si.iter().rev() {
            if i < self.strokes.len() {
                out.strokes.insert(0, self.strokes.remove(i));
            }
        }
        out
    }

    /// Cut out everything inside a rectangle (Selection marquee on a merge
    /// shape selects part of it): returns the inside piece, keeps the rest.
    pub fn extract_rect(&mut self, lo: Pt, hi: Pt) -> MergeShape {
        let rect: Vec<Pt> = vec![lo, pt(hi.x, lo.y), hi, pt(lo.x, hi.y)];
        self.extract_area(&[rect])
    }
    /// Cut out everything inside `area` (Lasso).
    pub fn extract_area(&mut self, area: &[Vec<Pt>]) -> MergeShape {
        let a = normalize(area);
        let mut inner = MergeShape::default();
        for r in &self.fills {
            let pieces = intersect_shapes(&[r.shp()], &a);
            inner.push_regions(pieces, r.fill, r.gradient.clone());
        }
        for s in &self.strokes {
            let line: Vec<P> = s.closed_pts().iter().map(|x| p(*x)).collect();
            let av: Vec<Shp> = a.clone();
            for k in line.clip_by(
                &av,
                FillRule::NonZero,
                ClipRule {
                    invert: false,
                    boundary_included: false,
                },
            ) {
                let pts: Vec<Pt> = k.iter().map(|x| q(*x)).collect();
                if pts.len() >= 2 && polyline_len(&pts) > 0.05 {
                    inner.strokes.push(StrokeEdge {
                        pts,
                        closed: false,
                        stroke: s.stroke,
                    });
                }
            }
        }
        self.cut(&a, true, true);
        inner
    }

    /// Drop a piece onto this shape: its fills cut what is under them, its
    /// strokes split fills and strokes (what happens when you release a
    /// dragged selection in merge mode).
    pub fn merge_in(&mut self, piece: &MergeShape) {
        for r in &piece.fills {
            self.add_fill(&r.contours, r.fill, r.gradient.clone(), true);
        }
        for s in &piece.strokes {
            self.add_stroke(&s.pts, s.closed, s.stroke);
        }
    }

    pub fn transform(&mut self, m: &Matrix) {
        for r in &mut self.fills {
            for c in &mut r.contours {
                for x in c.iter_mut() {
                    *x = m.apply(*x);
                }
            }
            if let Some(g) = &mut r.gradient {
                g.matrix = m.concat(&g.matrix);
            }
        }
        for s in &mut self.strokes {
            for x in s.pts.iter_mut() {
                *x = m.apply(*x);
            }
        }
    }

    pub fn bounds(&self) -> Option<(Pt, Pt)> {
        bbox(
            self.fills
                .iter()
                .flat_map(|r| r.contours.iter().flatten().copied())
                .chain(self.strokes.iter().flat_map(|s| s.pts.iter().copied())),
        )
    }
    pub fn part_bounds(&self, parts: &[Part]) -> Option<(Pt, Pt)> {
        bbox(parts.iter().flat_map(|x| -> Vec<Pt> {
            match x {
                Part::Fill(i) => self
                    .fills
                    .get(*i)
                    .map(|r| r.contours.concat())
                    .unwrap_or_default(),
                Part::Stroke(i) => self
                    .strokes
                    .get(*i)
                    .map(|s| s.pts.clone())
                    .unwrap_or_default(),
            }
        }))
    }

    /// Set the paint of fills / the style of strokes (Properties, Paint Bucket, Ink Bottle).
    pub fn set_fill(&mut self, parts: &[Part], fill: Option<Color>, gradient: Option<Gradient>) {
        for x in parts {
            if let Part::Fill(i) = x {
                if let Some(r) = self.fills.get_mut(*i) {
                    r.fill = fill;
                    r.gradient = gradient.clone();
                }
            }
        }
        self.merge_same_paint();
    }
    pub fn set_stroke(&mut self, parts: &[Part], stroke: Stroke) {
        for x in parts {
            if let Part::Stroke(i) = x {
                if let Some(s) = self.strokes.get_mut(*i) {
                    s.stroke = stroke;
                }
            }
        }
    }

    /// Everything as one filled+stroked path per part (Group / drawing object).
    pub fn to_shapes(&self) -> Vec<crate::model::Shape> {
        let mut v: Vec<crate::model::Shape> = self
            .fills
            .iter()
            .map(|r| crate::model::Shape {
                gradient: r.gradient.clone(),
                ..crate::model::Shape::new(r.path(), r.fill, None)
            })
            .collect();
        v.extend(
            self.strokes
                .iter()
                .map(|s| crate::model::Shape::new(s.path(), None, Some(s.stroke))),
        );
        v
    }

    /// Paint Bucket in an empty area enclosed by strokes: the enclosed face
    /// (closing gaps up to `gap`) becomes a fill. Returns false if `x` is not enclosed.
    pub fn fill_enclosed(&mut self, x: Pt, fill: Color, gap: f32) -> bool {
        use i_overlay::mesh::float::stroke::offset::StrokeOffset;
        use i_overlay::mesh::float::style::{LineCap, LineJoin, StrokeStyle};
        let r = (gap / 2.0).max(0.25) as f64;
        // thicken every stroke by the gap, union, and look for the hole around x
        let mut walls: Vec<Shp> = vec![];
        for s in &self.strokes {
            let line: Vec<P> = s.pts.iter().map(|a| p(*a)).collect();
            let style = StrokeStyle::new(r * 2.0)
                .line_join(LineJoin::Round(0.2))
                .start_cap(LineCap::Round(0.2))
                .end_cap(LineCap::Round(0.2));
            walls.extend(line.stroke(style, s.closed));
        }
        // existing fills are walls too
        walls.extend(self.fills.iter().map(|f| f.shp()));
        if walls.is_empty() {
            return false;
        }
        let empty: Vec<Shp> = vec![];
        let merged = walls.overlay(&empty, OverlayRule::Subject, FillRule::NonZero);
        for s in &merged {
            for hole in s.iter().skip(1) {
                let hc = vec![hole.iter().map(|a| q(*a)).collect::<Vec<Pt>>()];
                if point_in_contours(&hc, x) {
                    // grow the hole back under the strokes (they are drawn on top)
                    let face = normalize(&hc);
                    let grown = {
                        use i_overlay::mesh::float::outline::offset::OutlineOffset;
                        use i_overlay::mesh::float::style::OutlineStyle;
                        let st = OutlineStyle::new(r);
                        let mut g = vec![];
                        for f in &face {
                            g.extend(f.outline(&st));
                        }
                        g
                    };
                    let fills: Vec<Shp> = self.fills.iter().map(|f| f.shp()).collect();
                    let area = diff_shapes(&grown, &fills);
                    self.push_regions(area, Some(fill), None);
                    self.merge_same_paint();
                    return true;
                }
            }
        }
        false
    }

    /// Simplify: drop points that deviate less than `tol` (Modify ▸ Shape ▸ Optimize).
    pub fn optimize(&mut self, tol: f32) {
        for r in &mut self.fills {
            for c in &mut r.contours {
                let mut closed = c.clone();
                closed.push(c[0]);
                let mut s = crate::geom::simplify(&closed, tol);
                s.pop();
                if s.len() >= 3 {
                    *c = s;
                }
            }
        }
        for st in &mut self.strokes {
            let s = crate::geom::simplify(&st.pts, tol);
            if s.len() >= 2 {
                st.pts = s;
            }
        }
    }

    /// Smooth: Chaikin corner cutting (Modify ▸ Shape ▸ Smooth).
    pub fn smooth(&mut self) {
        fn chaikin(c: &[Pt], closed: bool) -> Vec<Pt> {
            let n = c.len();
            if n < 3 {
                return c.to_vec();
            }
            let mut out = vec![];
            if !closed {
                out.push(c[0]);
            }
            let m = if closed { n } else { n - 1 };
            for i in 0..m {
                let (a, b) = (c[i], c[(i + 1) % n]);
                out.push(a.lerp(b, 0.25));
                out.push(a.lerp(b, 0.75));
            }
            if !closed {
                out.push(c[n - 1]);
            }
            out
        }
        for r in &mut self.fills {
            for c in &mut r.contours {
                *c = chaikin(c, true);
            }
        }
        for s in &mut self.strokes {
            s.pts = chaikin(&s.pts, s.closed);
        }
    }

    /// Straighten: simplify hard so curves become straight runs (Modify ▸ Shape ▸ Straighten).
    pub fn straighten(&mut self) {
        let size = self
            .bounds()
            .map(|(lo, hi)| (hi.x - lo.x).max(hi.y - lo.y))
            .unwrap_or(100.0);
        self.optimize((size * 0.03).max(1.0));
    }

    /// Point-wise deformation of every vertex (edge bending).
    pub fn map_points(&mut self, mut f: impl FnMut(Pt) -> Pt) {
        for r in &mut self.fills {
            for c in &mut r.contours {
                for x in c.iter_mut() {
                    *x = f(*x);
                }
            }
        }
        for s in &mut self.strokes {
            for x in s.pts.iter_mut() {
                *x = f(*x);
            }
        }
    }

    /// Re-resolve overlaps after a free deformation: later regions win, same
    /// paints merge.
    pub fn resolve(&mut self) {
        let regions = std::mem::take(&mut self.fills);
        for r in regions {
            self.add_fill(&r.contours, r.fill, r.gradient.clone(), false);
        }
    }
}

/// Shape tween between two merge shapes: regions and strokes pair up in order.
pub fn morph(a: &MergeShape, b: &MergeShape, t: f32) -> MergeShape {
    let mut out = MergeShape::default();
    let n = a.fills.len().max(b.fills.len());
    for i in 0..n {
        match (a.fills.get(i), b.fills.get(i)) {
            (Some(x), Some(y)) => {
                let path = crate::geom::morph_paths(&x.path(), &y.path(), t);
                let fill = match (x.fill, y.fill) {
                    (Some(c), Some(d)) => Some(c.lerp(d, t)),
                    (c, d) => {
                        if t < 0.5 {
                            c
                        } else {
                            d
                        }
                    }
                };
                out.fills.push(FillRegion {
                    fill,
                    gradient: if t < 0.5 {
                        x.gradient.clone()
                    } else {
                        y.gradient.clone()
                    },
                    contours: path_polygons(&path),
                });
            }
            (Some(x), None) if t < 0.5 => out.fills.push(x.clone()),
            (None, Some(y)) if t >= 0.5 => out.fills.push(y.clone()),
            _ => {}
        }
    }
    let n = a.strokes.len().max(b.strokes.len());
    for i in 0..n {
        match (a.strokes.get(i), b.strokes.get(i)) {
            (Some(x), Some(y)) => {
                let path = crate::geom::morph_paths(&x.path(), &y.path(), t);
                if let Some((pts, closed)) = path_polylines(&path).into_iter().next() {
                    let stroke = Stroke {
                        color: x.stroke.color.lerp(y.stroke.color, t),
                        width: x.stroke.width + (y.stroke.width - x.stroke.width) * t,
                    };
                    out.strokes.push(StrokeEdge {
                        pts,
                        closed,
                        stroke,
                    });
                }
            }
            (Some(x), None) if t < 0.5 => out.strokes.push(x.clone()),
            (None, Some(y)) if t >= 0.5 => out.strokes.push(y.clone()),
            _ => {}
        }
    }
    out
}

// ------------------------------------------------------------ Combine Objects

/// Modify ▸ Combine Objects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoolOp {
    Union,
    Intersect,
    /// Top object punches a hole in the ones below.
    Punch,
    /// Top object keeps only what is under it.
    Crop,
}

/// Boolean op on two polygon sets (even-odd inputs); result as contours (even-odd).
pub fn bool_polys(a: &[Vec<Pt>], b: &[Vec<Pt>], op: BoolOp) -> Vec<Vec<Pt>> {
    let (sa, sb) = (normalize(a), normalize(b));
    let r = match op {
        BoolOp::Union => union_shapes(&sa, &sb),
        BoolOp::Intersect | BoolOp::Crop => intersect_shapes(&sa, &sb),
        BoolOp::Punch => diff_shapes(&sa, &sb),
    };
    r.iter().flat_map(contours_of).collect()
}

/// Contours → a closed straight-segment path.
pub fn polys_path(polys: &[Vec<Pt>]) -> Path {
    FillRegion {
        fill: None,
        gradient: None,
        contours: polys.to_vec(),
    }
    .path()
}

// ------------------------------------------------------------ edge bending

/// An edge grabbed for bending: the chain of boundary points between two
/// corners, resampled, plus the parameter where it was grabbed.
#[derive(Clone, Debug)]
pub struct Bend {
    base: MergeShape,
    /// For each vertex of `base` (fills then strokes, in order): parameter along the chain.
    params: Vec<Option<f32>>,
    grab: f32,
    /// Corner drag: falloff is linear from the corner.
    corner: bool,
}

fn all_polylines(m: &MergeShape) -> Vec<(Vec<Pt>, bool)> {
    let mut v: Vec<(Vec<Pt>, bool)> = m
        .fills
        .iter()
        .flat_map(|r| r.contours.iter().map(|c| (c.clone(), true)))
        .collect();
    v.extend(m.strokes.iter().map(|s| (s.pts.clone(), s.closed)));
    v
}

/// Turn angle at vertex i (radians).
fn turn(c: &[Pt], i: usize, closed: bool) -> f32 {
    let n = c.len();
    if !closed && (i == 0 || i + 1 == n) {
        return std::f32::consts::PI;
    }
    let a = c[(i + n - 1) % n];
    let b = c[i];
    let d = c[(i + 1) % n];
    let (u, v) = (pt(b.x - a.x, b.y - a.y), pt(d.x - b.x, d.y - b.y));
    let (lu, lv) = (
        (u.x * u.x + u.y * u.y).sqrt(),
        (v.x * v.x + v.y * v.y).sqrt(),
    );
    if lu < 1e-6 || lv < 1e-6 {
        return 0.0;
    }
    ((u.x * v.x + u.y * v.y) / (lu * lv))
        .clamp(-1.0, 1.0)
        .acos()
}

const CORNER: f32 = 0.6; // ~35°

impl MergeShape {
    /// Nearest boundary point to `x` within `tol`: (polyline index, segment, t, is a corner vertex).
    fn nearest_edge(&self, x: Pt, tol: f32) -> Option<(usize, usize, f32, Option<usize>)> {
        let lines = all_polylines(self);
        let mut best: Option<(f32, usize, usize, f32)> = None;
        for (li, (c, closed)) in lines.iter().enumerate() {
            let n = c.len();
            let segs = if *closed { n } else { n.saturating_sub(1) };
            for i in 0..segs {
                let (d, t) = seg_dist(x, c[i], c[(i + 1) % n]);
                if d <= tol && best.map(|b| d < b.0).unwrap_or(true) {
                    best = Some((d, li, i, t));
                }
            }
        }
        let (_, li, seg, t) = best?;
        let (c, closed) = &lines[li];
        let n = c.len();
        // near a corner vertex?
        let corner = [(seg, 0.0f32), ((seg + 1) % n, 1.0)]
            .into_iter()
            .find(|(v, at)| {
                (t - at).abs() * c[seg].dist(c[(seg + 1) % n]) <= tol
                    && turn(c, *v, *closed) > CORNER
            })
            .map(|(v, _)| v);
        Some((li, seg, t, corner))
    }

    /// Start bending at `x` (Selection tool on an edge or a corner).
    pub fn begin_bend(&self, x: Pt, tol: f32) -> Option<Bend> {
        let (li, seg, t, corner) = self.nearest_edge(x, tol)?;
        let lines = all_polylines(self);
        let (c, closed) = &lines[li];
        let n = c.len();
        if let Some(v) = corner {
            // corner: every coincident vertex moves fully; the chains leaving it fall off linearly
            let at = c[v];
            let mut params = vec![];
            let mut base = self.clone();
            // resample is not needed for corners; param = 1 at the corner, falloff by distance along chains
            let reach = chain_reach(&lines, at);
            base.map_points(|q| q);
            for (pl, cl) in all_polylines(&base) {
                let _ = cl;
                for qv in &pl {
                    params.push(reach(*qv));
                }
            }
            return Some(Bend {
                base,
                params,
                grab: 1.0,
                corner: true,
            });
        }
        // edge: walk to the corners on both sides of the grabbed segment
        let (mut a, mut b) = (seg, (seg + 1) % n);
        let mut guard = 0;
        while turn(c, a, *closed) < CORNER && guard < n {
            if !*closed && a == 0 {
                break;
            }
            a = (a + n - 1) % n;
            guard += 1;
        }
        guard = 0;
        while turn(c, b, *closed) < CORNER && guard < n {
            if !*closed && b == n - 1 {
                break;
            }
            b = (b + 1) % n;
            guard += 1;
        }
        // the chain from a to b (forward)
        let mut chain = vec![c[a]];
        let mut i = a;
        loop {
            i = (i + 1) % n;
            chain.push(c[i]);
            if i == b || chain.len() > n + 1 {
                break;
            }
        }
        let total = polyline_len(&chain).max(1e-3);
        // grab parameter
        let mut acc = 0.0;
        let mut grab = 0.5;
        let mut i = a;
        loop {
            let j = (i + 1) % n;
            let l = c[i].dist(c[j]);
            if i == seg {
                grab = (acc + l * t) / total;
                break;
            }
            acc += l;
            i = j;
            if i == b {
                break;
            }
        }
        let resampled = resample(&chain, (total / 2.0).clamp(8.0, 64.0) as usize);
        let base = insert_chain(self, &chain, &resampled);
        let mut params = vec![];
        for (pl, _) in all_polylines(&base) {
            for qv in &pl {
                params.push(param_on(&resampled, *qv, total));
            }
        }
        Some(Bend {
            base,
            params,
            grab: grab.clamp(0.02, 0.98),
            corner: false,
        })
    }
}

impl Bend {
    /// The shape with the grabbed point moved by `delta`.
    pub fn apply(&self, delta: Pt) -> MergeShape {
        let mut out = self.base.clone();
        let mut k = 0;
        let (g, corner) = (self.grab, self.corner);
        let w = |s: f32| -> f32 {
            if corner {
                return s;
            }
            let u = if s <= g { s / g } else { (1.0 - s) / (1.0 - g) };
            let u = u.clamp(0.0, 1.0);
            1.0 - (1.0 - u) * (1.0 - u)
        };
        out.map_points(|x| {
            let r = match self.params.get(k).copied().flatten() {
                Some(s) => {
                    let f = w(s);
                    pt(x.x + delta.x * f, x.y + delta.y * f)
                }
                None => x,
            };
            k += 1;
            r
        });
        out
    }
}

fn resample(chain: &[Pt], n: usize) -> Vec<Pt> {
    let total = polyline_len(chain);
    let n = n.max(2);
    let mut out = vec![chain[0]];
    let mut seg = 0;
    let mut acc = 0.0;
    for k in 1..n {
        let target = total * k as f32 / n as f32;
        while seg + 1 < chain.len() && acc + chain[seg].dist(chain[seg + 1]) < target {
            acc += chain[seg].dist(chain[seg + 1]);
            seg += 1;
        }
        if seg + 1 >= chain.len() {
            break;
        }
        let l = chain[seg].dist(chain[seg + 1]).max(1e-6);
        out.push(chain[seg].lerp(chain[seg + 1], (target - acc) / l));
    }
    out.push(chain[chain.len() - 1]);
    out
}

/// Parameter (0..1) of `x` along `chain` if it lies on it.
fn param_on(chain: &[Pt], x: Pt, total: f32) -> Option<f32> {
    let mut acc = 0.0;
    for w in chain.windows(2) {
        let (d, t) = seg_dist(x, w[0], w[1]);
        let l = w[0].dist(w[1]);
        if d < 0.05 {
            return Some((acc + l * t) / total);
        }
        acc += l;
    }
    None
}

/// Every polyline segment lying on `chain` gets the resampled points in between,
/// so the bend can curve it.
fn insert_chain(m: &MergeShape, chain: &[Pt], resampled: &[Pt]) -> MergeShape {
    let total = polyline_len(chain).max(1e-3);
    let fix = |c: &[Pt], closed: bool| -> Vec<Pt> {
        let n = c.len();
        let mut out = vec![];
        let segs = if closed { n } else { n - 1 };
        for i in 0..n {
            out.push(c[i]);
            if i >= segs {
                continue;
            }
            let (a, b) = (c[i], c[(i + 1) % n]);
            if let (Some(sa), Some(sb)) = (param_on(chain, a, total), param_on(chain, b, total)) {
                // the resampled points strictly between sa and sb, in direction a → b
                let mut mids: Vec<(f32, Pt)> = resampled
                    .iter()
                    .filter_map(|r| param_on(chain, *r, total).map(|s| (s, *r)))
                    .filter(|(s, _)| (*s > sa.min(sb) + 1e-4) && (*s < sa.max(sb) - 1e-4))
                    .collect();
                // only when the segment really runs along the chain (its midpoint is on it)
                if param_on(chain, a.lerp(b, 0.5), total).is_none() {
                    continue;
                }
                mids.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap());
                if sa > sb {
                    mids.reverse();
                }
                out.extend(mids.into_iter().map(|(_, r)| r));
            }
        }
        out
    };
    let mut out = m.clone();
    for r in &mut out.fills {
        for c in &mut r.contours {
            *c = fix(c, true);
        }
    }
    for s in &mut out.strokes {
        s.pts = fix(&s.pts, s.closed);
    }
    out
}

/// Corner falloff: 1 at the corner, decreasing linearly along the chains
/// that leave it (to the next corner); None elsewhere.
fn chain_reach(lines: &[(Vec<Pt>, bool)], at: Pt) -> impl Fn(Pt) -> Option<f32> {
    // collect, for each chain leaving the corner, its points with their falloff
    let mut marks: Vec<(Pt, f32)> = vec![(at, 1.0)];
    for (c, closed) in lines {
        let n = c.len();
        for v in 0..n {
            if c[v].dist(at) > 0.05 {
                continue;
            }
            for dir in [1isize, -1] {
                let mut chain = vec![c[v]];
                let mut i = v as isize;
                for _ in 0..n {
                    i += dir;
                    if !*closed && (i < 0 || i >= n as isize) {
                        break;
                    }
                    let k = i.rem_euclid(n as isize) as usize;
                    chain.push(c[k]);
                    if turn(c, k, *closed) > CORNER {
                        break;
                    }
                }
                let total = polyline_len(&chain).max(1e-3);
                let mut acc = 0.0;
                for w in chain.windows(2) {
                    acc += w[0].dist(w[1]);
                    marks.push((w[1], 1.0 - acc / total));
                }
            }
        }
    }
    move |x: Pt| {
        marks
            .iter()
            .filter(|(m, _)| m.dist(x) < 0.05)
            .map(|(_, f)| *f)
            .fold(None, |a: Option<f32>, f| Some(a.map_or(f, |a| a.max(f))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: f32, y: f32, w: f32, h: f32) -> Vec<Pt> {
        vec![pt(x, y), pt(x + w, y), pt(x + w, y + h), pt(x, y + h)]
    }
    fn red() -> Option<Color> {
        Some(Color::rgb(255, 0, 0))
    }
    fn blue() -> Option<Color> {
        Some(Color::rgb(0, 0, 255))
    }
    fn area(m: &MergeShape, i: usize) -> f64 {
        shape_area(&m.fills[i].shp()).abs()
    }

    #[test]
    fn new_fill_cuts_and_same_colour_merges() {
        let mut m = MergeShape::default();
        m.add_fill(&[rect(0., 0., 100., 100.)], red(), None, true);
        m.add_fill(&[rect(50., 0., 100., 100.)], blue(), None, true);
        assert_eq!(m.fills.len(), 2);
        let r = m.fills.iter().position(|f| f.fill == red()).unwrap();
        assert!(
            (area(&m, r) - 5000.0).abs() < 1.0,
            "blue cut the red square in half: {}",
            area(&m, r)
        );
        // red over both: merges with the red half into one region
        m.add_fill(&[rect(40., 40., 30., 20.)], red(), None, true);
        let reds: Vec<usize> = (0..m.fills.len())
            .filter(|&i| m.fills[i].fill == red())
            .collect();
        assert_eq!(reds.len(), 1, "touching red areas merge");
        // blue square got a notch
        let b = m.fills.iter().position(|f| f.fill == blue()).unwrap();
        assert!(
            (area(&m, b) - (10000.0 - 400.0)).abs() < 1.0,
            "{}",
            area(&m, b)
        );
    }

    #[test]
    fn line_splits_fill_and_lines_split_each_other() {
        let mut m = MergeShape::default();
        let st = Stroke {
            color: Color::rgb(0, 0, 0),
            width: 1.0,
        };
        m.add_fill(&[rect(0., 0., 100., 100.)], red(), None, true);
        m.add_stroke(&[pt(-10.0, 50.0), pt(110.0, 50.0)], false, st);
        assert_eq!(m.fills.len(), 2, "the line cut the square in two");
        assert_eq!(m.strokes.len(), 1);
        m.add_stroke(&[pt(50.0, -10.0), pt(50.0, 110.0)], false, st);
        assert_eq!(m.fills.len(), 4);
        assert_eq!(
            m.strokes.len(),
            4,
            "crossing lines split into four segments"
        );
        // the top-left quarter can be picked alone
        let hit = m.hit(pt(25.0, 25.0), 1.0).unwrap();
        assert!(matches!(hit, Part::Fill(_)));
        let piece = m.extract(&[hit]);
        assert_eq!(piece.fills.len(), 1);
        assert_eq!(m.fills.len(), 3);
    }

    #[test]
    fn shape_over_lines_erases_them_and_drop_merges() {
        let mut m = MergeShape::default();
        let st = Stroke {
            color: Color::rgb(0, 0, 0),
            width: 2.0,
        };
        m.add_stroke(&[pt(0.0, 50.0), pt(200.0, 50.0)], false, st);
        m.add_fill(&[rect(50., 0., 100., 100.)], blue(), None, true);
        let total: f32 = m.strokes.iter().map(|s| polyline_len(&s.pts)).sum();
        assert!(
            (total - 100.0).abs() < 0.5,
            "the fill covered the middle of the line: {total}"
        );
        // marquee-select the right half, drag it onto the left part
        let mut piece = m.extract_rect(pt(100.0, -10.0), pt(220.0, 110.0));
        assert!(!piece.fills.is_empty());
        piece.transform(&Matrix::translate(-80.0, 0.0));
        m.merge_in(&piece);
        assert_eq!(
            m.fills.iter().filter(|f| f.fill == blue()).count(),
            1,
            "dropped blue merges with blue"
        );
    }

    #[test]
    fn eraser_and_brush_modes() {
        let mut m = MergeShape::default();
        let st = Stroke {
            color: Color::rgb(0, 0, 0),
            width: 2.0,
        };
        m.add_fill(&[rect(0., 0., 100., 100.)], red(), None, true);
        m.add_stroke(&[pt(-20.0, 50.0), pt(120.0, 50.0)], false, st);
        // erase lines only
        let mut a = m.clone();
        a.erase(&[rect(40., 40., 20., 20.)], PaintMode::Lines, &[], None);
        let fill_area: f64 = (0..a.fills.len()).map(|i| area(&a, i)).sum();
        assert!((fill_area - 10000.0).abs() < 1.0);
        assert_eq!(a.strokes.len(), 2);
        // paint behind: only outside the square
        let mut b = m.clone();
        b.paint(
            &[rect(80., 80., 40., 40.)],
            Color::rgb(0, 255, 0),
            PaintMode::Behind,
            &[],
            None,
        );
        let g = b
            .fills
            .iter()
            .position(|f| f.fill == Some(Color::rgb(0, 255, 0)))
            .unwrap();
        assert!((area(&b, g) - (1600.0 - 400.0)).abs() < 1.0);
        // paint inside the lower half only
        let lower = m.fill_at(pt(50.0, 75.0)).unwrap();
        let mut c = m.clone();
        c.paint(
            &[rect(40., 40., 20., 20.)],
            Color::rgb(0, 255, 0),
            PaintMode::Inside,
            &[],
            Some(lower),
        );
        let g = c
            .fills
            .iter()
            .position(|f| f.fill == Some(Color::rgb(0, 255, 0)))
            .unwrap();
        assert!((area(&c, g) - 200.0).abs() < 1.0);
    }

    #[test]
    fn bucket_fills_an_enclosed_area_with_gaps() {
        let mut m = MergeShape::default();
        let st = Stroke {
            color: Color::rgb(0, 0, 0),
            width: 1.0,
        };
        // a square outline with a 3 px gap in the top edge
        m.add_stroke(&[pt(0.0, 0.0), pt(48.0, 0.0)], false, st);
        m.add_stroke(
            &[
                pt(51.0, 0.0),
                pt(100.0, 0.0),
                pt(100.0, 100.0),
                pt(0.0, 100.0),
                pt(0.0, 0.0),
            ],
            false,
            st,
        );
        assert!(
            !m.clone()
                .fill_enclosed(pt(50.0, 50.0), Color::rgb(0, 255, 0), 1.0),
            "small gap setting: leaks"
        );
        assert!(
            m.fill_enclosed(pt(50.0, 50.0), Color::rgb(0, 255, 0), 6.0),
            "closes the gap"
        );
        let a = area(&m, 0);
        assert!(a > 9000.0 && a < 10100.0, "{a}");
    }

    #[test]
    fn bending_an_edge_curves_shared_geometry() {
        let mut m = MergeShape::default();
        let st = Stroke {
            color: Color::rgb(0, 0, 0),
            width: 1.0,
        };
        m.add_path(&Path::rect(0.0, 0.0, 100.0, 100.0), red(), None, Some(st));
        let bend = m
            .begin_bend(pt(50.0, 0.0), 2.0)
            .expect("grabbed the top edge");
        let out = bend.apply(pt(0.0, -30.0));
        // the stroke's top midpoint and the fill's top midpoint both moved up by 30
        let top_fill = out.fills[0].contours[0]
            .iter()
            .map(|x| x.y)
            .fold(f32::MAX, f32::min);
        let top_line = out.strokes[0]
            .pts
            .iter()
            .map(|x| x.y)
            .fold(f32::MAX, f32::min);
        assert!(
            (top_fill + 30.0).abs() < 1.0 && (top_line + 30.0).abs() < 1.0,
            "{top_fill} {top_line}"
        );
        // the corners did not move
        assert!(out.fills[0].contours[0]
            .iter()
            .any(|x| x.dist(pt(0.0, 0.0)) < 0.01));
        // corner drag
        let c = m.begin_bend(pt(100.0, 100.0), 2.0).unwrap();
        let out = c.apply(pt(20.0, 20.0));
        assert!(out.fills[0].contours[0]
            .iter()
            .any(|x| x.dist(pt(120.0, 120.0)) < 0.01));
        assert!(out.fills[0].contours[0]
            .iter()
            .any(|x| x.dist(pt(0.0, 0.0)) < 0.01));
    }
}
