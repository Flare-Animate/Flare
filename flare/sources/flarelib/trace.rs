//! Modify › Bitmap › Trace Bitmap: turn a bitmap into vector fill regions.
//!
//! The approach follows OpenToonz's outline vectorizer (colour regions →
//! region borders → simplified outlines), written for our merge-shape model:
//!
//! 1. **Regions**: pixels join a region while their colour stays within
//!    `color_threshold` of the region's seed colour (4-connected flood fill).
//!    Transparent pixels (alpha < 50 %) belong to no region.
//! 2. **Minimum area**: regions smaller than `min_area` pixels are absorbed by
//!    the neighbour they share the longest border with.
//! 3. **Borders**: the pixel edges between two different regions form chains
//!    between junctions. Each chain is simplified **once** and used by both
//!    regions, so neighbouring fills meet exactly (no gaps, no overlaps).
//! 4. **Curve fit / corners**: chains are smoothed (Curve fit) and corners
//!    sharper than the Corner threshold are kept sharp.
//!
//! Each region becomes one `FillRegion` of a `MergeShape` (outer contour and
//! holes), coloured with the region's average colour.

use crate::geom::{pt, Pt};
use crate::merge::{FillRegion, MergeShape};
use crate::model::Color;
use std::collections::HashMap;

/// Trace Bitmap options (Animate's dialog).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TraceOpts {
    /// Colour difference (sum of RGB differences, 0–500) still counted as the same colour.
    pub color_threshold: u32,
    /// Regions smaller than this many pixels merge into a neighbour (1–1000).
    pub min_area: u32,
    pub curve_fit: CurveFit,
    pub corners: Corners,
    /// Outline: colour regions become fills. Centerline: dark line art
    /// becomes lines along its middle (OpenToonz's centerline vectorizer).
    pub mode: TraceMode,
    /// Centerline: pixels darker than this (0–255 luminance) are ink.
    pub line_threshold: u8,
    /// Centerline: one width per line instead of following the ink's thickness.
    pub uniform_width: bool,
}

