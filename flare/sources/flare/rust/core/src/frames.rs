//! Frame-level operations on a single layer, matching Animate's timeline
//! semantics. Pure functions over `Layer` so they are easy to test; the
//! `Editor` applies them across every layer in a frame selection.

use crate::model::*;

/// What a keyframe created at `f` should contain: the visible (possibly
/// tweened) content, or past the layer's end the last keyframe's content.
pub fn content_at(layer: &Layer, f: u32) -> (Vec<Element>, Tween) {
    if let Some((els, _)) = layer_elements_at(layer, f) {
        let tween = layer
            .span_at(f)
            .map(|(_, k, _)| k.tween)
            .unwrap_or(Tween::None);
        return (els, tween);
    }
    let last = layer
        .keyframes
        .values()
        .next_back()
        .map(|k| k.elements.clone())
        .unwrap_or_default();
    (last, Tween::None)
}

/// Keyframe starts in `a..=b` (empty when `a > b`, instead of panicking).
fn keys_in(layer: &Layer, a: u32, b: u32) -> Vec<u32> {
    if a > b {
        return vec![];
    }
    layer.keyframes.range(a..=b).map(|(k, _)| *k).collect()
}

fn ensure_first(layer: &mut Layer) {
    if layer.end == 0 {
        layer.end = 1;
    }
    layer.keyframes.entry(0).or_insert_with(Keyframe::blank);
}

/// A keyframe holding what frame `f` shows: content, the span's tween and,
/// inside a motion tween, the rest of its property keys (rebased to `f`).
pub fn keyframe_at(layer: &Layer, f: u32) -> Keyframe {
    let (els, tween) = content_at(layer, f);
    let mut kf = Keyframe::with(els);
    kf.tween = tween;
    if let Some((s, src, _)) = layer.span_at(f) {
        kf.playback_offset = src.playback_frame(f - s);
        kf.playback_reversed = src.playback_reversed;
        kf.ease_curve = src.ease_curve.clone();
        kf.orient_to_path = src.orient_to_path;
        if src.tween_boundary.is_some() {
            // A requested standalone pose is a sample. Timeline splits and
            // copies use retained_interval to preserve the authored curve.
            kf.tween = Tween::None;
            kf.ease_curve = None;
            kf.orient_to_path = false;
            return kf;
        }
        if let (Some(m), Some(el)) = (&src.motion, kf.elements.first()) {
            let mut head = (**m).clone();
            let pose = crate::motion::Pose::of(el, m.pivot);
            kf.motion = Some(Box::new(head.split(f - s, &pose)));
        }
    }
    kf
}

/// Insert a keyframe at `f`, retaining both authored source intervals when a
/// tween is split. Static spans keep their visible content and playback clock.
fn split_at(layer: &mut Layer, f: u32) {
    if layer.keyframes.contains_key(&f) {
        return;
    }
    if let Some((s, _, _)) = layer.span_at(f) {
        if let (Some(head), Some(tail)) = (
            retained_interval(layer, s, f),
            retained_interval(layer, f, layer.span_end(s)),
        ) {
            layer.keyframes.insert(s, head);
            layer.keyframes.insert(f, tail);
            return;
        }
    }
    let kf = keyframe_at(layer, f);
    if let Some((s, _, _)) = layer.span_at(f) {
        if let Some(pose) = kf
            .elements
            .first()
            .zip(kf.motion.as_ref())
            .map(|(el, m)| crate::motion::Pose::of(el, m.pivot))
        {
            if let Some(m) = layer.keyframes.get_mut(&s).and_then(|k| k.motion.as_mut()) {
                m.split(f - s, &pose);
            }
        }
    }
    layer.keyframes.insert(f, kf);
}

/// Pin the content that frame `f` currently shows into a keyframe at `f`, so
/// edits before `f` can't change it. No-op past the layer end.
fn pin(layer: &mut Layer, f: u32) {
    if f < layer.end && !layer.keyframes.contains_key(&f) {
        split_at(layer, f);
    }
}

fn shift_after(layer: &mut Layer, after: u32, delta: i64) {
    let moved: Vec<(u32, Keyframe)> = layer
        .keyframes
        .range(after + 1..)
        .map(|(k, v)| (*k, v.clone()))
        .collect();
    for (k, _) in &moved {
        layer.keyframes.remove(k);
    }
    for (k, v) in moved {
        let nk = (k as i64 + delta).max(0) as u32;
        layer.keyframes.insert(nk, v);
    }
}

