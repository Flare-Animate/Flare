//! The Paint Bucket, Ink Bottle and Eyedropper in the workspace. A click asks
//! `flare_render::paint_hits` what lies under it, top first, and `flare_tools::paint` what to
//! do with it. The bucket fills a fill, the inside of an empty outline, or the area lines around
//! the click close off, whatever drew them; the ink bottle outlines the shape; the eyedropper
//! takes the stroke it is on or the fill it is in, and otherwise the color that shows there.

use egui::{Color32, Pos2};
use flare_core::Rgba8;
use flare_document::{Shape, Stroke as ShapeStroke};
use flare_eval::Frame;
use flare_render::Viewport;
use flare_tools::paint::area::{InkGrid, MAX_AREA_PIXELS, enclosed_area};
use flare_tools::paint::{self, Fill, Sample};
use glamx::DVec2;

use crate::workspace::*;

/// How far apart, in screen points, the ends of a line may be for the Paint Bucket still to fill
/// what it closes: a loop drawn by hand seldom meets itself exactly (Animate's Close Small Gaps).
const GAP_POINTS: f32 = 10.0;

/// The most that gap can stand for on the stage, however far out the view is zoomed.
const MAX_GAP: f64 = 100.0;

/// The finest an area is looked for at, in pixels per stage unit: finer than the screen when
/// zoomed in, but no finer than this.
const MAX_AREA_DETAIL: f64 = 4.0;

/// What a Paint Bucket click does.
enum Pour {
    /// Repaint this shape's own fill.
    Shape(SpanRef),
    /// A new shape filling the area around the click: just above the element it was clicked on,
    /// or with none, under the drawings of the layer being drawn on.
    Area(Shape, Option<SpanRef>),
    /// The click is on a shape inside a group or symbol instance, which is painted by editing it.
    InsideSymbol,
    Nothing,
}

pub(in crate::workspace) fn rgba(color: Color32) -> Rgba8 {
    let [r, g, b, a] = color.to_array();
    Rgba8::new(r, g, b, a)
}

fn color(color: Rgba8) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a)
}

impl Workspace {
    /// The Paint Bucket and Ink Bottle: paints what `point` is on, as one undo step.
    pub(in crate::workspace) fn pour_paint(&mut self, point: Pos2) {
        let Some(scene) = self.scene_id() else { return };
        if self.tool == Tool::InkBottle {
            let at = match self.pour_target(point) {
                Pour::Shape(at) => Some(at),
                Pour::InsideSymbol => return self.report(tr!(self, "msg-paint-inside-symbol")),
                Pour::Area(..) | Pour::Nothing => self.text_box_hit(point),
            };
            let Some(at) = at else { return };
            match self.ink() {
                Ok(stroke) => {
                    let ink = Some(Some(stroke));
                    if let Some(commands) = ops::restyle(self.document(), scene, at, None, ink) {
                        self.run_batch(commands, "paint");
                    }
                }
                Err(error) => self.report(error.to_string()),
            }
            return;
        }
        match self.pour_target(point) {
            Pour::Shape(at) => {
                if let Some(commands) = paint::fill(self.document(), scene, at, self.bucket_fill())
                {
                    self.run_batch(commands, "paint");
                }
            }
            Pour::Area(shape, Some(above)) => {
                match paint::pour_above(self.document(), scene, above, shape.clone()) {
                    Some(commands) => self.run_batch(commands, "fill"),
                    // A keyframe that cannot take another element: the layer's drawings rules
                    // decide, and say why if they refuse.
                    None => self.draw_shape_under(shape),
                }
            }
            Pour::Area(shape, None) => self.draw_shape_under(shape),
            Pour::InsideSymbol => self.report(tr!(self, "msg-paint-inside-symbol")),
            Pour::Nothing => {
                // Text is painted anywhere in its box, as it is selected.
                if let Some(at) = self.text_box_hit(point)
                    && let Some(commands) =
                        paint::fill(self.document(), scene, at, self.bucket_fill())
                {
                    self.run_batch(commands, "paint");
                }
            }
        }
    }

