//! What a frame holds: symbol instances and the elements around them.

use crate::xml::Node;
use flare_core::{InstanceId, Transform2D};
use flare_document::{Anchor, BitmapInstance, Content, Instance, Placement};
use glamx::DAffine2;

use super::*;

impl Importer {
    pub(super) fn elements(&mut self, elements: &Node, place: &str, spot: Spot) -> Vec<Content> {
        let mut out = Vec::new();
        for element in &elements.children {
            match element.name.as_str() {
                "DOMShape" => out.extend(self.shape(element, place)),
                "DOMSymbolInstance" => out.extend(self.instance(element, place)),
                "DOMBitmapInstance" => match self.bitmap_instance(element, place) {
                    Some(content) => out.push(content),
                    None => self.keep(
                        Anchor::Layer(spot.layer),
                        Placement::FrameElement { frame: spot.frame },
                        element.to_xml(),
                        place,
                    ),
                },
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
        let transform = self.matrix(node, place)?;
        // A graphic's frame-loop setting; Animate leaves both off an instance that loops from the
        // first frame, and writes them on every graphic instance.
        let mode = match node.attr("loop") {
            None => flare_document::PlayMode::Loop,
            Some(name) => flare_document::PlayMode::from_name(name).unwrap_or_else(|| {
                self.note(
                    Severity::Info,
                    place,
                    format!("the frame setting \"{name}\" is not known; the instance loops"),
                );
                flare_document::PlayMode::Loop
            }),
        };
        let first_frame = node
            .attr("firstFrame")
            .and_then(|v| v.trim().parse::<u32>().ok())
            .unwrap_or(0);
        Some(Content::Instance(
            Instance::new(InstanceId::new(), symbol, transform)
                .with_playback(flare_document::Playback { mode, first_frame }),
        ))
    }

    /// A bitmap instance shows an imported library bitmap at its matrix; `None` when the bitmap
    /// was not imported (the element is then kept as source data).
    pub(super) fn bitmap_instance(&mut self, node: &Node, place: &str) -> Option<Content> {
        let target = node.attr("libraryItemName").unwrap_or_default();
        let Some(bitmap) = self.bitmaps.get(target).copied() else {
            self.note(
                Severity::Unsupported,
                place,
                format!(
                    "instance of bitmap \"{target}\" that is not imported is kept as source data"
                ),
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
        let transform = self.matrix(node, place)?;
        Some(Content::Bitmap(BitmapInstance::new(
            InstanceId::new(),
            bitmap,
            transform,
        )))
    }

    /// The element's `matrix/Matrix` as a transform; a missing value is the identity's, and a
    /// matrix that is not finite or not invertible is noted and gives `None`.
    fn matrix(&mut self, node: &Node, place: &str) -> Option<Transform2D> {
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
        Some(transform)
    }
}