/// F5: insert `n` frames at `at`, lengthening the span there. Past the end,
/// extends the layer up to and including `at`.
pub fn insert_frames(layer: &mut Layer, at: u32, n: u32) {
    if at >= layer.end {
        layer.end = at + 1;
        return;
    }
    shift_after(layer, at, n as i64);
    layer.end += n;
}

/// Shift+F5: delete frames `f0..=f1`; later frames move left.
pub fn remove_frames(layer: &mut Layer, f0: u32, f1: u32) {
    if f0 >= layer.end {
        return;
    }
    let f1 = f1.min(layer.end - 1);
    pin(layer, f0);
    pin(layer, f1 + 1);
    let n = f1 - f0 + 1;
    let doomed = keys_in(layer, f0, f1);
    for k in doomed {
        layer.keyframes.remove(&k);
    }
    shift_after(layer, f1, -(n as i64));
    layer.end -= n;
    ensure_first(layer);
}

/// F6 (keep content) / F7 (blank): make every frame in `f0..=f1` a keyframe.
pub fn convert_to_keyframes(layer: &mut Layer, f0: u32, f1: u32, blank: bool) {
    if blank {
        let authored_start = layer.keyframes.contains_key(&f0);
        pin(layer, f0);
        pin(layer, f1 + 1);
        if !authored_start && f0 < layer.end {
            layer.keyframes.insert(f0, Keyframe::blank());
        }
    }
    for f in f0..=f1 {
        if layer.keyframes.contains_key(&f) {
            continue;
        }
        if blank {
            layer.keyframes.insert(f, Keyframe::blank());
        } else if f < layer.end {
            split_at(layer, f);
        } else {
            let (els, _) = content_at(layer, f);
            layer.keyframes.insert(f, Keyframe::with(els));
        }
    }
    layer.end = layer.end.max(f1 + 1);
}

/// Shift+F6: turn keyframes in the range back into plain frames.
pub fn clear_keyframes(layer: &mut Layer, f0: u32, f1: u32) {
    let doomed = keys_in(layer, f0.max(1), f1);
    for k in doomed {
        // Removing a boundary introduced by a lossless split rejoins matching
        // source intervals, instead of stretching the shortened head's domain.
        if let Some((&previous, head)) = layer.keyframes.range(..k).next_back() {
            let tail = &layer.keyframes[&k];
            if let (Some(a), Some(b)) = (&head.tween_boundary, &tail.tween_boundary) {
                let head_len = k - previous;
                let tail_len = layer.span_end(k) - k;
                let adjacent = if a.reversed {
                    a.time_start == b.time_end
                } else {
                    a.time_end == b.time_start
                };
                let rate_a = (a.time_end - a.time_start) / head_len as f32;
                let rate_b = (b.time_end - b.time_start) / tail_len as f32;
                let mut comparable = tail.clone();
                comparable.label = head.label.clone();
                comparable.script = head.script.clone();
                comparable.playback_offset = head.playback_offset;
                comparable.tween_boundary = head.tween_boundary.clone();
                if adjacent
                    && (rate_a - rate_b).abs() < 0.00001
                    && a.duration == b.duration
                    && a.reversed == b.reversed
                    && a.elements == b.elements
                    && a.shape_hints == b.shape_hints
                    && head.playback_frame(head_len) == tail.playback_offset
                    && comparable == *head
                {
                    let (start, end) = (a.time_start.min(b.time_start), a.time_end.max(b.time_end));
                    let boundary = layer
                        .keyframes
                        .get_mut(&previous)
                        .unwrap()
                        .tween_boundary
                        .as_mut()
                        .unwrap();
                    boundary.time_start = start;
                    boundary.time_end = end;
                }
            }
        }
        layer.keyframes.remove(&k);
    }
}

/// Delete on frames ("Clear Frames"): the range becomes one blank keyframe;
/// frames after it keep what they showed.
pub fn clear_frames(layer: &mut Layer, f0: u32, f1: u32) {
    if f0 >= layer.end {
        return;
    }
    let f1 = f1.min(layer.end - 1);
    pin(layer, f0);
    pin(layer, f1 + 1);
    let doomed = keys_in(layer, f0, f1);
    for k in doomed {
        layer.keyframes.remove(&k);
    }
    layer.keyframes.insert(f0, Keyframe::blank());
}

