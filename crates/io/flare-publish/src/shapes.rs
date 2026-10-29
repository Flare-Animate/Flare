//! A Flare shape as a SWF `DefineShape4`.
//!
//! SWF stores an outline as edges that each say which fill lies on their left and right.
//! Which side of a contour is filled depends on the whole shape (a hole's material is
//! outside its contour), so it is decided from the geometry by probing both sides of each
//! contour with a non-zero winding test, not assumed from the contour's direction.

use flare_document::{PathSegment, Shape};
use flare_export::curves::{self, Point};
use swf::{
    CharacterId, Color, FillStyle, LineStyle, PointDelta, Rectangle, ShapeFlag, ShapeRecord,
    ShapeStyles, StyleChangeData, Twips,
};

const TOLERANCE_PX: f64 = 0.05;
const FLATTEN_STEPS: usize = 16;
/// How far from a contour, in pixels, the fill-side probes sit.
const PROBE_PX: f64 = 0.01;
const LIMIT: f64 = 1.0e9;

/// One piece of a contour: a line or a quadratic curve ending at `to`.
#[derive(Clone, Copy)]
enum Edge {
    Line { to: Point },
    Quad { control: Point, to: Point },
}

struct Contour {
    start: Point,
    edges: Vec<Edge>,
    closed: bool,
}

fn px(p: &flare_core::DocumentPoint) -> Point {
    (p.as_vec().x, p.as_vec().y)
}

/// Splits the path into contours, approximating cubics by quadratics.
fn contours(shape: &Shape, approximated: &mut bool) -> Vec<Contour> {
    let mut out: Vec<Contour> = Vec::new();
    let mut at = (0.0, 0.0);
    for segment in shape.path() {
        match segment {
            PathSegment::MoveTo(p) => {
                at = px(p);
                out.push(Contour {
                    start: at,
                    edges: Vec::new(),
                    closed: false,
                });
            }
            PathSegment::LineTo(p) => {
                at = px(p);
                if let Some(c) = out.last_mut() {
                    c.edges.push(Edge::Line { to: at });
                }
            }
            PathSegment::CubicTo(a, b, c) => {
                let cubic = [at, px(a), px(b), px(c)];
                let quads = curves::to_quads(cubic, TOLERANCE_PX);
                *approximated |= quads.len() > 1 || cubic_is_curved(cubic);
                if let Some(contour) = out.last_mut() {
                    for (control, to) in quads {
                        contour.edges.push(Edge::Quad { control, to });
                    }
                }
                at = cubic[3];
            }
            PathSegment::Close => {
                if let Some(c) = out.last_mut() {
                    c.closed = true;
                    at = c.start;
                }
            }
        }
    }
    out.retain(|c| !c.edges.is_empty());
    out
}

fn cubic_is_curved(c: [Point; 4]) -> bool {
    let dx = c[3].0 - 3.0 * c[2].0 + 3.0 * c[1].0 - c[0].0;
    let dy = c[3].1 - 3.0 * c[2].1 + 3.0 * c[1].1 - c[0].1;
    dx.hypot(dy) > 1e-9
}

/// The contour as a polygon (curves flattened), for winding tests.
fn polygon(contour: &Contour) -> Vec<Point> {
    let mut points = vec![contour.start];
    let mut at = contour.start;
    for edge in &contour.edges {
        match *edge {
            Edge::Line { to } => {
                points.push(to);
                at = to;
            }
            Edge::Quad { control, to } => {
                for step in 1..=FLATTEN_STEPS {
                    let t = step as f64 / FLATTEN_STEPS as f64;
                    let u = 1.0 - t;
                    points.push((
                        u * u * at.0 + 2.0 * u * t * control.0 + t * t * to.0,
                        u * u * at.1 + 2.0 * u * t * control.1 + t * t * to.1,
                    ));
                }
                at = to;
            }
        }
    }
    points
}

/// Non-zero winding number of `p` against every polygon, each implicitly closed.
fn winding(polygons: &[Vec<Point>], p: Point) -> i32 {
    let mut total = 0;
    for poly in polygons {
        for i in 0..poly.len() {
            let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
            let cross = (b.0 - a.0) * (p.1 - a.1) - (p.0 - a.0) * (b.1 - a.1);
            if a.1 <= p.1 && b.1 > p.1 && cross > 0.0 {
                total += 1;
            } else if a.1 > p.1 && b.1 <= p.1 && cross < 0.0 {
                total -= 1;
            }
        }
    }
    total
}

/// Which side of travel along `contour` is filled: (left, right).
fn filled_sides(contour: &Contour, all: &[Vec<Point>]) -> (bool, bool) {
    let poly = polygon(contour);
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let length = dx.hypot(dy);
        if length < 1e-6 {
            continue;
        }
        let mid = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
        // With y pointing down, the right-hand side of travel is (-dy, dx).
        let (nx, ny) = (-dy / length, dx / length);
        let right = (mid.0 + nx * PROBE_PX, mid.1 + ny * PROBE_PX);
        let left = (mid.0 - nx * PROBE_PX, mid.1 - ny * PROBE_PX);
        return (winding(all, left) != 0, winding(all, right) != 0);
    }
    (false, false)
}

fn twips(value: f64) -> i32 {
    (value * f64::from(Twips::TWIPS_PER_PIXEL))
        .round()
        .clamp(-LIMIT, LIMIT) as i32
}

struct Bounds {
    min: (i32, i32),
    max: (i32, i32),
    any: bool,
}

