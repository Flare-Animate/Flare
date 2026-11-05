//! Reads the drawing of a vector layer: `mesh.points` (each with a `position`) and `mesh.curves`
//! (each a list of point indices and whether it closes). The layout follows the MIT-licensed
//! godot_moho_importer (jpdurigan), which reads the same files. Bezier handles and fills are not
//! read, so a curve comes out as a polyline. The fill colour is the first filled shape's
//! (`shapes[].style.fill_color`).

use serde_json::Value;

use crate::error::MohoError;
use crate::limits::{MAX_MESH_CURVES, MAX_MESH_POINTS};
use crate::model::{Mesh, MeshCurve};

/// `budget` counts the points and curves kept so far across the project.
pub(crate) fn read(
    value: Option<&Value>,
    budget: &mut (usize, usize),
) -> Result<Option<Mesh>, MohoError> {
    let Some(mesh) = value.and_then(Value::as_object) else {
        return Ok(None);
    };
    let mut out = Mesh::default();
    (out.fill, out.fills) = fills(mesh.get("shapes"));
    for point in mesh
        .get("points")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        budget.0 += 1;
        if budget.0 > MAX_MESH_POINTS {
            return Err(MohoError::Limit {
                what: "mesh points",
                limit: MAX_MESH_POINTS,
            });
        }
        out.points.push(position(point.get("position")));
    }
    for curve in mesh
        .get("curves")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        budget.1 += 1;
        if budget.1 > MAX_MESH_CURVES {
            return Err(MohoError::Limit {
                what: "mesh curves",
                limit: MAX_MESH_CURVES,
            });
        }
        let mut indexed: Vec<usize> = curve
            .get("points")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|p| p.get("point").and_then(Value::as_u64))
            .filter_map(|i| usize::try_from(i).ok())
            .filter(|i| *i < out.points.len())
            .collect();
        indexed.truncate(MAX_MESH_POINTS);
        out.curves.push(MeshCurve {
            closed: curve
                .get("closed")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            points: indexed,
        });
    }
    Ok(Some(out))
}

/// A position is `{x, y}`, or an animated channel whose first value is `{x, y}`.
fn position(value: Option<&Value>) -> [f64; 2] {
    let Some(value) = value else {
        return [0.0, 0.0];
    };
    let direct =
        |v: &Value| -> Option<[f64; 2]> { Some([v.get("x")?.as_f64()?, v.get("y")?.as_f64()?]) };
    direct(value)
        .or_else(|| value.get("val")?.as_array()?.first().and_then(direct))
        .filter(|p| p[0].is_finite() && p[1].is_finite())
        .unwrap_or([0.0, 0.0])
}

/// Bounded so a hostile file with many shapes cannot make the distinct-colour list grow.
const MAX_SHAPES: usize = 100_000;

/// The first filled shape's colour, and how many distinct colours the filled shapes have.
fn fills(shapes: Option<&Value>) -> (Option<[f64; 4]>, usize) {
    let mut first = None;
    let mut seen: Vec<[f64; 4]> = Vec::new();
    for shape in shapes
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .take(MAX_SHAPES)
    {
        if !shape
            .get("has_fill")
            .and_then(Value::as_bool)
            .unwrap_or(true)
        {
            continue;
        }
        let Some(color) = shape
            .get("style")
            .and_then(|s| s.get("fill_color"))
            .and_then(colour)
        else {
            continue;
        };
        first.get_or_insert(color);
        if !seen.contains(&color) {
            seen.push(color);
        }
    }
    (first, seen.len())
}

/// A colour is `{r, g, b, a}`, or an animated channel whose first value is one.
fn colour(value: &Value) -> Option<[f64; 4]> {
    let direct = |v: &Value| -> Option<[f64; 4]> {
        let c = |k: &str| v.get(k)?.as_f64().filter(|x| x.is_finite());
        Some([
            c("r")?.clamp(0.0, 1.0),
            c("g")?.clamp(0.0, 1.0),
            c("b")?.clamp(0.0, 1.0),
            c("a").unwrap_or(1.0).clamp(0.0, 1.0),
        ])
    };
    direct(value).or_else(|| value.get("val")?.as_array()?.first().and_then(direct))
}
