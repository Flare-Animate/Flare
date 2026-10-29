//! Undo, redo and committing drawn shapes.

use egui::Pos2;
use flare_commands::Command;
use flare_core::{LayerId, Rgba8, ShapeId};
use flare_document::{Content, FrameSpan, Layer, Shape, Stroke as ShapeStroke};
use std::num::NonZeroU32;

use super::*;

impl Workspace {
    // ---- editing ---------------------------------------------------------------------

    pub fn undo(&mut self) {
        if self.active_mut().undo() {
            self.set_frame(self.frame);
            self.validate_selection();
        }
    }

    pub fn redo(&mut self) {
        if self.active_mut().redo() {
            self.set_frame(self.frame);
            self.validate_selection();
        }
    }

    /// Commits a rectangle as one undo step: a new layer plus its shape span.
    pub fn commit_rectangle(&mut self, a: Pos2, b: Pos2) {
        self.commit_shape(Tool::Rectangle, a, b);
    }

    /// Commits the dragged shape for a drawing tool as one undo step: a new layer plus
    /// its shape span. Lines need a stroke, so they use the stroke settings even when
    /// the Outline checkbox is off.
    pub(super) fn commit_shape(&mut self, tool: Tool, a: Pos2, b: Pos2) {
        let (x0, x1) = (a.x.min(b.x), a.x.max(b.x));
        let (y0, y1) = (a.y.min(b.y), a.y.max(b.y));
        let (w, h) = (f64::from(x1 - x0), f64::from(y1 - y0));
        let too_small = match tool {
            Tool::Line | Tool::PolyStar => (b - a).length() < 1.0,
            _ => w < 1.0 || h < 1.0,
        };
        if too_small {
            return; // a click, not a drag
        }
        if self.document().scenes().is_empty() {
            self.report("This document has no scene");
            return;
        }
        let fill = {
            let c = self.fill.to_array();
            Rgba8::new(c[0], c[1], c[2], c[3])
        };
        let stroke = {
            let c = self.stroke_color.to_array();
            ShapeStroke::new(
                f64::from(self.stroke_width),
                Rgba8::new(c[0], c[1], c[2], c[3]),
            )
        };
        let id = ShapeId::new();
        let (x0, y0) = (f64::from(x0), f64::from(y0));
        let (shape, name) = match tool {
            Tool::Rectangle => (
                Shape::rounded_rect(id, x0, y0, w, h, f64::from(self.rect_radius), fill),
                "Rectangle",
            ),
            Tool::PolyStar => {
                let radius = f64::from((b - a).length());
                // The first corner points where the drag went.
                let rotation = f64::from((b.x - a.x).atan2(a.y - b.y));
                let inner = self.poly_star.then_some(f64::from(self.poly_inner));
                let center = (f64::from(a.x), f64::from(a.y));
                let name = if self.poly_star { "Star" } else { "Polygon" };
                (
                    Shape::polygon(id, center, radius, self.poly_sides, rotation, inner, fill),
                    name,
                )
            }
            Tool::Ellipse => (
                Shape::ellipse(id, x0 + w / 2.0, y0 + h / 2.0, w / 2.0, h / 2.0, fill),
                "Ellipse",
            ),
            Tool::Line => {
                let from = (f64::from(a.x), f64::from(a.y));
                let to = (f64::from(b.x), f64::from(b.y));
                match &stroke {
                    Ok(stroke) => (Shape::line(id, from, to, *stroke), "Line"),
                    Err(error) => return self.report(error.to_string()),
                }
            }
            _ => return,
        };
        let Some(mut shape) = shape else {
            self.report("Shape coordinates must be finite");
            return;
        };
        if self.gradient_enabled && tool != Tool::Line {
            let end = {
                let c = self.gradient_end.to_array();
                Rgba8::new(c[0], c[1], c[2], c[3])
            };
            let (from, to) = if self.gradient_vertical {
                ((x0, y0), (x0, y0 + h))
            } else {
                ((x0, y0), (x0 + w, y0))
            };
            let point = |(x, y): (f64, f64)| flare_core::DocumentPoint::new(x, y).ok();
            let gradient = point(from)
                .zip(point(to))
                .and_then(|(from, to)| flare_document::Gradient::linear(from, to, fill, end).ok());
            match gradient {
                Some(gradient) => shape = shape.with_gradient(gradient),
                None => return self.report("The gradient needs a shape with some size"),
            }
        }
        if self.stroke_enabled && tool != Tool::Line {
            match stroke {
                Ok(stroke) => shape = shape.with_stroke(stroke),
                Err(error) => return self.report(error.to_string()),
            }
        }
        self.insert_shape_layer(shape, name);
    }

    /// Adds `shape` on a new layer spanning the scene (at least 24 frames), as one undo step,
    /// and makes that layer active.
    pub(super) fn insert_shape_layer(&mut self, shape: Shape, name: &str) {
        let Some(scene) = self.document().scenes().first() else {
            self.report("This document has no scene");
            return;
        };
        let scene_id = scene.id();
        let duration = u32::try_from(scene.timeline().frame_count())
            .unwrap_or(u32::MAX)
            .max(24);
        let span = FrameSpan::new(
            0,
            NonZeroU32::new(duration).unwrap_or(NonZeroU32::MIN),
            Content::Shape(shape),
        );
        let layer_id = LayerId::new();
        let layer = match Layer::new(layer_id, name, vec![]) {
            Ok(layer) => layer,
            Err(error) => return self.report(error.to_string()),
        };
        let batch = [
            Command::AddLayer {
                scene: scene_id,
                layer,
            },
            Command::InsertSpan {
                scene: scene_id,
                layer: layer_id,
                span,
            },
        ];
        match self
            .active_mut()
            .apply_labeled(batch, &format!("Draw {}", name.to_lowercase()))
        {
            Ok(()) => {
                self.status = None;
                self.current_layer = Some(layer_id);
            }
            Err(error) => self.report(format!("Cannot draw: {error}")),
        }
    }
}
