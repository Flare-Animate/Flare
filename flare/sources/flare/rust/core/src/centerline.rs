//! Trace Bitmap ▸ Centerline: line art becomes lines along the middle of the
//! ink, with the ink's thickness as a width profile. The approach follows
//! OpenToonz's centerline vectorizer (`toonzlib/tcenterline*.cpp`;
//! BSD-3-Clause, © DWANGO and OpenToonz contributors; see
//! THIRD-PARTY-NOTICES.md): separate ink from paper by a threshold, drop
//! specks, find the skeleton, follow it between junctions, measure thickness
//! by distance to the paper, and turn each run into a stroke. OpenToonz
//! skeletonizes polygonal outlines; this works on pixels (thinning and a
//! chamfer distance map), which gives the same kind of result for line art.

use crate::geom::{pt, Pt};
use crate::merge::MergeShape;
use crate::model::{Color, Stroke};
use crate::trace::{simplify_chain, TraceOpts};

/// Ink mask: straight-colour luminance below the threshold and mostly opaque.
fn ink_mask(rgba: &[u8], w: usize, h: usize, threshold: u8) -> Vec<bool> {
    (0..w * h)
        .map(|i| {
            let p = &rgba[i * 4..i * 4 + 4];
            let a = p[3] as f32;
            if a < 128.0 {
                return false;
            }
            // premultiplied over white
            let k = 255.0 - a;
            let (r, g, b) = (p[0] as f32 + k, p[1] as f32 + k, p[2] as f32 + k);
            0.299 * r + 0.587 * g + 0.114 * b < threshold as f32
        })
        .collect()
}

/// Remove 8-connected ink specks smaller than `min` pixels.
fn despeckle(m: &mut [bool], w: usize, h: usize, min: usize) {
    let mut seen = vec![false; m.len()];
    for s in 0..m.len() {
        if !m[s] || seen[s] {
            continue;
        }
        let mut comp = vec![s];
        seen[s] = true;
        let mut i = 0;
        while i < comp.len() {
            let c = comp[i];
            let (x, y) = ((c % w) as i32, (c / w) as i32);
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let (nx, ny) = (x + dx, y + dy);
                    if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                        continue;
                    }
                    let n = ny as usize * w + nx as usize;
                    if m[n] && !seen[n] {
                        seen[n] = true;
                        comp.push(n);
                    }
                }
            }
            i += 1;
        }
        if comp.len() < min {
            for c in comp {
                m[c] = false;
            }
        }
    }
}

/// Chamfer (3-4) distance to the nearest paper pixel, in pixels.
fn distance(m: &[bool], w: usize, h: usize) -> Vec<f32> {
    const BIG: u32 = 1 << 30;
    let mut d: Vec<u32> = m.iter().map(|&v| if v { BIG } else { 0 }).collect();
    let at = |d: &Vec<u32>, x: i32, y: i32| if x < 0 || y < 0 || x >= w as i32 || y >= h as i32 { 0 } else { d[y as usize * w + x as usize] };
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let i = y as usize * w + x as usize;
            if d[i] == 0 {
                continue;
            }
            let v = [at(&d, x - 1, y) + 3, at(&d, x, y - 1) + 3, at(&d, x - 1, y - 1) + 4, at(&d, x + 1, y - 1) + 4];
            d[i] = d[i].min(*v.iter().min().unwrap());
        }
    }
    for y in (0..h as i32).rev() {
        for x in (0..w as i32).rev() {
            let i = y as usize * w + x as usize;
            if d[i] == 0 {
                continue;
            }
            let v = [at(&d, x + 1, y) + 3, at(&d, x, y + 1) + 3, at(&d, x + 1, y + 1) + 4, at(&d, x - 1, y + 1) + 4];
            d[i] = d[i].min(*v.iter().min().unwrap());
        }
    }
    d.into_iter().map(|v| v as f32 / 3.0).collect()
}

/// Zhang–Suen thinning to a one-pixel skeleton.
fn thin(m: &[bool], w: usize, h: usize) -> Vec<bool> {
    let mut s = m.to_vec();
    let get = |s: &Vec<bool>, x: i32, y: i32| x >= 0 && y >= 0 && x < w as i32 && y < h as i32 && s[y as usize * w + x as usize];
    loop {
        let mut changed = false;
        for step in 0..2 {
            let mut kill = vec![];
            for y in 0..h as i32 {
                for x in 0..w as i32 {
                    if !s[y as usize * w + x as usize] {
                        continue;
                    }
                    // P2..P9 clockwise from north
                    let n = [get(&s, x, y - 1), get(&s, x + 1, y - 1), get(&s, x + 1, y), get(&s, x + 1, y + 1), get(&s, x, y + 1), get(&s, x - 1, y + 1), get(&s, x - 1, y), get(&s, x - 1, y - 1)];
                    let b = n.iter().filter(|v| **v).count();
                    if !(2..=6).contains(&b) {
                        continue;
                    }
                    let a = (0..8).filter(|&i| !n[i] && n[(i + 1) % 8]).count();
                    if a != 1 {
                        continue;
                    }
                    let (p2, p4, p6, p8) = (n[0], n[2], n[4], n[6]);
                    let ok = if step == 0 { !(p2 && p4 && p6) && !(p4 && p6 && p8) } else { !(p2 && p4 && p8) && !(p2 && p6 && p8) };
                    if ok {
                        kill.push(y as usize * w + x as usize);
                    }
                }
            }
            changed |= !kill.is_empty();
            for k in kill {
                s[k] = false;
            }
        }
        if !changed {
            return s;
        }
    }
}

