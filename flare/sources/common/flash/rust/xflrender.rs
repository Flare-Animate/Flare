//! XFL (Flash CS5/CS6/Animate) -> per-frame SVG. Renders main-timeline DOMShapes and
//! nested symbol instances (LIBRARY/*.xml) so imports are not blank. Edge grammar per XFLShape.h.
use roxmltree::{Document, Node};
use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::Path;

type M = [f64; 6]; // a b c d tx ty
const ID: M = [1., 0., 0., 1., 0., 0.];
fn mul(p: &M, q: &M) -> M {
    [
        p[0] * q[0] + p[2] * q[1],
        p[1] * q[0] + p[3] * q[1],
        p[0] * q[2] + p[2] * q[3],
        p[1] * q[2] + p[3] * q[3],
        p[0] * q[4] + p[2] * q[5] + p[4],
        p[1] * q[4] + p[3] * q[5] + p[5],
    ]
}
fn f(n: Node, a: &str, d: f64) -> f64 {
    n.attribute(a).and_then(|v| v.parse().ok()).unwrap_or(d)
}
fn kid<'a, 'i>(n: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
    n.children().find(|c| c.tag_name().name() == name)
}
fn kids<'a, 'i: 'a>(n: Node<'a, 'i>, name: &'a str) -> impl Iterator<Item = Node<'a, 'i>> + 'a {
    n.children().filter(move |c| c.tag_name().name() == name)
}
fn matrix(n: Node) -> M {
    match kid(n, "matrix").and_then(|m| kid(m, "Matrix")) {
        Some(m) => [
            f(m, "a", 1.),
            f(m, "b", 0.),
            f(m, "c", 0.),
            f(m, "d", 1.),
            f(m, "tx", 0.),
            f(m, "ty", 0.),
        ],
        None => ID,
    }
}

#[derive(Clone, Copy)]
struct Seg {
    a: (i64, i64),
    c: Option<(i64, i64)>,
    b: (i64, i64),
    f0: u32,
    f1: u32,
    s: u32,
}

fn num(t: &str) -> Option<i64> {
    if let Some(h) = t.strip_prefix('#') {
        let (i, fr) = h.split_once('.').unwrap_or((h, "0"));
        let v = (u32::from_str_radix(i, 16).ok()? as i32 as i64) * 256
            + i64::from_str_radix(fr, 16).ok()?;
        return Some((v as f64 / 256.).round() as i64); // 24.8 fixed-point twips
    }
    t.parse::<f64>().ok().map(|v| v.round() as i64)
}

/// Decode an `edges` attribute into segments (twips).
fn edges(s: &str, f0: u32, f1: u32, st: u32, out: &mut Vec<Seg>) {
    let mut toks: Vec<String> = vec![];
    for ch in s.chars() {
        if "!|/[]S".contains(ch) {
            toks.push(ch.to_string());
            toks.push(String::new());
        } else if ch.is_whitespace() {
            toks.push(String::new());
        } else if let Some(l) = toks.last_mut() {
            l.push(ch)
        } else {
            toks.push(ch.to_string())
        }
    }
    let toks: Vec<&str> = toks
        .iter()
        .map(|t| t.as_str())
        .filter(|t| !t.is_empty())
        .collect();
    let (mut i, mut cur) = (0, (0i64, 0i64));
    let pt =
        |i: usize| -> Option<(i64, i64)> { Some((num(toks.get(i)?)?, num(toks.get(i + 1)?)?)) };
    while i < toks.len() {
        match toks[i] {
            "!" => {
                if let Some(p) = pt(i + 1) {
                    cur = p
                }
                i += 3
            }
            "|" | "/" => {
                if let Some(p) = pt(i + 1) {
                    out.push(Seg {
                        a: cur,
                        c: None,
                        b: p,
                        f0,
                        f1,
                        s: st,
                    });
                    cur = p
                }
                i += 3
            }
            "[" | "]" => {
                if let (Some(c), Some(p)) = (pt(i + 1), pt(i + 3)) {
                    out.push(Seg {
                        a: cur,
                        c: Some(c),
                        b: p,
                        f0,
                        f1,
                        s: st,
                    });
                    cur = p
                }
                i += 5
            }
            "S" => i += 2,
            _ => i += 1,
        }
    }
}

