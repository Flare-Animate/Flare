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
    out: &mut Xml,
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
    out.open("DOMShape", []);
    if has_fill {
        out.open("fills", []);
        out.open("FillStyle", [a("index", 1)]);
        out.empty(
            "SolidColor",
            [a("color", hex(fill)), a("alpha", alpha(fill))],
        );
        out.close("FillStyle");
        out.close("fills");
    }
    if let Some(stroke) = stroke {
        out.open("strokes", []);
        out.open("StrokeStyle", [a("index", 1)]);
        out.open(
            "SolidStroke",
            [
                a("weight", num(stroke.width())),
                a("caps", caps_name(stroke.cap())),
                a("joints", joints_name(stroke.join())),
            ],
        );
        out.open("fill", []);
        out.empty(
            "SolidColor",
            [
                a("color", hex(stroke.color())),
                a("alpha", alpha(stroke.color())),
            ],
        );
        out.close("fill");
        out.close("SolidStroke");
        out.close("StrokeStyle");
        out.close("strokes");
    }
    out.open("edges", []);
    for subpath in encode_edges(shape, approximated) {
        let mut attrs = Vec::new();
        if has_fill {
            attrs.push(a("fillStyle1", 1));
        }
        if stroke.is_some() {
            attrs.push(a("strokeStyle", 1));
        }
        attrs.push(a("edges", subpath.edges));
        out.empty("Edge", attrs);
    }
    out.close("edges");
    out.close("DOMShape");
}

// ---- timelines ------------------------------------------------------------------------

pub(super) fn write_instance(out: &mut Xml, instance: &Instance, names: &Names) {
    let [m_a, m_b, m_c, m_d, tx, ty] = instance.transform().as_affine().to_cols_array();
    out.open(
        "DOMSymbolInstance",
        [
            a("libraryItemName", names.library_name(instance.symbol())),
            a("symbolType", "graphic"),
            a("loop", "loop"),
        ],
    );
    out.open("matrix", []);
    out.empty(
        "Matrix",
        [
            a("a", num(m_a)),
            a("b", num(m_b)),
            a("c", num(m_c)),
            a("d", num(m_d)),
            a("tx", num(tx)),
            a("ty", num(ty)),
        ],
    );
    out.close("matrix");
    out.close("DOMSymbolInstance");
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