fn nbrs(s: &[bool], w: usize, h: usize, i: usize) -> Vec<usize> {
    let (x, y) = ((i % w) as i32, (i / w) as i32);
    let mut v = vec![];
    for dy in -1..=1 {
        for dx in -1..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let (nx, ny) = (x + dx, y + dy);
            if nx >= 0 && ny >= 0 && nx < w as i32 && ny < h as i32 && s[ny as usize * w + nx as usize] {
                v.push(ny as usize * w + nx as usize);
            }
        }
    }
    v
}

/// Crossing number: 0→1 transitions around the 8-neighbourhood (clockwise from north).
fn crossings(s: &[bool], w: usize, h: usize, i: usize) -> usize {
    let (x, y) = ((i % w) as i32, (i / w) as i32);
    let g = |dx: i32, dy: i32| {
        let (nx, ny) = (x + dx, y + dy);
        nx >= 0 && ny >= 0 && nx < w as i32 && ny < h as i32 && s[ny as usize * w + nx as usize]
    };
    let n = [g(0, -1), g(1, -1), g(1, 0), g(1, 1), g(0, 1), g(-1, 1), g(-1, 0), g(-1, -1)];
    (0..8).filter(|&k| !n[k] && n[(k + 1) % 8]).count()
}

/// Skeleton runs: pixel paths between junctions / ends, and closed loops.
/// Junctions are judged by the crossing number, so the staircases thinning
/// leaves on slanted lines don't count as branches.
fn runs(s: &[bool], w: usize, h: usize) -> Vec<(Vec<usize>, bool)> {
    use std::collections::HashSet;
    let is_node = |i: usize| {
        let b = nbrs(s, w, h, i).len();
        b <= 1 || crossings(s, w, h, i) >= 3
    };
    let four = |a: usize, b: usize| (a % w == b % w) || (a / w == b / w);
    let mut visited: HashSet<usize> = HashSet::new();
    let mut links: HashSet<(usize, usize)> = HashSet::new();
    let mut out = vec![];
    let nodes: Vec<usize> = (0..s.len()).filter(|&i| s[i] && is_node(i)).collect();
    let node_set: HashSet<usize> = nodes.iter().copied().collect();
    for &n in &nodes {
        if nbrs(s, w, h, n).is_empty() {
            out.push((vec![n], false));
            continue;
        }
        let mut firsts = nbrs(s, w, h, n);
        firsts.sort_by_key(|&q| !four(n, q));
        for first in firsts {
            if node_set.contains(&first) {
                let k = (n.min(first), n.max(first));
                if links.insert(k) {
                    out.push((vec![n, first], false));
                }
                continue;
            }
            if visited.contains(&first) {
                continue;
            }
            visited.insert(first);
            let mut path = vec![n, first];
            let mut cur = first;
            loop {
                let mut next: Vec<usize> = nbrs(s, w, h, cur).into_iter().filter(|q| *q != path[path.len() - 2] && (node_set.contains(q) || !visited.contains(q))).collect();
                // a node next door ends the run; otherwise prefer straight (4-) steps
                if let Some(&nd) = next.iter().find(|q| node_set.contains(q) && **q != n || (node_set.contains(q) && path.len() > 3)) {
                    path.push(nd);
                    break;
                }
                next.retain(|q| !node_set.contains(q));
                next.sort_by_key(|&q| !four(cur, q));
                let Some(&nx) = next.first() else { break };
                visited.insert(nx);
                path.push(nx);
                cur = nx;
            }
            out.push((path, false));
        }
    }
    // loops with no node on them
    for i in 0..s.len() {
        if !s[i] || visited.contains(&i) || node_set.contains(&i) {
            continue;
        }
        let mut path = vec![i];
        visited.insert(i);
        let mut cur = i;
        loop {
            let mut next: Vec<usize> = nbrs(s, w, h, cur).into_iter().filter(|q| !visited.contains(q)).collect();
            next.sort_by_key(|&q| !four(cur, q));
            let Some(&nx) = next.first() else { break };
            visited.insert(nx);
            path.push(nx);
            cur = nx;
        }
        if path.len() > 3 {
            out.push((path, true));
        }
    }
    out
}