/// Apply a tween to every span overlapping `f0..=f1`. Animate needs an end
/// keyframe; one is created at the span's last frame when missing.
pub fn set_tween(layer: &mut Layer, f0: u32, f1: u32, tween: Tween) {
    let starts: Vec<u32> = layer
        .keyframes
        .keys()
        .copied()
        .filter(|&s| s <= f1 && layer.span_end(s) > f0)
        .collect();
    for s in starts {
        if s >= layer.end {
            continue;
        }
        let end = layer.span_end(s);
        if !matches!(tween, Tween::None) && !layer.keyframes.contains_key(&end) && end > s + 1 {
            let content = layer.keyframes[&s].elements.clone();
            layer.keyframes.insert(end - 1, Keyframe::with(content));
        }
        // The closing keyframe of the last span has nothing to tween to.
        let has_next = !keys_in(layer, s + 1, layer.end.saturating_sub(1)).is_empty();
        if let Some(k) = layer.keyframes.get_mut(&s) {
            k.tween = if has_next { tween } else { Tween::None };
            k.motion = None;
            k.tween_boundary = None;
        }
    }
}

/// A copied run of frames, relative to its first frame.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameClip {
    pub len: u32,
    pub keyframes: Vec<(u32, Keyframe)>,
}

pub fn copy_frames(layer: &Layer, f0: u32, f1: u32) -> FrameClip {
    let mut keyframes = Vec::new();
    let first = layer
        .keyframes
        .get(&f0)
        .cloned()
        .unwrap_or_else(|| keyframe_at(layer, f0));
    keyframes.push((0, first));
    for k in keys_in(layer, f0 + 1, f1) {
        keyframes.push((k - f0, layer.keyframes[&k].clone()));
    }
    // Preserve authored curves over arbitrary source intervals. Only malformed
    // classic/shape spans without an endpoint need the compatibility sampling.
    let mut sampled = std::collections::BTreeMap::new();
    for (&start, key) in layer.keyframes.range(..=f1) {
        let end = layer.span_end(start);
        if end <= f0 || key.tween == Tween::None {
            continue;
        }
        let clipped_end = if matches!(key.tween, Tween::Motion { .. }) {
            end > f1.saturating_add(1)
        } else {
            end > f1
        };
        if start < f0 || clipped_end {
            if let Some(head) =
                retained_interval(layer, start.max(f0), f1.saturating_add(1).min(end))
            {
                sampled.insert(start.max(f0) - f0, head);
                continue;
            }
        }
        if start < f0 || clipped_end {
            for f in start.max(f0)..end.min(f1.saturating_add(1)) {
                let (elements, _) = content_at(layer, f);
                let mut pose = layer
                    .keyframes
                    .get(&f)
                    .cloned()
                    .unwrap_or_else(Keyframe::blank);
                pose.elements = elements;
                pose.tween = Tween::None;
                pose.motion = None;
                pose.tween_boundary = None;
                pose.ease_curve = None;
                pose.shape_hints.clear();
                pose.orient_to_path = false;
                pose.playback_offset = key.playback_frame(f - start);
                pose.playback_reversed = key.playback_reversed;
                sampled.insert(f - f0, pose);
            }
        }
    }
    if !sampled.is_empty() {
        let mut all: std::collections::BTreeMap<_, _> = keyframes.into_iter().collect();
        all.extend(sampled);
        keyframes = all.into_iter().collect();
    }
    FrameClip {
        len: f1 - f0 + 1,
        keyframes,
    }
}

