//! Bitmaps to shapes: the Magic Wand's region of similar colour, cut out of a
//! broken-apart bitmap, and Modify > Bitmap > Trace Bitmap. Both follow the
//! pixels' edges.

use std::collections::{BTreeMap, HashMap, HashSet};

use tiny_skia::Pixmap;

use crate::doc::*;
use crate::erase::{self, Area};
use crate::hit::distance;

/// The Magic Wand's Smoothing: how far its region's outline may straighten the pixels' steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Smoothing {
    Pixels,
    Rough,
    #[default]
    Normal,
    Smooth,
}

impl Smoothing {
    pub const ALL: [Smoothing; 4] = [Smoothing::Pixels, Smoothing::Rough, Smoothing::Normal, Smoothing::Smooth];

    pub fn name(self) -> &'static str {
        match self {
            Smoothing::Pixels => "Pixels",
            Smoothing::Rough => "Rough",
            Smoothing::Normal => "Normal",
            Smoothing::Smooth => "Smooth",
        }
    }

    /// How far, in the bitmap's pixels, the outline may stray from the pixels' edges.
    // ponytail: straightened, not fitted with curves as Animate's smoother settings may be.
    fn tolerance(self) -> f32 {
        match self {
            Smoothing::Pixels => 0.0,
            Smoothing::Rough => 0.5,
            Smoothing::Normal => 1.0,
            Smoothing::Smooth => 2.0,
        }
    }
}

/// Whether two pixels' colours differ by `threshold` at most, their RGBA channels' differences summed.
fn similar(a: &[u8], b: &[u8], threshold: u32) -> bool {
    a.iter().zip(b).map(|(&x, &y)| x.abs_diff(y) as u32).sum::<u32>() <= threshold
}

/// Magic Wand: where shape `s`'s bitmap fill shows colours `similar` to the one
/// at `at` (in the coordinates `s` is placed in), joined to it, cut out of `s`
/// into a shape of its own, which is returned. None where `s` shows no bitmap
/// fill or its bitmap can't be read.
// ponytail: the region stays within the one tile of a tiled fill that's under `at`; and colours are
// compared with the one clicked, their channels' differences summed, where how Animate's Threshold
// reads them is unconfirmed (a gradient comes in bands). Compare neighbours if Animate's spreads.
pub fn magic_wand(doc: &Document, s: &mut Shape, at: Point, threshold: u32, smoothing: Smoothing) -> Option<Shape> {
    let p = s.matrix.inverse()?.apply(at);
    let Paint::Bitmap(f) = s.fills.get(fill_at(s, p)?.checked_sub(1)?)? else { return None };
    let px = doc.bitmap(&f.name)?.data.pixels()?;
    let (w, h) = (px.width() as i32, px.height() as i32);
    let (bx, by) = f.matrix.inverse()?.apply(p);
    // The tile under `at`, and its pixel there.
    let (tx, ty) = if f.clipped { (0, 0) } else { (bx.div_euclid(w as f32) as i32, by.div_euclid(h as f32) as i32) };
    let seed = ((bx.floor() as i32 - tx * w).clamp(0, w - 1), (by.floor() as i32 - ty * h).clamp(0, h - 1));
    let region = flood(px, seed, threshold);
    let to_stage = s.matrix * f.matrix * Matrix::translate((tx * w) as f32, (ty * h) as f32);
    let outlines: Vec<Vec<Point>> = outlines(&region, w, h)
        .iter()
        .map(|c| straighten(c, smoothing.tolerance()).into_iter().map(|q| to_stage.apply(q)).collect())
        .collect();
    erase::split(s, &Area::new(&outlines), at)
}

/// The fill drawn at `p` in shape `s`'s own coordinates, counted from 1: the
/// last one there, even-odd, as render.rs fills them.
fn fill_at(s: &Shape, p: Point) -> Option<usize> {
    let mut inside = vec![false; s.fills.len() + 1];
    for e in &s.edges {
        for line in e.paths.iter().map(crate::hit::polyline) {
            for w in line.windows(2) {
                let (a, b) = (w[0], w[1]);
                if (a.1 > p.1) != (b.1 > p.1) && p.0 < a.0 + (p.1 - a.1) / (b.1 - a.1) * (b.0 - a.0) {
                    for f in [e.fill0, e.fill1] {
                        if let Some(v) = inside.get_mut(f as usize) {
                            *v = !*v;
                        }
                    }
                }
            }
        }
    }
    (1..inside.len()).rev().find(|&i| inside[i])
}

