//! Shape characters: the edges of a `DefineShape`, bucketed by the style each borders, turned
//! into document shapes by the same code that turns XFL's into them.

use crate::edges::Segment;
use crate::xfl::{Severity, ShapeParts};
use flare_core::Rgba8;
use flare_document::{Cap, Content, Join};
use std::collections::{BTreeMap, HashMap};

use super::Movie;

/// A stroke as the shape builder takes it: width, color, cap and join.
type Stroke = (f64, Rgba8, Cap, Join);

/// The styles seen so far. A record can bring a new table whose numbers start again at one, so
/// each table's numbers are offset by the sizes of the tables before it and never collide.
#[derive(Default)]
struct Tables {
    fills: HashMap<u32, Option<Rgba8>>,
    strokes: HashMap<u32, Option<Stroke>>,
    fill_base: u32,
    line_base: u32,
    fill_len: u32,
    line_len: u32,
}

fn rgba(color: &swf::Color) -> Rgba8 {
    Rgba8::new(color.r, color.g, color.b, color.a)
}

impl Movie {
    /// The drawn shapes of a `DefineShape`, bottom first.
    pub(super) fn shape_contents(&mut self, shape: &swf::Shape, place: &str) -> Vec<Content> {
        let mut tables = Tables::default();
        self.add_styles(&shape.styles, &mut tables, place);
        let mut fill_edges: BTreeMap<u32, Vec<Segment>> = BTreeMap::new();
        let mut stroke_edges: BTreeMap<u32, Vec<Segment>> = BTreeMap::new();
        let twips = |t: swf::Twips| f64::from(t.get());
        let mut pen = (0.0_f64, 0.0_f64);
        let (mut fill0, mut fill1, mut line) = (None::<u32>, None::<u32>, None::<u32>);
        for record in &shape.shape {
            let segment = match record {
                swf::ShapeRecord::StyleChange(change) => {
                    if let Some(styles) = &change.new_styles {
                        self.add_styles(styles, &mut tables, place);
                        (fill0, fill1, line) = (None, None, None);
                    }
                    let key =
                        |index: u32, base: u32| (index != 0).then(|| base.saturating_add(index));
                    if let Some(i) = change.fill_style_0 {
                        fill0 = key(i, tables.fill_base);
                    }
                    if let Some(i) = change.fill_style_1 {
                        fill1 = key(i, tables.fill_base);
                    }
                    if let Some(i) = change.line_style {
                        line = key(i, tables.line_base);
                    }
                    if let Some(to) = change.move_to {
                        pen = (twips(to.x), twips(to.y));
                    }
                    continue;
                }
                swf::ShapeRecord::StraightEdge { delta } => {
                    let to = (pen.0 + twips(delta.dx), pen.1 + twips(delta.dy));
                    let from = std::mem::replace(&mut pen, to);
                    if from == to {
                        continue;
                    }
                    Segment::Line { from, to }
                }
                swf::ShapeRecord::CurvedEdge {
                    control_delta,
                    anchor_delta,
                } => {
                    let ctrl = (
                        pen.0 + twips(control_delta.dx),
                        pen.1 + twips(control_delta.dy),
                    );
                    let to = (
                        ctrl.0 + twips(anchor_delta.dx),
                        ctrl.1 + twips(anchor_delta.dy),
                    );
                    let from = std::mem::replace(&mut pen, to);
                    Segment::Quad { from, ctrl, to }
                }
            };
            // The right-hand fill follows the edge's direction, the left-hand one runs against
            // it, so each fill's boundary is oriented the same way, as XFL's are.
            if let Some(i) = fill1 {
                fill_edges.entry(i).or_default().push(segment);
            }
            if let Some(i) = fill0 {
                fill_edges.entry(i).or_default().push(segment.reversed());
            }
            if let Some(i) = line {
                stroke_edges.entry(i).or_default().push(segment);
            }
        }
        self.importer.shapes_from(
            ShapeParts {
                fills: tables.fills,
                strokes: tables.strokes,
                fill_edges,
                stroke_edges,
            },
            place,
        )
    }

    fn add_styles(&mut self, styles: &swf::ShapeStyles, tables: &mut Tables, place: &str) {
        tables.fill_base += tables.fill_len;
        tables.line_base += tables.line_len;
        tables.fill_len = u32::try_from(styles.fill_styles.len()).unwrap_or(u32::MAX);
        tables.line_len = u32::try_from(styles.line_styles.len()).unwrap_or(u32::MAX);
        for (i, style) in (1_u32..).zip(&styles.fill_styles) {
            let color = self.paint(style, place);
            tables
                .fills
                .insert(tables.fill_base.saturating_add(i), color);
        }
        for (i, style) in (1_u32..).zip(&styles.line_styles) {
            let stroke = self.paint(style.fill_style(), place).map(|color| {
                // A width of nothing is a hairline, which is drawn one pixel wide at any scale.
                let width = style.width().to_pixels();
                let cap = match style.start_cap() {
                    swf::LineCapStyle::Round => Cap::Round,
                    swf::LineCapStyle::None => Cap::Butt,
                    swf::LineCapStyle::Square => Cap::Square,
                };
                let join = match style.join_style() {
                    swf::LineJoinStyle::Round => Join::Round,
                    swf::LineJoinStyle::Bevel => Join::Bevel,
                    swf::LineJoinStyle::Miter(_) => Join::Miter,
                };
                (if width > 0.0 { width } else { 1.0 }, color, cap, join)
            });
            tables
                .strokes
                .insert(tables.line_base.saturating_add(i), stroke);
        }
    }

    /// The color a fill style paints with, or `None` for one that cannot be imported.
    fn paint(&mut self, style: &swf::FillStyle, place: &str) -> Option<Rgba8> {
        match style {
            swf::FillStyle::Color(color) => Some(rgba(color)),
            swf::FillStyle::LinearGradient(gradient)
            | swf::FillStyle::RadialGradient(gradient)
            | swf::FillStyle::FocalGradient { gradient, .. } => {
                self.note(
                    Severity::Warning,
                    place,
                    "a gradient takes the color of its first stop",
                );
                gradient.records.first().map(|record| rgba(&record.color))
            }
            swf::FillStyle::Bitmap { .. } => {
                self.note(
                    Severity::Unsupported,
                    place,
                    "picture fills are not imported",
                );
                None
            }
        }
    }
}
