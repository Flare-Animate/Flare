//! Paint brushes from the file's `PaintBrushDefinitions.xml`: each art brush
//! carries its artwork as SVG (`svgData` in the `brushParam` JSON). A stroke
//! drawn with one (`PaintBrushStroke paintBrushDefinition="n"`) becomes the
//! artwork stretched along the line: the art's width runs along the stroke's
//! length, its height across the stroke (scaled to the stroke weight), like
//! an Illustrator art brush set to stretch. Our own code, from the format as
//! it appears in files.

use ac_core::{pt, Pt};
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct ArtBrush {
    pub name: String,
    /// Closed polygons in art units (x along, y across).
    pub polys: Vec<Vec<Pt>>,
    pub w: f32,
    pub h: f32,
    pub flip_along: bool,
}

/// Read every art brush of a `PaintBrushDefinitions.xml`, by index.
/// Returns the brushes and notes about the ones that couldn't be used.
pub fn parse_definitions(xml: &str) -> (HashMap<u32, ArtBrush>, Vec<String>) {
    let mut out = HashMap::new();
    let mut notes = vec![];
    let Ok(doc) = roxmltree::Document::parse(xml) else {
        return (out, vec!["PaintBrushDefinitions.xml could not be read".into()]);
    };
    for d in doc.root_element().children().filter(|c| c.is_element() && c.tag_name().name() == "paintbrushdefinition") {
        let Some(index) = d.attribute("index").and_then(|v| v.parse::<u32>().ok()) else { continue };
        let Some(param) = d.attribute("brushParam") else { continue };
        let Ok(p) = serde_json::from_str::<HashMap<String, serde_json::Value>>(param) else {
            notes.push(format!("paint brush {index}: unreadable parameters"));
            continue;
        };
        let s = |k: &str| p.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
        let name = s("name");
        if s("FlashPaintBrushType") != "ArtBrush" {
            notes.push(format!("paint brush '{name}' ({}) drawn as a solid line", s("FlashPaintBrushType")));
            continue;
        }
        match art(&s("svgData")) {
            Some((polys, w, h)) => {
                // artDirection 0 = left to right; 2 = right to left (others treated as 0)
                let flip_along = (s("flipArt") == "true") ^ (s("artDirection") == "2");
                out.insert(index, ArtBrush { name, polys, w, h, flip_along });
            }
            None => notes.push(format!("paint brush '{name}': artwork could not be read")),
        }
    }
    (out, notes)
}

/// The artwork's outlines (flattened) and its size.
fn art(svg: &str) -> Option<(Vec<Vec<Pt>>, f32, f32)> {
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).ok()?;
    let size = tree.size();
    let mut polys = vec![];
    collect(tree.root(), &mut polys);
    polys.retain(|p| p.len() >= 3);
    (!polys.is_empty()).then_some((polys, size.width(), size.height()))
}

fn collect(g: &usvg::Group, out: &mut Vec<Vec<Pt>>) {
    for n in g.children() {
        match n {
            usvg::Node::Group(g) => collect(g, out),
            usvg::Node::Path(p) if p.fill().is_some() => {
                let t = p.abs_transform();
                let map = |x: f32, y: f32| pt(t.sx * x + t.kx * y + t.tx, t.ky * x + t.sy * y + t.ty);
                let mut cur: Vec<Pt> = vec![];
                let mut last = pt(0.0, 0.0);
                for seg in p.data().segments() {
                    use usvg::tiny_skia_path::PathSegment as S;
                    match seg {
                        S::MoveTo(a) => {
                            if cur.len() >= 3 {
                                out.push(std::mem::take(&mut cur));
                            }
                            cur.clear();
                            last = map(a.x, a.y);
                            cur.push(last);
                        }
                        S::LineTo(a) => {
                            last = map(a.x, a.y);
                            cur.push(last);
                        }
                        S::QuadTo(c, a) => {
                            let (c, a) = (map(c.x, c.y), map(a.x, a.y));
                            for i in 1..=6 {
                                let u = i as f32 / 6.0;
                                let v = 1.0 - u;
                                cur.push(pt(v * v * last.x + 2.0 * u * v * c.x + u * u * a.x, v * v * last.y + 2.0 * u * v * c.y + u * u * a.y));
                            }
                            last = a;
                        }
                        S::CubicTo(c1, c2, a) => {
                            let (c1, c2, a) = (map(c1.x, c1.y), map(c2.x, c2.y), map(a.x, a.y));
                            for i in 1..=8 {
                                let u = i as f32 / 8.0;
                                let v = 1.0 - u;
                                let (k0, k1, k2, k3) = (v * v * v, 3.0 * u * v * v, 3.0 * u * u * v, u * u * u);
                                cur.push(pt(k0 * last.x + k1 * c1.x + k2 * c2.x + k3 * a.x, k0 * last.y + k1 * c1.y + k2 * c2.y + k3 * a.y));
                            }
                            last = a;
                        }
                        S::Close => {
                            if cur.len() >= 3 {
                                out.push(std::mem::take(&mut cur));
                            }
                            cur.clear();
                        }
                    }
                }
                if cur.len() >= 3 {
                    out.push(cur);
                }
            }
            _ => {}
        }
    }
}