/// The pixels of `px` joined to `seed` through neighbours (up, down, left,
/// right) `similar` to it, as a mask.
fn flood(px: &Pixmap, seed: (i32, i32), threshold: u32) -> Vec<bool> {
    let (w, h) = (px.width() as i32, px.height() as i32);
    let at = |x: i32, y: i32| &px.data()[(y * w + x) as usize * 4..][..4];
    let color = at(seed.0, seed.1);
    let mut mask = vec![false; (w * h) as usize];
    mask[(seed.1 * w + seed.0) as usize] = true;
    let mut todo = vec![seed];
    while let Some((x, y)) = todo.pop() {
        for (nx, ny) in [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)] {
            if (0..w).contains(&nx) && (0..h).contains(&ny) && !mask[(ny * w + nx) as usize] && similar(at(nx, ny), color, threshold) {
                mask[(ny * w + nx) as usize] = true;
                todo.push((nx, ny));
            }
        }
    }
    mask
}

/// The outlines of the pixels set in `mask` (`w` × `h`, row by row), along
/// their edges: each a closed run of corners with the set pixels on its right,
/// clockwise on screen around an area, counterclockwise around a hole. Pixels
/// touching only at a corner get outlines of their own.
fn outlines(mask: &[bool], w: i32, h: i32) -> Vec<Vec<Point>> {
    let set = |x: i32, y: i32| x >= 0 && y >= 0 && x < w && y < h && mask[(y * w + x) as usize];
    // Each pixel side between a set pixel and an unset one: where it starts, and which way it goes.
    let mut edges: Vec<((i32, i32), (i32, i32))> = Vec::new();
    let mut from: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
    for y in 0..h {
        for x in 0..w {
            if !set(x, y) {
                continue;
            }
            let sides = [
                (!set(x, y - 1), (x, y), (1, 0)),
                (!set(x + 1, y), (x + 1, y), (0, 1)),
                (!set(x, y + 1), (x + 1, y + 1), (-1, 0)),
                (!set(x - 1, y), (x, y + 1), (0, -1)),
            ];
            for (open, start, dir) in sides {
                if open {
                    from.entry(start).or_default().push(edges.len());
                    edges.push((start, dir));
                }
            }
        }
    }
    let mut used = vec![false; edges.len()];
    let mut out = Vec::new();
    for first in 0..edges.len() {
        let mut e = first;
        let mut corners = Vec::new();
        while !used[e] {
            used[e] = true;
            let ((x, y), (dx, dy)) = edges[e];
            let end = (x + dx, y + dy);
            // Turning right first keeps pixels that touch at a corner apart; each side has one next.
            let next = [(-dy, dx), (dx, dy), (dy, -dx)].into_iter().find_map(|d| from.get(&end)?.iter().copied().find(|&i| edges[i].1 == d));
            let Some(n) = next else { break };
            if edges[n].1 != (dx, dy) {
                corners.push((end.0 as f32, end.1 as f32));
            }
            e = n;
        }
        if corners.len() > 2 {
            out.push(corners);
        }
    }
    out
}

/// `outline` (closed) with runs of corners straightened while none strays over
/// `tolerance` (Douglas–Peucker); as it is when that would leave no area.
fn straighten(outline: &[Point], tolerance: f32) -> Vec<Point> {
    fn run(points: &[Point], tolerance: f32, out: &mut Vec<Point>) {
        let (a, b) = (points[0], points[points.len() - 1]);
        let far = (1..points.len() - 1).map(|i| (i, distance(points[i], a, b))).max_by(|x, y| x.1.total_cmp(&y.1));
        match far {
            Some((i, d)) if d > tolerance => {
                run(&points[..=i], tolerance, out);
                run(&points[i..], tolerance, out);
            }
            _ => out.push(a),
        }
    }
    if tolerance <= 0.0 || outline.len() < 4 {
        return outline.to_vec();
    }
    // Split at the first corner and the one farthest from it.
    let d2 = |p: Point| (p.0 - outline[0].0).powi(2) + (p.1 - outline[0].1).powi(2);
    let far = (1..outline.len()).max_by(|&i, &j| d2(outline[i]).total_cmp(&d2(outline[j]))).unwrap_or(1);
    let mut out = Vec::new();
    run(&outline[..=far], tolerance, &mut out);
    run(&[&outline[far..], &outline[..1]].concat(), tolerance, &mut out);
    if out.len() > 2 { out } else { outline.to_vec() }
}

