//! Import of SVG drawings as a one-frame project. `usvg` parses the file and resolves what SVG
//! leaves relative (`use`, styles, units, `viewBox`), so every path arrives with its full
//! transform and this only converts geometry and paint into shapes.
//!
//! Imported: paths (and the basic shapes, which usvg turns into paths), solid fills and strokes
//! with opacity, linear gradients as two-colour gradients, groups (flattened into one layer,
//! their transforms and opacity applied). Reported and skipped: radial gradients (first stop
//! colour), patterns, dashes, text, pictures, clips, masks and filters.

use std::num::NonZeroU32;

use flare_core::{DocumentId, DocumentPoint, LayerId, Rgba8, SceneId, ShapeId};
use flare_document::{
    Cap, Content, Document, FrameSpan, Gradient, Join, Layer, MAX_ELEMENTS, MAX_STAGE_SIDE,
    PathSegment, Scene, Shape, Stage, Stroke, Timeline,
};
use usvg::tiny_skia_path::{PathSegment as Seg, Point};
use usvg::{Node, Paint, Transform};

use crate::xfl::{Diagnostic, Imported, Severity};

/// Most bytes of SVG text read.
const MAX_SVG_BYTES: usize = 32 * 1024 * 1024;
/// Most group levels followed; a deeper file is cut off, not recursed into.
const MAX_DEPTH: usize = 64;

struct Walk {
    shapes: Vec<Content>,
    notes: Vec<Diagnostic>,
    skipped: [bool; 4],
}

pub(crate) fn import(bytes: &[u8]) -> Result<Imported, String> {
    if bytes.len() > MAX_SVG_BYTES {
        return Err("the SVG is too large".into());
    }
    // A drawing may not read other files or addresses; embedded pictures are skipped.
    let options = usvg::Options {
        image_href_resolver: usvg::ImageHrefResolver {
            resolve_data: Box::new(|_, _, _| None),
            resolve_string: Box::new(|_, _| None),
        },
        ..usvg::Options::default()
    };
    let tree = usvg::Tree::from_data(bytes, &options).map_err(|e| e.to_string())?;
    let side = |v: f32| {
        let v = f64::from(v).ceil();
        if v.is_finite() && v >= 1.0 && v <= f64::from(MAX_STAGE_SIDE) {
            Ok(v as u32)
        } else {
            Err(format!("a size of {v} is outside the supported stage size"))
        }
    };
    let size = tree.size();
    let stage = Stage::new(side(size.width())?, side(size.height())?, Rgba8::WHITE)
        .map_err(|e| e.to_string())?;
    let mut walk = Walk {
        shapes: Vec::new(),
        notes: Vec::new(),
        skipped: [false; 4],
    };
    walk.group(tree.root(), 1.0, 0);
    let mut contents = walk.shapes.into_iter();
    let spans = match contents.next() {
        Some(first) => {
            let span = FrameSpan::new(0, NonZeroU32::MIN, first)
                .with_extras(contents.collect())
                .map_err(|e| e.to_string())?;
            vec![span]
        }
        None => Vec::new(),
    };
    let layer = Layer::new(LayerId::new(), "SVG", spans).map_err(|e| e.to_string())?;
    let scene = Scene::new(SceneId::new(), "Scene 1", Timeline::new(vec![layer]));
    let rate = crate::frame_rate("24")?;
    let document = Document::new(DocumentId::new(), rate, vec![scene], Vec::new())
        .map_err(|e| format!("{e:?}"))?
        .with_stage(stage);
    Ok(Imported {
        document,
        diagnostics: walk.notes,
    })
}

impl Walk {
    fn skip(&mut self, which: usize, severity: Severity, message: &str) {
        if !self.skipped[which] {
            self.skipped[which] = true;
            self.notes.push(Diagnostic {
                severity,
                location: "svg".into(),
                message: message.into(),
            });
        }
    }

    fn group(&mut self, group: &usvg::Group, opacity: f32, depth: usize) {
        if depth > MAX_DEPTH {
            self.skip(
                3,
                Severity::Warning,
                "groups nested too deeply were skipped",
            );
            return;
        }
        let opacity = opacity * group.opacity().get();
        if group.clip_path().is_some() || group.mask().is_some() || !group.filters().is_empty() {
            self.skip(
                0,
                Severity::Unsupported,
                "clips, masks and filters are not imported; the art is shown unclipped",
            );
        }
        for node in group.children() {
            if self.shapes.len() >= MAX_ELEMENTS {
                self.skip(
                    1,
                    Severity::Warning,
                    "shapes past the element limit were skipped",
                );
                return;
            }
            match node {
                Node::Group(g) => self.group(g, opacity, depth + 1),
                Node::Path(p) => self.path(p, opacity),
                Node::Image(_) => self.skip(2, Severity::Unsupported, "pictures are not imported"),
                Node::Text(_) => self.skip(2, Severity::Unsupported, "text is not imported"),
            }
        }
    }