fn color(n: Node) -> Option<(String, f64)> {
    let c = n.descendants().find(|d| d.attribute("color").is_some())?;
    Some((c.attribute("color")?.to_string(), f(c, "alpha", 1.)))
}

struct Ctx<'a> {
    lib: &'a Path,
    cache: HashMap<String, Option<String>>,
    log: Vec<String>,
    depth: u32,
}

impl<'a> Ctx<'a> {
    fn shape(&mut self, n: Node, m: &M, svg: &mut String) {
        let m = mul(m, &matrix(n));
        let mut fills = HashMap::new();
        let mut strokes = HashMap::new();
        if let Some(fs) = kid(n, "fills") {
            for s in kids(fs, "FillStyle") {
                if kid(s, "SolidColor").is_none() {
                    self.log
                        .push("gradient/bitmap fill approximated with first color".into())
                }
                fills.insert(
                    f(s, "index", 0.) as u32,
                    color(s).unwrap_or(("#000000".into(), 1.)),
                );
            }
        }
        if let Some(ss) = kid(n, "strokes") {
            for s in kids(ss, "StrokeStyle") {
                let w = s
                    .descendants()
                    .find(|d| d.attribute("weight").is_some())
                    .map_or(1., |d| f(d, "weight", 1.));
                strokes.insert(
                    f(s, "index", 0.) as u32,
                    (color(s).unwrap_or(("#000000".into(), 1.)), w),
                );
            }
        }
        let mut segs = vec![];
        if let Some(es) = kid(n, "edges") {
            for e in kids(es, "Edge") {
                if let Some(s) = e.attribute("edges") {
                    edges(
                        s,
                        f(e, "fillStyle0", 0.) as u32,
                        f(e, "fillStyle1", 0.) as u32,
                        f(e, "strokeStyle", 0.) as u32,
                        &mut segs,
                    )
                }
            }
        }
        let tr = format!(
            "matrix({} {} {} {} {} {})",
            m[0], m[1], m[2], m[3], m[4], m[5]
        );
        let p = |q: (i64, i64)| format!("{} {}", q.0 as f64 / 20., q.1 as f64 / 20.);
        let mut ids: Vec<_> = fills.keys().copied().collect();
        ids.sort();
        for id in ids {
            // Fill = edges with this style on the right (fillStyle1) as-is, fillStyle0 reversed; chain into loops.
            let mut pool: Vec<Seg> = segs.iter().filter(|s| s.f1 == id).copied().collect();
            pool.extend(segs.iter().filter(|s| s.f0 == id).map(|s| Seg {
                a: s.b,
                b: s.a,
                ..*s
            }));
            let mut d = String::new();
            let mut used = vec![false; pool.len()];
            let mut by_start: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
            for (i, s) in pool.iter().enumerate() {
                by_start.entry(s.a).or_default().push(i)
            }
            for st in 0..pool.len() {
                if used[st] {
                    continue;
                }
                let (mut i, start) = (st, pool[st].a);
                let _ = write!(d, "M{}", p(start));
                loop {
                    used[i] = true;
                    let s = pool[i];
                    match s.c {
                        Some(c) => {
                            let _ = write!(d, "Q{} {}", p(c), p(s.b));
                        }
                        None => {
                            let _ = write!(d, "L{}", p(s.b));
                        }
                    }
                    if s.b == start {
                        break;
                    }
                    match by_start
                        .get(&s.b)
                        .and_then(|v| v.iter().copied().find(|&j| !used[j]))
                    {
                        Some(j) => i = j,
                        None => break,
                    }
                }
                d.push('Z');
            }
            if !d.is_empty() {
                let (c, a) = &fills[&id];
                let _ = write!(
                    svg,
                    "<path transform=\"{tr}\" fill=\"{c}\" fill-opacity=\"{a}\" d=\"{d}\"/>"
                );
            }
        }
        let mut ids: Vec<_> = strokes.keys().copied().collect();
        ids.sort();
        for id in ids {
            let mut d = String::new();
            for s in segs.iter().filter(|s| s.s == id) {
                let _ = write!(d, "M{}", p(s.a));
                match s.c {
                    Some(c) => {
                        let _ = write!(d, "Q{} {}", p(c), p(s.b));
                    }
                    None => {
                        let _ = write!(d, "L{}", p(s.b));
                    }
                }
            }
            if !d.is_empty() {
                let ((c, a), w) = &strokes[&id];
                let _ = write!(svg, "<path transform=\"{tr}\" fill=\"none\" stroke=\"{c}\" stroke-opacity=\"{a}\" stroke-width=\"{w}\" stroke-linecap=\"round\" stroke-linejoin=\"round\" d=\"{d}\"/>");
            }
        }
    }

