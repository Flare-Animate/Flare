//! Paper.js paths as Flare shapes. A Wick path is `["Path", props]`, or `["CompoundPath", props]`
//! whose children are inline paths. A segment is a point, or `[point, handleIn, handleOut]` with
//! the handles relative to the point. `matrix`, when present, is the path's transform; without
//! one the points are already in the parent's space.

use flare_core::{DocumentPoint, Rgba8};
use flare_document::{Cap, Join, MAX_SEGMENTS, PathSegment, Stroke};
use glamx::{DAffine2, DVec2};
use serde_json::Value;

use super::color::{Paint, paint};
use super::error::WickError;
use super::json::{array, number};
use super::limits::MAX_COORDINATE;

/// One point of a path with its handles, all in the path's own space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Anchor {
    point: DVec2,
    into: DVec2,
    out: DVec2,
}

/// A path ready to be a shape: its segments in the document's space, the solid fill, the stroke,
/// and whether it used a gradient (not imported).
pub(super) struct Drawn {
    pub segments: Vec<PathSegment>,
    pub fill: Option<Rgba8>,
    pub stroke: Option<Stroke>,
    pub gradient: bool,
}

/// The kind and the properties of a Paper.js item.
pub(super) fn split(json: &Value) -> Result<(&str, &Value), WickError> {
    match json.as_array().map(Vec::as_slice) {
        Some([Value::String(kind), props @ Value::Object(_)]) => Ok((kind, props)),
        _ => Err(WickError::BadPath("is not a [kind, properties] pair")),
    }
}

/// The item's transform: its `matrix` as `[a, b, c, d, e, f]`, or the identity.
pub(super) fn matrix(props: &Value) -> Result<DAffine2, WickError> {
    let Some(list) = array(props, "matrix") else {
        return Ok(DAffine2::IDENTITY);
    };
    let numbers: Option<Vec<f64>> = list.iter().map(Value::as_f64).collect();
    match numbers.as_deref() {
        Some([a, b, c, d, e, f])
            if [a, b, c, d, e, f]
                .iter()
                .all(|v| v.is_finite() && v.abs() <= MAX_COORDINATE) =>
        {
            Ok(DAffine2::from_cols_array(&[*a, *b, *c, *d, *e, *f]))
        }
        _ => Err(WickError::BadPath("has a matrix that is not six numbers")),
    }
}

/// The path's segments as anchors. A segment is a point, or a point with its two handles.
pub(super) fn anchors(segments: &[Value]) -> Result<Vec<Anchor>, WickError> {
    if segments.len() > MAX_SEGMENTS {
        return Err(WickError::TooLarge("a path's segments"));
    }
    segments
        .iter()
        .map(|segment| match segment.as_array().map(Vec::as_slice) {
            Some([_, _]) => Ok(Anchor {
                point: point(segment)?,
                into: DVec2::ZERO,
                out: DVec2::ZERO,
            }),
            Some([p, into, out]) => Ok(Anchor {
                point: point(p)?,
                into: point(into)?,
                out: point(out)?,
            }),
            _ => Err(WickError::BadPath(
                "has a segment that is not a point with handles",
            )),
        })
        .collect()
}

fn point(value: &Value) -> Result<DVec2, WickError> {
    let pair = value
        .as_array()
        .map(Vec::as_slice)
        .and_then(|p| match p {
            [x, y] => Some((x.as_f64()?, y.as_f64()?)),
            _ => None,
        })
        .ok_or(WickError::BadPath("has a point that is not two numbers"))?;
    let (x, y) = pair;
    if !x.is_finite() || !y.is_finite() || x.abs() > MAX_COORDINATE || y.abs() > MAX_COORDINATE {
        return Err(WickError::BadPath("has a point out of range"));
    }
    Ok(DVec2::new(x, y))
}

/// The segments of a closed or open run of anchors, transformed by `m`. Two anchors with no handles
/// between them make a straight line; anything else a cubic curve.
pub(super) fn segments(
    anchors: &[Anchor],
    closed: bool,
    m: DAffine2,
) -> Result<Vec<PathSegment>, WickError> {
    let (Some(first), Some(last)) = (anchors.first(), anchors.last()) else {
        return Ok(Vec::new());
    };
    let mut out = vec![PathSegment::MoveTo(doc_point(
        m.transform_point2(first.point),
    )?)];
    for pair in anchors.windows(2) {
        push_curve(&mut out, &pair[0], &pair[1], m)?;
    }
    if closed {
        push_curve(&mut out, last, first, m)?;
        out.push(PathSegment::Close);
    }
    Ok(out)
}

