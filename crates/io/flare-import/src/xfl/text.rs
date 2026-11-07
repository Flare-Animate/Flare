//! Text fields (`DOMStaticText`, `DOMDynamicText`, `DOMInputText`) as text objects. Their words
//! and the style of their first run are set with flare-text, so the field becomes an outline
//! shape that still remembers its words. The variable name of a dynamic or input field, and the
//! styles of later runs, are not carried; the diagnostics say so.

use crate::xml::Node;
use flare_core::{DocumentPoint, Rgba8, ShapeId};
use flare_document::{Content, TextSource};
use flare_text::TextEngine;

use super::*;

/// The size Animate gives a run that has none.
const DEFAULT_SIZE: f64 = 12.0;

impl Importer {
    /// The text field as a text object, or `None` when it cannot be set; the caller then keeps the
    /// element as source data.
    pub(super) fn text_field(&mut self, node: &Node, place: &str) -> Option<Content> {
        let runs: Vec<&Node> = node
            .child("textRuns")
            .into_iter()
            .flat_map(|r| r.children_named("DOMTextRun"))
            .collect();
        let mut words = String::new();
        let mut attrs: Vec<Option<&Node>> = Vec::new();
        for run in &runs {
            if let Some(characters) = run.child("characters") {
                words.push_str(&characters.text);
            }
            attrs.push(run.child("textAttrs").and_then(|a| a.child("DOMTextAttrs")));
        }
        // Animate writes a paragraph break as a carriage return.
        let words = words.replace('\r', "\n");
        let first = attrs.first().copied().flatten();
        let style = |key: &str| first.and_then(|a| a.attr(key));
        if attrs.iter().skip(1).any(|a| {
            let a = a.as_ref();
            ["face", "size", "bold", "italic", "fillColor"]
                .iter()
                .any(|key| a.and_then(|n| n.attr(key)) != style(key))
        }) {
            self.note(
                Severity::Info,
                place,
                "text runs with different styles are set in the first run's style",
            );
        }
        if node.name != "DOMStaticText" {
            self.note(
                Severity::Warning,
                place,
                format!(
                    "{} keeps its words as static text; its variable name and field behaviour are not carried",
                    node.name
                ),
            );
        }
        if words.trim().is_empty() {
            self.note(
                Severity::Info,
                place,
                "an empty text field was not imported",
            );
            return None;
        }

        let matrix = self.matrix(node, place)?.as_affine();
        let number = |key: &str, default: f64| {
            node.attr(key)
                .and_then(|v| v.trim().parse::<f64>().ok())
                .filter(|v| v.is_finite())
                .unwrap_or(default)
        };
        // The field's corner is in its own coordinates; the matrix places it on the stage.
        let corner =
            matrix.transform_point2(glamx::DVec2::new(number("left", 0.0), number("top", 0.0)));
        let linear = matrix.matrix2;
        let det = linear.x_axis.x * linear.y_axis.y - linear.y_axis.x * linear.x_axis.y;
        if linear.x_axis.y != 0.0 || linear.y_axis.x != 0.0 {
            self.note(
                Severity::Info,
                place,
                "a rotated or skewed text field is placed by its corner and scaled by its area",
            );
        }
        let size = style("size")
            .and_then(|s| s.trim().parse::<f64>().ok())
            .filter(|s| s.is_finite() && *s > 0.0)
            .unwrap_or(DEFAULT_SIZE)
            * det.abs().sqrt();
        let made_source = DocumentPoint::new(corner.x, corner.y).and_then(|origin| {
            TextSource::new(
                words,
                style("face").unwrap_or("sans-serif"),
                size as f32,
                style("bold") == Some("true"),
                style("italic") == Some("true"),
                origin,
            )
            .map_err(|_| flare_core::NonFiniteCoordinate)
        });
        let Ok(source) = made_source else {
            self.note(
                Severity::Unsupported,
                place,
                "a text field with an unusable size or position was kept as source data",
            );
            return None;
        };
        let fill = style("fillColor")
            .and_then(|c| color_from(c, style("alpha")))
            .unwrap_or(Rgba8::new(0, 0, 0, 255));
        let made =
            self.text
                .get_or_insert_with(TextEngine::new)
                .shape(&source, ShapeId::new(), fill);
        match made {
            Ok(shape) => Some(Content::Shape(shape)),
            Err(error) => {
                self.note(
                    Severity::Unsupported,
                    place,
                    format!("text could not be set ({error}); kept as source data"),
                );
                None
            }
        }
    }
}
