//! Timelines, layers and frames.

use crate::xml::Node;
use flare_core::LayerId;
use flare_document::{Anchor, Content, FrameSpan, Layer, Placement, Timeline, Tween};

use super::*;

impl Importer {
    pub(super) fn timeline(&mut self, timeline: &Node, location: &str, owner: Anchor) -> Timeline {
        let Some(layers_node) = timeline.child("layers") else {
            return Timeline::default();
        };
        let mut layers: Vec<Layer> = Vec::new();
        // Folders seen so far, by the index children refer to them by.
        let mut folders: std::collections::HashMap<usize, LayerId> =
            std::collections::HashMap::new();
        for (source_index, layer) in layers_node.children_named("DOMLayer").enumerate() {
            let name = layer.attr("name").unwrap_or("Layer");
            let place = format!("{location} / layer \"{name}\"");
            let parent = layer
                .attr("parentLayerIndex")
                .and_then(|v| v.parse::<usize>().ok())
                .and_then(|i| folders.get(&i).copied());
            match layer.attr("layerType") {
                None | Some("normal") => {}
                Some("folder") => {
                    let id = LayerId::new();
                    if let Ok(folder) = Layer::new(id, name, Vec::new()) {
                        folders.insert(source_index, id);
                        layers.push(
                            folder
                                .with_folder(true)
                                .with_parent(parent)
                                .with_visible(bool_attr(layer, "visible", true))
                                .with_locked(bool_attr(layer, "locked", false)),
                        );
                    }
                    continue;
                }
                Some(other) => {
                    self.note(
                        Severity::Unsupported,
                        &place,
                        format!("{other} layers are not imported; the layer was skipped"),
                    );
                    let index = u32::try_from(source_index).unwrap_or(u32::MAX);
                    self.keep(owner, Placement::LayerAt { index }, layer.to_xml(), &place);
                    continue;
                }
            }
            if layer.attr("parentLayerIndex").is_some() && parent.is_none() {
                self.note(
                    Severity::Info,
                    &place,
                    "membership of something other than a folder is not kept",
                );
            }
            let first = layers.len();
            self.layer(layer, name, &place, &mut layers);
            if parent.is_some() {
                for made in &mut layers[first..] {
                    *made = made.clone().with_parent(parent);
                }
            }
        }
        Timeline::new(layers)
    }

    /// Imports one DOMLayer. A keyframe may hold several elements but a span holds one, so
    /// extra elements go to additional layers right below, named "<name> (2)" and so on.
    pub(super) fn layer(&mut self, layer: &Node, name: &str, place: &str, out: &mut Vec<Layer>) {
        let mut slots: Vec<Vec<FrameSpan>> = Vec::new();
        // The first layer made for this source layer owns what could not be imported from it.
        let first_id = LayerId::new();
        let frames = layer
            .child("frames")
            .into_iter()
            .flat_map(|f| f.children_named("DOMFrame"));
        for frame in frames {
            let Some(index) = frame.attr("index").and_then(|v| v.parse::<u32>().ok()) else {
                self.note(
                    Severity::Unsupported,
                    place,
                    "a frame without a valid index was skipped",
                );
                continue;
            };
            let duration = match frame.attr("duration") {
                None => 1,
                Some(text) => match text.parse::<u32>() {
                    Ok(n) if n >= 1 => n,
                    _ => {
                        self.note(
                            Severity::Unsupported,
                            place,
                            format!(
                                "frame {index} has an invalid duration {text:?} and was skipped"
                            ),
                        );
                        continue;
                    }
                },
            };
            let Some(duration) = std::num::NonZeroU32::new(duration) else {
                continue;
            };
            let here = format!("{place} / frame {index}");
            let spot = Spot {
                layer: first_id,
                frame: index,
            };
            self.frame_extras(frame, &here, spot);
            let contents = frame
                .child("elements")
                .map(|e| self.elements(e, &here, spot))
                .unwrap_or_default();
            let contents = if contents.is_empty() {
                vec![Content::Empty]
            } else {
                contents
            };
            let tween = self.tween_for(frame, &contents, &here);
            for (slot, content) in contents.into_iter().enumerate() {
                if slots.len() <= slot {
                    slots.push(Vec::new());
                }
                let mut span = FrameSpan::new(index, duration, content);
                if slot == 0 {
                    span = span.with_tween(tween);
                }
                slots[slot].push(span);
            }
        }
        if slots.len() > 1 {
            self.note(
                Severity::Info,
                place,
                format!(
                    "{} elements share a keyframe; the extras were placed on additional layers above the first, preserving stacking",
                    slots.len()
                ),
            );
        }
        let visible = bool_attr(layer, "visible", true);
        let locked = bool_attr(layer, "locked", false);
        // XFL lists a frame's elements bottom to top, so later elements must sit above
        // earlier ones: the extra layers go above the first, highest slot first.
        let mut made = Vec::new();
        for (slot, spans) in slots.into_iter().enumerate() {
            let layer_name = if slot == 0 {
                name.to_owned()
            } else {
                format!("{name} ({})", slot + 1)
            };
            let id = if slot == 0 { first_id } else { LayerId::new() };
            match Layer::new(id, layer_name, spans) {
                Ok(l) => made.push(l.with_visible(visible).with_locked(locked)),
                Err(_) => self.note(
                    Severity::Unsupported,
                    place,
                    "frames overlap; the layer was skipped",
                ),
            }
        }
        out.extend(made.into_iter().rev());
    }

    pub(super) fn frame_extras(&mut self, frame: &Node, place: &str, spot: Spot) {
        for child in &frame.children {
            let (severity, message) = match child.name.as_str() {
                "elements" => continue,
                "Actionscript" => (Severity::Unsupported, "frame scripts are not imported"),
                "sound" | "DOMSound" => (Severity::Unsupported, "frame sounds are not imported"),
                "tweens" | "Motion" | "motionObjectXML" => (
                    Severity::Unsupported,
                    "motion-tween property editors are not imported",
                ),
                other => {
                    self.note(
                        Severity::Info,
                        place,
                        format!("<{other}> is kept as source data"),
                    );
                    self.keep(
                        Anchor::Layer(spot.layer),
                        Placement::FrameExtra { frame: spot.frame },
                        child.to_xml(),
                        place,
                    );
                    continue;
                }
            };
            self.note(severity, place, message);
            self.keep(
                Anchor::Layer(spot.layer),
                Placement::FrameExtra { frame: spot.frame },
                child.to_xml(),
                place,
            );
        }
        if frame.attr("name").is_some() {
            self.note(Severity::Info, place, "frame labels are not imported");
        }
    }

    pub(super) fn tween_for(&mut self, frame: &Node, contents: &[Content], place: &str) -> Tween {
        match frame.attr("tweenType") {
            None | Some("none") => Tween::None,
            Some("motion") if matches!(contents.first(), Some(Content::Instance(_))) => {
                if frame.attr("acceleration").is_some_and(|a| a != "0") {
                    self.note(
                        Severity::Warning,
                        place,
                        "tween easing is ignored; the tween is linear",
                    );
                }
                Tween::Linear
            }
            Some("motion") => {
                self.note(
                    Severity::Unsupported,
                    place,
                    "only instance transforms can be tweened",
                );
                Tween::None
            }
            Some(other) => {
                self.note(
                    Severity::Unsupported,
                    place,
                    format!("{other} tweens are not imported"),
                );
                Tween::None
            }
        }
    }
}