fn push_curve(
    out: &mut Vec<PathSegment>,
    a: &Anchor,
    b: &Anchor,
    m: DAffine2,
) -> Result<(), WickError> {
    let end = doc_point(m.transform_point2(b.point))?;
    if a.out == DVec2::ZERO && b.into == DVec2::ZERO {
        out.push(PathSegment::LineTo(end));
    } else {
        let c1 = doc_point(m.transform_point2(a.point + a.out))?;
        let c2 = doc_point(m.transform_point2(b.point + b.into))?;
        out.push(PathSegment::CubicTo(c1, c2, end));
    }
    Ok(())
}

fn doc_point(p: DVec2) -> Result<DocumentPoint, WickError> {
    DocumentPoint::new(p.x, p.y).map_err(|_| WickError::BadPath("has a point that is not finite"))
}

/// Reads a path or a compound path. `parent` is the transform of everything it sits in. A path
/// with no segments gives `None`.
pub(super) fn read(json: &Value, parent: DAffine2) -> Result<Option<Drawn>, WickError> {
    let (kind, props) = split(json)?;
    let total = parent * matrix(props)?;
    let segs = match kind {
        "Path" => {
            let list = array(props, "segments").map_or(&[][..], Vec::as_slice);
            let closed = props
                .get("closed")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            segments(&anchors(list)?, closed, total)?
        }
        "CompoundPath" => {
            let mut all = Vec::new();
            for child in array(props, "children").map_or(&[][..], Vec::as_slice) {
                if let Some(drawn) = read(child, total)? {
                    all.extend(drawn.segments);
                    if all.len() > MAX_SEGMENTS {
                        return Err(WickError::TooLarge("a path's segments"));
                    }
                }
            }
            all
        }
        _ => return Err(WickError::BadPath("is not a path")),
    };
    if segs.is_empty() {
        return Ok(None);
    }
    Ok(Some(finish(segs, props)))
}

fn finish(segments: Vec<PathSegment>, props: &Value) -> Drawn {
    let fill_paint = paint(props.get("fillColor"));
    let stroke_paint = paint(props.get("strokeColor"));
    let stroke = match (stroke_paint, number(props, "strokeWidth")) {
        (Paint::Solid(color), Some(width)) => Stroke::new(width, color).ok().map(|s| {
            s.with_cap(cap(props.get("strokeCap").and_then(Value::as_str)))
                .with_join(join(props.get("strokeJoin").and_then(Value::as_str)))
        }),
        _ => None,
    };
    Drawn {
        segments,
        fill: match fill_paint {
            Paint::Solid(color) => Some(color),
            _ => None,
        },
        stroke,
        gradient: fill_paint == Paint::Other || stroke_paint == Paint::Other,
    }
}

fn cap(name: Option<&str>) -> Cap {
    match name {
        Some("round") => Cap::Round,
        Some("square") => Cap::Square,
        _ => Cap::Butt,
    }
}

fn join(name: Option<&str>) -> Join {
    match name {
        Some("round") => Join::Round,
        Some("bevel") => Join::Bevel,
        _ => Join::Miter,
    }
}

/// Every anchor of the path, in the document's space, for measuring a clip's bounds.
pub(super) fn points(
    json: &Value,
    parent: DAffine2,
    out: &mut Vec<DVec2>,
) -> Result<(), WickError> {
    let (kind, props) = split(json)?;
    let total = parent * matrix(props)?;
    match kind {
        "Path" => {
            for anchor in anchors(array(props, "segments").map_or(&[][..], Vec::as_slice))? {
                out.push(total.transform_point2(anchor.point));
            }
        }
        "CompoundPath" => {
            for child in array(props, "children").map_or(&[][..], Vec::as_slice) {
                points(child, total, out)?;
            }
        }
        _ => {}
    }
    Ok(())
}
