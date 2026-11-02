//! XFL `edges` strings: parsing into straight/quadratic pieces, chaining
//! pieces into loops (fills) and polylines (strokes), and writing them back.
//!
//! Grammar (from observing files written by the authoring tool):
//! - coordinates are twips (1/20 px): decimal (`-22.5`) or hex fixed point
//!   `#XX.YY` (two's-complement integer part — 24-bit, so `#FFFE40.06` is
//!   negative — and an 8-bit fraction)
//! - `!x y` move to, `|x y` or `/x y` line to, `[cx cy x y` or `]cx cy x y`
//!   quadratic curve to
//! - `S<n>` selection markers are ignored; the separate `cubics` attribute
//!   (authoring hints) is ignored

use ac_core::geom::{pt, Path, Pt, Seg};

/// One straight or quadratic piece of an edge.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Piece {
    pub a: Pt,
    pub ctrl: Option<Pt>,
    pub b: Pt,
}

impl Piece {
    pub fn reversed(self) -> Piece {
        Piece { a: self.b, ctrl: self.ctrl, b: self.a }
    }
    /// Points after `a`, flattened.
    pub fn flatten_into(&self, out: &mut Vec<Pt>) {
        match self.ctrl {
            None => out.push(self.b),
            Some(c) => {
                let len = self.a.dist(c) + c.dist(self.b);
                let n = ((len / 4.0).ceil() as usize).clamp(2, 24);
                for i in 1..=n {
                    let t = i as f32 / n as f32;
                    let u = 1.0 - t;
                    out.push(pt(u * u * self.a.x + 2.0 * u * t * c.x + t * t * self.b.x, u * u * self.a.y + 2.0 * u * t * c.y + t * t * self.b.y));
                }
            }
        }
    }
}

fn number(s: &[u8], i: &mut usize) -> Result<f32, String> {
    while *i < s.len() && s[*i].is_ascii_whitespace() {
        *i += 1;
    }
    let start = *i;
    if *i < s.len() && s[*i] == b'#' {
        *i += 1;
        let neg = *i < s.len() && s[*i] == b'-';
        if neg {
            *i += 1;
        }
        let hs = *i;
        while *i < s.len() && (s[*i].is_ascii_hexdigit() || s[*i] == b'.') {
            *i += 1;
        }
        let tok = std::str::from_utf8(&s[hs..*i]).unwrap_or("");
        let (int, frac) = tok.split_once('.').unwrap_or((tok, ""));
        // the integer part is two's complement: 24-bit as written by the
        // authoring tool (#FFFE40.06 = -447.98), 32-bit when 7–8 digits appear
        let iv = if int.is_empty() {
            0
        } else {
            let u = u32::from_str_radix(int, 16).map_err(|e| format!("bad hex '{tok}': {e}"))?;
            if int.len() <= 6 {
                if u >= 0x80_0000 {
                    u as i32 - 0x100_0000
                } else {
                    u as i32
                }
            } else {
                u as i32
            }
        };
        let fv = if frac.is_empty() { 0 } else { u32::from_str_radix(frac, 16).map_err(|e| format!("bad hex '{tok}': {e}"))? };
        let fscale = 16f32.powi(frac.len() as i32);
        // fixed point: the fraction always adds (FFFFFFFF.80 = -0.5)
        let mut v = iv as f32 + fv as f32 / fscale;
        if neg {
            v = -v;
        }
        return Ok(v / 20.0);
    }
    while *i < s.len() && (s[*i].is_ascii_digit() || matches!(s[*i], b'.' | b'-' | b'+' | b'e' | b'E')) {
        *i += 1;
    }
    let tok = std::str::from_utf8(&s[start..*i]).unwrap_or("");
    tok.parse::<f32>().map(|v| v / 20.0).map_err(|_| format!("expected a number at {start}, got '{tok}'"))
}

