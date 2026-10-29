//! Frames, layers and timelines.

use flare_document::{Anchor, Attachment, Content, FrameSpan, Layer, Placement, Timeline, Tween};

use super::*;

pub(super) struct Ctx<'a> {
    pub(super) names: &'a Names,
    pub(super) notes: Vec<Diagnostic>,
    pub(super) approximated: bool,
    /// Source data kept by the importer, and which of it has been written back.
    pub(super) attachments: &'a [Attachment],
    pub(super) used: Vec<bool>,
}

impl<'a> Ctx<'a> {
    /// The unused attachments on `anchor` whose placement satisfies `wanted`, marked as
    /// used. Malformed ones are reported and skipped rather than written.
    pub(super) fn take(
        &mut self,
        anchor: Anchor,
        wanted: impl Fn(Placement) -> bool,
        place: &str,
    ) -> Vec<&'a str> {
        let mut out = Vec::new();
        for (index, attachment) in self.attachments.iter().enumerate() {
            if self.used[index] || attachment.anchor() != anchor || !wanted(attachment.placement())
            {
                continue;
            }
            self.used[index] = true;
            if attachment.format() != "xfl" {
                self.notes.push(Diagnostic {
                    severity: Severity::Warning,
                    location: place.to_owned(),
                    message: format!(
                        "preserved source data in the unknown format {:?} was not written",
                        attachment.format()
                    ),
                });
                continue;
            }
            if well_formed(attachment.content()) {
                out.push(attachment.content());
            } else {
                self.notes.push(Diagnostic {
                    severity: Severity::Warning,
                    location: place.to_owned(),
                    message: "preserved source data is not well-formed XML and was not written"
                        .into(),
                });
            }
        }
        out
    }
}

pub(super) fn write_frame(
    out: &mut Xml,
    span: &FrameSpan,
    layer: Anchor,
    place: &str,
    ctx: &mut Ctx<'_>,
) {
    let mut attrs = vec![
        a("index", span.start()),
        a("duration", span.duration().get()),
    ];
    if let (Tween::Linear, Content::Instance(_)) = (span.tween(), span.content()) {
        attrs.push(a("tweenType", "motion"));
    }
    out.open("DOMFrame", attrs);
    out.open("elements", []);
    match span.content() {
        Content::Empty => {}
        Content::Instance(instance) => write_instance(out, instance, ctx.names),
        Content::Shape(shape) => {
            let Ctx {
                notes,
                approximated,
                ..
            } = ctx;
            write_shape(out, shape, place, notes, approximated);
        }
    }
    let frame = span.start();
    for xml in ctx.take(layer, |p| p == Placement::FrameElement { frame }, place) {
        out.raw(xml);
    }
    out.close("elements");
    for xml in ctx.take(layer, |p| p == Placement::FrameExtra { frame }, place) {
        out.raw(xml);
    }
    out.close("DOMFrame");
}

