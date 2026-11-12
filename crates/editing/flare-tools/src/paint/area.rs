//! The area the Paint Bucket fills between lines, found in a picture of where there is ink: the
//! pixels a flood from the click reaches without crossing ink, when it cannot get out. Lines
//! drawn with any tool close an area, as they do in Animate, however many separate shapes they
//! are; a gap narrower than the one asked for counts as closed.

use std::collections::{BTreeMap, VecDeque};

/// The most pixels an area is looked for in.
pub const MAX_AREA_PIXELS: usize = 1 << 21;

/// The most pixel edges the outline of an area may have; past it the area is too ragged to
/// trace and is refused.
const MAX_OUTLINE_EDGES: usize = 1 << 20;

/// The most a gap is closed by on each side, in pixels.
const MAX_REACH: usize = 256;

/// How far, in pixels, the traced outline may stray from the pixels' own edge. Under a pixel, so
/// the staircase of the pixels becomes straight lines without moving the edge visibly.
const OUTLINE_TOLERANCE: f64 = 0.7;

/// A picture of where there is ink, row by row.
#[derive(Debug, Clone, Copy)]
pub struct InkGrid<'a> {
    width: usize,
    height: usize,
    ink: &'a [bool],
}

impl<'a> InkGrid<'a> {
    /// `None` unless `ink` has `width * height` pixels, at least one and at most
    /// [`MAX_AREA_PIXELS`].
    pub fn new(width: usize, height: usize, ink: &'a [bool]) -> Option<Self> {
        let fits = width.checked_mul(height) == Some(ink.len())
            && !ink.is_empty()
            && ink.len() <= MAX_AREA_PIXELS;
        fits.then_some(Self { width, height, ink })
    }
}

/// One closed loop of an area's outline, in pixel coordinates (corners of pixels).
pub type Outline = Vec<(f64, f64)>;

/// The outline of the area around the pixel `seed` that ink closes off, letting through no gap
/// narrower than `gap` pixels: loops that together wind once around every pixel of the area and
/// not at all around any other, so they fill it by the non-zero rule, holes included. The area
/// reaches a pixel under the ink around it, so no seam shows between it and the lines.
///
/// `None` when the seed is on ink, or the area is not closed (the flood reaches the edge of the
/// picture), or its outline is too ragged to trace.
pub fn enclosed_area(grid: &InkGrid<'_>, seed: (usize, usize), gap: f64) -> Option<Vec<Outline>> {
    let (w, h) = (grid.width, grid.height);
    if seed.0 >= w || seed.1 >= h {
        return None;
    }
    // Distances are measured square-wise, a pixel diagonally counting as one: ink grown by half
    // the gap closes every gap narrower than it, and the area grown back by as much and a pixel
    // more reaches a pixel under the lines around it and never past them.
    let reach = if gap.is_finite() {
        (gap / 2.0).round().clamp(0.0, MAX_REACH as f64) as usize
    } else {
        0
    };
    let blocked = grow(grid.ink, w, h, reach);
    let start = free_near(grid, &blocked, seed.1 * w + seed.0, reach)?;
    let area = flood(&blocked, w, h, start)?;
    let grown = grow(&area, w, h, reach + 1);
    let outlines = trace(&grown, w, h)?;
    Some(outlines.iter().filter_map(simplify).collect())
}