/// Modify > Bitmap > Trace Bitmap: bitmap `b` as a shape of flat colours over
/// its box ((0, 0) to its size). Pixels `similar` (within `threshold`) to the
/// first of an area join it through their neighbours, areas under `min_area`
/// pixels go to a neighbour, and each takes its average colour; see-through
/// areas stay empty. None if the bitmap can't be read.
// ponytail: outlines follow the pixels' edges, as Animate's "Pixels" curve fit; its smoother
// fits and Corner threshold aren't built. Share-and-straighten the runs between corners if wanted.
// A noisy photo traced fine (threshold 10, areas of 1 px: ~37k fills from 400 × 300) draws in
// 0.3 s but stalls the Paint Bucket, whose graph search in paint.rs is quadratic; Animate's
// defaults (100, 8) give a shape it fills in 0.2 s (debug builds).
pub fn trace(b: &BitmapItem, threshold: u32, min_area: u32) -> Option<Shape> {
    let px = b.data.pixels()?;
    let (w, h) = (px.width() as i32, px.height() as i32);
    let n = (w * h) as usize;
    let at = |i: usize| &px.data()[i * 4..][..4];
    let neighbours = |i: usize| {
        let (x, y) = (i as i32 % w, i as i32 / w);
        [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)].into_iter().filter(move |&(x, y)| x >= 0 && y >= 0 && x < w && y < h).map(move |(x, y)| (y * w + x) as usize)
    };
    // Areas, each grown from the first pixel not yet in one.
    let mut area = vec![usize::MAX; n];
    let mut count = 0;
    for start in 0..n {
        if area[start] != usize::MAX {
            continue;
        }
        area[start] = count;
        let mut todo = vec![start];
        while let Some(i) = todo.pop() {
            for j in neighbours(i) {
                if area[j] == usize::MAX && similar(at(j), at(start), threshold) {
                    area[j] = count;
                    todo.push(j);
                }
            }
        }
        count += 1;
    }
    // Small areas join the area next to them.
    fn root(parent: &mut [usize], mut a: usize) -> usize {
        while parent[a] != a {
            parent[a] = parent[parent[a]];
            a = parent[a];
        }
        a
    }
    let mut parent: Vec<usize> = (0..count).collect();
    let mut size = vec![0u32; count];
    area.iter().for_each(|&a| size[a] += 1);
    for _ in 0..16 {
        let mut joined = false;
        for i in 0..n {
            let a = root(&mut parent, area[i]);
            if size[a] >= min_area {
                continue;
            }
            if let Some(b) = neighbours(i).map(|j| root(&mut parent, area[j])).find(|&b| b != a) {
                parent[a] = b;
                size[b] += size[a];
                joined = true;
            }
        }
        if !joined {
            break;
        }
    }
    // Each area's fill, counted from 1 by its first pixel, and its average colour (premultiplied sums).
    let mut id = vec![0u32; count];
    let mut sums: Vec<[u64; 5]> = Vec::new();
    let mut labels = vec![0u32; n];
    for i in 0..n {
        let a = root(&mut parent, area[i]);
        if id[a] == 0 {
            sums.push([0; 5]);
            id[a] = sums.len() as u32;
        }
        labels[i] = id[a];
        let s = &mut sums[id[a] as usize - 1];
        at(i).iter().enumerate().for_each(|(c, &v)| s[c] += v as u64);
        s[4] += 1;
    }
    let fills: Vec<Paint> = sums
        .iter()
        .map(|s| {
            let alpha = s[3] as f32 / s[4] as f32 / 255.0;
            let c = |v: u64| if alpha > 0.0 { (v as f32 / s[4] as f32 / 255.0 / alpha).min(1.0) } else { 0.0 };
            Paint::Solid([c(s[0]), c(s[1]), c(s[2]), alpha])
        })
        .collect();
    let clear: Vec<bool> = sums.iter().map(|s| s[3] * 255 < s[4]).collect();
    let lab = |x: i32, y: i32| {
        if x < 0 || y < 0 || x >= w || y >= h {
            return 0;
        }
        let l = labels[(y * w + x) as usize];
        if clear[l as usize - 1] { 0 } else { l }
    };
    // Runs of pixel sides between two areas, by (fill on the left, fill on the right).
    let mut runs: BTreeMap<(u32, u32), Vec<((i32, i32), (i32, i32))>> = BTreeMap::new();
    for y in 0..=h {
        let mut x = 0;
        while x < w {
            // Going right along the row: above is on the left.
            let pair = (lab(x, y - 1), lab(x, y));
            let start = x;
            while x < w && (lab(x, y - 1), lab(x, y)) == pair {
                x += 1;
            }
            if pair.0 != pair.1 {
                runs.entry(pair).or_default().push(((start, y), (x, y)));
            }
        }
    }
    for x in 0..=w {
        let mut y = 0;
        while y < h {
            // Going down the column: the left-hand pixel is on the right.
            let pair = (lab(x, y), lab(x - 1, y));
            let start = y;
            while y < h && (lab(x, y), lab(x - 1, y)) == pair {
                y += 1;
            }
            if pair.0 != pair.1 {
                runs.entry(pair).or_default().push(((x, start), (x, y)));
            }
        }
    }
    let pt = |(x, y): (i32, i32)| (x as f32, y as f32);
    let mut edges = Vec::new();
    for ((fill0, fill1), runs) in runs {
        // Chained end to start into paths, from the runs no other leads into.
        let mut by_start: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
        runs.iter().enumerate().for_each(|(i, r)| by_start.entry(r.0).or_default().push(i));
        let ends: HashSet<(i32, i32)> = runs.iter().map(|r| r.1).collect();
        let heads = (0..runs.len()).filter(|&i| !ends.contains(&runs[i].0));
        let mut used = vec![false; runs.len()];
        let mut paths = Vec::new();
        for i in heads.chain(0..runs.len()) {
            if used[i] {
                continue;
            }
            used[i] = true;
            let mut path = Path { start: pt(runs[i].0), segs: vec![Seg::Line(pt(runs[i].1))] };
            let mut end = runs[i].1;
            while let Some(j) = by_start.get(&end).and_then(|v| v.iter().copied().find(|&j| !used[j])) {
                used[j] = true;
                path.segs.push(Seg::Line(pt(runs[j].1)));
                end = runs[j].1;
            }
            paths.push(path);
        }
        edges.push(Edge { fill0, fill1, stroke: 0, paths });
    }
    Some(Shape { matrix: Matrix::IDENTITY, fills, strokes: Vec::new(), edges, extra: Extra::default() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bitmap::tests::png;
    use crate::render::render;

    /// Twice the area inside `c`: positive clockwise on screen.
    fn area(c: &[Point]) -> f32 {
        (0..c.len()).map(|i| (c[i], c[(i + 1) % c.len()])).map(|(a, b)| a.0 * b.1 - b.0 * a.1).sum()
    }

    #[test]
    fn outlines_go_round_areas_and_holes() {
        // A ring with a hole in its middle, and a pixel touching it only at a corner.
        let rows = ["###.", "#.#.", "###.", "...#"];
        let mask: Vec<bool> = rows.iter().flat_map(|r| r.chars().map(|c| c == '#')).collect();
        let mut found: Vec<(f32, usize)> = outlines(&mask, 4, 4).iter().map(|c| (area(c) / 2.0, c.len())).collect();
        found.sort_by(|a, b| a.0.total_cmp(&b.0));
        assert_eq!(found, [(-1.0, 4), (1.0, 4), (9.0, 4)], "the hole runs the other way; the corner pixel stands alone");
        // Steps straighten into a slope once they may stray half a pixel.
        let stairs = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (2.0, 1.0), (2.0, 2.0), (3.0, 2.0), (3.0, 3.0), (0.0, 3.0)];
        assert_eq!(straighten(&stairs, 0.0).len(), 8);
        assert_eq!(straighten(&stairs, 0.75), [(0.0, 0.0), (3.0, 3.0), (0.0, 3.0)]);
    }

    /// The 2 × 2 checker (red, green / blue, white) at 10× from (10, 10), broken apart.
    fn broken_checker() -> (Document, Shape) {
        let colors = [[255, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255], [255, 255, 255, 255]];
        let mut doc = Document::new(60.0, 60.0, 24.0);
        let mut item = BitmapItem::new("checker.png", png(2, 2, |x, y| colors[(y * 2 + x) as usize]));
        item.smoothing = false;
        doc.media.push(Media::Bitmap(item));
        let inst = BitmapInstance { name: "checker.png".into(), matrix: Matrix { a: 10.0, d: 10.0, tx: 10.0, ty: 10.0, ..Matrix::IDENTITY }, extra: Extra::default() };
        let shape = crate::bitmap::broken_apart(&doc, &inst).unwrap();
        (doc, shape)
    }

    #[test]
    fn the_magic_wand_cuts_out_similar_colours() {
        let (mut doc, mut shape) = broken_checker();
        let part = magic_wand(&doc, &mut shape, (14.0, 16.0), 20, Smoothing::Normal).unwrap();
        let frame = &mut doc.scenes[0].layers[0].frames[0];
        frame.elements = vec![Element::Shape(shape.clone()), Element::Shape(part.clone())];
        let tl = &doc.scenes[0];
        let r = |element| crate::hit::ElementRef { layer: 0, keyframe: 0, element };
        let b = crate::hit::bounds(&doc, tl, 0, &[r(1)]).unwrap();
        assert_eq!((b.min, b.max), ((10.0, 10.0), (20.0, 20.0)), "the red block alone");
        assert_eq!(crate::hit::hit_test(&doc, tl, 0, (15.0, 15.0), 0.0), Some(r(1)));
        assert_eq!(crate::hit::hit_test(&doc, tl, 0, (25.0, 15.0), 0.0), Some(r(0)), "the rest stays");
        // Drawn together, they're the bitmap as it was; without the part, a hole.
        let pixel = |doc: &Document, x, y| {
            let p = render(doc, 0, 1.0).unwrap().0.pixel(x, y).unwrap();
            [p.red(), p.green(), p.blue()]
        };
        assert_eq!((pixel(&doc, 15, 15), pixel(&doc, 25, 25)), ([255, 0, 0], [255, 255, 255]));
        doc.scenes[0].layers[0].frames[0].elements.pop();
        assert_eq!((pixel(&doc, 15, 15), pixel(&doc, 25, 15)), ([255, 255, 255], [0, 255, 0]));
        // A high threshold takes in the neighbours too; a solid fill gives nothing.
        let (_, mut whole) = broken_checker();
        let all = magic_wand(&doc, &mut whole, (14.0, 16.0), 1020, Smoothing::Pixels).unwrap();
        assert!(whole.edges.is_empty() && !all.edges.is_empty(), "{whole:?}");
        let mut solid = crate::draw::rectangle([(0.0, 0.0), (10.0, 10.0)], 0.0, None, Some(Paint::Solid([1.0; 4])));
        assert!(magic_wand(&doc, &mut solid, (5.0, 5.0), 20, Smoothing::Normal).is_none());
    }

    #[test]
    fn trace_bitmap_makes_flat_areas() {
        let (doc, _) = broken_checker();
        let s = trace(doc.bitmap("checker.png").unwrap(), 100, 1).unwrap();
        assert_eq!(s.fills, [[1.0, 0.0, 0.0, 1.0], [0.0, 1.0, 0.0, 1.0], [0.0, 0.0, 1.0, 1.0], [1.0; 4]].map(Paint::Solid));
        // Each colour where its pixel was; the edges between two areas carry both fills.
        let mut doc = Document::new(2.0, 2.0, 24.0);
        doc.background = [0.0, 0.0, 0.0, 1.0];
        doc.scenes[0].layers[0].frames[0].elements.push(Element::Shape(s.clone()));
        let (pm, _) = render(&doc, 0, 10.0).unwrap();
        let at = |x, y| pm.pixel(x, y).map(|p| [p.red(), p.green(), p.blue()]).unwrap();
        assert_eq!([at(5, 5), at(15, 5), at(5, 15), at(15, 15)], [[255, 0, 0], [0, 255, 0], [0, 0, 255], [255, 255, 255]]);
        assert!(s.edges.iter().any(|e| (e.fill0, e.fill1) == (1, 2) || (e.fill0, e.fill1) == (2, 1)));
        // Areas under the minimum go to a neighbour.
        let speck = BitmapItem::new("speck.png", png(3, 3, |x, y| if (x, y) == (1, 1) { [0, 0, 0, 255] } else { [255, 255, 255, 255] }));
        assert_eq!(trace(&speck, 10, 1).unwrap().fills.len(), 2);
        let one = trace(&speck, 10, 2).unwrap();
        assert_eq!(one.fills.len(), 1);
        assert!(one.edges.iter().all(|e| [(0, 1), (1, 0)].contains(&(e.fill0, e.fill1))), "only round the outside");
    }
}
