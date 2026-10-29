//! Shapes as XFL edges and instances.

use crate::curves::{self, Point};
use flare_document::{Instance, PathSegment, Shape};

use super::*;

// ---- shapes ---------------------------------------------------------------------------

pub(super) fn twips(p: Point) -> Point {
    (p.0 * TWIPS_PER_PX, p.1 * TWIPS_PER_PX)
}

pub(super) fn pt(p: Point) -> String {
    format!("{} {}", num(p.0), num(p.1))
}

pub(super) struct Subpath {
    pub(super) edges: String,
}

/// Encodes a shape's path as XFL edge strings, one per subpath.
pub(super) fn encode_edges(shape: &Shape, approximated: &mut bool) -> Vec<Subpath> {
    let xy = |p: &flare_core::DocumentPoint| twips((p.as_vec().x, p.as_vec().y));
    let mut out: Vec<Subpath> = Vec::new();
    let mut current: Option<String> = None;
    let (mut start, mut at): (Point, Point) = ((0.0, 0.0), (0.0, 0.0));
    let flush = |current: &mut Option<String>, out: &mut Vec<Subpath>| {
        if let Some(edges) = current.take()
            && edges.contains(['|', '['])
        {
            out.push(Subpath { edges });
        }
    };
    for segment in shape.path() {
        match segment {
            PathSegment::MoveTo(p) => {
                flush(&mut current, &mut out);
                (start, at) = (xy(p), xy(p));
                current = Some(format!("!{}", pt(start)));
            }
            PathSegment::LineTo(p) => {
                at = xy(p);
                if let Some(edges) = &mut current {
                    edges.push_str(&format!("|{}", pt(at)));
                }
            }
            PathSegment::CubicTo(a, b, c) => {
                let cubic = [at, xy(a), xy(b), xy(c)];
                let quads = curves::to_quads(cubic, CURVE_TOLERANCE_PX * TWIPS_PER_PX);
                if quads.len() > 1 || cubic_is_curved(cubic) {
                    *approximated = true;
                }
                if let Some(edges) = &mut current {
                    for (control, end) in quads {
                        edges.push_str(&format!("[{} {}", pt(control), pt(end)));
                    }
                }
                at = cubic[3];
            }
            PathSegment::Close => {
                if let Some(edges) = &mut current
                    && at != start
                {
                    edges.push_str(&format!("|{}", pt(start)));
                }
                flush(&mut current, &mut out);
                at = start;
            }
        }
    }
    flush(&mut current, &mut out);
    out
}

pub(super) fn cubic_is_curved(c: [Point; 4]) -> bool {
    let dx = c[3].0 - 3.0 * c[2].0 + 3.0 * c[1].0 - c[0].0;
    let dy = c[3].1 - 3.0 * c[2].1 + 3.0 * c[1].1 - c[0].1;
    dx.hypot(dy) > 1e-9
}

pub(super) fn write_shape(
    out: &mut String,
    shape: &Shape,
    place: &str,
    notes: &mut Vec<Diagnostic>,
    approximated: &mut bool,
) {
    let fill = shape.flat_fill();
    let has_fill = fill.a > 0;
    if shape.gradient().is_some() {
        notes.push(Diagnostic {
            severity: Severity::Warning,
            location: place.to_owned(),
            message: "a gradient was written as a flat color".into(),
        });
    }
    let stroke = shape.stroke();
    if !has_fill && stroke.is_none() {
        notes.push(Diagnostic {
            severity: Severity::Info,
            location: place.to_owned(),
            message: "an invisible shape (no fill, no stroke) was skipped".into(),
        });
        return;
    }
    out.push_str("<DOMShape>");
    if has_fill {
        out.push_str(&format!(
            "<fills><FillStyle index=\"1\"><SolidColor color=\"{}\" alpha=\"{}\"/></FillStyle></fills>",
            hex(fill),
            alpha(fill)
        ));
    }
    if let Some(stroke) = stroke {
        out.push_str(&format!(
            "<strokes><StrokeStyle index=\"1\"><SolidStroke weight=\"{}\" caps=\"{}\" joints=\"{}\"><fill><SolidColor color=\"{}\" alpha=\"{}\"/></fill></SolidStroke></StrokeStyle></strokes>",
            num(stroke.width()),
            caps_name(stroke.cap()),
            joints_name(stroke.join()),
            hex(stroke.color()),
            alpha(stroke.color())
        ));
    }
    out.push_str("<edges>");
    for subpath in encode_edges(shape, approximated) {
        let mut attrs = String::new();
        if has_fill {
            attrs.push_str(" fillStyle1=\"1\"");
        }
        if stroke.is_some() {
            attrs.push_str(" strokeStyle=\"1\"");
        }
        out.push_str(&format!("<Edge{attrs} edges=\"{}\"/>", subpath.edges));
    }
    out.push_str("</edges></DOMShape>");
}

// ---- timelines ------------------------------------------------------------------------

pub(super) fn write_instance(out: &mut String, instance: &Instance, names: &Names) {
    let [a, b, c, d, tx, ty] = instance.transform().as_affine().to_cols_array();
    out.push_str(&format!(
        "<DOMSymbolInstance libraryItemName=\"{}\" symbolType=\"graphic\" loop=\"loop\"><matrix><Matrix a=\"{}\" b=\"{}\" c=\"{}\" d=\"{}\" tx=\"{}\" ty=\"{}\"/></matrix></DOMSymbolInstance>",
        esc(names.library_name(instance.symbol())),
        num(a), num(b), num(c), num(d), num(tx), num(ty)
    ));
}

/// XFL's names for line ends (`none` is a flat end) and corners.
fn caps_name(cap: flare_document::Cap) -> &'static str {
    match cap {
        flare_document::Cap::Butt => "none",
        flare_document::Cap::Round => "round",
        flare_document::Cap::Square => "square",
    }
}

fn joints_name(join: flare_document::Join) -> &'static str {
    match join {
        flare_document::Join::Round => "round",
        flare_document::Join::Bevel => "bevel",
        flare_document::Join::Miter => "miter",
    }
}