fn retained_interval(layer: &Layer, from: u32, end: u32) -> Option<Keyframe> {
    let (start, key, _) = layer.span_at(from)?;
    if key.tween == Tween::None || end <= from {
        return None;
    }
    let span_end = layer.span_end(start);
    let target = if key.tween.is_motion() {
        None
    } else {
        layer.keyframes.get(&span_end)
    };
    if !key.tween.is_motion() && target.is_none() && key.tween_boundary.is_none() {
        return None;
    }
    let mut boundary = key
        .tween_boundary
        .as_deref()
        .cloned()
        .unwrap_or_else(|| TweenBoundary {
            duration: span_end - start,
            time_start: 0.0,
            time_end: (span_end - start) as f32,
            reversed: false,
            elements: target.map(|k| k.elements.clone()).unwrap_or_default(),
            shape_hints: target.map(|k| k.shape_hints.clone()).unwrap_or_default(),
            pose_edits: vec![],
        });
    let step = (boundary.time_end - boundary.time_start) / (span_end - start) as f32;
    let (lo, hi) = if boundary.reversed {
        (span_end - end, span_end - from)
    } else {
        (from - start, end - start)
    };
    let origin = boundary.time_start;
    boundary.time_start = origin + lo as f32 * step;
    boundary.time_end = origin + hi as f32 * step;
    let mut result = key.clone();
    result.tween_boundary = Some(Box::new(boundary));
    result.playback_offset = key.playback_frame(from - start);
    if from != start {
        result.label = None;
        result.script = None;
    }
    Some(result)
}

/// Compatibility fallback for object edits which cannot yet rebase authored
/// properties in a retained interval. Keeps evaluated poses and clocks. Studio
/// announces this conversion; lossless pose editing remains a P1 requirement.
pub fn materialize_retained_boundaries(layer: &mut Layer, f0: u32, f1: u32) {
    if !layer
        .keyframes
        .range(..=f1)
        .any(|(&start, key)| key.tween_boundary.is_some() && layer.span_end(start) > f0)
    {
        return;
    }
    let source = layer.clone();
    for (&start, key) in source.keyframes.range(..=f1) {
        let end = source.span_end(start);
        if end <= f0 || key.tween_boundary.is_none() {
            continue;
        }
        for f in start..end {
            let (elements, clock) = layer_elements_at(&source, f).unwrap();
            let mut pose = Keyframe::with(elements);
            pose.playback_offset = clock;
            pose.playback_reversed = key.playback_reversed;
            if f == start {
                pose.label = key.label.clone();
                pose.script = key.script.clone();
            }
            layer.keyframes.insert(f, pose);
        }
    }
}

/// Paste replaces frames `at..at+len`; frames after keep their content.
pub fn paste_frames(layer: &mut Layer, at: u32, clip: &FrameClip) {
    let end = at + clip.len;
    // A later disjoint paste must not stretch the preceding retained clip into
    // the unselected gap merely by extending the layer's duration.
    if at > layer.end
        && layer
            .keyframes
            .values()
            .next_back()
            .is_some_and(|k| k.tween_boundary.is_some())
    {
        layer.keyframes.insert(layer.end, Keyframe::blank());
    }
    pin(layer, at);
    pin(layer, end);
    let doomed = keys_in(layer, at, end - 1);
    for k in doomed {
        layer.keyframes.remove(&k);
    }
    for (rel, kf) in &clip.keyframes {
        layer.keyframes.insert(at + rel, kf.clone());
    }
    layer.end = layer.end.max(end);
    ensure_first(layer);
}

/// Drag frames: move the keyframes in `f0..=f1` by `delta` frames, like
/// dragging them in Animate. Spans around the old position stretch to fill
/// the gap; keyframes in the destination block are replaced. If the block
/// holds no keyframe, what its first frame shows is dropped as a keyframe.
pub fn move_frames(layer: &mut Layer, f0: u32, f1: u32, delta: i64) {
    if delta == 0 || f0 >= layer.end {
        return;
    }
    let delta = delta.max(-(f0 as i64));
    let f1 = f1.min(layer.end - 1);
    if layer.keyframes.values().any(|k| k.tween_boundary.is_some()) {
        let clip = copy_frames(layer, f0, f1);
        clear_frames(layer, f0, f1);
        paste_frames(layer, (f0 as i64 + delta) as u32, &clip);
        return;
    }
    let keys = keys_in(layer, f0, f1);
    let block: Vec<(u32, Keyframe)> = if keys.is_empty() {
        vec![(0, keyframe_at(layer, f0))]
    } else {
        let b = keys
            .iter()
            .map(|k| (k - f0, layer.keyframes[k].clone()))
            .collect();
        for k in &keys {
            layer.keyframes.remove(k);
        }
        b
    };
    let n0 = (f0 as i64 + delta) as u32;
    let n1 = n0 + (f1 - f0);
    for k in keys_in(layer, n0, n1) {
        layer.keyframes.remove(&k);
    }
    for (rel, kf) in block {
        layer.keyframes.insert(n0 + rel, kf);
    }
    layer.end = layer.end.max(n1 + 1);
    ensure_first(layer);
}