impl Default for TraceOpts {
    fn default() -> TraceOpts {
        TraceOpts { color_threshold: 100, min_area: 8, curve_fit: CurveFit::Normal, corners: Corners::Normal, mode: TraceMode::Outline, line_threshold: 128, uniform_width: false }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TraceMode {
    #[default]
    Outline,
    Centerline,
}

impl TraceMode {
    pub fn from_name(s: &str) -> Option<TraceMode> {
        match s.to_ascii_lowercase().as_str() {
            "outline" | "fills" => Some(TraceMode::Outline),
            "centerline" | "centreline" | "lines" => Some(TraceMode::Centerline),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CurveFit {
    Pixels,
    VeryTight,
    Tight,
    Normal,
    Smooth,
    VerySmooth,
}

impl CurveFit {
    pub const ALL: [CurveFit; 6] = [CurveFit::Pixels, CurveFit::VeryTight, CurveFit::Tight, CurveFit::Normal, CurveFit::Smooth, CurveFit::VerySmooth];
    pub fn name(self) -> &'static str {
        match self {
            CurveFit::Pixels => "Pixels",
            CurveFit::VeryTight => "Very Tight",
            CurveFit::Tight => "Tight",
            CurveFit::Normal => "Normal",
            CurveFit::Smooth => "Smooth",
            CurveFit::VerySmooth => "Very Smooth",
        }
    }
    /// Simplification tolerance in pixels, and smoothing passes.
    pub(crate) fn params(self) -> (f32, usize) {
        match self {
            CurveFit::Pixels => (0.0, 0),
            CurveFit::VeryTight => (0.35, 1),
            CurveFit::Tight => (0.6, 1),
            CurveFit::Normal => (0.9, 2),
            CurveFit::Smooth => (1.4, 3),
            CurveFit::VerySmooth => (2.2, 4),
        }
    }
    pub fn from_name(s: &str) -> Option<CurveFit> {
        let k = s.to_lowercase().replace([' ', '_', '-'], "");
        CurveFit::ALL.into_iter().find(|c| c.name().to_lowercase().replace(' ', "") == k)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Corners {
    Many,
    Normal,
    Few,
}

impl Corners {
    pub const ALL: [Corners; 3] = [Corners::Many, Corners::Normal, Corners::Few];
    pub fn name(self) -> &'static str {
        match self {
            Corners::Many => "Many corners",
            Corners::Normal => "Normal",
            Corners::Few => "Few corners",
        }
    }
    /// Turning angle (degrees) above which a vertex stays a sharp corner.
    pub(crate) fn angle(self) -> f32 {
        match self {
            Corners::Many => 50.0,
            Corners::Normal => 75.0,
            Corners::Few => 110.0,
        }
    }
    pub fn from_name(s: &str) -> Option<Corners> {
        match s.to_lowercase().replace([' ', '_', '-'], "").as_str() {
            "many" | "manycorners" => Some(Corners::Many),
            "normal" => Some(Corners::Normal),
            "few" | "fewcorners" => Some(Corners::Few),
            _ => None,
        }
    }
}

const NONE: u32 = u32::MAX;

/// Trace premultiplied RGBA pixels (`w` × `h`) into fill regions, in pixel
/// coordinates (0,0 = the bitmap's top-left).
pub fn trace(rgba: &[u8], w: u32, h: u32, opts: &TraceOpts) -> MergeShape {
    if opts.mode == TraceMode::Centerline {
        return crate::centerline::trace(rgba, w, h, opts);
    }
    let (w, h) = (w as usize, h as usize);
    if w == 0 || h == 0 || rgba.len() < w * h * 4 {
        return MergeShape::default();
    }
    // straight colours
    let px: Vec<[u8; 4]> = rgba
        .chunks_exact(4)
        .map(|p| {
            let a = p[3];
            if a == 0 {
                [0, 0, 0, 0]
            } else {
                let u = |c: u8| ((c as u32 * 255 + a as u32 / 2) / a as u32).min(255) as u8;
                [u(p[0]), u(p[1]), u(p[2]), a]
            }
        })
        .collect();
    let mut label = vec![NONE; w * h];
    let mut regions: Vec<Region> = vec![];
    let th = opts.color_threshold.min(765) as i32;
    let mut stack = vec![];
    for start in 0..w * h {
        if label[start] != NONE || px[start][3] < 128 {
            continue;
        }
        let id = regions.len() as u32;
        let seed = px[start];
        let mut r = Region::default();
        label[start] = id;
        stack.push(start);
        while let Some(i) = stack.pop() {
            r.add(px[i]);
            let (x, y) = (i % w, i / w);
            let mut try_push = |j: usize| {
                if label[j] == NONE && px[j][3] >= 128 && diff(px[j], seed) <= th {
                    label[j] = id;
                    stack.push(j);
                }
            };
            if x > 0 {
                try_push(i - 1);
            }
            if x + 1 < w {
                try_push(i + 1);
            }
            if y > 0 {
                try_push(i - w);
            }
            if y + 1 < h {
                try_push(i + w);
            }
        }
        regions.push(r);
    }
    absorb_small(&mut label, &mut regions, w, h, opts.min_area.max(1));
    let chains = border_chains(&label, w, h);
    let (tol, smooth) = opts.curve_fit.params();
    let corner = opts.corners.angle();
    let simplified: Vec<Vec<Pt>> = chains.iter().map(|c| simplify_chain(&c.pts, c.closed, tol, smooth, corner)).collect();
    // assemble each region's contours from the chains bordering it
    let mut by_region: HashMap<u32, Vec<(usize, bool)>> = HashMap::new();
    for (k, c) in chains.iter().enumerate() {
        // `left` is on the chain's left in its direction; `right` gets it reversed
        if c.left != NONE {
            by_region.entry(c.left).or_default().push((k, false));
        }
        if c.right != NONE {
            by_region.entry(c.right).or_default().push((k, true));
        }
    }
    let mut out = MergeShape::default();
    let mut ids: Vec<u32> = by_region.keys().copied().collect();
    ids.sort();
    for id in ids {
        let r = &regions[id as usize];
        if r.n == 0 {
            continue;
        }
        let contours = link_loops(&by_region[&id], &simplified, &chains);
        if contours.is_empty() {
            continue;
        }
        out.fills.push(FillRegion { fill: Some(r.color()), gradient: None, bitmap: None, contours });
    }
    out
}

#[derive(Default, Clone)]
struct Region {
    n: u64,
    sum: [u64; 4],
}

impl Region {
    fn add(&mut self, p: [u8; 4]) {
        self.n += 1;
        for (s, v) in self.sum.iter_mut().zip(p) {
            *s += v as u64;
        }
    }
    fn merge(&mut self, o: &Region) {
        self.n += o.n;
        for c in 0..4 {
            self.sum[c] += o.sum[c];
        }
    }
    fn color(&self) -> Color {
        let n = self.n.max(1);
        let c = |k: usize| ((self.sum[k] + n / 2) / n) as u8;
        // traced fills are opaque where the bitmap was mostly opaque
        Color { r: c(0), g: c(1), b: c(2), a: if c(3) > 240 { 255 } else { c(3) } }
    }
}

fn diff(a: [u8; 4], b: [u8; 4]) -> i32 {
    (a[0] as i32 - b[0] as i32).abs() + (a[1] as i32 - b[1] as i32).abs() + (a[2] as i32 - b[2] as i32).abs()
}

/// Merge regions under `min_area` pixels into the neighbour with the longest
/// shared border (repeatedly, smallest first).
fn absorb_small(label: &mut [u32], regions: &mut [Region], w: usize, h: usize, min_area: u32) {
    loop {
        let mut small: Vec<u32> = (0..regions.len() as u32).filter(|&r| regions[r as usize].n > 0 && regions[r as usize].n < min_area as u64).collect();
        if small.is_empty() {
            return;
        }
        small.sort_by_key(|&r| regions[r as usize].n);
        // shared border lengths between region pairs
        let mut border: HashMap<(u32, u32), u32> = HashMap::new();
        for y in 0..h {
            for x in 0..w {
                let a = label[y * w + x];
                for (nx, ny) in [(x + 1, y), (x, y + 1)] {
                    if nx < w && ny < h {
                        let b = label[ny * w + nx];
                        if a != b && a != NONE && b != NONE {
                            *border.entry((a.min(b), a.max(b))).or_default() += 1;
                        }
                    }
                }
            }
        }
        let mut target: HashMap<u32, u32> = HashMap::new();
        for &s in &small {
            if target.contains_key(&s) {
                continue;
            }
            let best = border.iter().filter(|((a, b), _)| *a == s || *b == s).map(|((a, b), n)| (if *a == s { *b } else { *a }, *n)).filter(|(o, _)| !target.contains_key(o)).max_by_key(|(o, n)| (*n, regions[*o as usize].n));
            if let Some((o, _)) = best {
                target.insert(s, o);
            }
        }
        if target.is_empty() {
            return; // isolated specks: keep them
        }
        for (s, o) in &target {
            let r = regions[*s as usize].clone();
            regions[*o as usize].merge(&r);
            regions[*s as usize] = Region::default();
        }
        for l in label.iter_mut() {
            if let Some(o) = target.get(l) {
                *l = *o;
            }
        }
    }
}

/// A run of pixel edges with the same region on each side.
struct Chain {
    pts: Vec<Pt>,
    closed: bool,
    /// Region on the left of the chain's direction (NONE = outside).
    left: u32,
    right: u32,
}

/// Lattice point id.
fn vid(x: usize, y: usize, w: usize) -> usize {
    y * (w + 1) + x
}

/// Every pixel edge between two different labels, as directed edges with the
/// lower-indexed… region on the left, chained between junctions.
fn border_chains(label: &[u32], w: usize, h: usize) -> Vec<Chain> {
    let at = |x: isize, y: isize| -> u32 {
        if x < 0 || y < 0 || x >= w as isize || y >= h as isize {
            NONE
        } else {
            label[y as usize * w + x as usize]
        }
    };
    // directed edges (from, to, left label, right label); y grows downwards,
    // "left" means left when walking from→to in screen space
    let mut edges: Vec<(usize, usize, u32, u32)> = vec![];
    for y in 0..=h {
        for x in 0..w {
            // horizontal edge between pixel (x, y-1) above and (x, y) below
            let (above, below) = (at(x as isize, y as isize - 1), at(x as isize, y as isize));
            if above != below {
                // walking right (+x): left side is above (−y)
                edges.push((vid(x, y, w), vid(x + 1, y, w), above, below));
            }
        }
    }
    for y in 0..h {
        for x in 0..=w {
            let (lft, rgt) = (at(x as isize - 1, y as isize), at(x as isize, y as isize));
            if lft != rgt {
                // walking down (+y): left side is +x
                edges.push((vid(x, y, w), vid(x, y + 1, w), rgt, lft));
            }
        }
    }
    let pos = |v: usize| pt((v % (w + 1)) as f32, (v / (w + 1)) as f32);
    // undirected adjacency to find junctions (degree != 2)
    let mut deg: HashMap<usize, u32> = HashMap::new();
    for e in &edges {
        *deg.entry(e.0).or_default() += 1;
        *deg.entry(e.1).or_default() += 1;
    }
    // adjacency: vertex -> edge indices
    let mut adj: HashMap<usize, Vec<usize>> = HashMap::new();
    for (k, e) in edges.iter().enumerate() {
        adj.entry(e.0).or_default().push(k);
        adj.entry(e.1).or_default().push(k);
    }
    let mut used = vec![false; edges.len()];
    let mut chains = vec![];
    // walk from an edge in its own orientation, continuing through degree-2
    // vertices along edges with the same label pair
    let walk = |k0: usize, used: &mut Vec<bool>| -> Chain {
        let (a, b, l, r) = edges[k0];
        used[k0] = true;
        let mut pts = vec![pos(a), pos(b)];
        let mut cur = b;
        let mut closed = false;
        loop {
            if cur == a {
                closed = true;
                break;
            }
            if deg[&cur] != 2 {
                break;
            }
            let next = adj[&cur].iter().copied().find(|&k| !used[k]);
            let Some(k) = next else { break };
            let (p, q, l2, r2) = edges[k];
            // orient the next edge to continue from `cur`
            let (to, nl, nr) = if p == cur { (q, l2, r2) } else { (p, r2, l2) };
            if (nl, nr) != (l, r) {
                break;
            }
            used[k] = true;
            pts.push(pos(to));
            cur = to;
        }
        Chain { pts, closed, left: l, right: r }
    };
    // first chains starting at junctions, then the remaining loops
    let mut order: Vec<usize> = (0..edges.len()).collect();
    order.sort_by_key(|&k| if deg[&edges[k].0] != 2 { 0 } else { 1 });
    for k in order {
        if used[k] {
            continue;
        }
        let mut c = walk(k, &mut used);
        // extend backwards from the start through degree-2 vertices
        if !c.closed && deg[&edges[k].0] == 2 {
            let start = edges[k].0;
            let mut back = vec![];
            let mut cur = start;
            loop {
                if deg[&cur] != 2 {
                    break;
                }
                let next = adj[&cur].iter().copied().find(|&j| !used[j]);
                let Some(j) = next else { break };
                let (p, q, l2, r2) = edges[j];
                // walking backwards: the edge must end at `cur` in chain orientation
                let (from, nl, nr) = if q == cur { (p, l2, r2) } else { (q, r2, l2) };
                if (nl, nr) != (c.left, c.right) {
                    break;
                }
                used[j] = true;
                back.push(pos(from));
                cur = from;
                if pos(cur) == *c.pts.last().unwrap() {
                    c.closed = true;
                    break;
                }
            }
            back.reverse();
            back.extend(c.pts);
            c.pts = back;
            if c.closed {
                c.pts.pop();
            }
        }
        if c.closed && c.pts.len() > 1 && c.pts.first() == c.pts.last() {
            c.pts.pop();
        }
        chains.push(c);
    }
    chains
}

/// Smooth and simplify a chain; open chains keep their end points (shared
/// junctions), and corners sharper than `corner_deg` stay sharp.
pub(crate) fn simplify_chain(pts: &[Pt], closed: bool, tol: f32, passes: usize, corner_deg: f32) -> Vec<Pt> {
    if tol <= 0.0 {
        return merge_collinear(pts, closed);
    }
    let n0 = pts.len();
    // corners, judged on the staircase over a short window
    let turn = |i: usize| -> f32 {
        let k = 2usize;
        let (a, b) = if closed { (pts[(i + n0 - k) % n0], pts[(i + k) % n0]) } else { (pts[i.saturating_sub(k)], pts[(i + k).min(n0 - 1)]) };
        let p = pts[i];
        let (u, v) = (pt(p.x - a.x, p.y - a.y), pt(b.x - p.x, b.y - p.y));
        let d = (u.x * v.x + u.y * v.y) / ((u.x.hypot(u.y) * v.x.hypot(v.y)).max(1e-6));
        d.clamp(-1.0, 1.0).acos().to_degrees()
    };
    // the true edge runs through the middle of each pixel step: keep corner
    // vertices (and an open chain's junction end points) exactly, and the
    // midpoints of the steps in between
    let mut seq: Vec<(Pt, bool)> = vec![];
    for i in 0..n0 {
        let end = !closed && (i == 0 || i == n0 - 1);
        if end || turn(i) > corner_deg {
            seq.push((pts[i], true));
        }
        if closed || i + 1 < n0 {
            seq.push((mid(pts[i], pts[(i + 1) % n0]), false));
        }
    }
    let n = seq.len();
    let fixed: Vec<bool> = seq.iter().map(|s| s.1).collect();
    // Taubin smoothing (a shrinking step, then an inflating one) so outlines
    // get smoother without getting smaller; fixed points stay
    let mut cur: Vec<Pt> = seq.iter().map(|s| s.0).collect();
    for k in 0..passes * 2 {
        let f = if k % 2 == 0 { 0.5 } else { -0.53 };
        let prev = cur.clone();
        for i in 0..n {
            if fixed[i] {
                continue;
            }
            let (a, b) = if closed { (prev[(i + n - 1) % n], prev[(i + 1) % n]) } else if i == 0 || i == n - 1 { continue } else { (prev[i - 1], prev[i + 1]) };
            let m = mid(a, b);
            cur[i] = pt(prev[i].x + f * (m.x - prev[i].x), prev[i].y + f * (m.y - prev[i].y));
        }
    }
    // Douglas–Peucker between fixed points picks the control points…
    let mut keep = fixed.clone();
    if !keep.iter().any(|k| *k) {
        keep[0] = true;
        keep[n / 2] = true;
    }
    let ks: Vec<usize> = (0..n).filter(|&i| keep[i]).collect();
    let segs: Vec<(usize, usize)> = if closed { ks.iter().enumerate().map(|(j, &a)| (a, ks[(j + 1) % ks.len()])).collect() } else { ks.windows(2).map(|w| (w[0], w[1])).collect() };
    for (a, b) in segs {
        dp(&cur, a, b, n, tol * 0.5, &mut keep);
    }
    let ctrl: Vec<(Pt, bool)> = (0..n).filter(|&i| keep[i]).map(|i| (cur[i], fixed[i])).collect();
    if ctrl.len() < if closed { 3 } else { 2 } {
        return merge_collinear(pts, closed);
    }
    // …and a Catmull-Rom curve through them draws the outline (corners stay sharp)
    curve_through(&ctrl, closed)
}

/// A smooth polyline through control points (uniform Catmull-Rom); at
/// corner points the curve has no tangent (sharp).
fn curve_through(c: &[(Pt, bool)], closed: bool) -> Vec<Pt> {
    let n = c.len();
    let get = |i: isize| -> (Pt, bool) {
        if closed {
            c[i.rem_euclid(n as isize) as usize]
        } else {
            c[i.clamp(0, n as isize - 1) as usize]
        }
    };
    let segs = if closed { n } else { n - 1 };
    let mut out = vec![];
    for i in 0..segs as isize {
        let (p1, k1) = get(i);
        let (p2, k2) = get(i + 1);
        // corner points (and chain ends) pin the tangent: use the point itself
        let p0 = if k1 { p1 } else { get(i - 1).0 };
        let p3 = if k2 { p2 } else { get(i + 2).0 };
        let steps = (((p2.x - p1.x).hypot(p2.y - p1.y)) / 2.0).ceil().clamp(1.0, 12.0) as usize;
        for s in 0..steps {
            let t = s as f32 / steps as f32;
            let (t2, t3) = (t * t, t * t * t);
            let f = |a: f32, b: f32, c2: f32, d: f32| 0.5 * ((2.0 * b) + (-a + c2) * t + (2.0 * a - 5.0 * b + 4.0 * c2 - d) * t2 + (-a + 3.0 * b - 3.0 * c2 + d) * t3);
            out.push(pt(f(p0.x, p1.x, p2.x, p3.x), f(p0.y, p1.y, p2.y, p3.y)));
        }
    }
    if !closed {
        out.push(c[n - 1].0);
    }
    out
}

fn mid(a: Pt, b: Pt) -> Pt {
    pt((a.x + b.x) / 2.0, (a.y + b.y) / 2.0)
}

/// Douglas–Peucker on the (cyclic) index range a..b.
fn dp(p: &[Pt], a: usize, b: usize, n: usize, tol: f32, keep: &mut [bool]) {
    let len = if b > a { b - a } else { b + n - a };
    if len < 2 {
        return;
    }
    let (pa, pb) = (p[a], p[b]);
    let (dx, dy) = (pb.x - pa.x, pb.y - pa.y);
    let l = dx.hypot(dy).max(1e-6);
    let mut best = (0.0f32, a);
    for s in 1..len {
        let i = (a + s) % n;
        let q = p[i];
        let d = if l < 1e-5 { (q.x - pa.x).hypot(q.y - pa.y) } else { ((q.x - pa.x) * dy - (q.y - pa.y) * dx).abs() / l };
        if d > best.0 {
            best = (d, i);
        }
    }
    if best.0 > tol {
        keep[best.1] = true;
        dp(p, a, best.1, n, tol, keep);
        dp(p, best.1, b, n, tol, keep);
    }
}

/// Drop points in the middle of straight runs (Curve fit: Pixels).
fn merge_collinear(pts: &[Pt], closed: bool) -> Vec<Pt> {
    let n = pts.len();
    if n < 3 {
        return pts.to_vec();
    }
    (0..n)
        .filter(|&i| {
            if !closed && (i == 0 || i == n - 1) {
                return true;
            }
            let (a, b, c) = (pts[(i + n - 1) % n], pts[i], pts[(i + 1) % n]);
            ((b.x - a.x) * (c.y - b.y) - (b.y - a.y) * (c.x - b.x)).abs() > 1e-6
        })
        .map(|i| pts[i])
        .collect()
}

/// Join a region's (possibly reversed) chains into closed contours.
fn link_loops(parts: &[(usize, bool)], simp: &[Vec<Pt>], chains: &[Chain]) -> Vec<Vec<Pt>> {
    let mut loops = vec![];
    let mut open: Vec<Vec<Pt>> = vec![];
    for &(k, rev) in parts {
        let mut p = simp[k].clone();
        if rev {
            p.reverse();
        }
        if chains[k].closed {
            if p.len() >= 3 {
                loops.push(p);
            }
        } else {
            open.push(p);
        }
    }
    let key = |p: Pt| ((p.x * 8.0).round() as i64, (p.y * 8.0).round() as i64);
    // chains are linked end-to-start (junction points are shared exactly)
    while let Some(mut cur) = open.pop() {
        let mut guard = 0;
        loop {
            let end = key(*cur.last().unwrap());
            if end == key(cur[0]) {
                cur.pop();
                if cur.len() >= 3 {
                    loops.push(cur);
                }
                break;
            }
            let Some(j) = open.iter().position(|q| key(q[0]) == end) else { break };
            let next = open.swap_remove(j);
            cur.extend(next.into_iter().skip(1));
            guard += 1;
            if guard > 100_000 {
                break;
            }
        }
    }
    loops
}

#[cfg(test)]
mod tests {
    use super::*;

    fn img(w: u32, h: u32, f: impl Fn(u32, u32) -> [u8; 4]) -> Vec<u8> {
        (0..h).flat_map(|y| (0..w).map(move |x| (x, y))).flat_map(|(x, y)| f(x, y)).collect()
    }

    fn area(c: &[Pt]) -> f32 {
        let n = c.len();
        (0..n).map(|i| c[i].x * c[(i + 1) % n].y - c[(i + 1) % n].x * c[i].y).sum::<f32>().abs() / 2.0
    }

    #[test]
    fn two_colour_halves_meet_exactly() {
        let rgba = img(20, 10, |x, _| if x < 10 { [255, 0, 0, 255] } else { [0, 0, 255, 255] });
        for fit in CurveFit::ALL {
            let m = trace(&rgba, 20, 10, &TraceOpts { curve_fit: fit, ..Default::default() });
            assert_eq!(m.fills.len(), 2, "{fit:?}");
            let total: f32 = m.fills.iter().map(|r| r.contours.iter().map(|c| area(c)).sum::<f32>()).sum();
            assert!((total - 200.0).abs() < 0.5, "{fit:?}: areas add up to the bitmap ({total})");
            assert!(m.fills.iter().any(|r| r.fill == Some(Color::rgb(255, 0, 0))));
        }
    }

    #[test]
    fn holes_threshold_and_min_area() {
        // white square with a black 6x6 hole, and one stray grey pixel
        let rgba = img(16, 16, |x, y| {
            if (5..11).contains(&x) && (5..11).contains(&y) {
                [0, 0, 0, 255]
            } else if (x, y) == (1, 1) {
                [235, 235, 235, 255]
            } else {
                [255, 255, 255, 255]
            }
        });
        let m = trace(&rgba, 16, 16, &TraceOpts { color_threshold: 10, min_area: 4, ..Default::default() });
        assert_eq!(m.fills.len(), 2, "the stray pixel joins the white region");
        let white = m.fills.iter().find(|r| r.fill.is_some_and(|c| c.r > 200)).unwrap();
        assert_eq!(white.contours.len(), 2, "outer contour + the hole");
        // a high threshold merges grey with white without min-area help
        let m = trace(&rgba, 16, 16, &TraceOpts { color_threshold: 100, min_area: 1, ..Default::default() });
        assert_eq!(m.fills.len(), 2);
        // a low one keeps the speck when min area allows it
        let m = trace(&rgba, 16, 16, &TraceOpts { color_threshold: 10, min_area: 1, ..Default::default() });
        assert_eq!(m.fills.len(), 3);
    }

    #[test]
    fn transparent_pixels_stay_empty_and_curves_get_smooth() {
        // an opaque disc on a transparent background
        let rgba = img(40, 40, |x, y| {
            let (dx, dy) = (x as f32 - 19.5, y as f32 - 19.5);
            if dx * dx + dy * dy < 15.0 * 15.0 { [20, 160, 60, 255] } else { [0, 0, 0, 0] }
        });
        let px = trace(&rgba, 40, 40, &TraceOpts { curve_fit: CurveFit::Pixels, ..Default::default() });
        let smooth = trace(&rgba, 40, 40, &TraceOpts { curve_fit: CurveFit::VerySmooth, corners: Corners::Few, ..Default::default() });
        assert_eq!(px.fills.len(), 1);
        assert_eq!(smooth.fills.len(), 1);
        // pixel fit keeps the 90° steps; a smooth fit has no sharp turns left
        let max_turn = |c: &[Pt]| {
            let n = c.len();
            (0..n)
                .map(|i| {
                    let (a, b, d) = (c[(i + n - 1) % n], c[i], c[(i + 1) % n]);
                    let (u, v) = (pt(b.x - a.x, b.y - a.y), pt(d.x - b.x, d.y - b.y));
                    ((u.x * v.x + u.y * v.y) / (u.x.hypot(u.y) * v.x.hypot(v.y)).max(1e-6)).clamp(-1.0, 1.0).acos().to_degrees()
                })
                .fold(0.0f32, f32::max)
        };
        assert!(max_turn(&px.fills[0].contours[0]) > 80.0);
        assert!(max_turn(&smooth.fills[0].contours[0]) < 35.0, "smooth: {}", max_turn(&smooth.fills[0].contours[0]));
        let a = area(&smooth.fills[0].contours[0]);
        assert!((a - std::f32::consts::PI * 225.0).abs() < 40.0, "still a disc of radius 15: {a}");
    }
}
