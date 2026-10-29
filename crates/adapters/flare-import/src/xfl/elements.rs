//! What a frame holds: symbol instances and the elements around them.

use crate::xml::Node;
use flare_core::{InstanceId, Transform2D};
use flare_document::{Anchor, Content, Instance, Placement};
use glamx::DAffine2;

use super::*;

impl Importer {
    pub(super) fn elements(&mut self, elements: &Node, place: &str, spot: Spot) -> Vec<Content> {
        let mut out = Vec::new();
        for element in &elements.children {
            match element.name.as_str() {
                "DOMShape" => out.extend(self.shape(element, place)),
                "DOMSymbolInstance" => out.extend(self.instance(element, place)),
                "DOMGroup" => {
                    self.note(
                        Severity::Info,
                        place,
                        "groups are flattened into their members",
                    );
                    if let Some(members) = element.child("members") {
                        out.extend(self.elements(members, place, spot));
                    }
                }
                other => {
                    self.note(
                        Severity::Unsupported,
                        place,
                        format!("{other} is not imported"),
                    );
                    self.keep(
                        Anchor::Layer(spot.layer),
                        Placement::FrameElement { frame: spot.frame },
                        element.to_xml(),
                        place,
                    );
                }
            }
        }
        out
    }

    pub(super) fn instance(&mut self, node: &Node, place: &str) -> Option<Content> {
        let target = node.attr("libraryItemName").unwrap_or_default();
        let Some(symbol) = self.symbols.get(target).copied() else {
            self.note(
                Severity::Unsupported,
                place,
                format!("instance of missing symbol \"{target}\" was skipped"),
            );
            return None;
        };
        if node.child("color").is_some() {
            self.note(
                Severity::Unsupported,
                place,
                "instance color effects are ignored",
            );
        }
        if node.child("filters").is_some() {
            self.note(Severity::Unsupported, place, "instance filters are ignored");
        }
        if node.attr("blendMode").is_some_and(|b| b != "normal") {
            self.note(Severity::Unsupported, place, "blend modes are ignored");
        }
        let matrix = node.child("matrix").and_then(|m| m.child("Matrix"));
        let get = |key: &str, default: f64| {
            matrix.and_then(|m| m.attr(key)).map_or(Some(default), |v| {
                v.parse::<f64>().ok().filter(|n| n.is_finite())
            })
        };
        let values = [
            get("a", 1.0),
            get("b", 0.0),
            get("c", 0.0),
            get("d", 1.0),
            get("tx", 0.0),
            get("ty", 0.0),
        ];
        let transform = if let [Some(a), Some(b), Some(c), Some(d), Some(tx), Some(ty)] = values {
            Transform2D::new(DAffine2::from_cols_array(&[a, b, c, d, tx, ty])).ok()
        } else {
            None
        };
        let Some(transform) = transform else {
            self.note(
                Severity::Unsupported,
                place,
                "an instance with an invalid matrix was skipped",
            );
            return None;
        };
        Some(Content::Instance(Instance::new(
            InstanceId::new(),
            symbol,
            transform,
        )))
    }
}