impl Bounds {
    fn add(&mut self, p: (i32, i32)) {
        if self.any {
            self.min = (self.min.0.min(p.0), self.min.1.min(p.1));
            self.max = (self.max.0.max(p.0), self.max.1.max(p.1));
        } else {
            (self.min, self.max, self.any) = (p, p, true);
        }
    }

    fn rect(&self, grow: i32) -> Rectangle<Twips> {
        Rectangle {
            x_min: Twips::new(self.min.0.saturating_sub(grow)),
            x_max: Twips::new(self.max.0.saturating_add(grow)),
            y_min: Twips::new(self.min.1.saturating_sub(grow)),
            y_max: Twips::new(self.max.1.saturating_add(grow)),
        }
    }
}

fn color(c: flare_core::Rgba8) -> Color {
    Color {
        r: c.r,
        g: c.g,
        b: c.b,
        a: c.a,
    }
}

/// The shape as a `DefineShape4`, or `None` when nothing would be drawn.
pub fn define_shape(id: CharacterId, shape: &Shape, approximated: &mut bool) -> Option<swf::Shape> {
    let fill = shape.flat_fill();
    let has_fill = fill.a > 0;
    let stroke = shape.stroke();
    if !has_fill && stroke.is_none() {
        return None;
    }
    let contours = contours(shape, approximated);
    if contours.is_empty() {
        return None;
    }
    let polygons: Vec<Vec<Point>> = contours.iter().map(polygon).collect();

    let mut styles = ShapeStyles {
        fill_styles: Vec::new(),
        line_styles: Vec::new(),
    };
    if has_fill {
        styles.fill_styles.push(FillStyle::Color(color(fill)));
    }
    let mut grow = 0;
    if let Some(stroke) = stroke {
        let width = Twips::new(twips(stroke.width()).max(1));
        grow = width.get() / 2 + 1;
        styles.line_styles.push(
            LineStyle::new()
                .with_width(width)
                .with_color(color(stroke.color()))
                .with_start_cap(cap_style(stroke.cap()))
                .with_end_cap(cap_style(stroke.cap()))
                .with_join_style(join_style(stroke.join())),
        );
    }
    let line = u32::from(stroke.is_some());

    let (mut records, mut bounds) = (
        Vec::new(),
        Bounds {
            min: (0, 0),
            max: (0, 0),
            any: false,
        },
    );
    for contour in &contours {
        let start = (twips(contour.start.0), twips(contour.start.1));
        bounds.add(start);
        let (left, right) = if has_fill {
            filled_sides(contour, &polygons)
        } else {
            (false, false)
        };
        // An edge between two filled regions (or none) carries no fill of its own.
        let (fill0, fill1) = match (left, right) {
            (true, false) => (1, 0),
            (false, true) => (0, 1),
            _ => (0, 0),
        };
        records.push(ShapeRecord::StyleChange(Box::new(StyleChangeData {
            move_to: Some(swf::Point {
                x: Twips::new(start.0),
                y: Twips::new(start.1),
            }),
            fill_style_0: Some(fill0),
            fill_style_1: Some(fill1),
            line_style: Some(line),
            new_styles: None,
        })));
        let mut cur = start;
        let mut delta = |to: (f64, f64), bounds: &mut Bounds| {
            let next = (twips(to.0), twips(to.1));
            bounds.add(next);
            let d = (next.0.saturating_sub(cur.0), next.1.saturating_sub(cur.1));
            cur = next;
            PointDelta {
                dx: Twips::new(d.0),
                dy: Twips::new(d.1),
            }
        };
        for edge in &contour.edges {
            records.push(match *edge {
                Edge::Line { to } => ShapeRecord::StraightEdge {
                    delta: delta(to, &mut bounds),
                },
                Edge::Quad { control, to } => ShapeRecord::CurvedEdge {
                    control_delta: delta(control, &mut bounds),
                    anchor_delta: delta(to, &mut bounds),
                },
            });
        }
        // A closed contour ends with an explicit closing edge (stroked, like the rest). A fill
        // must also be closed, so an open filled contour is closed with an unstroked edge.
        if cur != start && (contour.closed || has_fill) {
            if !contour.closed && stroke.is_some() {
                records.push(ShapeRecord::StyleChange(Box::new(StyleChangeData {
                    move_to: None,
                    fill_style_0: None,
                    fill_style_1: None,
                    line_style: Some(0),
                    new_styles: None,
                })));
            }
            let closing = PointDelta {
                dx: Twips::new(start.0.saturating_sub(cur.0)),
                dy: Twips::new(start.1.saturating_sub(cur.1)),
            };
            records.push(ShapeRecord::StraightEdge { delta: closing });
        }
    }
    Some(swf::Shape {
        version: 4,
        id,
        shape_bounds: bounds.rect(grow),
        edge_bounds: bounds.rect(0),
        flags: ShapeFlag::HAS_SCALING_STROKES | ShapeFlag::NON_ZERO_WINDING_RULE,
        styles,
        shape: records,
    })
}

fn cap_style(cap: flare_document::Cap) -> swf::LineCapStyle {
    match cap {
        flare_document::Cap::Butt => swf::LineCapStyle::None,
        flare_document::Cap::Round => swf::LineCapStyle::Round,
        flare_document::Cap::Square => swf::LineCapStyle::Square,
    }
}

fn join_style(join: flare_document::Join) -> swf::LineJoinStyle {
    match join {
        flare_document::Join::Round => swf::LineJoinStyle::Round,
        flare_document::Join::Bevel => swf::LineJoinStyle::Bevel,
        // A miter limit of 3 is Flash's own default.
        flare_document::Join::Miter => swf::LineJoinStyle::Miter(swf::Fixed8::from_f32(3.0)),
    }
}