/// `set` grown by `by` pixels every way, to a square around each of its pixels: a count of the
/// set pixels in a sliding window along each row, then down each column.
fn grow(set: &[bool], w: usize, h: usize, by: usize) -> Vec<bool> {
    if by == 0 {
        return set.to_vec();
    }
    let mut across = vec![false; set.len()];
    for (row, out) in set.chunks_exact(w).zip(across.chunks_exact_mut(w)) {
        let mut count = row[..by.min(w)].iter().filter(|&&on| on).count();
        for x in 0..w {
            if x + by < w {
                count += usize::from(row[x + by]);
            }
            if x > by {
                count -= usize::from(row[x - by - 1]);
            }
            out[x] = count > 0;
        }
    }
    let mut out = vec![false; set.len()];
    let mut count = vec![0u32; w];
    let add = |count: &mut [u32], row: &[bool], sign: bool| {
        for (c, &on) in count.iter_mut().zip(row) {
            if on {
                *c = if sign { *c + 1 } else { *c - 1 };
            }
        }
    };
    for y in 0..by.min(h) {
        add(&mut count, &across[y * w..(y + 1) * w], true);
    }
    for y in 0..h {
        if y + by < h {
            add(&mut count, &across[(y + by) * w..(y + by + 1) * w], true);
        }
        if y > by {
            add(&mut count, &across[(y - by - 1) * w..(y - by) * w], false);
        }
        for (o, &c) in out[y * w..(y + 1) * w].iter_mut().zip(&count) {
            *o = c > 0;
        }
    }
    out
}

/// The pixel to flood from: the seed, or when the grown ink covers it (a click close to a line),
/// the nearest pixel it does not, reached without crossing the ink itself.
fn free_near(grid: &InkGrid<'_>, blocked: &[bool], seed: usize, reach: usize) -> Option<usize> {
    if grid.ink[seed] {
        return None;
    }
    if !blocked[seed] {
        return Some(seed);
    }
    let w = grid.width;
    let steps = 2 * reach + 2;
    let mut seen = std::collections::HashSet::from([seed]);
    let mut queue = VecDeque::from([(seed, 0usize)]);
    while let Some((at, depth)) = queue.pop_front() {
        if !blocked[at] {
            return Some(at);
        }
        if depth == steps {
            continue;
        }
        for next in neighbours(at, w, grid.height) {
            if !grid.ink[next] && seen.insert(next) {
                queue.push_back((next, depth + 1));
            }
        }
    }
    None
}

fn neighbours(at: usize, w: usize, h: usize) -> impl Iterator<Item = usize> {
    let (x, y) = (at % w, at / w);
    [
        (x > 0).then(|| at - 1),
        (x + 1 < w).then(|| at + 1),
        (y > 0).then(|| at - w),
        (y + 1 < h).then(|| at + w),
    ]
    .into_iter()
    .flatten()
}

/// The pixels reached from `start` without crossing `blocked`, or `None` when they reach the
/// edge of the picture: the area is open. Filled a run of a row at a time.
fn flood(blocked: &[bool], w: usize, h: usize, start: usize) -> Option<Vec<bool>> {
    let mut area = vec![false; blocked.len()];
    let free = |i: usize, area: &[bool]| !blocked[i] && !area[i];
    let mut stack = vec![start];
    while let Some(seed) = stack.pop() {
        if !free(seed, &area) {
            continue;
        }
        let (y, row) = (seed / w, seed - seed % w);
        let (mut left, mut right) = (seed, seed);
        while left > row && free(left - 1, &area) {
            left -= 1;
        }
        while right + 1 < row + w && free(right + 1, &area) {
            right += 1;
        }
        if y == 0 || y + 1 == h || left == row || right + 1 == row + w {
            return None;
        }
        area[left..=right].fill(true);
        for next_row in [row - w, row + w] {
            // One seed for each run of free pixels next to this one.
            let mut in_run = false;
            for x in left - row..=right - row {
                let i = next_row + x;
                let open = free(i, &area);
                if open && !in_run {
                    stack.push(i);
                }
                in_run = open;
            }
        }
    }
    Some(area)
}