    fn elements(&mut self, els: Node, m: &M, t: u32, svg: &mut String) {
        // XFL lists elements back-to-front.
        for e in els.children().filter(|c| c.is_element()) {
            match e.tag_name().name() {
                "DOMShape" => self.shape(e, m, svg),
                "DOMGroup" => {
                    let gm = mul(m, &matrix(e));
                    if let Some(ms) = kid(e, "members") {
                        self.elements(ms, &gm, t, svg)
                    }
                }
                "DOMSymbolInstance" => {
                    let name = e.attribute("libraryItemName").unwrap_or("");
                    let ft = if e.attribute("symbolType") == Some("graphic") {
                        f(e, "firstFrame", 0.) as u32 + t
                    } else {
                        0
                    };
                    self.symbol(name, &mul(m, &matrix(e)), ft, svg);
                }
                other => self
                    .log
                    .push(format!("skipped element <{other}> (not rendered yet)")),
            }
        }
    }

    fn symbol(&mut self, name: &str, m: &M, t: u32, svg: &mut String) {
        if self.depth > 16 {
            self.log.push(format!("symbol '{name}' nested too deep"));
            return;
        }
        if !self.cache.contains_key(name) {
            let p = self.lib.join(format!("{name}.xml"));
            let s = std::fs::read_to_string(&p).ok();
            if s.is_none() {
                self.log
                    .push(format!("missing library symbol '{name}' ({})", p.display()))
            }
            self.cache.insert(name.to_string(), s);
        }
        let Some(src) = self.cache[name].clone() else {
            return;
        };
        let Ok(doc) = Document::parse(&src) else {
            self.log.push(format!("bad XML in symbol '{name}'"));
            return;
        };
        let Some(tl) = doc.descendants().find(|n| n.has_tag_name("DOMTimeline")) else {
            return;
        };
        self.depth += 1;
        let len = tl_len(tl).max(1);
        self.timeline(tl, m, t % len, svg);
        self.depth -= 1;
    }

    fn timeline(&mut self, tl: Node, m: &M, t: u32, svg: &mut String) {
        let layers: Vec<_> = tl
            .descendants()
            .filter(|n| n.has_tag_name("DOMLayer"))
            .collect();
        for l in layers.iter().rev() {
            if matches!(l.attribute("layerType"), Some("guide" | "folder" | "mask")) {
                continue;
            }
            if let Some(fr) = frame_at(*l, t) {
                if let Some(els) = kid(fr, "elements") {
                    self.elements(els, m, t - f(fr, "index", 0.) as u32, svg)
                }
            }
        }
    }
}

fn frame_at<'a, 'i>(l: Node<'a, 'i>, t: u32) -> Option<Node<'a, 'i>> {
    kid(l, "frames")?
        .children()
        .filter(|c| c.has_tag_name("DOMFrame"))
        .find(|fr| {
            let i = f(*fr, "index", 0.) as u32;
            i <= t && t < i + f(*fr, "duration", 1.) as u32
        })
}
fn tl_len(tl: Node) -> u32 {
    tl.descendants()
        .filter(|n| n.has_tag_name("DOMFrame"))
        .map(|fr| f(fr, "index", 0.) as u32 + f(fr, "duration", 1.) as u32)
        .max()
        .unwrap_or(0)
}

