//! Undo, redo and committing drawn shapes.

use egui::Pos2;
use flare_core::{Rgba8, ShapeId};
use flare_document::{Shape, Stroke as ShapeStroke};

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

    /// Commits a rectangle as one undo step: it joins the active layer's keyframe.
    pub fn commit_rectangle(&mut self, a: Pos2, b: Pos2) {
        self.commit_shape(Tool::Rectangle, a, b);
    }

    /// Commits the dragged shape for a drawing tool as one undo step: it joins the keyframe of
    /// the active layer at the playhead. Lines need a stroke, so they use the stroke settings even when
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
            let message = tr!(self, "tools-no-scene");
            self.report(message);
            return;
        }
        let rectangle_name = tr!(self, "tools-layer-rectangle");
        let fill = {
            let c = self.paint.fill.to_array();
            Rgba8::new(c[0], c[1], c[2], c[3])
        };
        let stroke = {
            let c = self.paint.stroke_color.to_array();
            ShapeStroke::new(
                f64::from(self.paint.stroke_width),
                Rgba8::new(c[0], c[1], c[2], c[3]),
            )
            .map(|stroke| {
                stroke
                    .with_cap(self.paint.stroke_cap)
                    .with_join(self.paint.stroke_join)
            })
        };
        let id = ShapeId::new();
        let (x0, y0) = (f64::from(x0), f64::from(y0));
        let (shape, name) = match tool {
            Tool::Rectangle => (
                Shape::rounded_rect(id, x0, y0, w, h, f64::from(self.rect_radius), fill),
                rectangle_name,
            ),
            Tool::PolyStar => {
                let radius = f64::from((b - a).length());
                // The first corner points where the drag went.
                let rotation = f64::from((b.x - a.x).atan2(a.y - b.y));
                let inner = self.poly_star.then_some(f64::from(self.poly_inner));
                let center = (f64::from(a.x), f64::from(a.y));
                let name = self.tr(if self.poly_star {
                    "tools-layer-star"
                } else {
                    "tools-layer-polygon"
                });
                (
                    Shape::polygon(id, center, radius, self.poly_sides, rotation, inner, fill),
                    name,
                )
            }
            Tool::Ellipse => (
                Shape::ellipse(id, x0 + w / 2.0, y0 + h / 2.0, w / 2.0, h / 2.0, fill),
                tr!(self, "tools-layer-ellipse"),
            ),
            Tool::Line => {
                let from = (f64::from(a.x), f64::from(a.y));
                let to = (f64::from(b.x), f64::from(b.y));
                match &stroke {
                    Ok(stroke) => (
                        Shape::line(id, from, to, *stroke),
                        tr!(self, "tools-layer-line"),
                    ),
                    Err(error) => return self.report(error.to_string()),
                }
            }
            _ => return,
        };
        let Some(mut shape) = shape else {
            let message = tr!(self, "tools-not-finite");
            self.report(message);
            return;
        };
        if self.paint.gradient_enabled && tool != Tool::Line {
            let end = {
                let c = self.paint.gradient_end.to_array();
                Rgba8::new(c[0], c[1], c[2], c[3])
            };
            let (from, to) = if self.paint.gradient_vertical {
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
                None => {
                    let message = tr!(self, "tools-gradient-size");
                    return self.report(message);
                }
            }
        }
        if self.paint.stroke_enabled && tool != Tool::Line {
            match stroke {
                Ok(stroke) => shape = shape.with_stroke(stroke),
                Err(error) => return self.report(error.to_string()),
            }
        }
        self.draw_shape(shape, &name);
        if self.status.is_none() {
            self.settle_committed_shape(tool, a, b);
        }
    }

    /// Keeps what a geometry tool just committed on screen from the release until the stage
    /// picture shows it: the drag preview ends with the release, and the picture with the new
    /// shape comes a frame or more later, which showed as a flash.
    fn settle_committed_shape(&mut self, tool: Tool, a: Pos2, b: Pos2) {
        use super::stroke_preview::{PreviewStroke, SettleShape};
        let outline = (self.paint.stroke_enabled && self.paint.stroke_width > 0.0)
            .then_some((self.paint.stroke_width, self.paint.stroke_color));
        let rect = Rect::from_two_pos(a, b);
        match tool {
            Tool::Line if self.paint.stroke_color.a() == 255 => {
                let half = self.paint.stroke_width / 2.0;
                self.settle_geometry(
                    vec![PreviewStroke {
                        points: vec![a, b],
                        halves: vec![half, half],
                        color: self.paint.stroke_color,
                    }],
                    Vec::new(),
                );
            }
            Tool::Rectangle => self.settle_geometry(
                Vec::new(),
                vec![SettleShape::Rect {
                    rect,
                    radius: self.rect_radius,
                    fill: self.paint.fill,
                    outline,
                }],
            ),
            Tool::Ellipse => self.settle_geometry(
                Vec::new(),
                vec![SettleShape::Ellipse {
                    center: rect.center(),
                    radius: rect.size() / 2.0,
                    fill: self.paint.fill,
                    outline,
                }],
            ),
            _ => {}
        }
    }
}
