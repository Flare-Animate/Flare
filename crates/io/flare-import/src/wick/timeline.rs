//! A Wick timeline as Flare layers. Each layer's frames become spans. A frame with tweens becomes a
//! run of spans: each holds the clip at one tween's transform, and the span's Flare tween carries
//! the motion to the next, with the easing Wick names. Paths and pictures are not tweened in Wick,
//! so they repeat in every span of the run.

use std::num::NonZeroU32;

use flare_core::LayerId;
use flare_document::{
    Anchor, Content, FrameSpan, Layer, MAX_ELEMENTS, SpanInterp, Timeline, Tween,
};
use serde_json::Value;

use super::ctx::{Ctx, Inst, Values};
use super::easing;
use super::error::WickError;
use super::json::{child_ids, number, text, whole};
use super::limits::{MAX_FRAME, MAX_LAYERS};
use crate::xfl::Severity;

/// Builds the timeline of a clip or button, or of the root clip as the scene.
pub(super) fn build(
    ctx: &mut Ctx,
    timeline_id: &str,
    owner: Anchor,
) -> Result<Timeline, WickError> {
    let timeline = ctx.objects.of_class(timeline_id, "Timeline")?;
    let layer_ids = child_ids(timeline)?;
    if layer_ids.len() > MAX_LAYERS {
        return Err(WickError::TooLarge("the layers of a timeline"));
    }
    let mut layers = Vec::with_capacity(layer_ids.len());
    for (index, layer_id) in layer_ids.iter().enumerate() {
        let layer = ctx.objects.of_class(layer_id, "Layer")?;
        let name = text(layer, "name")
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .map_or_else(|| format!("Layer {}", index + 1), str::to_owned);
        let mut spans = Vec::new();
        for frame_id in child_ids(layer)? {
            frame_spans(ctx, &frame_id, &owner, &name, &mut spans)?;
        }
        let hidden = layer
            .get("hidden")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let locked = layer
            .get("locked")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let made = Layer::new(LayerId::new(), name, spans).map_err(|_| WickError::Overlap)?;
        layers.push(made.with_visible(!hidden).with_locked(locked));
    }
    Ok(Timeline::new(layers))
}

/// One Wick tween: the frame it falls on, its transform and its easing.
struct Key {
    at: u32,
    values: Values,
    easing: String,
}