/// The edges between the pixels of `set` and the rest, joined into closed loops. Each edge runs
/// with the set on the same side, so the loops' windings add up to one inside it and none
/// outside, however the loops are joined where two corners touch.
fn trace(set: &[bool], w: usize, h: usize) -> Option<Vec<Outline>> {
    let corner = |x: usize, y: usize| y * (w + 1) + x;
    let inside = |x: usize, y: usize| set[y * w + x];
    let mut edges: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    let mut count = 0usize;
    for y in 0..h {
        for x in 0..w {
            if !inside(x, y) {
                continue;
            }
            let interior = y > 0
                && y + 1 < h
                && x > 0
                && x + 1 < w
                && inside(x, y - 1)
                && inside(x, y + 1)
                && inside(x - 1, y)
                && inside(x + 1, y);
            if interior {
                continue;
            }
            let sides = [
                (y == 0 || !inside(x, y - 1), corner(x, y), corner(x + 1, y)),
                (
                    x + 1 == w || !inside(x + 1, y),
                    corner(x + 1, y),
                    corner(x + 1, y + 1),
                ),
                (
                    y + 1 == h || !inside(x, y + 1),
                    corner(x + 1, y + 1),
                    corner(x, y + 1),
                ),
                (x == 0 || !inside(x - 1, y), corner(x, y + 1), corner(x, y)),
            ];
            for (open, from, to) in sides {
                if open {
                    edges.entry(from).or_default().push(to);
                    count += 1;
                    if count > MAX_OUTLINE_EDGES {
                        return None;
                    }
                }
            }
        }
    }
    let point = |c: usize| ((c % (w + 1)) as f64, (c / (w + 1)) as f64);
    let mut loops = Vec::new();
    while let Some((&start, _)) = edges.iter().next() {
        let mut corners = vec![point(start)];
        let mut at = start;
        while let Some(next) = take_edge(&mut edges, at)
            && next != start
        {
            corners.push(point(next));
            at = next;
        }
        loops.push(corners);
    }
    Some(loops)
}

fn take_edge(edges: &mut BTreeMap<usize, Vec<usize>>, from: usize) -> Option<usize> {
    let outgoing = edges.get_mut(&from)?;
    let next = outgoing.pop();
    if outgoing.is_empty() {
        edges.remove(&from);
    }
    next
}

/// `loop_` with the points that stray less than [`OUTLINE_TOLERANCE`] from a straight line
/// between their neighbours left out; `None` when too little is left to enclose anything.
fn simplify(loop_: &Outline) -> Option<Outline> {
    if loop_.len() < 3 {
        return None;
    }
    let first = loop_[0];
    let far = (1..loop_.len()).max_by(|&a, &b| {
        let da = distance_sq(loop_[a], first);
        let db = distance_sq(loop_[b], first);
        da.total_cmp(&db)
    })?;
    let mut closed = loop_.clone();
    closed.push(first);
    let mut keep = vec![false; closed.len()];
    keep[0] = true;
    keep[far] = true;
    keep[closed.len() - 1] = true;
    let mut stack = vec![(0, far), (far, closed.len() - 1)];
    while let Some((a, b)) = stack.pop() {
        let mut worst = (0.0, 0);
        for i in a + 1..b {
            let d = off_line(closed[i], closed[a], closed[b]);
            if d > worst.0 {
                worst = (d, i);
            }
        }
        if worst.0 > OUTLINE_TOLERANCE {
            keep[worst.1] = true;
            stack.push((a, worst.1));
            stack.push((worst.1, b));
        }
    }
    closed.pop();
    let kept: Outline = closed
        .into_iter()
        .zip(keep)
        .filter_map(|(p, k)| k.then_some(p))
        .collect();
    (kept.len() >= 3).then_some(kept)
}

fn distance_sq(a: (f64, f64), b: (f64, f64)) -> f64 {
    (a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)
}

/// How far `p` is from the segment `a`-`b`.
fn off_line(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let length_sq = dx * dx + dy * dy;
    if length_sq == 0.0 {
        return distance_sq(p, a).sqrt();
    }
    let t = (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / length_sq).clamp(0.0, 1.0);
    distance_sq(p, (a.0 + t * dx, a.1 + t * dy)).sqrt()
}