/// Parse an `edges` attribute into pieces (pixels).
pub fn parse(s: &str) -> Result<Vec<Piece>, String> {
    let b = s.as_bytes();
    let mut i = 0;
    let mut pen: Option<Pt> = None;
    let mut out = vec![];
    while i < b.len() {
        let c = b[i];
        i += 1;
        match c {
            c if c.is_ascii_whitespace() => {}
            b'!' => pen = Some(pt(number(b, &mut i)?, number(b, &mut i)?)),
            b'|' | b'/' => {
                let p = pt(number(b, &mut i)?, number(b, &mut i)?);
                let a = pen.ok_or("line before move")?;
                out.push(Piece { a, ctrl: None, b: p });
                pen = Some(p);
            }
            b'[' | b']' => {
                let ctrl = pt(number(b, &mut i)?, number(b, &mut i)?);
                let p = pt(number(b, &mut i)?, number(b, &mut i)?);
                let a = pen.ok_or("curve before move")?;
                out.push(Piece { a, ctrl: Some(ctrl), b: p });
                pen = Some(p);
            }
            b'S' => {
                while i < b.len() && b[i].is_ascii_digit() {
                    i += 1;
                }
            }
            other => return Err(format!("unexpected '{}' in edges", other as char)),
        }
    }
    Ok(out)
}

/// Back-compat helper: an `edges` string as one path (move/line/quad).
pub fn parse_path(s: &str) -> Result<Path, String> {
    let mut segs = vec![];
    let mut pen: Option<Pt> = None;
    for p in parse(s)? {
        if pen.map(|q| q.dist(p.a) > 1e-3).unwrap_or(true) {
            segs.push(Seg::M(p.a));
        }
        segs.push(match p.ctrl {
            None => Seg::L(p.b),
            Some(c) => Seg::Q(c, p.b),
        });
        pen = Some(p.b);
    }
    Ok(Path { segs })
}

fn key(p: Pt) -> (i64, i64) {
    ((p.x * 40.0).round() as i64, (p.y * 40.0).round() as i64)
}

/// Chain oriented pieces end-to-start. Returns (chain, closed).
pub fn chain(pieces: &[Piece]) -> Vec<(Vec<Piece>, bool)> {
    use std::collections::HashMap;
    let mut by_start: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    for (i, p) in pieces.iter().enumerate() {
        by_start.entry(key(p.a)).or_default().push(i);
    }
    let mut used = vec![false; pieces.len()];
    let mut out = vec![];
    // start open chains at pieces nobody leads into, then do the loops
    let ends: std::collections::HashSet<(i64, i64)> = pieces.iter().map(|p| key(p.b)).collect();
    let mut order: Vec<usize> = (0..pieces.len()).filter(|&i| !ends.contains(&key(pieces[i].a))).collect();
    order.extend(0..pieces.len());
    for s in order {
        if used[s] {
            continue;
        }
        used[s] = true;
        let start = key(pieces[s].a);
        let mut cur = vec![pieces[s]];
        loop {
            let end = key(cur.last().unwrap().b);
            if end == start {
                break;
            }
            let next = by_start.get(&end).and_then(|v| v.iter().copied().find(|&j| !used[j]));
            match next {
                Some(j) => {
                    used[j] = true;
                    cur.push(pieces[j]);
                }
                None => break,
            }
        }
        let closed = key(cur.last().unwrap().b) == start;
        out.push((cur, closed));
    }
    out
}

/// A chain as polygon points (closing point not repeated).
pub fn chain_points(ch: &[Piece], closed: bool) -> Vec<Pt> {
    let mut v = vec![ch[0].a];
    for p in ch {
        p.flatten_into(&mut v);
    }
    if closed && v.len() > 1 && v[0].dist(*v.last().unwrap()) < 0.05 {
        v.pop();
    }
    v
}

