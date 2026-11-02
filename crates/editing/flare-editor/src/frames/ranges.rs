//! Frame operations on a range: taking a run of frames out (Cut Frames) and reversing their order
//! (Reverse Frames).

use std::fmt;

use super::*;

/// Why a range of frames cannot be reversed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReverseRefusal {
    /// The range is backwards, or holds no keyframe.
    NothingThere,
    /// A keyframe starts or ends outside the range, so reversing would cut it in two.
    CutsAKeyframe,
}

impl fmt::Display for ReverseRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NothingThere => "there are no keyframes in those frames to reverse",
            Self::CutsAKeyframe => {
                "select whole keyframes to reverse them: a keyframe's frames go together"
            }
        })
    }
}

/// Takes frames `from..=to` out of `layer`: the spans they cut through are shortened, a span they
/// cover entirely goes, and what comes after moves left by the number of frames taken, with the
/// sound cues beside it. `None` when the range is backwards or holds no span.
pub fn remove_frames(scene: SceneId, layer: &Layer, from: u32, to: u32) -> Option<Vec<Command>> {
    if from > to {
        return None;
    }
    let stop = u64::from(to) + 1;
    let taken = u32::try_from(stop - u64::from(from)).ok()?;
    if !layer
        .spans()
        .iter()
        .any(|s| u64::from(s.start()) < stop && s.end() > u64::from(from))
    {
        return None;
    }
    let mut out = Vec::with_capacity(layer.spans().len());
    for s in layer.spans() {
        if s.end() <= u64::from(from) {
            out.push(s.clone());
        } else if u64::from(s.start()) >= stop {
            out.push(shifted(s, s.start() - taken)?);
        } else {
            let overlap = s.end().min(stop) - u64::from(s.start()).max(u64::from(from));
            let left = u64::from(s.duration().get()) - overlap;
            let start = s.start().min(from);
            if left > 0 {
                out.push(retime(s, start, u32::try_from(left).ok()?)?);
            }
        }
    }
    let mut commands = replace_spans(scene, layer, out);
    let cues: Vec<SoundCue> = layer
        .cues()
        .iter()
        .filter(|c| c.start < from || u64::from(c.start) >= stop)
        .map(|c| SoundCue {
            start: if u64::from(c.start) >= stop {
                c.start - taken
            } else {
                c.start
            },
            ..*c
        })
        .collect();
    if cues.as_slice() != layer.cues() {
        commands.push(Command::SetCues {
            scene,
            layer: layer.id(),
            cues,
        });
    }
    Some(commands)
}

/// Reverses the keyframes in frames `from..=to` of `layer`: the last becomes the first, with the
/// gaps mirrored too. Every keyframe that touches the range must lie wholly inside it. A tween
/// goes with the pair of keyframes it joined, now the other way round, so its easing is reversed;
/// the tween of the last keyframe of the range led out of it and is dropped. Sound cues stay where
/// they are: a sound cannot play backwards.
pub fn reverse_frames(
    scene: SceneId,
    layer: &Layer,
    from: u32,
    to: u32,
) -> Result<Vec<Command>, ReverseRefusal> {
    if from > to {
        return Err(ReverseRefusal::NothingThere);
    }
    let stop = u64::from(to) + 1;
    let touched: Vec<&FrameSpan> = layer
        .spans()
        .iter()
        .filter(|s| u64::from(s.start()) < stop && s.end() > u64::from(from))
        .collect();
    if touched.is_empty() {
        return Err(ReverseRefusal::NothingThere);
    }
    if touched.iter().any(|s| s.start() < from || s.end() > stop) {
        return Err(ReverseRefusal::CutsAKeyframe);
    }
    let mut out: Vec<FrameSpan> = layer
        .spans()
        .iter()
        .filter(|s| !touched.iter().any(|t| t.start() == s.start()))
        .cloned()
        .collect();
    for (i, s) in touched.iter().enumerate() {
        let start = u32::try_from(u64::from(from) + stop - s.end())
            .map_err(|_| ReverseRefusal::NothingThere)?;
        // Every element stays; only the motion is rebuilt below.
        let mut mirrored = s.held().retimed(start, s.duration());
        // The tween that led from the span before this one into it now leads out of it.
        if let Some(before) = i.checked_sub(1).map(|b| touched[b])
            && before.end() == u64::from(s.start())
        {
            mirrored = mirrored
                .with_tween(before.tween())
                .with_ease(before.ease().saturating_neg());
        }
        out.push(mirrored);
    }
    out.sort_by_key(FrameSpan::start);
    Ok(replace_spans(scene, layer, out))
}