    /// What a click at `point` lands on: for the bucket, a fill or an area to fill; for the ink
    /// bottle, any shape there. Shapes on locked layers are looked through, as selection looks
    /// through them, but their lines still close an area.
    fn pour_target(&self, point: Pos2) -> Pour {
        let Some(scene) = self.scene_id() else {
            return Pour::Nothing;
        };
        let Ok(frame) =
            flare_eval::evaluate_frame_without_camera(self.document(), scene, self.frame)
        else {
            return Pour::Nothing;
        };
        let at = stage_point(point);
        let gap = self.paint_gap();
        let top = flare_render::paint_hits(&frame, at, gap)
            .into_iter()
            .find_map(|hit| {
                let drawn = &frame.shapes[hit.index];
                let span = ops::locate_hit(self.document(), scene, self.frame, drawn)?;
                self.on_unlocked_layer(span)
                    .then_some((hit, span, !drawn.path.is_empty()))
            });
        let Some((hit, span, inside)) = top else {
            return match self.tool {
                Tool::Bucket => self
                    .area_at(&frame, 0, at, gap)
                    .map_or(Pour::Nothing, |shape| Pour::Area(shape, None)),
                _ => Pour::Nothing,
            };
        };
        let own = if inside {
            Pour::InsideSymbol
        } else {
            Pour::Shape(span)
        };
        if self.tool != Tool::Bucket || (hit.fillable && !hit.on_fill) {
            return own;
        }
        if !hit.fillable {
            // A line on top, with nothing of its own to fill there.
            return Pour::Nothing;
        }
        // On a fill: an area the lines over it close off, or else the fill itself.
        match self.area_at(&frame, hit.index + 1, at, gap) {
            Some(shape) => Pour::Area(shape, Some(span)),
            None => own,
        }
    }

    /// A shape filling the area around `at` that the shapes `frame.shapes[from..]` close off,
    /// looked for at the detail the stage is shown at. `None` when they leave it open.
    fn area_at(&self, frame: &Frame<'_>, from: usize, at: DVec2, gap: f64) -> Option<Shape> {
        let bounds = flare_render::ink_bounds(frame, from)?;
        let inside = (bounds.x..=bounds.x + bounds.width).contains(&at.x)
            && (bounds.y..=bounds.y + bounds.height).contains(&at.y);
        if !inside {
            return None;
        }
        // A pixel of margin all round, so the flood can tell an open area by reaching the edge.
        let shown = f64::from(self.stage_scale());
        let mut detail = if shown.is_finite() { shown } else { 1.0 }.clamp(1.0, MAX_AREA_DETAIL);
        let pixels = |detail: f64| {
            (
                (bounds.width * detail).ceil() + 2.0,
                (bounds.height * detail).ceil() + 2.0,
            )
        };
        let (w, h) = pixels(detail);
        let budget = MAX_AREA_PIXELS as f64;
        if w * h > budget {
            detail *= (budget / (w * h)).sqrt() * 0.99;
        }
        let (w, h) = pixels(detail);
        if !(w * h <= budget && w >= 1.0 && h >= 1.0) {
            return None;
        }
        let area = Viewport {
            x: bounds.x - 1.0 / detail,
            y: bounds.y - 1.0 / detail,
            width: w / detail,
            height: h / detail,
        };
        let (w, h) = (w as u32, h as u32);
        let ink = flare_render::ink_mask(frame, from, &area, w, h).ok()?;
        let grid = InkGrid::new(w as usize, h as usize, &ink)?;
        let seed = (
            ((at.x - area.x) * detail) as usize,
            ((at.y - area.y) * detail) as usize,
        );
        let outlines = enclosed_area(&grid, seed, gap * detail)?;
        let to_stage = |(x, y): (f64, f64)| (area.x + x / detail, area.y + y / detail);
        paint::area_shape(&outlines, to_stage, self.bucket_fill())
    }