/// Chains as one path (curves kept).
pub fn chains_path(chains: &[(Vec<Piece>, bool)]) -> Path {
    let mut segs = vec![];
    for (ch, closed) in chains {
        segs.push(Seg::M(ch[0].a));
        for p in ch {
            segs.push(match p.ctrl {
                None => Seg::L(p.b),
                Some(c) => Seg::Q(c, p.b),
            });
        }
        if *closed {
            segs.push(Seg::Z);
        }
    }
    Path { segs }
}

// ---------------------------------------------------------------- writing

fn tw(v: f32) -> String {
    format!("{}", (v * 20.0).round() as i64)
}

/// Write pieces as an edges string (twips, integer).
pub fn write(subpaths: &[Vec<Piece>]) -> String {
    let mut s = String::new();
    for sp in subpaths {
        let Some(first) = sp.first() else { continue };
        s.push_str(&format!("!{} {}", tw(first.a.x), tw(first.a.y)));
        for p in sp {
            match p.ctrl {
                None => s.push_str(&format!("|{} {}", tw(p.b.x), tw(p.b.y))),
                Some(c) => s.push_str(&format!("[{} {} {} {}", tw(c.x), tw(c.y), tw(p.b.x), tw(p.b.y))),
            }
        }
    }
    s
}

/// A polygon (closed) or polyline as pieces.
pub fn poly_pieces(pts: &[Pt], closed: bool) -> Vec<Piece> {
    let mut v: Vec<Piece> = pts.windows(2).map(|w| Piece { a: w[0], ctrl: None, b: w[1] }).collect();
    if closed && pts.len() > 2 && pts[0].dist(*pts.last().unwrap()) > 1e-4 {
        v.push(Piece { a: *pts.last().unwrap(), ctrl: None, b: pts[0] });
    }
    v
}

/// Split a path into subpaths of pieces (cubics become two quadratics).
/// Returns (pieces, closed) per subpath.
pub fn path_pieces(path: &Path) -> Vec<(Vec<Piece>, bool)> {
    let mut out: Vec<(Vec<Piece>, bool)> = vec![];
    let mut cur: Vec<Piece> = vec![];
    let mut start = pt(0.0, 0.0);
    let mut pen = pt(0.0, 0.0);
    let flush = |cur: &mut Vec<Piece>, out: &mut Vec<(Vec<Piece>, bool)>, closed: bool| {
        if !cur.is_empty() {
            out.push((std::mem::take(cur), closed));
        }
    };
    for s in &path.segs {
        match *s {
            Seg::M(p) => {
                flush(&mut cur, &mut out, false);
                start = p;
                pen = p;
            }
            Seg::L(p) => {
                cur.push(Piece { a: pen, ctrl: None, b: p });
                pen = p;
            }
            Seg::Q(c, p) => {
                cur.push(Piece { a: pen, ctrl: Some(c), b: p });
                pen = p;
            }
            Seg::C(c1, c2, p) => {
                // split at t = 0.5 and fit a quadratic to each half
                let m = |a: Pt, b: Pt| a.lerp(b, 0.5);
                let (p01, p12, p23) = (m(pen, c1), m(c1, c2), m(c2, p));
                let (p012, p123) = (m(p01, p12), m(p12, p23));
                let mid = m(p012, p123);
                let q = |p0: Pt, a: Pt, b: Pt, p3: Pt| pt((3.0 * (a.x + b.x) - (p0.x + p3.x)) / 4.0, (3.0 * (a.y + b.y) - (p0.y + p3.y)) / 4.0);
                cur.push(Piece { a: pen, ctrl: Some(q(pen, p01, p012, mid)), b: mid });
                cur.push(Piece { a: mid, ctrl: Some(q(mid, p123, p23, p)), b: p });
                pen = p;
            }
            Seg::Z => {
                if pen.dist(start) > 1e-4 {
                    cur.push(Piece { a: pen, ctrl: None, b: start });
                }
                pen = start;
                flush(&mut cur, &mut out, true);
            }
        }
    }
    flush(&mut cur, &mut out, false);
    out
}