/// The brush's artwork laid along `line` (a polyline in shape space) with
/// the given thickness: polygons to fill (even-odd).
pub fn along(b: &ArtBrush, line: &[Pt], thickness: f32) -> Vec<Vec<Pt>> {
    let mut cum = vec![0.0f32];
    for w in line.windows(2) {
        cum.push(cum.last().unwrap() + w[0].dist(w[1]));
    }
    let total = *cum.last().unwrap_or(&0.0);
    if line.len() < 2 || total < 1e-3 || b.w <= 0.0 || b.h <= 0.0 {
        return vec![];
    }
    // normals at the vertices (averaged where segments meet) so the art bends smoothly
    let seg_n: Vec<Pt> = line
        .windows(2)
        .map(|w| {
            let (dx, dy) = (w[1].x - w[0].x, w[1].y - w[0].y);
            let l = (dx * dx + dy * dy).sqrt().max(1e-6);
            pt(-dy / l, dx / l)
        })
        .collect();
    let vn: Vec<Pt> = (0..line.len())
        .map(|i| {
            let a = seg_n[i.saturating_sub(1).min(seg_n.len() - 1)];
            let c = seg_n[i.min(seg_n.len() - 1)];
            let (x, y) = (a.x + c.x, a.y + c.y);
            let l = (x * x + y * y).sqrt();
            if l < 1e-3 { c } else { pt(x / l, y / l) }
        })
        .collect();
    let place = |x: f32, y: f32| -> Pt {
        let mut t = (x / b.w).clamp(0.0, 1.0);
        if b.flip_along {
            t = 1.0 - t;
        }
        let s = t * total;
        let i = cum.partition_point(|&c| c <= s).clamp(1, line.len() - 1) - 1;
        let seg = (cum[i + 1] - cum[i]).max(1e-6);
        let u = ((s - cum[i]) / seg).clamp(0.0, 1.0);
        let p = pt(line[i].x + (line[i + 1].x - line[i].x) * u, line[i].y + (line[i + 1].y - line[i].y) * u);
        let n = pt(vn[i].x + (vn[i + 1].x - vn[i].x) * u, vn[i].y + (vn[i + 1].y - vn[i].y) * u);
        let off = (y / b.h - 0.5) * thickness;
        pt(p.x + n.x * off, p.y + n.y * off)
    };
    // long art edges are split so they follow the curve of the line
    let step = b.w / 64.0;
    b.polys
        .iter()
        .map(|poly| {
            let mut v = vec![];
            for k in 0..poly.len() {
                let (a, c) = (poly[k], poly[(k + 1) % poly.len()]);
                let n = ((c.x - a.x).abs() / step).ceil().max(1.0) as usize;
                for j in 0..n {
                    let u = j as f32 / n as f32;
                    v.push(place(a.x + (c.x - a.x) * u, a.y + (c.y - a.y) * u));
                }
            }
            v
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEFS: &str = r#"<paintbrushdefinitions xmlns="http://ns.adobe.com/xfl/2008/">
  <paintbrushdefinition index="1" brushParam='{"FlashPaintBrushType":"ArtBrush","artDirection":"0","flipArt":"false","name":"Bar","svgData":"&lt;svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 100 20\" width=\"100\" height=\"20\"&gt;&lt;path d=\"M0,5 L100,5 L100,15 L0,15 Z\"/&gt;&lt;/svg&gt;"}'/>
  <paintbrushdefinition index="2" brushParam='{"FlashPaintBrushType":"PatternBrush","name":"Pat"}'/>
</paintbrushdefinitions>"#;

    #[test]
    fn reads_art_brushes_and_lays_them_along_a_line() {
        let (b, notes) = parse_definitions(DEFS);
        assert_eq!(b.len(), 1);
        assert!(notes.iter().any(|n| n.contains("Pat")), "{notes:?}");
        let bar = &b[&1];
        assert_eq!((bar.w, bar.h), (100.0, 20.0));
        // along a horizontal line 0..200 with thickness 10: the bar is half the height → y from -2.5 to 2.5
        let polys = along(bar, &[pt(0.0, 0.0), pt(200.0, 0.0)], 10.0);
        let all: Vec<Pt> = polys.concat();
        let (minx, maxx) = all.iter().fold((f32::MAX, f32::MIN), |m, p| (m.0.min(p.x), m.1.max(p.x)));
        let (miny, maxy) = all.iter().fold((f32::MAX, f32::MIN), |m, p| (m.0.min(p.y), m.1.max(p.y)));
        assert!(minx.abs() < 1e-3 && (maxx - 200.0).abs() < 1e-3, "{minx} {maxx}");
        assert!((miny + 2.5).abs() < 1e-3 && (maxy - 2.5).abs() < 1e-3, "{miny} {maxy}");
        // around a right angle the art bends (points on both legs)
        let polys = along(bar, &[pt(0.0, 0.0), pt(100.0, 0.0), pt(100.0, 100.0)], 10.0);
        let all: Vec<Pt> = polys.concat();
        assert!(all.iter().any(|p| p.y > 90.0) && all.iter().any(|p| p.x < 10.0));
    }
}