    /// The Eyedropper: takes the paint under `point` into the fill or stroke controls, or with
    /// `stroke_only` (Shift) the stroke of the shape there. Where no shape has paint to take,
    /// the color that shows there is taken as the fill: a picture's, or the stage's.
    pub(in crate::workspace) fn take_paint(&mut self, point: Pos2, stroke_only: bool) {
        let Some(scene) = self.scene_id() else { return };
        let gap = self.paint_gap();
        let sample = {
            let Ok(frame) =
                flare_eval::evaluate_frame_without_camera(self.document(), scene, self.frame)
            else {
                return;
            };
            let at = stage_point(point);
            // Colors are taken from locked layers and from inside symbols too: taking one
            // changes nothing there.
            flare_render::paint_hits(&frame, at, gap)
                .into_iter()
                .find_map(|hit| {
                    let shape = &frame.shapes[hit.index].shape;
                    paint::sample(shape, hit.on_stroke, hit.on_fill, stroke_only)
                })
                .or_else(|| {
                    (!stroke_only)
                        .then(|| self.color_shown(&frame, at))
                        .flatten()
                        .map(|shown| Sample::Fill(shown, None))
                })
        };
        match sample {
            Some(Sample::Fill(fill, gradient)) => {
                self.paint.fill = color(gradient.map_or(fill, |g| g.start()));
                self.paint.gradient_enabled = gradient.is_some();
                if let Some(gradient) = gradient {
                    self.paint.gradient_end = color(gradient.end());
                    self.paint.gradient_vertical = paint::runs_vertically(&gradient);
                }
                self.color_target = ColorTarget::Fill;
            }
            Some(Sample::Stroke(stroke)) => {
                self.paint.stroke_enabled = true;
                self.paint.stroke_color = color(stroke.color());
                self.paint.stroke_width = stroke.width() as f32;
                self.paint.stroke_cap = stroke.cap();
                self.paint.stroke_join = stroke.join();
                self.paint.stroke_style = stroke.style();
                self.color_target = ColorTarget::Stroke;
            }
            None => {}
        }
    }

    /// The color shown at `at`, over one screen pixel: on the stage, the artwork over the stage
    /// color; on the pasteboard, only artwork, so `None` where nothing is drawn there.
    fn color_shown(&self, frame: &Frame<'_>, at: DVec2) -> Option<Rgba8> {
        let stage = self.document().stage();
        let on_stage = (0.0..f64::from(stage.width())).contains(&at.x)
            && (0.0..f64::from(stage.height())).contains(&at.y);
        let under = if on_stage {
            stage.background()
        } else {
            Rgba8::TRANSPARENT
        };
        let shown = f64::from(self.stage_scale());
        let side = 1.0 / if shown.is_finite() { shown } else { 1.0 }.max(0.01);
        let view = Viewport {
            x: at.x - side / 2.0,
            y: at.y - side / 2.0,
            width: side,
            height: side,
        };
        let image = flare_render::render_viewport(frame, &view, 1, 1, under).ok()?;
        image.pixel(0, 0).filter(|shown| on_stage || shown.a > 0)
    }

    /// The gap, in stage units, the Paint Bucket closes at the current zoom.
    fn paint_gap(&self) -> f64 {
        let scale = self.stage_scale();
        if scale.is_finite() && scale > 0.0 {
            (f64::from(GAP_POINTS) / f64::from(scale)).min(MAX_GAP)
        } else {
            f64::from(GAP_POINTS)
        }
    }

    /// What the Paint Bucket pours: the fill color, or the gradient when one is on.
    fn bucket_fill(&self) -> Fill {
        let start = rgba(self.paint.fill);
        if self.paint.gradient_enabled {
            Fill::Linear {
                start,
                end: rgba(self.paint.gradient_end),
                vertical: self.paint.gradient_vertical,
            }
        } else {
            Fill::Solid(start)
        }
    }

    /// The stroke the Ink Bottle puts on: the stroke controls as they are.
    fn ink(&self) -> Result<ShapeStroke, flare_document::ShapeError> {
        Ok(ShapeStroke::new(
            f64::from(self.paint.stroke_width),
            rgba(self.paint.stroke_color),
        )?
        .with_cap(self.paint.stroke_cap)
        .with_join(self.paint.stroke_join)
        .with_style(self.paint.stroke_style))
    }
}

fn stage_point(point: Pos2) -> DVec2 {
    DVec2::new(f64::from(point.x), f64::from(point.y))
}
