//! Where an area the Paint Bucket fills goes: under the lines that close it, so they still show
//! whole over it.

use std::num::NonZeroU32;

use flare_commands::Command;
use flare_core::{DocumentPoint, SceneId, ShapeId};
use flare_document::{Content, Document, PathSegment, Shape, Tween};

use super::area::Outline;
use super::{Fill, gradient_across};
use crate::common::*;
use crate::drawing::{DrawPlan, DrawRefusal, Target, draw_shapes};

/// A filled shape of the area `outlines` trace, mapped onto the stage by `to_stage`, painted
/// with `fill`. `None` when there is nothing to fill or a point does not land on the stage.
pub fn area_shape(
    outlines: &[Outline],
    to_stage: impl Fn((f64, f64)) -> (f64, f64),
    fill: Fill,
) -> Option<Shape> {
    let mut path = Vec::new();
    for outline in outlines {
        let mut points = outline.iter().map(|p| {
            let (x, y) = to_stage(*p);
            DocumentPoint::new(x, y).ok()
        });
        path.push(PathSegment::MoveTo(points.next()??));
        for point in points {
            path.push(PathSegment::LineTo(point?));
        }
        path.push(PathSegment::Close);
    }
    let start = match fill {
        Fill::Solid(color) => color,
        Fill::Linear { start, .. } => start,
    };
    let shape = Shape::new(ShapeId::new(), path, start).ok()?;
    Some(match fill {
        Fill::Solid(_) => shape,
        Fill::Linear {
            start,
            end,
            vertical,
        } => match gradient_across(&shape, start, end, vertical) {
            Some(gradient) => shape.with_gradient(gradient),
            None => shape,
        },
    })
}

/// The commands that put `shape` just above the element `at` names, in its span: the area
/// filled over a shape is drawn over it and under whatever drew its edges. `None` when there is
/// no such element or the span cannot take another (a tween, or too many elements).
pub fn pour_above(
    document: &Document,
    scene: SceneId,
    at: SpanRef,
    shape: Shape,
) -> Option<Vec<Command>> {
    let span = find_span(document, scene, at)?;
    if span.tween() != Tween::None || at.element >= span.element_count() {
        return None;
    }
    let mut elements: Vec<Content> = span.elements().cloned().collect();
    elements.insert(at.element + 1, Content::Shape(shape));
    let grown = span.with_elements(elements).ok().flatten()?;
    Some(replace_span(scene, at, grown))
}

/// The commands that draw `shape` under everything in `target`'s keyframe at `frame`, by the
/// rules the drawing tools follow ([`draw_shapes`]), so an area filled on empty stage sits under
/// the lines around it.
pub fn pour_under(
    document: &Document,
    scene: SceneId,
    target: Target,
    frame: u32,
    shape: Shape,
    new_layer: &str,
) -> Result<DrawPlan, DrawRefusal> {
    let duration: Option<NonZeroU32> = None;
    let mut plan = draw_shapes(
        document,
        scene,
        target,
        frame,
        duration,
        vec![shape],
        new_layer,
    )?;
    let Some(placed) = plan.placed.first().copied() else {
        return Ok(plan);
    };
    if placed.element > 0
        && let Some(Command::InsertSpan { span, .. }) = plan.commands.last_mut()
    {
        *span = span
            .with_element_moved(placed.element, 0)
            .ok_or(DrawRefusal::Held)?;
        plan.placed = vec![SpanRef {
            element: 0,
            ..placed
        }];
    }
    Ok(plan)
}