/// Join runs that meet end to end where nothing else does (after pruning).
fn join(mut runs: Vec<(Vec<usize>, bool)>) -> Vec<(Vec<usize>, bool)> {
    loop {
        let mut joined = false;
        'outer: for i in 0..runs.len() {
            if runs[i].1 {
                continue;
            }
            for end_i in [0usize, 1] {
                let e = if end_i == 0 { runs[i].0[0] } else { *runs[i].0.last().unwrap() };
                // runs touching this end
                let touching: Vec<(usize, usize)> = (0..runs.len())
                    .filter(|&j| j != i && !runs[j].1)
                    .flat_map(|j| {
                        let r = &runs[j].0;
                        let mut v = vec![];
                        if r[0] == e {
                            v.push((j, 0));
                        }
                        if *r.last().unwrap() == e {
                            v.push((j, 1));
                        }
                        v
                    })
                    .collect();
                if touching.len() != 1 {
                    continue;
                }
                let (j, end_j) = touching[0];
                let mut a = runs[i].0.clone();
                let mut b = runs[j].0.clone();
                if end_i == 0 {
                    a.reverse();
                }
                if end_j == 1 {
                    b.reverse();
                }
                a.extend(b.into_iter().skip(1));
                let closed = a.len() > 3 && a[0] == *a.last().unwrap();
                if closed {
                    a.pop();
                }
                let (lo, hi) = (i.min(j), i.max(j));
                runs.remove(hi);
                runs.remove(lo);
                runs.push((a, closed));
                joined = true;
                break 'outer;
            }
        }
        if !joined {
            return runs;
        }
    }
}