/// Render each main-timeline layer, frame by frame, to `out/L<layer>_<n>.svg` (identical frames share
/// a file). Writes `out/frames.tsv` (layer, name, row, file) and returns a human-readable log.
pub fn export(xfl_dir: &Path, out: &Path) -> Result<(usize, String), String> {
    let src = std::fs::read_to_string(xfl_dir.join("DOMDocument.xml"))
        .map_err(|e| format!("DOMDocument.xml: {e}"))?;
    let doc = Document::parse(&src).map_err(|e| format!("DOMDocument.xml XML error: {e}"))?;
    let root = doc.root_element();
    let (w, h) = (f(root, "width", 550.), f(root, "height", 400.));
    let tl = doc
        .descendants()
        .find(|n| n.has_tag_name("DOMTimeline"))
        .ok_or("no DOMTimeline in DOMDocument.xml")?;
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let mut cx = Ctx {
        lib: &xfl_dir.join("LIBRARY"),
        cache: HashMap::new(),
        log: vec![],
        depth: 0,
    };
    let mut log = format!(
        "XFL {} ({w}x{h}), timeline '{}', {} frame(s)\n",
        xfl_dir.display(),
        tl.attribute("name").unwrap_or(""),
        tl_len(tl)
    );
    let mut tsv = String::new();
    let mut files = 0;
    let total = tl_len(tl).min(10000);
    let layers: Vec<_> = tl
        .descendants()
        .filter(|n| n.has_tag_name("DOMLayer"))
        .collect();
    for (li, l) in layers.iter().enumerate() {
        let name = l.attribute("name").unwrap_or("");
        if matches!(l.attribute("layerType"), Some("guide" | "folder" | "mask")) {
            let _ = writeln!(
                log,
                "layer {li} '{name}': skipped ({} layer)",
                l.attribute("layerType").unwrap()
            );
            continue;
        }
        let mut seen: HashMap<String, String> = HashMap::new();
        let (mut rows, mut empty) = (0, 0);
        for t in 0..total {
            let mut body = String::new();
            if let Some(fr) = frame_at(*l, t) {
                if let Some(els) = kid(fr, "elements") {
                    cx.elements(els, &ID, t - f(fr, "index", 0.) as u32, &mut body)
                }
            }
            if body.is_empty() {
                empty += 1;
                continue;
            }
            let file = match seen.get(&body) {
                Some(fl) => fl.clone(),
                None => {
                    let fl = format!("L{li}_{}.svg", seen.len() + 1);
                    let svg = format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\">{body}</svg>\n");
                    std::fs::write(out.join(&fl), svg).map_err(|e| e.to_string())?;
                    files += 1;
                    seen.insert(body, fl.clone());
                    fl
                }
            };
            let _ = writeln!(
                tsv,
                "{li}\t{}\t{t}\t{file}",
                name.replace(['\t', '\n'], " ")
            );
            rows += 1;
        }
        let _ = writeln!(
            log,
            "layer {li} '{name}': {rows} frame(s) with art, {} drawing(s), {empty} empty",
            seen.len()
        );
    }
    std::fs::write(out.join("frames.tsv"), tsv).map_err(|e| e.to_string())?;
    let mut counts: HashMap<String, usize> = HashMap::new();
    for m in cx.log {
        *counts.entry(m).or_default() += 1
    }
    let mut c: Vec<_> = counts.into_iter().collect();
    c.sort();
    for (m, n) in c {
        let _ = writeln!(log, "note: {m} (x{n})");
    }
    let _ = writeln!(log, "wrote {files} SVG drawing(s) to {}", out.display());
    Ok((files, log))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn edge_decode() {
        let mut v = vec![];
        edges("!0 0|200 0[200 200 0 200S2/0 0", 0, 1, 0, &mut v);
        assert_eq!(v.len(), 3);
        assert_eq!(v[1].c, Some((200, 200)));
        let mut v = vec![];
        edges("!#A.80 0|0 0", 0, 1, 0, &mut v);
        assert_eq!(v[0].a.0, 11); // #A.80 = 10.5 twips
    }
    #[test]
    fn samples() {
        // Real CS6-style XFL sample if present (FLARE_XFL_SAMPLE=extracted dir).
        let Ok(d) = std::env::var("FLARE_XFL_SAMPLE") else {
            return;
        };
        let out = std::env::temp_dir().join("flare_xfl_test");
        let (n, log) = export(Path::new(&d), &out).unwrap();
        println!("{log}");
        assert!(n > 0, "no art rendered");
    }
}
