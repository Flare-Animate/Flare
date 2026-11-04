//! Keyframe tracks of one thing (a layer or a bone) as placements on a Flare timeline: one span
//! per key frame, each holding the pose at that frame, moving linearly to the next unless the
//! Moho segment is a step.

use std::num::NonZeroU32;

use flare_core::Transform2D;
use flare_document::Tween;
use glamx::{DAffine2, DVec2};

use crate::limits::MAX_FRAME;
use crate::model::{Channel, Key, KeyValue};

#[derive(Default, Clone, Copy)]
pub(crate) struct Rig<'a> {
    pub translation: Option<&'a Channel>,
    pub rotation: Option<&'a Channel>,
    pub scale: Option<&'a Channel>,
}

pub(crate) struct Placed {
    pub start: u32,
    pub duration: NonZeroU32,
    pub transform: Transform2D,
    pub tween: Tween,
}

pub(crate) struct Placement {
    pub spans: Vec<Placed>,
    /// A segment used an interpolation Flare's spans cannot express (smooth, ease, bounce) and
    /// became linear.
    pub approximated: bool,
}

fn is_hold(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "step" | "hold" | "constant" | "none"
    )
}

fn components(value: &KeyValue) -> Option<Vec<f64>> {
    match value {
        KeyValue::Number(n) => Some(vec![*n]),
        KeyValue::Vector(v) => Some(v.clone()),
        _ => None,
    }
}

fn usable(channel: Option<&Channel>) -> Option<&Channel> {
    channel.filter(|c| !c.muted && !c.is_empty())
}

/// The value at `frame`, and whether the segment it sits in holds. Keys need not be in order.
fn sample(channel: &Channel, frame: i64) -> Option<(Vec<f64>, bool)> {
    let mut keys: Vec<(usize, &Key)> = channel.keys.iter().enumerate().collect();
    keys.sort_by_key(|(_, k)| k.frame);
    let (_, first) = keys.first()?;
    let (_, last) = keys.last()?;
    if frame <= first.frame {
        return Some((components(&first.value)?, true));
    }
    if frame >= last.frame {
        return Some((components(&last.value)?, true));
    }
    let at = keys.iter().rposition(|(_, k)| k.frame <= frame)?;
    let (index, a) = keys[at];
    let (_, b) = keys[at + 1];
    let hold = channel.interp.get(index).is_some_and(|n| is_hold(n));
    let (va, vb) = (components(&a.value)?, components(&b.value)?);
    if hold || va.len() != vb.len() {
        return Some((va, true));
    }
    let t = (frame - a.frame) as f64 / (b.frame - a.frame) as f64;
    Some((
        va.iter().zip(&vb).map(|(x, y)| x + (y - x) * t).collect(),
        false,
    ))
}

/// Whether a segment of `channel` starts at `frame` and is not a hold.
fn segment_moves(channel: &Channel, frame: i64) -> bool {
    let mut keys: Vec<(usize, &Key)> = channel.keys.iter().enumerate().collect();
    keys.sort_by_key(|(_, k)| k.frame);
    keys.windows(2).any(|w| {
        w[0].1.frame == frame
            && w[1].1.frame > frame
            && !channel.interp.get(w[0].0).is_some_and(|n| is_hold(n))
    })
}

fn eased(channel: &Channel) -> bool {
    channel
        .interp
        .iter()
        .any(|n| !n.is_empty() && !is_hold(n) && !n.eq_ignore_ascii_case("linear"))
}

/// `origin` is where Moho's (0, 0) lies in the document, `units_to_px` the scale of its units.
/// Rotation is negated and y flipped because Moho's y axis points up. A one-number scale applies
/// to x only when `scale_x_only` (a bone's), else to both axes.
pub(crate) fn place(
    rig: Rig<'_>,
    end_frame: i64,
    origin: [f64; 2],
    units_to_px: f64,
    scale_x_only: bool,
) -> Placement {
    let channels = [
        usable(rig.translation),
        usable(rig.rotation),
        usable(rig.scale),
    ];
    let mut frames: Vec<i64> = vec![0];
    for channel in channels.iter().flatten() {
        frames.extend(channel.keys.iter().map(|k| k.frame));
    }
    frames.retain(|f| (0..=MAX_FRAME).contains(f));
    frames.sort_unstable();
    frames.dedup();
    let approximated = channels.iter().flatten().any(|c| eased(c));
    let mut spans = Vec::with_capacity(frames.len());
    for (i, frame) in frames.iter().enumerate() {
        let sampled = |c: Option<&Channel>| c.and_then(|c| sample(c, *frame));
        let moving = channels.iter().flatten().any(|c| segment_moves(c, *frame));
        let t = sampled(channels[0]).map_or([0.0, 0.0], |(v, _)| {
            [
                v.first().copied().unwrap_or(0.0),
                v.get(1).copied().unwrap_or(0.0),
            ]
        });
        let angle = sampled(channels[1]).map_or(0.0, |(v, _)| v.first().copied().unwrap_or(0.0));
        let scale = sampled(channels[2]).map_or(DVec2::ONE, |(v, _)| {
            let x = v.first().copied().unwrap_or(1.0);
            match v.get(1) {
                Some(y) => DVec2::new(x, *y),
                None if scale_x_only => DVec2::new(x, 1.0),
                None => DVec2::new(x, x),
            }
        });
        let matrix = DAffine2::from_scale_angle_translation(
            scale,
            -angle,
            DVec2::new(
                origin[0] + t[0] * units_to_px,
                origin[1] - t[1] * units_to_px,
            ),
        );
        let Ok(transform) = Transform2D::new(matrix) else {
            continue;
        };
        let next = frames
            .get(i + 1)
            .copied()
            .unwrap_or(end_frame.max(*frame) + 1);
        let length = u32::try_from(next - frame).ok().and_then(NonZeroU32::new);
        let (Ok(start), Some(duration)) = (u32::try_from(*frame), length) else {
            continue;
        };
        let has_next = i + 1 < frames.len();
        spans.push(Placed {
            start,
            duration,
            transform,
            tween: if moving && has_next {
                Tween::Linear
            } else {
                Tween::None
            },
        });
    }
    Placement {
        spans,
        approximated,
    }
}