/// Trace premultiplied RGBA line art into lines (pixel coordinates).
pub fn trace(rgba: &[u8], w: u32, h: u32, o: &TraceOpts) -> MergeShape {
    let (w, h) = (w as usize, h as usize);
    let mut out = MergeShape::default();
    if w == 0 || h == 0 || rgba.len() < w * h * 4 {
        return out;
    }
    let mut ink = ink_mask(rgba, w, h, o.line_threshold);
    despeckle(&mut ink, w, h, o.min_area.max(1) as usize);
    let dist = distance(&ink, w, h);
    let skel = thin(&ink, w, h);
    // thickness: twice the distance to the paper at the ridge (the skeleton
    // can sit a pixel off it on slanted lines, so take the local maximum)
    let width_at = |i: usize| {
        let (x, y) = ((i % w) as i32, (i / w) as i32);
        let mut m = dist[i];
        for dy in -1..=1 {
            for dx in -1..=1 {
                let (nx, ny) = (x + dx, y + dy);
                if nx >= 0 && ny >= 0 && nx < w as i32 && ny < h as i32 {
                    m = m.max(dist[ny as usize * w + nx as usize]);
                }
            }
        }
        (2.0 * m - 1.0).max(1.0)
    };
    // prune short spurs (an end that is no longer than the ink is thick) and junction knots
    let deg = |i: usize| nbrs(&skel, w, h, i).len();
    let mut rs: Vec<(Vec<usize>, bool)> = runs(&skel, w, h)
        .into_iter()
        .filter(|(p, closed)| {
            if *closed {
                return p.len() >= 4;
            }
            let (a, b) = (p[0], *p.last().unwrap());
            let thick = p.iter().map(|&i| width_at(i)).fold(1.0f32, f32::max);
            let ends = [a, b].iter().filter(|&&e| deg(e) == 1).count();
            let len = p.len() as f32;
            match ends {
                2 => len >= 2.0 || thick > 1.5, // an isolated dash or dot stays
                1 => len > thick.max(3.0),
                _ => len > 3.0,
            }
        })
        .collect();
    rs = join(rs);
    let (tol, passes) = o.curve_fit.params();
    for (path, closed) in rs {
        let pts: Vec<Pt> = path.iter().map(|&i| pt((i % w) as f32 + 0.5, (i / w) as f32 + 0.5)).collect();
        let widths: Vec<f32> = path.iter().map(|&i| width_at(i)).collect();
        // colour: average straight colour of the ink along the run
        let (mut r, mut g, mut b, mut n) = (0u32, 0u32, 0u32, 0u32);
        for &i in &path {
            let p = &rgba[i * 4..i * 4 + 4];
            if p[3] > 0 {
                let u = |c: u8| (c as u32 * 255 / p[3] as u32).min(255);
                r += u(p[0]);
                g += u(p[1]);
                b += u(p[2]);
                n += 1;
            }
        }
        let color = match (r.checked_div(n), g.checked_div(n), b.checked_div(n)) {
            (Some(r), Some(g), Some(b)) => Color::rgb(r as u8, g as u8, b as u8),
            _ => Color::rgb(0, 0, 0),
        };
        let simple = if pts.len() >= 3 { simplify_chain(&pts, closed, tol.max(0.35), passes.max(1), o.corners.angle()) } else { pts.clone() };
        if simple.len() < 2 {
            continue;
        }
        // width profile along the run (smoothed), relative to its median
        let mut sorted = widths.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let base = sorted[sorted.len() / 2].max(0.75);
        let profile = if o.uniform_width || widths.len() < 4 {
            None
        } else {
            let k = 12.min(widths.len());
            let prof: Vec<(f32, f32)> = (0..k)
                .map(|j| {
                    let t = j as f32 / (k - 1).max(1) as f32;
                    let c = ((widths.len() - 1) as f32 * t).round() as usize;
                    let lo = c.saturating_sub(2);
                    let hi = (c + 3).min(widths.len());
                    let avg = widths[lo..hi].iter().sum::<f32>() / (hi - lo) as f32;
                    (t, (avg / base).clamp(0.2, 5.0))
                })
                .collect();
            (prof.iter().any(|(_, m)| (m - 1.0).abs() > 0.15)).then_some(prof)
        };
        out.add_stroke_w(&simple, closed, Stroke { color, width: base }, profile);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trace::TraceMode;

    fn canvas(w: usize, h: usize) -> Vec<u8> {
        vec![255; w * h * 4]
    }
    fn ink(px: &mut [u8], w: usize, x: usize, y: usize) {
        px[(y * w + x) * 4..(y * w + x) * 4 + 4].copy_from_slice(&[0, 0, 0, 255]);
    }

    #[test]
    fn a_thick_line_becomes_one_centred_stroke_with_its_width() {
        let (w, h) = (60, 30);
        let mut px = canvas(w, h);
        // a 5 px thick horizontal bar from x 5 to 55, centred on y = 15
        for y in 13..18 {
            for x in 5..55 {
                ink(&mut px, w, x, y);
            }
        }
        let o = TraceOpts { mode: TraceMode::Centerline, ..Default::default() };
        let m = trace(&px, w as u32, h as u32, &o);
        assert_eq!(m.strokes.len(), 1, "{:?}", m.strokes.iter().map(|s| s.pts.len()).collect::<Vec<_>>());
        let s = &m.strokes[0];
        assert!(s.pts.iter().all(|p| (p.y - 15.5).abs() < 1.0), "on the centre line");
        let (lo, hi) = s.pts.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.x), b.max(p.x)));
        assert!(lo < 10.0 && hi > 50.0, "spans the bar: {lo} {hi}");
        assert!((s.stroke.width - 5.0).abs() <= 1.0, "width ≈ ink thickness: {}", s.stroke.width);
        assert_eq!(s.stroke.color, Color::rgb(0, 0, 0));
    }

    #[test]
    fn crossing_lines_keep_their_branches_and_specks_go() {
        let (w, h) = (64, 64);
        let mut px = canvas(w, h);
        for i in 8..56 {
            for t in 0..3 {
                ink(&mut px, w, i, 31 + t); // horizontal
                ink(&mut px, w, 31 + t, i); // vertical
            }
        }
        ink(&mut px, w, 3, 3); // a speck
        let o = TraceOpts { mode: TraceMode::Centerline, min_area: 4, ..Default::default() };
        let m = trace(&px, w as u32, h as u32, &o);
        let n = m.strokes.len();
        assert!((2..=6).contains(&n), "a cross: {n} strokes");
        let all: Vec<Pt> = m.strokes.iter().flat_map(|s| s.pts.clone()).collect();
        for target in [pt(9.0, 32.5), pt(55.0, 32.5), pt(32.5, 9.0), pt(32.5, 55.0)] {
            assert!(all.iter().any(|p| p.dist(target) < 3.5), "reaches {target:?}");
        }
        assert!(!all.iter().any(|p| p.dist(pt(3.5, 3.5)) < 2.0), "speck removed");
    }

    #[test]
    fn a_ring_traces_as_a_closed_line() {
        let (w, h) = (50, 50);
        let mut px = canvas(w, h);
        for y in 0..h {
            for x in 0..w {
                let d = ((x as f32 - 25.0).powi(2) + (y as f32 - 25.0).powi(2)).sqrt();
                if (d - 15.0).abs() < 1.6 {
                    ink(&mut px, w, x, y);
                }
            }
        }
        let o = TraceOpts { mode: TraceMode::Centerline, ..Default::default() };
        let m = trace(&px, w as u32, h as u32, &o);
        assert_eq!(m.strokes.len(), 1);
        assert!(m.strokes[0].closed);
        assert!(m.strokes[0].pts.iter().all(|p| ((p.x - 25.0).hypot(p.y - 25.0) - 15.0).abs() < 1.5));
    }
}