/// Convert to Frame-by-Frame Animation: every frame of the tweens in
/// `f0..=f1` becomes a keyframe holding what it showed.
pub fn convert_to_frame_by_frame(layer: &mut Layer, f0: u32, f1: u32) {
    if f0 >= layer.end {
        return;
    }
    let f1 = f1.min(layer.end - 1);
    // Evaluate the original curve, not the shortened/retimed result of pinning
    // the boundaries. Sampling must also retain nested graphic playback phase.
    let source = layer.clone();
    // Keep the exterior as retained authored intervals; only the selection bakes.
    pin(layer, f0);
    pin(layer, f1 + 1);
    let starts: Vec<u32> = layer
        .keyframes
        .iter()
        .filter(|(&s, k)| s <= f1 && layer.span_end(s) > f0 && !matches!(k.tween, Tween::None))
        .map(|(s, _)| *s)
        .collect();
    let mut out = vec![];
    for s in starts {
        let e = layer.span_end(s);
        for f in s.max(f0)..e.min(f1 + 1) {
            if let Some((els, clock)) = layer_elements_at(&source, f) {
                let mut kf = Keyframe::with(els);
                kf.playback_offset = clock;
                if let Some((_, src, _)) = source.span_at(f) {
                    kf.playback_reversed = src.playback_reversed;
                }
                if let Some(src) = source.keyframes.get(&f) {
                    kf.label = src.label.clone();
                    kf.script = src.script.clone();
                }
                out.push((f, kf));
            }
        }
    }
    for (f, kf) in out {
        layer.keyframes.insert(f, kf);
    }
}

/// Reverse Frames: frame f in `f0..=f1` shows what frame `f0 + f1 − f` showed.
/// Retained source intervals run backwards without approximating/rebuilding ease,
/// shape correspondence or rotation turns. Authored metadata mirrors its frame.
pub fn reverse_frames(layer: &mut Layer, f0: u32, f1: u32) {
    if f0 >= layer.end {
        return;
    }
    let f1 = f1.min(layer.end - 1);
    if f1 <= f0 {
        return;
    }
    pin(layer, f0);
    pin(layer, f1 + 1);
    let metadata: Vec<_> = layer
        .keyframes
        .range(f0..=f1)
        .filter(|(_, k)| k.label.is_some() || k.script.is_some())
        .map(|(&f, _)| f)
        .collect();
    for f in metadata {
        pin(layer, f + 1);
    }
    let keys = keys_in(layer, f0, f1);
    let mirror = |f: u32| f0 + f1 - f;
    let mut placed: std::collections::BTreeMap<u32, Keyframe> = Default::default();
    for &k in &keys {
        let e = layer.span_end(k).min(f1 + 1);
        let key = &layer.keyframes[&k];
        let mut reversed = retained_interval(layer, k, e).unwrap_or_else(|| key.clone());
        if let Some(boundary) = &mut reversed.tween_boundary {
            boundary.reversed = !boundary.reversed;
        }
        reversed.playback_offset = key.playback_frame(e - k - 1);
        reversed.playback_reversed = !key.playback_reversed;
        placed.insert(mirror(e - 1), reversed);
    }
    for k in keys {
        layer.keyframes.remove(&k);
    }
    layer.keyframes.extend(placed);
}

/// Drag the end of a layer's last span: lengthen/shorten the layer. Motion
/// tween keys stretch with the span (as in Animate).
pub fn set_last_span_end(layer: &mut Layer, new_end: u32) {
    let Some((&start, _)) = layer.keyframes.iter().next_back() else {
        return;
    };
    let new_end = new_end.max(start + 1);
    let old_len = layer.end.saturating_sub(start);
    let new_len = new_end - start;
    if layer.keyframes[&start].tween_boundary.is_none() {
        if let Some(m) = layer
            .keyframes
            .get_mut(&start)
            .and_then(|k| k.motion.as_mut())
        {
            m.fit(old_len, new_len, true);
        }
    }
    layer.end = new_end;
}