/// Signed area (y down: positive = clockwise on screen) of a closed chain,
/// with curves flattened.
pub fn signed_area(pts: &[Pt]) -> f32 {
    let n = pts.len();
    (0..n).map(|i| {
        let (a, b) = (pts[i], pts[(i + 1) % n]);
        a.x * b.y - b.x * a.y
    }).sum::<f32>() / 2.0
}

pub fn point_in(poly: &[Pt], x: Pt) -> bool {
    let mut inside = false;
    let n = poly.len();
    let mut j = n.wrapping_sub(1);
    for i in 0..n {
        let (a, b) = (poly[i], poly[j]);
        if (a.y > x.y) != (b.y > x.y) && x.x < (b.x - a.x) * (x.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_lines_curves_hex_and_markers() {
        let p = parse("!0 0|200 0[400 0 400 200|0 200S2|0 0").unwrap();
        assert_eq!(p.len(), 4);
        assert_eq!(p[1].ctrl, Some(pt(20.0, 0.0)));
        assert_eq!(p[1].b, pt(20.0, 10.0));
        let h = parse("!#14.00 #28.80|#-14 0").unwrap();
        assert!((h[0].a.x - 1.0).abs() < 1e-5 && (h[0].a.y - 2.025).abs() < 1e-5);
        assert!((h[0].b.x + 1.0).abs() < 1e-5);
        // 24-bit two's complement integer part, as real files write it
        let n24 = parse("!#FFFE40.06 0|#FFFFEC 0").unwrap();
        assert!((n24[0].a.x - (-448.0 + 6.0 / 256.0) / 20.0).abs() < 1e-4, "{:?}", n24[0].a);
        assert!((n24[0].b.x + 1.0).abs() < 1e-5);
        // 32-bit two's complement integer part
        let n = parse("!#FFFFFFEC 0|0 0").unwrap();
        assert!((n[0].a.x + 1.0).abs() < 1e-5);
        assert!(parse("!0 0 ?").is_err());
        // as one path: pen re-statements don't start new subpaths
        let p = parse_path("!0 0|200 0[400 0 400 200|0 200S2|0 0").unwrap();
        assert_eq!(p.segs[0], Seg::M(pt(0.0, 0.0)));
        assert_eq!(p.segs[2], Seg::Q(pt(20.0, 0.0), pt(20.0, 10.0)));
        assert_eq!(p.segs.len(), 5);
    }

    #[test]
    fn chains_pieces_into_loops_whatever_their_order() {
        let sq = poly_pieces(&[pt(0.0, 0.0), pt(10.0, 0.0), pt(10.0, 10.0), pt(0.0, 10.0)], true);
        let mut shuffled = vec![sq[2], sq[0], sq[3], sq[1]];
        shuffled.push(Piece { a: pt(50.0, 0.0), ctrl: None, b: pt(60.0, 0.0) });
        let ch = chain(&shuffled);
        assert_eq!(ch.len(), 2);
        assert!(ch.iter().any(|(c, closed)| *closed && c.len() == 4));
        assert!(ch.iter().any(|(c, closed)| !*closed && c.len() == 1));
        // writing and reading back is exact to the twip
        let s = write(std::slice::from_ref(&sq));
        assert_eq!(parse(&s).unwrap(), sq);
    }

    #[test]
    fn cubics_become_quadratics_close_to_the_curve() {
        let mut p = Path { segs: vec![Seg::M(pt(0.0, 0.0)), Seg::C(pt(0.0, 50.0), pt(100.0, 50.0), pt(100.0, 0.0))] };
        let pieces = path_pieces(&p);
        assert_eq!(pieces[0].0.len(), 2);
        // midpoint of the cubic is (50, 37.5); the quads meet there
        assert!(pieces[0].0[0].b.dist(pt(50.0, 37.5)) < 1e-4);
        p.segs.push(Seg::Z);
        let closed = path_pieces(&p);
        assert!(closed[0].1 && closed[0].0.len() == 3);
    }
}
