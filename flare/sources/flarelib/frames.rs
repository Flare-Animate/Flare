//! Frame-level operations on a single layer, matching Animate's timeline
//! semantics. Pure functions over `Layer` so they are easy to test; the
//! `Editor` applies them across every layer in a frame selection.

use crate::model::*;
use std::cell::Cell;

thread_local! {
    // Flare adaptation: the `retain-tween-curves` feature (Flare Studio) makes retention the default.
    static RETAIN: Cell<bool> = const { Cell::new(cfg!(feature = "retain-tween-curves")) };
}

/// Flare's timeline semantics: splitting, partly copying, clearing or
/// reversing a tween keeps the original curve (a `TweenBoundary` per piece)
/// instead of re-tweening from the sampled pose as Animate does. Off by
/// default (on with the `retain-tween-curves` feature); per thread, so an app
/// (or a test) can switch it for its own work.
pub fn set_retain_tween_curves(on: bool) {
    RETAIN.with(|r| r.set(on));
}
pub fn retain_tween_curves() -> bool {
    RETAIN.with(|r| r.get())
}
/// Curve retention applies: switched on, or the layer already holds retained pieces.
fn curves(layer: &Layer) -> bool {
    retain_tween_curves() || layer.keyframes.values().any(|k| k.tween_boundary.is_some())
}