/// `mask_parent` is the output index of the mask layer that masks this one, if any, and
/// `folder_parent` that of the folder it sits in. XFL has one parent index per layer, so a layer
/// that is both masked and in a folder keeps its folder.
pub(super) fn write_layer(
    out: &mut Xml,
    layer: &Layer,
    mask_parent: Option<usize>,
    folder_parent: Option<usize>,
    place: &str,
    ctx: &mut Ctx<'_>,
) {
    let mut attrs = vec![a("name", layer.name())];
    if layer.is_folder() {
        attrs.push(a("layerType", "folder"));
        if let Some(parent) = folder_parent {
            attrs.push(a("parentLayerIndex", parent));
        }
    } else if layer.is_guide() {
        attrs.push(a("layerType", "guide"));
        if let Some(parent) = folder_parent {
            attrs.push(a("parentLayerIndex", parent));
        }
    } else if layer.is_mask() {
        // Animate keeps mask layers locked; the layers they mask point back at them.
        attrs.push(a("layerType", "mask"));
        attrs.push(a("locked", "true"));
    } else if let Some(parent) = folder_parent {
        attrs.push(a("parentLayerIndex", parent));
        if mask_parent.is_some() {
            ctx.notes.push(Diagnostic {
                severity: Severity::Warning,
                location: format!("{place} / layer \"{}\"", layer.name()),
                message: "a layer in a folder cannot also be masked in XFL; the mask was dropped"
                    .into(),
            });
        }
    } else if let Some(parent) = mask_parent {
        attrs.push(a("parentLayerIndex", parent));
    }
    if let Some(color) = layer.outline() {
        attrs.push(a("color", hex(color)));
    }
    if !layer.is_visible() {
        attrs.push(a("visible", "false"));
    }
    if layer.is_locked() && !layer.is_mask() {
        attrs.push(a("locked", "true"));
    }
    out.open("DOMLayer", attrs);
    out.open("frames", []);
    let here = format!("{place} / layer \"{}\"", layer.name());
    if layer.spans().is_empty() {
        // Animate requires every layer to have a frame; keep the layer alive on re-import.
        if !layer.is_folder() {
            ctx.notes.push(Diagnostic {
                severity: Severity::Info,
                location: here.clone(),
                message: "an empty layer was written with one empty frame".into(),
            });
        }
        out.open("DOMFrame", [a("index", 0), a("duration", 1)]);
        out.open("elements", []);
        let anchor = Anchor::Layer(layer.id());
        for xml in ctx.take(anchor, |p| p == Placement::FrameElement { frame: 0 }, &here) {
            out.raw(xml);
        }
        out.close("elements");
        for xml in ctx.take(anchor, |p| p == Placement::FrameExtra { frame: 0 }, &here) {
            out.raw(xml);
        }
        out.close("DOMFrame");
    }
    let mut cursor = 0_u64;
    for span in layer.spans() {
        if u64::from(span.start()) > cursor {
            let gap = u64::from(span.start()) - cursor;
            ctx.notes.push(Diagnostic {
                severity: Severity::Warning,
                location: here.clone(),
                message: format!(
                    "a gap of {gap} frame(s) before frame {} became empty frames",
                    span.start()
                ),
            });
            out.open("DOMFrame", [a("index", cursor), a("duration", gap)]);
            out.empty("elements", []);
            out.close("DOMFrame");
        }
        write_frame(out, span, Anchor::Layer(layer.id()), &here, ctx);
        cursor = span.end();
    }
    out.close("frames");
    out.close("DOMLayer");
}

pub(super) fn write_timeline(
    out: &mut Xml,
    name: &str,
    timeline: &Timeline,
    place: &str,
    owner: Anchor,
    ctx: &mut Ctx<'_>,
) {
    out.open("DOMTimeline", [a("name", name)]);
    out.open("layers", []);
    let count = timeline.layers().len();
    // Layers written so far, kept ones included: a mask is referred to by this index.
    let mut written = 0_usize;
    let mut mask_index: Option<usize> = None;
    // Where each layer went in the output, for folders to be referred to by.
    let mut indexes: std::collections::HashMap<flare_core::LayerId, usize> =
        std::collections::HashMap::new();
    for (position, layer) in timeline.layers().iter().enumerate() {
        // Whole layers that were kept go back at their original index among the layers.
        let at =
            |p: Placement| matches!(p, Placement::LayerAt { index } if index as usize == position);
        for xml in ctx.take(owner, at, place) {
            out.raw(xml);
            written += 1;
        }
        // A mask masks, and a motion guide guides, the layer directly below it in the document;
        // XFL points the layer below at either by index.
        let parent = mask_index.take();
        let folder = layer.parent().and_then(|id| indexes.get(&id).copied());
        indexes.insert(layer.id(), written);
        write_layer(out, layer, parent, folder, place, ctx);
        if layer.is_mask() || layer.is_guide() {
            mask_index = Some(written);
        }
        written += 1;
    }
    // Any whose index lies beyond the layers there are now go last.
    let after = |p: Placement| matches!(p, Placement::LayerAt { index } if index as usize >= count);
    for xml in ctx.take(owner, after, place) {
        out.raw(xml);
    }
    out.close("layers");
    out.close("DOMTimeline");
}