fn frame_spans(
    ctx: &mut Ctx,
    frame_id: &str,
    owner: &Anchor,
    layer: &str,
    spans: &mut Vec<FrameSpan>,
) -> Result<(), WickError> {
    let objects = ctx.objects;
    let frame = objects.of_class(frame_id, "Frame")?;
    let start = whole(number(frame, "start"), 1, MAX_FRAME).ok_or(WickError::BadField("start"))?;
    let end = whole(number(frame, "end"), u64::from(start), MAX_FRAME)
        .ok_or(WickError::BadField("end"))?;
    let len = end - start + 1;
    let duration = NonZeroU32::new(len).ok_or(WickError::BadField("end"))?;
    let at0 = start - 1;
    let place = format!("frame {start} of layer \"{layer}\"");
    ctx.scripts(frame_id, frame, *owner, &place);
    if let Some(label) = text(frame, "name").map(str::trim).filter(|n| !n.is_empty()) {
        let content = format!("{place}: frame label \"{label}\"");
        ctx.keep(frame_id, owner, content);
    }
    if frame.get("sound").is_some_and(|v| !v.is_null()) {
        ctx.note(
            frame_id,
            "sound",
            Severity::Unsupported,
            "sounds are not imported",
        );
    }

    let mut instances: Vec<Inst> = Vec::new();
    let mut drawing: Vec<String> = Vec::new();
    let mut tweens: Vec<&Value> = Vec::new();
    for child in child_ids(frame)? {
        match objects.class(&child) {
            "Clip" | "Button" => instances.push(ctx.instance(&child)?),
            "Path" => drawing.push(child),
            "Tween" => tweens.push(objects.of_class(&child, "Tween")?),
            _ => ctx.note(
                frame_id,
                "frame child",
                Severity::Unsupported,
                "items of an unknown kind in a frame are not imported",
            ),
        }
    }

    let mut keys: Vec<Key> = Vec::new();
    for tween in &tweens {
        // Wick drops a tween that is not on the frame, so one out of range is skipped.
        let Some(at) = whole(number(tween, "playheadPosition"), 1, u64::from(len)) else {
            continue;
        };
        if number(tween, "fullRotations").is_some_and(|r| r != 0.0) {
            ctx.note(frame_id, "full rotations", Severity::Warning, "full rotations of tweens are not carried; the angle keeps only the turn within a circle");
        }
        let opacity = tween
            .get("transformation")
            .and_then(|t| t.get("opacity"))
            .and_then(Value::as_f64);
        if opacity.is_some_and(|o| o != 1.0) {
            ctx.note(
                frame_id,
                "tween opacity",
                Severity::Warning,
                "opacity of tweens is not imported; Flare instances have no alpha",
            );
        }
        keys.push(Key {
            at,
            values: Values::read(tween.get("transformation")),
            easing: text(tween, "easingType").unwrap_or("none").to_owned(),
        });
    }
    keys.sort_by_key(|k| k.at);
    keys.dedup_by_key(|k| k.at);

    if keys.is_empty() || instances.is_empty() {
        if !keys.is_empty() {
            ctx.note(
                frame_id,
                "tween without clip",
                Severity::Info,
                "tweens in a frame with no clip are ignored, as Wick does",
            );
        }
        let mut elements = plain(ctx, &instances, &drawing)?;
        spans.push(span(at0, duration, &mut elements, Tween::None, None)?);
        return Ok(());
    }

    if instances.len() > 1 {
        ctx.note(frame_id, "several clips tweened", Severity::Info, "a frame with several clips moves the first clip; the others are held at their stored transform");
    }
    if keys.first().is_some_and(|k| k.at > 1) {
        let lead =
            NonZeroU32::new(keys[0].at - 1).ok_or(WickError::BadField("playheadPosition"))?;
        let mut elements = plain(ctx, &instances, &drawing)?;
        spans.push(span(at0, lead, &mut elements, Tween::None, None)?);
    }
    for (index, key) in keys.iter().enumerate() {
        let next = keys.get(index + 1).map(|n| n.at);
        let until = next.unwrap_or(len + 1);
        let run = NonZeroU32::new(until - key.at).ok_or(WickError::BadField("playheadPosition"))?;
        let mut elements = Vec::new();
        for inst in &instances {
            elements.push(inst.content(key.values)?);
        }
        elements.extend(ctx.drawing(&drawing)?);
        let first_at = at0 + (key.at - 1);
        let (tween, interp) = if next.is_some() {
            (Tween::Linear, easing::interp(&key.easing)?)
        } else {
            (Tween::None, None)
        };
        spans.push(span(first_at, run, &mut elements, tween, interp)?);
    }
    Ok(())
}

/// The frame's own content: its clips at their stored transforms, then its drawing.
fn plain(ctx: &mut Ctx, instances: &[Inst], drawing: &[String]) -> Result<Vec<Content>, WickError> {
    let mut out = Vec::new();
    for inst in instances {
        out.push(inst.content(inst.values)?);
    }
    out.extend(ctx.drawing(drawing)?);
    Ok(out)
}

/// A span whose first element is the one its tween moves, the others held with it.
fn span(
    start: u32,
    duration: NonZeroU32,
    elements: &mut Vec<Content>,
    tween: Tween,
    interp: Option<SpanInterp>,
) -> Result<FrameSpan, WickError> {
    if elements.len() > MAX_ELEMENTS {
        return Err(WickError::TooLarge("the elements of a frame"));
    }
    let first = if elements.is_empty() {
        Content::Empty
    } else {
        elements.remove(0)
    };
    let mut made = FrameSpan::new(start, duration, first)
        .with_tween(tween)
        .with_interp(interp);
    if !elements.is_empty() {
        made = made
            .with_extras(std::mem::take(elements))
            .map_err(|_| WickError::TooLarge("the elements of a frame"))?;
    }
    Ok(made)
}