/// What a keyframe created at `f` should contain: the visible (possibly
/// tweened) content, or past the layer's end the last keyframe's content.
pub fn content_at(layer: &Layer, f: u32) -> (Vec<Element>, Tween) {
    if let Some((els, _)) = layer_elements_at(layer, f) {
        let tween = layer.span_at(f).map(|(_, k, _)| k.tween).unwrap_or(Tween::None);
        return (els, tween);
    }
    let last = layer.keyframes.values().next_back().map(|k| k.elements.clone()).unwrap_or_default();
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
        kf.ease_curve = src.ease_curve.clone();
        kf.orient_to_path = src.orient_to_path;
        kf.camera = src.camera.clone();
        kf.playback_offset = src.playback_frame(f - s);
        kf.playback_reversed = src.playback_reversed;
        if src.tween_boundary.is_some() {
            // A standalone pose from a retained piece is a sample; splits and
            // copies use `retained_interval` to keep the curve.
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

/// Insert a keyframe at `f` that keeps what the frame shows, and end a
/// motion tween's keys there.
fn split_at(layer: &mut Layer, f: u32) {
    if layer.keyframes.contains_key(&f) {
        return;
    }
    if curves(layer) {
        if let Some((s, _, _)) = layer.span_at(f) {
            if let (Some(head), Some(tail)) = (retained_interval(layer, s, f), retained_interval(layer, f, layer.span_end(s))) {
                layer.keyframes.insert(s, head);
                layer.keyframes.insert(f, tail);
                return;
            }
        }
    }
    let kf = keyframe_at(layer, f);
    if let Some((s, _, _)) = layer.span_at(f) {
        if let Some(pose) = kf.elements.first().zip(kf.motion.as_ref()).map(|(el, m)| crate::motion::Pose::of(el, m.pivot)) {
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
    let moved: Vec<(u32, Keyframe)> = layer.keyframes.range(after + 1..).map(|(k, v)| (*k, v.clone())).collect();
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
    if curves(layer) {
        pin(layer, f0);
    }
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
    if blank && retain_tween_curves() {
        // Flare: F7 blanks only the range; what follows keeps showing.
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
        rejoin_retained(layer, k);
        layer.keyframes.remove(&k);
    }
}

/// Removing the boundary of a lossless split rejoins the two retained pieces
/// (instead of stretching the shortened head over both). From Flare.
fn rejoin_retained(layer: &mut Layer, k: u32) {
    let Some((&previous, head)) = layer.keyframes.range(..k).next_back() else { return };
    let tail = &layer.keyframes[&k];
    let (Some(a), Some(b)) = (&head.tween_boundary, &tail.tween_boundary) else { return };
    let head_len = k - previous;
    let tail_len = layer.span_end(k) - k;
    let adjacent = if a.reversed { a.time_start == b.time_end } else { a.time_end == b.time_start };
    let rate_a = (a.time_end - a.time_start) / head_len as f32;
    let rate_b = (b.time_end - b.time_start) / tail_len.max(1) as f32;
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
        if let Some(bd) = layer.keyframes.get_mut(&previous).and_then(|k| k.tween_boundary.as_mut()) {
            bd.time_start = start;
            bd.time_end = end;
        }
    }
}

/// Delete on frames ("Clear Frames"): the range becomes one blank keyframe;
/// frames after it keep what they showed.
pub fn clear_frames(layer: &mut Layer, f0: u32, f1: u32) {
    if f0 >= layer.end {
        return;
    }
    let f1 = f1.min(layer.end - 1);
    if curves(layer) {
        pin(layer, f0);
    }
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
    let first = layer.keyframes.get(&f0).cloned().unwrap_or_else(|| keyframe_at(layer, f0));
    keyframes.push((0, first));
    for k in keys_in(layer, f0 + 1, f1) {
        keyframes.push((k - f0, layer.keyframes[&k].clone()));
    }
    if curves(layer) {
        retain_clipped(layer, f0, f1, &mut keyframes);
    }
    FrameClip { len: f1 - f0 + 1, keyframes }
}

/// Flare: tweens cut by the copied range keep their curve over the copied
/// part; only spans with no end to tween to fall back to per-frame poses.
fn retain_clipped(layer: &Layer, f0: u32, f1: u32, keyframes: &mut Vec<(u32, Keyframe)>) {
    let mut sampled = std::collections::BTreeMap::new();
    for (&start, key) in layer.keyframes.range(..=f1) {
        let end = layer.span_end(start);
        if end <= f0 || key.tween == Tween::None {
            continue;
        }
        let clipped_end = if key.tween.is_motion() { end > f1.saturating_add(1) } else { end > f1 };
        if !(start < f0 || clipped_end) {
            continue;
        }
        if let Some(head) = retained_interval(layer, start.max(f0), f1.saturating_add(1).min(end)) {
            sampled.insert(start.max(f0) - f0, head);
            continue;
        }
        for f in start.max(f0)..end.min(f1.saturating_add(1)) {
            let (elements, _) = content_at(layer, f);
            let mut pose = layer.keyframes.get(&f).cloned().unwrap_or_else(Keyframe::blank);
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
    if !sampled.is_empty() {
        let mut all: std::collections::BTreeMap<_, _> = std::mem::take(keyframes).into_iter().collect();
        all.extend(sampled);
        *keyframes = all.into_iter().collect();
    }
}

/// The part `from..end` of the tween span containing `from`, as a keyframe
/// whose `TweenBoundary` keeps the original curve. None for untweened spans
/// and classic/shape tweens without an end keyframe. From Flare.
fn retained_interval(layer: &Layer, from: u32, end: u32) -> Option<Keyframe> {
    let (start, key, _) = layer.span_at(from)?;
    if key.tween == Tween::None || end <= from {
        return None;
    }
    let span_end = layer.span_end(start);
    let target = if key.tween.is_motion() { None } else { layer.keyframes.get(&span_end) };
    if !key.tween.is_motion() && target.is_none() && key.tween_boundary.is_none() {
        return None;
    }
    let mut boundary = key.tween_boundary.as_deref().cloned().unwrap_or_else(|| TweenBoundary {
        duration: span_end - start,
        time_start: 0.0,
        time_end: (span_end - start) as f32,
        reversed: false,
        elements: target.map(|k| k.elements.clone()).unwrap_or_default(),
        shape_hints: target.map(|k| k.shape_hints.clone()).unwrap_or_default(),
        pose_edits: vec![],
    });
    let step = (boundary.time_end - boundary.time_start) / (span_end - start) as f32;
    let (lo, hi) = if boundary.reversed { (span_end - end, span_end - from) } else { (from - start, end - start) };
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

/// Turn retained pieces overlapping `f0..=f1` into per-frame poses (for edits
/// that can't yet be expressed as corrections over the original curve).
pub fn materialize_retained_boundaries(layer: &mut Layer, f0: u32, f1: u32) {
    if !layer.keyframes.range(..=f1).any(|(&start, key)| key.tween_boundary.is_some() && layer.span_end(start) > f0) {
        return;
    }
    let source = layer.clone();
    for (&start, key) in source.keyframes.range(..=f1) {
        let end = source.span_end(start);
        if end <= f0 || key.tween_boundary.is_none() {
            continue;
        }
        for f in start..end {
            let Some((elements, clock)) = layer_elements_at(&source, f) else { continue };
            let mut pose = Keyframe::with(elements);
            pose.playback_offset = clock;
            pose.playback_reversed = key.playback_reversed;
            if f == start {
                pose.label = key.label.clone();
                pose.label_kind = key.label_kind;
                pose.script = key.script.clone();
                pose.sound = key.sound.clone();
            }
            layer.keyframes.insert(f, pose);
        }
    }
}

/// Paste replaces frames `at..at+len`; frames after keep their content.
pub fn paste_frames(layer: &mut Layer, at: u32, clip: &FrameClip) {
    let end = at + clip.len;
    // A later disjoint paste must not stretch a retained piece into the gap.
    if at > layer.end && layer.keyframes.values().next_back().is_some_and(|k| k.tween_boundary.is_some()) {
        layer.keyframes.insert(layer.end, Keyframe::blank());
    }
    if curves(layer) {
        pin(layer, at);
    }
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

/// Paste Frames: the clip replaces the selected frames `f0..=f1`, and what
/// follows moves by the difference in length (Paste and Overwrite Frames is
/// `paste_frames`, which keeps later frames where they are).
pub fn paste_frames_insert(layer: &mut Layer, f0: u32, f1: u32, clip: &FrameClip) {
    if f0 >= layer.end {
        return paste_frames(layer, f0, clip);
    }
    let f1 = f1.clamp(f0, layer.end - 1);
    pin(layer, f1 + 1);
    if curves(layer) {
        pin(layer, f0);
    }
    for k in keys_in(layer, f0, f1) {
        layer.keyframes.remove(&k);
    }
    let delta = clip.len as i64 - (f1 - f0 + 1) as i64;
    shift_after(layer, f1, delta);
    layer.end = (layer.end as i64 + delta).max(1) as u32;
    for (rel, kf) in &clip.keyframes {
        layer.keyframes.insert(f0 + rel, kf.clone());
    }
    layer.end = layer.end.max(f0 + clip.len);
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
        let b = keys.iter().map(|k| (k - f0, layer.keyframes[k].clone())).collect();
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
    // Sample the original curve (and nested graphic clocks) before cutting
    // the tweens at the range ends so the rest keeps its timing. (Flare fix.)
    let source = layer.clone();
    pin(layer, f0);
    pin(layer, f1 + 1);
    let starts: Vec<u32> = layer.keyframes.iter().filter(|(&s, k)| s <= f1 && layer.span_end(s) > f0 && !matches!(k.tween, Tween::None)).map(|(s, _)| *s).collect();
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
                    kf.label_kind = src.label_kind;
                    kf.script = src.script.clone();
                    kf.sound = src.sound.clone();
                }
                out.push((f, kf));
            }
        }
    }
    for (f, kf) in out {
        layer.keyframes.insert(f, kf);
    }
}

/// Synchronize Symbols: graphic instances keep counting across the keyframes
/// in `f0..=f1` instead of restarting at their First Frame. Classic tweens get
/// their Sync option; between untweened keyframes the later instance of the
/// same symbol (same stacking position) gets its First Frame advanced by the
/// keyframe gap. Returns how many keyframes or instances changed.
pub fn synchronize_symbols(layer: &mut Layer, f0: u32, f1: u32) -> usize {
    type First = Option<(SymbolId, LoopMode, u32)>;
    let firsts = |els: &[Element]| -> Vec<First> {
        els.iter()
            .map(|e| match &e.kind {
                ElementKind::Instance(i) => Some((i.symbol, i.loop_mode, i.first_frame)),
                _ => None,
            })
            .collect()
    };
    let start = layer.keyframes.range(..=f0).next_back().map(|(k, _)| *k).unwrap_or(f0);
    let keys: Vec<u32> = layer.keyframes.range(start..=f1).map(|(k, _)| *k).collect();
    let Some(&k0) = keys.first() else { return 0 };
    // where each instance of the previous keyframe really starts (synced tweens shift it)
    let mut eff: Vec<First> = firsts(&layer.keyframes[&k0].elements);
    let mut n = 0;
    for w in keys.windows(2) {
        let (pk, k) = (w[0], w[1]);
        let step = k - pk;
        let tween = layer.keyframes[&pk].tween;
        let classic = matches!(tween, Tween::Classic { .. });
        if classic && !layer.keyframes[&pk].sync {
            layer.keyframes.get_mut(&pk).expect("key").sync = true;
            n += 1;
        }
        let cur = firsts(&layer.keyframes[&k].elements);
        let carried = |i: usize, c: &First| match (c, eff.get(i).copied().flatten()) {
            (Some((sym, mode, _)), Some((psym, pmode, pf))) if *sym == psym && *mode == pmode && matches!(mode, LoopMode::Loop | LoopMode::PlayOnce) => Some((psym, pmode, pf.saturating_add(step))),
            _ => None,
        };
        let next: Vec<First> = cur.iter().enumerate().map(|(i, c)| carried(i, c).or(*c)).collect();
        if !classic && tween == Tween::None {
            for (e, to) in layer.keyframes.get_mut(&k).expect("key").elements.iter_mut().zip(&next) {
                if let (ElementKind::Instance(i), Some((sym, _, f))) = (&mut e.kind, to) {
                    if i.symbol == *sym && i.first_frame != *f {
                        i.first_frame = *f;
                        n += 1;
                    }
                }
            }
        }
        // shape and motion tweens break the chain
        eff = if classic || tween == Tween::None { next } else { cur };
    }
    n
}

/// Reverse Frames: frame f in `f0..=f1` shows what frame `f0 + f1 − f` showed.
/// Tweens keep working (backwards, with mirrored eases).
pub fn reverse_frames(layer: &mut Layer, f0: u32, f1: u32) {
    if f0 >= layer.end {
        return;
    }
    let f1 = f1.min(layer.end - 1);
    if f1 <= f0 {
        return;
    }
    if curves(layer) {
        return reverse_retained(layer, f0, f1);
    }
    pin(layer, f1 + 1);
    if !layer.keyframes.contains_key(&f0) {
        if layer.span_at(f0).is_some() {
            split_at(layer, f0);
        } else {
            return;
        }
    }
    let keys = keys_in(layer, f0, f1);
    let mirror = |f: u32| f0 + f1 - f;
    let mut placed: std::collections::BTreeMap<u32, Keyframe> = Default::default();
    let mut tween_starts: Vec<(u32, Keyframe)> = vec![];
    for &k in &keys {
        let e = layer.span_end(k).min(f1 + 1);
        let kf = layer.keyframes[&k].clone();
        let (r0, r1) = (mirror(e - 1), mirror(k));
        let target = layer.keyframes.get(&layer.span_end(k)).filter(|_| layer.span_end(k) <= f1 + 1);
        match kf.tween {
            Tween::Motion { .. } => {
                let mut kf2 = kf.clone();
                if let Some(m) = kf2.motion.as_mut() {
                    m.reverse(e - k);
                }
                kf2.ease_curve = kf.ease_curve.as_ref().map(|c| c.reversed());
                kf2.tween = Tween::Motion { ease: -kf.tween.ease() };
                placed.insert(r0, kf2);
            }
            Tween::Classic { .. } | Tween::Shape { .. } if target.is_some() => {
                let mut end = kf.clone();
                end.tween = Tween::None;
                end.ease_curve = None;
                placed.entry(r1).or_insert(end);
                let mut start = if r0 > f0 {
                    target.unwrap().clone()
                } else {
                    let (els, _) = content_at(layer, e - 1);
                    Keyframe::with(els)
                };
                start.label = None;
                start.script = None;
                start.tween = match kf.tween {
                    Tween::Classic { ease, rotate_turns } => Tween::Classic { ease: -ease, rotate_turns: -rotate_turns },
                    Tween::Shape { ease } => Tween::Shape { ease: -ease },
                    t => t,
                };
                start.ease_curve = kf.ease_curve.as_ref().map(|c| c.reversed());
                start.orient_to_path = kf.orient_to_path;
                start.motion = None;
                tween_starts.push((if r0 > f0 { r0 - 1 } else { r0 }, start));
            }
            _ => {
                let mut st = kf.clone();
                st.tween = Tween::None;
                placed.entry(r0).or_insert(st);
            }
        }
    }
    for (f, kf) in tween_starts {
        placed.insert(f, kf);
    }
    for k in keys {
        layer.keyframes.remove(&k);
    }
    layer.keyframes.extend(placed);
}

/// Flare's Reverse Frames: retained pieces run backwards over their original
/// curve (ease, shape correspondence and turns kept exactly), nested graphic
/// clocks reverse too, and labels/scripts stay on their own frames.
fn reverse_retained(layer: &mut Layer, f0: u32, f1: u32) {
    pin(layer, f0);
    pin(layer, f1 + 1);
    let metadata: Vec<u32> = layer.keyframes.range(f0..=f1).filter(|(_, k)| k.label.is_some() || k.script.is_some()).map(|(&f, _)| f).collect();
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
        if let Some(b) = &mut reversed.tween_boundary {
            b.reversed = !b.reversed;
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

/// Drag the end of the span starting at `start` to `new_end` (exclusive):
/// later keyframes move with it (Animate's Ctrl-drag of a span's last
/// frame); a motion tween's property keys stretch to the new length.
pub fn set_span_end(layer: &mut Layer, start: u32, new_end: u32) {
    if !layer.keyframes.contains_key(&start) {
        return;
    }
    let next = layer.keyframes.range(start + 1..).next().map(|(&k, _)| k);
    let Some(old_end) = next else { return set_last_span_end(layer, new_end) };
    let new_end = new_end.max(start + 1);
    let delta = new_end as i64 - old_end as i64;
    if delta == 0 {
        return;
    }
    // a retained piece maps its source time onto the new length by itself
    if layer.keyframes[&start].tween_boundary.is_none() {
        if let Some(m) = layer.keyframes.get_mut(&start).and_then(|k| k.motion.as_mut()) {
            m.fit(old_end - start, new_end - start, true);
        }
    }
    let later: Vec<u32> = layer.keyframes.range(old_end..).map(|(&k, _)| k).collect();
    let moved: Vec<(u32, Keyframe)> = later.iter().map(|k| (*k, layer.keyframes.remove(k).unwrap())).collect();
    for (k, kf) in moved {
        layer.keyframes.insert((k as i64 + delta) as u32, kf);
    }
    layer.end = (layer.end as i64 + delta).max(1) as u32;
}

pub fn set_last_span_end(layer: &mut Layer, new_end: u32) {
    let Some((&start, _)) = layer.keyframes.iter().next_back() else { return };
    let new_end = new_end.max(start + 1);
    let old_len = layer.end.saturating_sub(start);
    let new_len = new_end - start;
    if layer.keyframes[&start].tween_boundary.is_none() {
        if let Some(m) = layer.keyframes.get_mut(&start).and_then(|k| k.motion.as_mut()) {
            m.fit(old_len, new_len, true);
        }
    }
    layer.end = new_end;
}

/// Prepare a document for formats that know only Animate's model (XFL/.fla):
/// retained tween pieces become frame-by-frame poses, and keyframe playback
/// clocks fold into each graphic instance's First Frame (reversed clocks
/// become the reverse loop modes). Returns warnings for the export log.
pub fn for_interchange(doc: &Document) -> (Document, Vec<String>) {
    let mut d = doc.clone();
    let mut warn = vec![];
    let durations: std::collections::HashMap<SymbolId, u32> = d.library.iter().map(|s| (s.id, s.timeline.frame_count())).collect();
    let mut baked = false;
    let mut fold = |tl: &mut Timeline| {
        for l in &mut tl.layers {
            if l.keyframes.values().any(|k| k.tween_boundary.is_some()) {
                let end = l.end.saturating_sub(1);
                materialize_retained_boundaries(l, 0, end);
                baked = true;
            }
            for kf in l.keyframes.values_mut() {
                if kf.playback_offset == 0 && !kf.playback_reversed {
                    continue;
                }
                for el in &mut kf.elements {
                    if let ElementKind::Instance(i) = &mut el.kind {
                        let dur = durations.get(&i.symbol).copied().unwrap_or(1).max(1);
                        let first = u64::from(i.first_frame) + u64::from(kf.playback_offset);
                        i.first_frame = match i.loop_mode {
                            LoopMode::Loop | LoopMode::LoopReverse => (first % u64::from(dur)) as u32,
                            _ => first.min(u64::from(dur - 1)) as u32,
                        };
                        if kf.playback_reversed {
                            i.loop_mode = match i.loop_mode {
                                LoopMode::Loop => LoopMode::LoopReverse,
                                LoopMode::PlayOnce => LoopMode::PlayReverseOnce,
                                LoopMode::LoopReverse => LoopMode::Loop,
                                LoopMode::PlayReverseOnce => LoopMode::PlayOnce,
                                m => m,
                            };
                        }
                    }
                }
                kf.playback_offset = 0;
                kf.playback_reversed = false;
            }
        }
    };
    fold(&mut d.scene);
    for t in &mut d.more_scenes {
        fold(t);
    }
    for s in &mut d.library {
        fold(&mut s.timeline);
    }
    if baked {
        warn.push("retained tween pieces (curves kept across splits) exported frame by frame".into());
    }
    (d, warn)
}
