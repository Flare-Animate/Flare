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
        // Motion guides by source index: the layer below one points at it, which needs no note.
        let mut guides: std::collections::HashSet<usize> = std::collections::HashSet::new();
        for (source_index, layer) in layers_node.children_named("DOMLayer").enumerate() {
            let name = layer.attr("name").unwrap_or("Layer");
            let outline = layer.attr("color").and_then(|c| super::color_from(c, None));
            let place = format!("{location} / layer \"{name}\"");
            let parent = layer
                .attr("parentLayerIndex")
                .and_then(|v| v.parse::<usize>().ok())
                .and_then(|i| folders.get(&i).copied());
            match layer.attr("layerType") {
                None | Some("normal") => {}
                Some("guide") => {
                    // A motion guide: its frames are imported like any layer's and the layer is
                    // marked, so the layer below it follows its path.
                    guides.insert(source_index);
                    let first = layers.len();
                    self.layer(layer, name, &place, &mut layers);
                    for made in &mut layers[first..] {
                        *made = made
                            .clone()
                            .with_guide(true)
                            .with_parent(parent)
                            .with_outline(outline);
                    }
                    continue;
                }
                Some("folder") => {
                    let id = LayerId::new();
                    if let Ok(folder) = Layer::new(id, name, Vec::new()) {
                        folders.insert(source_index, id);
                        layers.push(
                            folder
                                .with_folder(true)
                                .with_parent(parent)
                                .with_outline(outline)
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
            let points_at_guide = layer
                .attr("parentLayerIndex")
                .and_then(|v| v.parse::<usize>().ok())
                .is_some_and(|i| guides.contains(&i));
            if layer.attr("parentLayerIndex").is_some() && parent.is_none() && !points_at_guide {
                self.note(
                    Severity::Info,
                    &place,
                    "membership of something other than a folder is not kept",
                );
            }
            let first = layers.len();
            self.layer(layer, name, &place, &mut layers);
            for made in &mut layers[first..] {
                if parent.is_some() {
                    *made = made.clone().with_parent(parent);
                }
                if outline.is_some() {
                    *made = made.clone().with_outline(outline);
                }
            }
        }
        Timeline::new(layers)
    }

    /// Imports one DOMLayer. A keyframe may hold several elements; they stay together in one span,
    /// drawn in the order the file lists them (bottom to top), as Animate keeps them.
    pub(super) fn layer(&mut self, layer: &Node, name: &str, place: &str, out: &mut Vec<Layer>) {
        let mut spans: Vec<FrameSpan> = Vec::new();
        // The layer owns what could not be imported from it.
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
            let (tween, ease) = self.tween_for(frame, &contents, &here);
            let mut contents = contents.into_iter();
            let Some(first) = contents.next() else {
                continue;
            };
            let extras: Vec<Content> = contents.collect();
            let mut span = FrameSpan::new(index, duration, first);
            if extras.is_empty() {
                span = span.with_tween(tween).with_ease(ease);
            } else {
                if tween != Tween::None {
                    self.note(
                        Severity::Info,
                        &here,
                        "a keyframe of several elements cannot tween; it is held",
                    );
                }
                let count = extras.len() + 1;
                span = match span.clone().with_extras(extras) {
                    Ok(many) => many,
                    Err(error) => {
                        self.note(
                            Severity::Unsupported,
                            &here,
                            format!("{count} elements could not share a keyframe ({error}); only the first was imported"),
                        );
                        span
                    }
                };
            }
            spans.push(span);
        }
        let visible = bool_attr(layer, "visible", true);
        let locked = bool_attr(layer, "locked", false);
        match Layer::new(first_id, name, spans) {
            Ok(l) => out.push(l.with_visible(visible).with_locked(locked)),
            Err(_) => self.note(
                Severity::Unsupported,
                place,
                "frames overlap; the layer was skipped",
            ),
        }
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

    /// The tween a keyframe asks for, and its easing (XFL's `acceleration`, -100 to 100).
    pub(super) fn tween_for(
        &mut self,
        frame: &Node,
        contents: &[Content],
        place: &str,
    ) -> (Tween, i8) {
        let ease = frame
            .attr("acceleration")
            .and_then(|a| a.trim().parse::<f64>().ok())
            .filter(|a| a.is_finite())
            .map_or(0, |a| a.round().clamp(-100.0, 100.0) as i8);
        match (frame.attr("tweenType"), contents.first()) {
            (None | Some("none"), _) => (Tween::None, 0),
            (Some("motion"), Some(Content::Instance(_))) => (Tween::Linear, ease),
            (Some("motion"), _) => {
                self.note(
                    Severity::Unsupported,
                    place,
                    "only instance transforms can be tweened",
                );
                (Tween::None, 0)
            }
            (Some("shape"), Some(Content::Shape(_))) => (Tween::Shape, ease),
            (Some("shape"), _) => {
                self.note(
                    Severity::Unsupported,
                    place,
                    "a shape tween needs a drawn shape here; it was not imported",
                );
                (Tween::None, 0)
            }
            (Some(other), _) => {
                self.note(
                    Severity::Unsupported,
                    place,
                    format!("{other} tweens are not imported"),
                );
                (Tween::None, 0)
            }
        }
    }
}
