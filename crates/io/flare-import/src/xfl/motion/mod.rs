//! Motion object tweens (Animate CS4 and later). A tween keyframe of an instance keeps its
//! motion in an `AnimationCore`: a curve per property, in ticks. Flare has one transform per
//! frame, so the curves are sampled into one keyframe per frame of the tween, each with the
//! matrix the curves give there. The keyframes are exact at each frame; the curves themselves
//! are kept as source data, so an export still carries them.

mod curves;
mod pose;

use super::*;
use crate::xml::Node;
use curves::{Channel, Motion};
use flare_core::{InstanceId, Transform2D};
use flare_document::{Content, FrameSpan, Instance};
use glamx::DVec2;
use pose::Change;
use std::num::NonZeroU32;

/// The longest motion object tween that is sampled. Beyond it the tween stays linear and says so.
pub(super) const MAX_SAMPLED_FRAMES: u32 = 4096;

impl Importer {
    /// The frame spans of a motion object tween on an instance: one held frame per frame of the
    /// tween, each at the matrix its curves give. `None` when the frame has no curves or they
    /// cannot be sampled; the caller then keeps the linear tween and the reason is noted.
    pub(super) fn motion_spans(
        &mut self,
        frame: &Node,
        instance: &Instance,
        index: u32,
        duration: NonZeroU32,
        place: &str,
    ) -> Option<Vec<FrameSpan>> {
        let Some(core) = find_core(frame) else {
            self.note(
                Severity::Unsupported,
                place,
                "motion object without curves; the tween moves the instance in a straight line",
            );
            return None;
        };
        let motion = curves::read(core, self.frame_rate);
        for gap in &motion.gaps {
            self.note(Severity::Unsupported, place, gap.clone());
        }
        if !motion.animates_anything() {
            return None;
        }
        if duration.get() > MAX_SAMPLED_FRAMES {
            self.note(
                Severity::Unsupported,
                place,
                format!(
                    "a motion object of {} frames is longer than the {MAX_SAMPLED_FRAMES} that are sampled; it moves in a straight line",
                    duration.get()
                ),
            );
            return None;
        }
        let base = instance.transform().as_affine();
        let pivot = pivot_of(frame);
        let mut spans = Vec::with_capacity(duration.get() as usize);
        for offset in 0..duration.get() {
            let matrix = if offset == 0 {
                base
            } else {
                let change = change_at(&motion, offset, self.frame_rate);
                match pose::apply(base, change, pivot) {
                    Some(matrix) => matrix,
                    None => {
                        self.note(
                            Severity::Unsupported,
                            place,
                            "a motion object gives a matrix that is not finite; it moves in a straight line",
                        );
                        return None;
                    }
                }
            };
            let Ok(transform) = Transform2D::new(matrix) else {
                self.note(
                    Severity::Unsupported,
                    place,
                    "a motion object gives a matrix that is not finite; it moves in a straight line",
                );
                return None;
            };
            let made = Instance::new(InstanceId::new(), instance.symbol(), transform)
                .with_playback(instance.playback());
            spans.push(FrameSpan::new(
                index.saturating_add(offset),
                NonZeroU32::MIN,
                Content::Instance(made),
            ));
        }
        Some(spans)
    }
}

/// The `AnimationCore` of a frame's motion object, as Animate writes it inside `motionObjectXML`
/// (or directly in the frame).
fn find_core(frame: &Node) -> Option<&Node> {
    frame
        .child("motionObjectXML")
        .and_then(|m| m.child("AnimationCore"))
        .or_else(|| frame.child("AnimationCore"))
}

/// The transformation point of the instance in the tween's first element, in its own
/// coordinates. The curves turn and scale about it; it is the origin when the file has none.
fn pivot_of(frame: &Node) -> DVec2 {
    let point = frame
        .child("elements")
        .and_then(|e| e.child("DOMSymbolInstance"))
        .and_then(|i| i.child("transformationPoint"))
        .and_then(|t| t.child("Point"));
    let number = |key: &str| {
        point
            .and_then(|p| p.attr(key))
            .and_then(|v| v.trim().parse::<f64>().ok())
            .filter(|v| v.is_finite())
    };
    match (number("x"), number("y")) {
        (Some(x), Some(y)) => DVec2::new(x, y),
        _ => DVec2::ZERO,
    }
}

/// What the curves add to the start matrix at `offset` frames into the tween. Values are taken
/// relative to the first tick, so the tween starts on the instance's own matrix.
fn change_at(motion: &Motion, offset: u32, frame_rate: f64) -> Change {
    let ticks = f64::from(offset) * motion.ticks_per_second / frame_rate;
    let delta = |channel: Channel| match (motion.value(channel, ticks), motion.value(channel, 0.0))
    {
        (Some(now), Some(start)) => now - start,
        _ => 0.0,
    };
    let scale = |channel: Channel| match (motion.value(channel, ticks), motion.value(channel, 0.0))
    {
        // A start of 0 has no ratio to take, so the value is read as a percentage of the size.
        (Some(now), Some(start)) if start != 0.0 => now / start,
        (Some(now), _) => now / 100.0,
        _ => 1.0,
    };
    Change {
        dx: delta(Channel::MoveX),
        dy: delta(Channel::MoveY),
        angle: delta(Channel::Rotation).to_radians(),
        skew_x: delta(Channel::SkewX).to_radians(),
        scale_x: scale(Channel::ScaleX),
        scale_y: scale(Channel::ScaleY),
    }
}