    fn path(&mut self, path: &usvg::Path, opacity: f32) {
        if !path.is_visible() {
            return;
        }
        let ts = path.abs_transform();
        let Some(segments) = convert_path(path.data(), ts) else {
            return;
        };
        let mut gradient = None;
        let fill = match path.fill() {
            Some(f) => {
                let (color, g) = self.paint(f.paint(), f.opacity().get() * opacity, ts);
                gradient = g;
                color
            }
            None => Rgba8::new(0, 0, 0, 0),
        };
        let Ok(mut shape) = Shape::new(ShapeId::new(), segments, fill) else {
            return;
        };
        if let Some(g) = gradient {
            shape = shape.with_gradient(g);
        }
        if let Some(s) = path.stroke() {
            let (color, _) = self.paint(s.paint(), 1.0, ts);
            let scale = (f64::from(ts.sx * ts.sy - ts.kx * ts.ky)).abs().sqrt();
            let width = (f64::from(s.width().get()) * scale).max(0.01);
            if let Ok(stroke) = Stroke::new_with_opacity(
                width,
                color,
                f64::from(s.opacity().get() * opacity).clamp(0.0, 1.0),
            ) {
                shape = shape.with_stroke(
                    stroke
                        .with_cap(cap(s.linecap()))
                        .with_join(join(s.linejoin())),
                );
            }
            if s.dasharray().is_some() {
                self.skip(
                    0,
                    Severity::Unsupported,
                    "dashes are not imported; lines are solid",
                );
            }
        }
        self.shapes.push(Content::Shape(shape));
    }

    fn paint(&mut self, paint: &Paint, alpha: f32, ts: Transform) -> (Rgba8, Option<Gradient>) {
        let color = |c: usvg::Color, a: f32| {
            Rgba8::new(
                c.red,
                c.green,
                c.blue,
                (a.clamp(0.0, 1.0) * 255.0).round() as u8,
            )
        };
        let stops = |s: &[usvg::Stop]| match (s.first(), s.last()) {
            (Some(a), Some(b)) => Some((
                color(a.color(), a.opacity().get() * alpha),
                color(b.color(), b.opacity().get() * alpha),
            )),
            _ => None,
        };
        match paint {
            Paint::Color(c) => (color(*c, alpha), None),
            Paint::LinearGradient(g) => {
                let Some((a, b)) = stops(g.stops()) else {
                    return (Rgba8::new(0, 0, 0, 0), None);
                };
                let map = ts.pre_concat(g.transform());
                let point = |x: f32, y: f32| {
                    let mut p = Point::from_xy(x, y);
                    map.map_point(&mut p);
                    DocumentPoint::new(f64::from(p.x), f64::from(p.y)).ok()
                };
                let made = point(g.x1(), g.y1())
                    .zip(point(g.x2(), g.y2()))
                    .and_then(|(p, q)| Gradient::linear(p, q, a, b).ok());
                (a, made)
            }
            Paint::RadialGradient(g) => {
                self.skip(
                    0,
                    Severity::Unsupported,
                    "radial gradients are not imported; the first stop colour is used",
                );
                (
                    stops(g.stops()).map_or(Rgba8::new(0, 0, 0, 0), |(a, _)| a),
                    None,
                )
            }
            Paint::Pattern(_) => {
                self.skip(0, Severity::Unsupported, "patterns are not imported");
                (Rgba8::new(0, 0, 0, 0), None)
            }
        }
    }
}

fn cap(c: usvg::LineCap) -> Cap {
    match c {
        usvg::LineCap::Butt => Cap::Butt,
        usvg::LineCap::Round => Cap::Round,
        usvg::LineCap::Square => Cap::Square,
    }
}

fn join(j: usvg::LineJoin) -> Join {
    match j {
        usvg::LineJoin::Round => Join::Round,
        usvg::LineJoin::Bevel => Join::Bevel,
        _ => Join::Miter,
    }
}

/// The path in document space; `None` when a point is not a finite number.
fn convert_path(path: &usvg::tiny_skia_path::Path, ts: Transform) -> Option<Vec<PathSegment>> {
    let pt = |p: Point| {
        let mut p = p;
        ts.map_point(&mut p);
        DocumentPoint::new(f64::from(p.x), f64::from(p.y)).ok()
    };
    let mut out = Vec::new();
    let mut last = Point::zero();
    for seg in path.segments() {
        match seg {
            Seg::MoveTo(p) => {
                out.push(PathSegment::MoveTo(pt(p)?));
                last = p;
            }
            Seg::LineTo(p) => {
                out.push(PathSegment::LineTo(pt(p)?));
                last = p;
            }
            Seg::QuadTo(c, p) => {
                // A quadratic as the cubic with the same curve.
                let c1 = Point::from_xy(
                    last.x + 2.0 / 3.0 * (c.x - last.x),
                    last.y + 2.0 / 3.0 * (c.y - last.y),
                );
                let c2 =
                    Point::from_xy(p.x + 2.0 / 3.0 * (c.x - p.x), p.y + 2.0 / 3.0 * (c.y - p.y));
                out.push(PathSegment::CubicTo(pt(c1)?, pt(c2)?, pt(p)?));
                last = p;
            }
            Seg::CubicTo(a, b, p) => {
                out.push(PathSegment::CubicTo(pt(a)?, pt(b)?, pt(p)?));
                last = p;
            }
            Seg::Close => out.push(PathSegment::Close),
        }
    }
    Some(out)
}

#[cfg(test)]
#[path = "svg_tests.rs"]
mod tests;
