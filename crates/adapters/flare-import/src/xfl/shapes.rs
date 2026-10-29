//! Drawn shapes: contours and fills turned into document shapes.

use crate::edges::{self, Contour, Segment};
use crate::xml::Node;
use flare_core::{Rgba8, ShapeId};
use flare_document::{Content, PathSegment, Shape, Stroke};
use std::collections::{BTreeMap, HashMap};

use super::*;

impl Importer {
    pub(super) fn shape(&mut self, node: &Node, place: &str) -> Vec<Content> {
        let mut fills: HashMap<u32, Option<Rgba8>> = HashMap::new();
        for style in node
            .child("fills")
            .into_iter()
            .flat_map(|f| f.children_named("FillStyle"))
        {
            let Some(index) = style.attr("index").and_then(|v| v.parse().ok()) else {
                continue;
            };
            let color = style
                .child("SolidColor")
                .and_then(|c| color_from(c.attr("color").unwrap_or("#000000"), c.attr("alpha")));
            if color.is_none() {
                self.note(
                    Severity::Unsupported,
                    place,
                    "gradient and bitmap fills are not imported",
                );
            }
            fills.insert(index, color);
        }
        let mut strokes: HashMap<u32, Option<(f64, Rgba8)>> = HashMap::new();
        for style in node
            .child("strokes")
            .into_iter()
            .flat_map(|s| s.children_named("StrokeStyle"))
        {
            let Some(index) = style.attr("index").and_then(|v| v.parse().ok()) else {
                continue;
            };
            let solid = style.child("SolidStroke");
            let color = solid
                .and_then(|s| s.child("fill"))
                .and_then(|f| f.child("SolidColor"))
                .and_then(|c| color_from(c.attr("color").unwrap_or("#000000"), c.attr("alpha")));
            let weight = solid.map_or(Some(1.0), |s| {
                s.attr("weight")
                    .map_or(Some(1.0), |w| w.parse::<f64>().ok())
            });
            let resolved = color.zip(weight).map(|(c, w)| (w, c));
            if resolved.is_none() {
                self.note(
                    Severity::Unsupported,
                    place,
                    "only solid strokes are imported",
                );
            }
            strokes.insert(index, resolved);
        }

        let mut fill_edges: BTreeMap<u32, Vec<Segment>> = BTreeMap::new();
        let mut stroke_edges: BTreeMap<u32, Vec<Segment>> = BTreeMap::new();
        for edge in node
            .child("edges")
            .into_iter()
            .flat_map(|e| e.children_named("Edge"))
        {
            let segments = match edges::decode_edges(edge.attr("edges").unwrap_or_default()) {
                Ok(segments) => segments,
                Err(error) => {
                    self.note(
                        Severity::Unsupported,
                        place,
                        format!("an edge with bad data was skipped ({error})"),
                    );
                    continue;
                }
            };
            let index = |key: &str| {
                edge.attr(key)
                    .and_then(|v| v.parse::<u32>().ok())
                    .filter(|i| *i > 0)
            };
            // The right-hand fill follows the edge direction, the left-hand one is reversed so
            // each fill's boundary is oriented consistently.
            if let Some(i) = index("fillStyle1") {
                fill_edges
                    .entry(i)
                    .or_default()
                    .extend(segments.iter().copied());
            }
            if let Some(i) = index("fillStyle0") {
                fill_edges
                    .entry(i)
                    .or_default()
                    .extend(segments.iter().rev().map(Segment::reversed));
            }
            if let Some(i) = index("strokeStyle") {
                stroke_edges
                    .entry(i)
                    .or_default()
                    .extend(segments.iter().copied());
            }
        }

        let mut out = Vec::new();
        for (index, segments) in fill_edges {
            let Some(Some(color)) = fills.get(&index).copied() else {
                continue;
            };
            let contours = edges::chain(&segments);
            if contours.iter().any(|c| !c.closed) {
                self.note(Severity::Info, place, "an open fill outline was closed");
            }
            self.push_shape(&mut out, &contours, color, None, true, place);
        }
        for (index, segments) in stroke_edges {
            let Some(Some((weight, color))) = strokes.get(&index).copied() else {
                continue;
            };
            let contours = edges::chain(&segments);
            self.push_shape(
                &mut out,
                &contours,
                Rgba8::TRANSPARENT,
                Some((weight, color)),
                false,
                place,
            );
        }
        out
    }

    pub(super) fn push_shape(
        &mut self,
        out: &mut Vec<Content>,
        contours: &[Contour],
        fill: Rgba8,
        stroke: Option<(f64, Rgba8)>,
        close: bool,
        place: &str,
    ) {
        // Edge coordinates are twips; document units are pixels.
        let point = |(x, y): edges::Point| flare_core::DocumentPoint::new(x / 20.0, y / 20.0).ok();
        let mut path = Vec::new();
        for contour in contours {
            let Some(first) = contour.segments.first().and_then(|s| point(s.from())) else {
                continue;
            };
            path.push(PathSegment::MoveTo(first));
            for segment in &contour.segments {
                let converted = match *segment {
                    Segment::Line { to, .. } => point(to).map(PathSegment::LineTo),
                    Segment::Quad { from, ctrl, to } => {
                        let (c1, c2) = edges::quad_to_cubic(from, ctrl, to);
                        point(c1)
                            .zip(point(c2))
                            .zip(point(to))
                            .map(|((a, b), c)| PathSegment::CubicTo(a, b, c))
                    }
                };
                if let Some(c) = converted {
                    path.push(c);
                }
            }
            if close || contour.closed {
                path.push(PathSegment::Close);
            }
        }
        if path.is_empty() {
            return;
        }
        match Shape::new(ShapeId::new(), path, fill) {
            Ok(shape) => {
                let shape = match stroke {
                    Some((weight, color)) => match Stroke::new(weight, color) {
                        Ok(stroke) => shape.with_stroke(stroke),
                        Err(error) => {
                            self.note(
                                Severity::Unsupported,
                                place,
                                format!("a stroke was skipped ({error})"),
                            );
                            return;
                        }
                    },
                    None => shape,
                };
                out.push(Content::Shape(shape));
            }
            Err(error) => self.note(
                Severity::Unsupported,
                place,
                format!("a shape was skipped ({error})"),
            ),
        }
    }
}
