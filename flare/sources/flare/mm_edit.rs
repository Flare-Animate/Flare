//! Timeline editing: Animate's frame and layer commands. Frames are 0-based.
//! Every command keeps a layer's keyframes back to back from frame 0, each
//! starting where the previous one ends.

use crate::display::tween_at;
use crate::doc::*;

/// What Create Classic Tween sets: no easing, turning the shorter way.
const CLASSIC: Tween = Tween::Classic { ease: 0.0, rotate: Rotate::Auto };

// ponytail: no overflow checks, so frames near u32::MAX panic (debug) or wrap. The editor's
// timeline never gets there; clamp `f` and `n` if scripts ever drive these commands.
impl Layer {
    /// Insert Frame (F5): lengthens the span holding `f` by `n` frames, so
    /// later keyframes move right. At or past the layer's end, it stretches
    /// the last span until the layer reaches the last of the `n` frames (`f`
    /// itself when `n` is 1); an empty layer gets a blank keyframe.
    pub fn insert_frames(&mut self, f: u32, n: u32) {
        if n == 0 {
            return;
        }
        match self.keyframe_at(f) {
            Some(i) => {
                let key = &mut self.frames[i];
                key.duration += n;
                // A motion tween's keyframes past `f` move along.
                if let Tween::Motion(m) = &mut key.tween {
                    m.insert_frames(f - key.index, n);
                }
                for k in &mut self.frames[i + 1..] {
                    k.index += n;
                }
            }
            None => self.extend_to(f + n - 1),
        }
    }

    /// Insert Keyframe (F6): makes `f` a keyframe holding what is on stage
    /// there: a classic tween's in-between, and graphic symbols carrying on
    /// from the frame they had reached. It splits the span holding `f`, and
    /// both halves keep its tween, so a linear tween plays on unchanged while
    /// an eased or spinning one now eases or spins within each half. Past the
    /// layer's end, the last span first stretches up to `f`.
    ///
    /// On a keyframe it acts on the next frame instead, as Animate does with a
    /// keyframe selected; nothing changes if that is a keyframe too. Returns
    /// the frame it acted on.
    pub fn insert_keyframe(&mut self, f: u32) -> u32 {
        self.split(f, true)
    }

    /// Insert Blank Keyframe (F7): as [`Layer::insert_keyframe`], but the new
    /// keyframe is empty and has no tween.
    pub fn insert_blank_keyframe(&mut self, f: u32) -> u32 {
        self.split(f, false)
    }

    /// Remove Frames (Shift+F5): removes frames `f..f + n`, shortening the
    /// spans they fall in and dropping spans left empty, so later keyframes
    /// move left. A keyframe that loses its first frames keeps its contents.
    /// Removing every frame leaves one blank keyframe.
    pub fn remove_frames(&mut self, f: u32, n: u32) {
        let end = f.saturating_add(n);
        // How many of frames a..b go.
        let cut = |a: u32, b: u32| b.min(end).saturating_sub(a.max(f));
        let had_frames = !self.frames.is_empty();
        for k in &mut self.frames {
            // A motion tween's keyframes on the frames that go, go too.
            if let Tween::Motion(m) = &mut k.tween
                && cut(k.index, k.index + k.duration) > 0
            {
                m.remove_frames(f.saturating_sub(k.index), cut(k.index, k.index + k.duration), k.duration);
            }
            k.duration -= cut(k.index, k.index + k.duration);
            k.index -= cut(0, k.index);
        }
        self.frames.retain(|k| k.duration > 0);
        if had_frames && self.frames.is_empty() {
            self.frames.push(blank(0, 1));
        }
    }

    /// Clear Keyframe (Shift+F6): the keyframe starting at `f` becomes plain
    /// frames of the span before it, which now runs, and tweens, on to the
    /// next keyframe. The first keyframe stays.
    pub fn clear_keyframe(&mut self, f: u32) {
        let Some(i) = self.frames.iter().position(|k| k.index == f).filter(|&i| i > 0) else { return };
        let cleared = self.frames.remove(i);
        self.frames[i - 1].duration += cleared.duration;
    }

    /// Create Classic Tween on the span holding `f`. A classic tween already
    /// there keeps its settings.
    pub fn create_classic_tween(&mut self, f: u32) {
        // ponytail: Animate first turns shapes, or several objects, into a "Tween N" graphic
        // symbol. Without that the tween holds, as broken ones do, until each end has one instance.
        let Some(key) = self.keyframe_at(f).map(|i| &mut self.frames[i]) else { return };
        if !matches!(key.tween, Tween::Classic { .. }) {
            key.tween = CLASSIC;
        }
    }

    /// Remove Tween from the span holding `f`. A motion tween's target stays as the
    /// span's first frame shows it.
    pub fn remove_tween(&mut self, f: u32) {
        if let Some(i) = self.keyframe_at(f) {
            if let Some(el) = crate::motion::target_at(&self.frames[i], 0) {
                self.frames[i].elements = vec![el];
            }
            self.frames[i].tween = Tween::None;
        }
    }

    /// One past the layer's last frame.
    pub fn frame_count(&self) -> u32 {
        self.frames.last().map_or(0, |k| k.index + k.duration)
    }

    /// Convert to Keyframes (F6 on several frames): each of frames f..f+n
    /// becomes a keyframe holding what is on stage there, or with `blank`
    /// (Convert to Blank Keyframes, F7) an empty one. Keyframes already there
    /// stay, emptied when `blank`. Past the layer's end, the last span first
    /// stretches to reach them.
    pub fn convert_to_keyframes(&mut self, f: u32, n: u32, blank: bool) {
        if n == 0 {
            return;
        }
        self.extend_to(f + n - 1);
        for at in f..f + n {
            // Inside a motion tween, F6 keys its properties instead.
            if !blank && self.key_motion(at) {
                continue;
            }
            self.split_at(at, !blank);
            if blank && let Some(k) = self.frames.iter_mut().find(|k| k.index == at) {
                k.elements.clear();
                k.tween = Tween::None;
            }
        }
    }

    /// Clear Keyframe (Shift+F6) on frames f..f+n: each keyframe starting
    /// there, but the layer's first, becomes plain frames of the span before it.
    pub fn clear_keyframes(&mut self, f: u32, n: u32) {
        let range = f..f.saturating_add(n);
        let starts: Vec<u32> = self.frames.iter().map(|k| k.index).filter(|i| range.contains(i)).collect();
        for at in starts.into_iter().rev() {
            self.clear_keyframe(at);
        }
    }

    /// Copy Frames: what frames f..f+n show, as keyframes from frame 0. A
    /// range starting inside a span starts with a keyframe holding what is on
    /// stage there; it comes up short where the layer ends first.
    pub fn copy_frames(&self, f: u32, n: u32) -> Vec<Frame> {
        let end = f.saturating_add(n);
        let mut clip = Vec::new();
        for (i, key) in self.frames.iter().enumerate() {
            let (from, to) = (key.index.max(f), (key.index + key.duration).min(end));
            if from >= to {
                continue;
            }
            let tween = crate::motion::copied(key, from - key.index, to - key.index);
            let mut k = if from == key.index {
                Frame { tween, ..key.clone() }
            } else {
                let elements = on_stage(key, self.frames.get(i + 1), from - key.index);
                Frame { index: 0, duration: 0, tween, elements, extra: carried_extra(&key.extra) }
            };
            (k.index, k.duration) = (from - f, to - from);
            clip.push(k);
        }
        clip
    }

    /// Paste Frames: puts `clip`, copied from `n` frames, at frame `f`,
    /// pushing what is there right when `insert`, or replacing it (Paste and
    /// Overwrite Frames). Frames after the pasted ones keep showing what they
    /// did; a layer ending before `f` first stretches to it. Frames the clip
    /// comes up short of are a blank keyframe, if anything follows them.
    pub fn paste_frames(&mut self, f: u32, n: u32, clip: &[Frame], insert: bool) {
        if n == 0 {
            return;
        }
        if f > 0 {
            self.extend_to(f - 1);
        }
        self.split_at(f, true);
        if insert {
            self.frames.iter_mut().filter(|k| k.index >= f).for_each(|k| k.index += n);
        } else {
            self.split_at(f + n, true);
            self.frames.retain(|k| !(f..f + n).contains(&k.index));
        }
        let mut pasted: Vec<Frame> = clip
            .iter()
            .filter(|k| k.index < n)
            .map(|k| Frame { index: f + k.index, duration: k.duration.min(n - k.index), ..k.clone() })
            .collect();
        let at = self.frames.partition_point(|k| k.index < f);
        let filled = pasted.last().map_or(f, |k| k.index + k.duration);
        if filled < f + n && at < self.frames.len() {
            pasted.push(blank(filled, f + n - filled));
        }
        self.frames.splice(at..at, pasted);
    }

    /// Clear Frames (Alt+Backspace): frames f..f+n, as far as the layer
    /// goes, become one blank keyframe; frames after them keep what they show.
    pub fn clear_frames(&mut self, f: u32, n: u32) {
        let n = n.min(self.frame_count().saturating_sub(f));
        self.paste_frames(f, n, &[blank(0, n)], false);
    }

    /// Dragging a lone keyframe: frame `at` becomes a keyframe with `key`'s
    /// contents and tween, its span running on to the next keyframe. The
    /// layer first stretches to reach `at`.
    pub fn put_keyframe(&mut self, at: u32, key: &Frame) {
        self.extend_to(at);
        self.split_at(at, false);
        if let Some(k) = self.frames.iter_mut().find(|k| k.index == at) {
            (k.elements, k.tween, k.extra) = (key.elements.clone(), key.tween.clone(), key.extra.clone());
        }
    }

    /// Whether Reverse Frames can act on frames f..f+n: they must start and end with a keyframe.
    pub fn can_reverse(&self, f: u32, n: u32) -> bool {
        n > 1 && [f, f + n - 1].iter().all(|&at| self.frames.iter().any(|k| k.index == at))
    }

    /// Reverse Frames: mirrors the keyframes of frames f..f+n, which start and
    /// end with one, so they play backward and tweens run the other way. Each
    /// keyframe's hold mirrors too, so it now leads up to the keyframe.
    // ponytail: a motion tween's property keyframes aren't mirrored inside its span, so its
    // tween goes to the span mirrored before it as is. Mirror them (rebased) if that bites.
    pub fn reverse_frames(&mut self, f: u32, n: u32) {
        if !self.can_reverse(f, n) {
            return;
        }
        let last = f + n - 1;
        self.split_at(last + 1, true);
        let start = |at: u32| self.frames.iter().position(|k| k.index == at).unwrap_or(0);
        let (a, b) = (start(f), start(last));
        let run: Vec<Frame> = self.frames.drain(a..=b).collect();
        let tweens: Vec<Tween> = run.iter().map(|k| k.tween.clone()).collect();
        let m = run.len();
        let mut mirrored: Vec<Frame> = run.into_iter().rev().collect();
        for k in &mut mirrored {
            k.index = f + last - k.index;
        }
        // The span after each mirrored keyframe is the one that led up to it, tween and all.
        for j in 0..m {
            let duration = mirrored.get(j + 1).map_or(1, |next| next.index - mirrored[j].index);
            mirrored[j].duration = duration;
            mirrored[j].tween = tweens[if j + 1 < m { m - 2 - j } else { m - 1 }].clone();
        }
        self.frames.splice(a..a, mirrored);
    }

    /// Dragging frames f..f+n away: they show the span before them again,
    /// or nothing at the layer's start. A `lone` keyframe takes its span
    /// along; otherwise the frames after the block keep what they show.
    fn vacate(&mut self, f: u32, n: u32, lone: bool) {
        if !lone && self.frames.iter().any(|k| (f..f + n).contains(&k.index)) {
            self.split_at(f + n, true);
        }
        self.clear_keyframes(f, n);
        if f == 0
            && let Some(first) = self.frames.first_mut()
        {
            *first = blank(0, first.duration);
        }
    }

    /// Stretches the last span until the layer reaches frame `last`. An empty
    /// layer gets a blank keyframe.
    fn extend_to(&mut self, last: u32) {
        match self.frames.last_mut() {
            Some(k) => k.duration = k.duration.max((last + 1).saturating_sub(k.index)),
            None => self.frames.push(blank(0, last + 1)),
        }
    }

    /// F6 and F7: splits a keyframe off at `f`, holding what is on stage there if `copy`.
    fn split(&mut self, f: u32, copy: bool) -> u32 {
        let at = if self.frames.iter().any(|k| k.index == f) { f + 1 } else { f };
        // Inside a motion tween (or just past one), F6 keys its properties instead.
        if copy && self.key_motion(at) {
            return at;
        }
        self.extend_to(at);
        self.split_at(at, copy);
        at
    }

    /// Starts a keyframe at `at` by splitting the span holding it; the new
    /// keyframe holds what is on stage there if `copy`, else nothing. Where a
    /// keyframe starts already, or past the layer's end, nothing changes.
    pub(crate) fn split_at(&mut self, at: u32, copy: bool) {
        let Some(i) = self.keyframe_at(at) else { return };
        let key = &self.frames[i];
        let offset = at - key.index;
        if offset == 0 {
            return;
        }
        let mut new = Frame {
            index: at,
            duration: key.duration - offset,
            tween: if copy { key.tween.clone() } else { Tween::None },
            elements: if copy { on_stage(key, self.frames.get(i + 1), offset) } else { Vec::new() },
            extra: if copy { carried_extra(&key.extra) } else { crate::layer_tree::rig_link(&key.extra) },
        };
        // A motion tween's keyframes go with the frames they're on.
        if let Some(tail) = crate::motion::split(&mut self.frames[i], offset).filter(|_| copy) {
            new.tween = Tween::Motion(Box::new(tail));
        }
        self.frames[i].duration = offset;
        self.frames.insert(i + 1, new);
    }
}

impl Timeline {
    /// New Layer: inserts an empty layer at `at` (0 is the top; clamped to the
    /// bottom). As in Animate, its blank keyframe runs the timeline's length.
    /// Returns where it went.
    pub fn insert_layer(&mut self, at: usize, name: &str) -> usize {
        let frames = vec![blank(0, self.frame_count().max(1))];
        self.insert_layers(at, vec![Layer { name: name.into(), kind: LayerKind::Normal, frames, extra: Extra::default() }])
    }

    /// New Folder: inserts an empty folder at `at`, as `insert_layer` does.
    /// `Timeline::adopt` puts it in the folder it lands in.
    pub fn insert_folder(&mut self, at: usize, name: &str) -> usize {
        self.insert_layers(at, vec![Layer { name: name.into(), kind: LayerKind::Folder, frames: Vec::new(), extra: Extra::default() }])
    }

    /// Paste Layers and Duplicate Layers: inserts `new` at `at` (clamped to
    /// the bottom), outside any folder or mask. Returns where they went.
    pub fn insert_layers(&mut self, at: usize, new: Vec<Layer>) -> usize {
        let (at, count) = (at.min(self.layers.len()), new.len());
        let order: Vec<Option<usize>> = (0..at).map(Some).chain(std::iter::repeat_n(None, count)).chain((at..self.layers.len()).map(Some)).collect();
        self.rearrange(&order, new);
        at
    }

    /// Delete Layers: removes layers `rows`, unless that would leave none.
    pub fn remove_layers(&mut self, rows: &[usize]) {
        let order: Vec<Option<usize>> = (0..self.layers.len()).filter(|i| !rows.contains(i)).map(Some).collect();
        if !order.is_empty() {
            self.rearrange(&order, Vec::new());
        }
    }

    /// Dragging layers: moves layers `rows` (top first, no repeats) together
    /// into the gap above layer `gap`, `layers.len()` being the bottom.
    /// Returns where the first of them went.
    pub fn move_layers(&mut self, rows: &[usize], gap: usize) -> usize {
        let gap = gap.min(self.layers.len());
        let to = gap - rows.iter().filter(|&&l| l < gap).count();
        let mut order: Vec<Option<usize>> = (0..self.layers.len()).filter(|i| !rows.contains(i)).map(Some).collect();
        order.splice(to..to, rows.iter().map(|&l| Some(l)));
        self.rearrange(&order, Vec::new());
        to
    }

    /// Rebuilds the layer list: `order` says, top first, which old layer goes
    /// at each place, None taking the next of `new`. Each kept layer's XFL
    /// parentLayerIndex (its folder, or the mask over it) then follows that
    /// layer, or, if it's gone, the one that held it; new layers get none.
    /// A moved layer stays in its folder or under its mask wherever it lands;
    /// `Timeline::move_into` regroups by drop position.
    fn rearrange(&mut self, order: &[Option<usize>], new: Vec<Layer>) {
        const PARENT: &str = "parentLayerIndex";
        let parents: Vec<Option<usize>> = self.layers.iter().map(|l| l.extra.get(PARENT).and_then(|p| p.parse().ok())).collect();
        let mut old: Vec<Option<Layer>> = self.layers.drain(..).map(Some).collect();
        let mut new = new.into_iter();
        self.layers = order.iter().filter_map(|o| o.map_or_else(|| new.next(), |i| old[i].take())).collect();
        let place = |p: usize| order.iter().position(|&o| o == Some(p));
        for (layer, o) in self.layers.iter_mut().zip(order) {
            // Up past removed layers, at most once round each (a file can loop them).
            let mut parent = o.and_then(|i| parents[i]);
            for _ in 0..parents.len() {
                match parent {
                    Some(p) if place(p).is_none() => parent = parents.get(p).copied().flatten(),
                    _ => break,
                }
            }
            layer.extra.set(PARENT, parent.and_then(place).map(|p| p.to_string()).as_deref());
        }
    }

    /// Dragging frames: moves frames f..f+n of layers `rows` so the block
    /// starts at frame `to.1` of layer `to.0`, over what is there, or copies
    /// them there with `copy` (Alt+drag). A lone keyframe moves as a keyframe:
    /// its span goes along, and the span it lands in ends there. Folders, rows
    /// with nothing in the block and rows past the last layer are skipped.
    pub fn move_frames(&mut self, rows: std::ops::Range<usize>, f: u32, n: u32, to: (usize, u32), copy: bool) {
        let rows = rows.start.min(self.layers.len())..rows.end.min(self.layers.len());
        let moved: Vec<Option<(bool, Vec<Frame>)>> = self.layers[rows.clone()]
            .iter()
            .map(|l| {
                let lone = n == 1 && l.frames.iter().any(|k| k.index == f);
                Some((lone, l.copy_frames(f, n))).filter(|(_, clip)| l.kind != LayerKind::Folder && !clip.is_empty())
            })
            .collect();
        if !copy {
            for (layer, m) in self.layers[rows].iter_mut().zip(&moved) {
                if let Some((lone, _)) = m {
                    layer.vacate(f, n, *lone);
                }
            }
        }
        for (i, m) in moved.iter().enumerate() {
            let (Some((lone, clip)), Some(layer)) = (m, self.layers.get_mut(to.0 + i)) else { continue };
            match clip.first() {
                _ if layer.kind == LayerKind::Folder => {}
                Some(key) if *lone => layer.put_keyframe(to.1, key),
                _ => layer.paste_frames(to.1, n, clip, false),
            }
        }
    }
}

/// What `key`'s span shows `offset` frames in, as the contents of a keyframe there.
pub(crate) fn on_stage(key: &Frame, next: Option<&Frame>, offset: u32) -> Vec<Element> {
    // A tween's in-between: a classic or motion tween's target, or a shape tween's shapes.
    let tweened = tween_at(key, next, offset)
        .map(Element::Instance)
        .or_else(|| crate::motion::target_at(key, offset))
        .or_else(|| crate::shape_tween::at(key, next, offset).map(Element::Shape));
    let mut elements = match tweened {
        Some(el) => vec![el],
        None => key.elements.clone(),
    };
    play_on(&mut elements, offset);
    elements
}

/// Moves graphic symbols `frames` frames on, so they carry on where they were.
fn play_on(elements: &mut [Element], frames: u32) {
    // ponytail: first_frame can pass the symbol's length (display.rs wraps it). Wrapping it here
    // needs the library; do it when XFL saving lands.
    for el in elements {
        match el {
            Element::Instance(i) if i.kind == SymbolKind::Graphic && i.looping != Loop::SingleFrame => {
                i.first_frame += frames;
            }
            Element::Group(members) => play_on(members, frames),
            _ => {}
        }
    }
}

fn blank(index: u32, duration: u32) -> Frame {
    Frame { index, duration, tween: Tween::None, elements: Vec::new(), extra: Extra::default() }
}

/// What F6 carries over of a keyframe's other XFL settings: tween details, but
/// not its label or script, which stay on the original keyframe as in Animate.
fn carried_extra(extra: &Extra) -> Extra {
    let not_script = |xml: &&String| !xml.trim_start().starts_with("<Actionscript");
    Extra {
        attrs: extra.attrs.iter().filter(|(k, _)| k != "name" && k != "labelType").cloned().collect(),
        before: extra.before.iter().filter(not_script).cloned().collect(),
        after: extra.after.iter().filter(not_script).cloned().collect(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::display::display_list;

    /// A layer of keyframes lasting `durations`, marked a, b, c... so copies can be told apart.
    fn layer(durations: &[u32]) -> Layer {
        let mut index = 0;
        let frames = durations.iter().zip('a'..).map(|(&duration, mark)| {
            let k = Frame { index, duration, tween: Tween::None, elements: vec![Element::Unsupported { name: mark.into(), xml: String::new() }], extra: Extra::default() };
            index += duration;
            k
        });
        Layer { name: "Layer_1".into(), kind: LayerKind::Normal, frames: frames.collect(), extra: Extra::default() }
    }

    /// Each keyframe as its mark (`_` if blank, `*` for other contents) and
    /// duration, e.g. "a2 _3", after checking they run back to back from frame 0.
    fn spans(l: &Layer) -> String {
        let mut end = 0;
        let spans: Vec<_> = l
            .frames
            .iter()
            .map(|k| {
                assert!(k.index == end && k.duration > 0, "not back to back: {:?}", l.frames);
                end += k.duration;
                let mark = match &k.elements[..] {
                    [Element::Unsupported { name: m, .. }] => m.as_str(),
                    [] => "_",
                    _ => "*",
                };
                format!("{mark}{}", k.duration)
            })
            .collect();
        spans.join(" ")
    }

    #[test]
    fn insert_frames() {
        let f5 = |durations: &[u32], f, n| {
            let mut l = layer(durations);
            l.insert_frames(f, n);
            spans(&l)
        };
        assert_eq!(f5(&[5, 3], 2, 1), "a6 b3");
        assert_eq!(f5(&[5, 3], 0, 2), "a7 b3");
        // On a keyframe, its own span grows.
        assert_eq!(f5(&[5, 3, 2], 5, 3), "a5 b6 c2");
        assert_eq!(f5(&[5, 3], 7, 1), "a5 b4");
        // From the end on, the layer stretches to the last inserted frame.
        assert_eq!(f5(&[5, 3], 8, 1), "a5 b4");
        assert_eq!(f5(&[5, 3], 12, 2), "a5 b9");
        assert_eq!(f5(&[], 4, 1), "_5");
        assert_eq!(f5(&[5, 3], 2, 0), "a5 b3");
    }

    #[test]
    fn insert_keyframe() {
        // "frame acted on: spans after".
        let f6 = |durations: &[u32], f| {
            let mut l = layer(durations);
            let at = l.insert_keyframe(f);
            format!("{at}: {}", spans(&l))
        };
        assert_eq!(f6(&[5, 3], 2), "2: a2 a3 b3");
        // On a keyframe it acts on the next frame, unless that is a keyframe too.
        assert_eq!(f6(&[5, 3], 0), "1: a1 a4 b3");
        assert_eq!(f6(&[1, 3], 0), "1: a1 b3");
        assert_eq!(f6(&[5, 3], 7), "7: a5 b2 b1");
        assert_eq!(f6(&[5, 1], 5), "6: a5 b1 b1");
        // Past the end, the last span stretches up to the new keyframe.
        assert_eq!(f6(&[5, 3], 10), "10: a5 b5 b1");
        assert_eq!(f6(&[], 3), "3: _3 _1");
        assert_eq!(f6(&[], 0), "0: _1");
    }

    #[test]
    fn insert_keyframe_keeps_the_stage() {
        // Graphic "g" draws a shape at x = 0, 1, 2, 3 on its four frames, so x
        // on stage shows both the tween and which frame of "g" plays.
        let mut g = layer(&[1, 1, 1, 1]);
        for (k, x) in g.frames.iter_mut().zip([0.0, 1.0, 2.0, 3.0]) {
            let matrix = Matrix { tx: x, ..Matrix::IDENTITY };
            k.elements = vec![Element::Shape(Shape { matrix, fills: vec![], strokes: vec![], edges: vec![], extra: Extra::default() })];
        }
        let mut doc = Document::new(550.0, 400.0, 24.0);
        let timeline = Timeline { name: "g".into(), layers: vec![g], extra: Extra::default() };
        doc.library.insert("g".into(), Symbol { kind: SymbolKind::Graphic, timeline, extra: Extra::default() });

        let graphic = |x, alpha, looping, first_frame| Instance {
            symbol: "g".into(),
            kind: SymbolKind::Graphic,
            matrix: Matrix { tx: x, ..Matrix::IDENTITY },
            pivot: (0.0, 0.0),
            color: ColorTransform { mult: [1.0, 1.0, 1.0, alpha], add: [0.0; 4] },
            looping,
            first_frame,
            extra: Extra::default(),
        };
        // Over frames 0..10, a classic tween of "g" from x 100 at alpha 1 to x 400 at alpha 0.5.
        let l = &mut doc.scenes[0].layers[0];
        *l = layer(&[10, 1]);
        l.frames[0].tween = CLASSIC;
        l.frames[0].elements = vec![Element::Instance(graphic(100.0, 1.0, Loop::Loop, 0))];
        l.frames[1].elements = vec![Element::Instance(graphic(400.0, 0.5, Loop::Loop, 0))];
        // Below it, untweened: "g" looping inside a group, on a single frame, and playing once.
        let mut held = layer(&[11]);
        held.frames[0].elements = vec![
            Element::Group(vec![Element::Instance(graphic(0.0, 1.0, Loop::Loop, 0))]),
            Element::Instance(graphic(20.0, 1.0, Loop::SingleFrame, 2)),
            Element::Instance(graphic(40.0, 1.0, Loop::PlayOnce, 0)),
        ];
        doc.scenes[0].layers.push(held);
        let before: Vec<_> = (0..11).map(|f| stage(&doc, f)).collect();

        let l = &mut doc.scenes[0].layers[0];
        assert_eq!([l.insert_keyframe(5), l.insert_keyframe(2), l.insert_keyframe(10)], [5, 2, 11]);
        assert_eq!(spans(l), "*2 *3 *5 *1 *1");
        let tweened: Vec<_> = l.frames.iter().map(|k| k.tween == CLASSIC).collect();
        assert_eq!(tweened, [true, true, true, false, false]);
        let held = &mut doc.scenes[0].layers[1];
        assert_eq!([held.insert_keyframe(3), held.insert_keyframe(6)], [3, 6]);
        let near = |got: &[(f32, f32)], want: &[(f32, f32)]| {
            got.len() == want.len()
                && got.iter().zip(want).all(|(&(x, a), &(wx, wa))| (x - wx).abs() < 1e-3 && (a - wa).abs() < 1e-5)
        };
        for (f, want) in (0..).zip(&before) {
            let got = stage(&doc, f);
            assert!(near(&got, want), "frame {f}: {got:?}, was {want:?}");
        }
        // The new frame 11 carries "g" on to its frame 1.
        assert!(near(&stage(&doc, 11), &[(401.0, 0.5)]), "{:?}", stage(&doc, 11));
    }

    /// The x and alpha of each thing on stage at frame `f` of scene 1, bottom up.
    fn stage(doc: &Document, f: u32) -> Vec<(f32, f32)> {
        let list = display_list(doc, &doc.scenes[0], f, &mut BTreeSet::new());
        list.iter().map(|p| (p.matrix.tx, p.color.mult[3])).collect()
    }

    #[test]
    fn insert_blank_keyframe() {
        let f7 = |durations: &[u32], f| {
            let mut l = layer(durations);
            let at = l.insert_blank_keyframe(f);
            format!("{at}: {}", spans(&l))
        };
        assert_eq!(f7(&[5, 3], 2), "2: a2 _3 b3");
        assert_eq!(f7(&[5, 3], 0), "1: a1 _4 b3");
        assert_eq!(f7(&[5, 3], 5), "6: a5 b1 _2");
        assert_eq!(f7(&[1, 3], 0), "1: a1 b3");
        assert_eq!(f7(&[5, 3], 7), "7: a5 b2 _1");
        assert_eq!(f7(&[5, 3], 10), "10: a5 b5 _1");
        assert_eq!(f7(&[], 0), "0: _1");
    }

    #[test]
    fn remove_frames() {
        let rm = |durations: &[u32], f, n| {
            let mut l = layer(durations);
            l.remove_frames(f, n);
            spans(&l)
        };
        assert_eq!(rm(&[5, 3], 1, 2), "a3 b3");
        // A keyframe that loses its first frames keeps its contents.
        assert_eq!(rm(&[5, 3, 2], 3, 4), "a3 b1 c2");
        assert_eq!(rm(&[5, 3, 2], 5, 3), "a5 c2");
        assert_eq!(rm(&[5, 3], 0, 5), "b3");
        assert_eq!(rm(&[5, 3], 7, 1), "a5 b2");
        assert_eq!(rm(&[5, 3], 6, 10), "a5 b1");
        assert_eq!(rm(&[5, 3], 8, 2), "a5 b3");
        // Removing every frame leaves a blank keyframe; an empty layer stays empty.
        assert_eq!(rm(&[5, 3], 0, 8), "_1");
        assert_eq!(rm(&[], 0, 1), "");
    }

    #[test]
    fn clear_keyframe() {
        let clear = |durations: &[u32], f| {
            let mut l = layer(durations);
            l.clear_keyframe(f);
            spans(&l)
        };
        assert_eq!(clear(&[5, 3, 2], 5), "a8 c2");
        assert_eq!(clear(&[5, 3, 2], 8), "a5 b5");
        // Not the first keyframe, nor frames that aren't keyframes or are past the end.
        assert_eq!(clear(&[5, 3], 0), "a5 b3");
        assert_eq!(clear(&[5, 3], 6), "a5 b3");
        assert_eq!(clear(&[5, 3], 9), "a5 b3");
    }

    #[test]
    fn tween_spans() {
        let tweens = |l: &Layer| l.frames.iter().map(|k| k.tween.clone()).collect::<Vec<_>>();
        let custom = Tween::Classic { ease: 0.5, rotate: Rotate::Cw(1) };
        let mut l = layer(&[5, 3, 2]);
        l.create_classic_tween(0);
        l.create_classic_tween(7);
        l.create_classic_tween(12); // past the end: nothing
        l.frames[1].tween = custom.clone();
        l.create_classic_tween(6); // keeps custom settings
        assert_eq!(tweens(&l), [CLASSIC, custom.clone(), Tween::None]);

        // Frames inserted or removed inside a tween lengthen or shorten it.
        l.insert_frames(1, 2);
        l.remove_frames(8, 1);
        assert_eq!(spans(&l), "a7 b2 c2");
        // A blank keyframe cuts the tween short; clearing it lets the tween run on.
        assert_eq!(l.insert_blank_keyframe(3), 3);
        assert_eq!(spans(&l), "a3 _4 b2 c2");
        assert_eq!(tweens(&l), [CLASSIC, Tween::None, custom.clone(), Tween::None]);
        l.clear_keyframe(3);
        assert_eq!(spans(&l), "a7 b2 c2");
        l.remove_tween(1);
        assert_eq!(tweens(&l), [Tween::None, custom, Tween::None]);
    }

    #[test]
    fn layers() {
        let mut tl = Timeline { name: "Scene 1".into(), layers: vec![layer(&[5, 3])], extra: Extra::default() };
        assert_eq!(tl.insert_layer(9, "Layer_2"), 1);
        assert_eq!(tl.insert_layer(0, "Layer_3"), 0);
        // New layers run the timeline's length.
        assert_eq!(spans(&tl.layers[0]), "_8");
        let names = |tl: &Timeline| tl.layers.iter().map(|l| l.name.clone()).collect::<Vec<_>>().join(" ");
        assert_eq!(names(&tl), "Layer_3 Layer_1 Layer_2");
        assert_eq!(tl.move_layers(&[0, 2], 2), 1);
        assert_eq!(names(&tl), "Layer_1 Layer_3 Layer_2");
        assert_eq!(tl.move_layers(&[1], 0), 0);
        assert_eq!(names(&tl), "Layer_3 Layer_1 Layer_2");
        tl.remove_layers(&[0, 2, 7]);
        assert_eq!(names(&tl), "Layer_1");
        // The only layer left can't go.
        tl.remove_layers(&[0]);
        assert_eq!(tl.layers.len(), 1);
        // Folders have no frames.
        assert_eq!(tl.insert_folder(0, "Folder_1"), 0);
        assert_eq!((tl.layers[0].kind, spans(&tl.layers[0]).as_str(), tl.frame_count()), (LayerKind::Folder, "", 8));
    }

    #[test]
    fn layer_changes_keep_parent_links() {
        // A folder holding a mask over a masked layer, then a free layer.
        let mut tl = Timeline { name: "Scene 1".into(), layers: ["F", "M", "m", "x"].map(|n| Layer { name: n.into(), ..layer(&[1]) }).into(), extra: Extra::default() };
        for (l, parent) in [(1, "0"), (2, "1")] {
            tl.layers[l].extra.set("parentLayerIndex", Some(parent));
        }
        let links = |tl: &Timeline| {
            let link = |l: &Layer| format!("{}{}", l.name, l.extra.get("parentLayerIndex").map_or(String::new(), |p| format!(">{p}")));
            tl.layers.iter().map(link).collect::<Vec<_>>().join(" ")
        };
        tl.insert_layer(0, "n");
        assert_eq!(links(&tl), "n F M>1 m>2 x");
        tl.move_layers(&[4], 0);
        assert_eq!(links(&tl), "x n F M>2 m>3");
        // Copies come in outside any folder; links to deleted layers go.
        let copy = tl.layers[4].clone();
        tl.insert_layers(1, vec![copy]);
        assert_eq!(links(&tl), "x m n F M>3 m>4");
        tl.remove_layers(&[3]);
        assert_eq!(links(&tl), "x m n M m>3");
    }

    #[test]
    fn convert_to_keyframes() {
        let convert = |durations: &[u32], f, n, blank| {
            let mut l = layer(durations);
            l.convert_to_keyframes(f, n, blank);
            spans(&l)
        };
        assert_eq!(convert(&[5, 3], 1, 3, false), "a1 a1 a1 a2 b3");
        // Keyframes already there stay; past the end the layer stretches first.
        assert_eq!(convert(&[5, 3], 4, 2, false), "a4 a1 b3");
        assert_eq!(convert(&[2], 3, 2, false), "a3 a1 a1");
        // Blank: new keyframes are empty, and so are the ones already there.
        assert_eq!(convert(&[5, 3], 4, 2, true), "a4 _1 _3");
        assert_eq!(convert(&[], 1, 1, true), "_1 _1");
        assert_eq!(convert(&[5, 3], 2, 0, true), "a5 b3");
    }

    #[test]
    fn clear_keyframes() {
        let clear = |durations: &[u32], f, n| {
            let mut l = layer(durations);
            l.clear_keyframes(f, n);
            spans(&l)
        };
        assert_eq!(clear(&[2, 2, 2, 2], 1, 5), "a6 d2");
        // The first keyframe stays.
        assert_eq!(clear(&[2, 2, 2], 0, 9), "a6");
    }

    #[test]
    fn copy_and_paste_frames() {
        let source = layer(&[5, 3, 2]);
        // Frames 3..7: the end of span a, as a keyframe of its own, then the start of b.
        let clip = source.copy_frames(3, 4);
        assert_eq!(spans(&Layer { frames: clip.clone(), ..layer(&[]) }), "a2 b2");
        // A copy running past the layer's end comes up short.
        assert_eq!(spans(&Layer { frames: source.copy_frames(8, 5), ..layer(&[]) }), "c2");
        assert!(source.copy_frames(10, 2).is_empty());

        let paste = |durations: &[u32], f, clip: &[Frame], insert| {
            let mut l = layer(durations);
            l.paste_frames(f, 4, clip, insert);
            spans(&l)
        };
        // Paste Frames pushes the rest right, splitting the span it lands in.
        assert_eq!(paste(&[4, 4], 1, &clip, true), "a1 a2 b2 a3 b4");
        assert_eq!(paste(&[4, 4], 0, &clip, true), "a2 b2 a4 b4");
        // Paste and Overwrite replaces as many frames as were copied.
        assert_eq!(paste(&[4, 4], 1, &clip, false), "a1 a2 b2 b3");
        assert_eq!(paste(&[4, 4], 6, &clip, false), "a4 b2 a2 b2");
        // Past the end, the layer stretches to the pasted frames.
        assert_eq!(paste(&[2], 5, &clip, false), "a5 a2 b2");
        assert_eq!(paste(&[], 0, &clip, true), "a2 b2");
        // Frames the clip lacks are blank where something follows.
        let short = &source.copy_frames(8, 4);
        assert_eq!(paste(&[8], 2, short, false), "a2 c2 _2 a2");
        assert_eq!(paste(&[3], 2, short, false), "a2 c2");
    }

    #[test]
    fn clear_frames() {
        let clear = |durations: &[u32], f, n| {
            let mut l = layer(durations);
            l.clear_frames(f, n);
            spans(&l)
        };
        assert_eq!(clear(&[5, 3], 1, 2), "a1 _2 a2 b3");
        // As far as the layer goes, and no further.
        assert_eq!(clear(&[5, 3], 6, 5), "a5 b1 _2");
        assert_eq!(clear(&[5, 3], 0, 8), "_8");
        assert_eq!(clear(&[5, 3], 9, 1), "a5 b3");
    }

    #[test]
    fn reverse_frames() {
        let reverse = |durations: &[u32], f, n| {
            let mut l = layer(durations);
            let can = l.can_reverse(f, n);
            l.reverse_frames(f, n);
            format!("{can}: {}", spans(&l))
        };
        assert_eq!(reverse(&[3, 2, 1], 0, 6), "true: c2 b3 a1");
        assert_eq!(reverse(&[3, 2, 1], 3, 3), "true: a3 c2 b1");
        // A span running on past the last keyframe keeps showing it there.
        assert_eq!(reverse(&[3, 2, 4], 0, 6), "true: c2 b3 a1 c3");
        // It needs a keyframe at both ends.
        assert_eq!(reverse(&[3, 2, 1], 0, 5), "false: a3 b2 c1");
        assert_eq!(reverse(&[3, 2, 1], 3, 1), "false: a3 b2 c1");

        // A tween now runs from the later keyframe back to the earlier one.
        let mut l = layer(&[4, 1]);
        l.frames[0].tween = CLASSIC;
        l.reverse_frames(0, 5);
        assert_eq!(spans(&l), "b4 a1");
        assert_eq!((l.frames[0].tween.clone(), l.frames[1].tween.clone()), (CLASSIC, Tween::None));
    }

    #[test]
    fn move_frames() {
        let timeline = |rows: &[&[u32]]| Timeline { name: "Scene 1".into(), layers: rows.iter().map(|d| layer(d)).collect(), extra: Extra::default() };
        let moved = |rows: &[&[u32]], from: std::ops::Range<usize>, f, n, to, copy| {
            let mut tl = timeline(rows);
            tl.move_frames(from, f, n, to, copy);
            tl.layers.iter().map(spans).collect::<Vec<_>>().join(" | ")
        };
        // A lone keyframe takes its span along; the span before fills in behind it.
        assert_eq!(moved(&[&[5, 5]], 0..1, 5, 1, (0, 8), false), "a8 b2");
        assert_eq!(moved(&[&[5, 5]], 0..1, 5, 1, (0, 2), false), "a2 b8");
        // Dragging a tween's end keyframe out lengthens it.
        assert_eq!(moved(&[&[10, 1]], 0..1, 10, 1, (0, 15), false), "a15 b1");
        // The first keyframe leaves blank frames behind.
        assert_eq!(moved(&[&[3, 2]], 0..1, 0, 1, (0, 1), false), "_1 a2 b2");
        // A block lands over what is there, here on the layer below.
        assert_eq!(moved(&[&[2, 3, 2], &[6]], 0..1, 2, 3, (1, 1), false), "a5 c2 | a1 b3 a2");
        // Two rows at once; frames after a block keep showing what they did.
        assert_eq!(moved(&[&[6], &[6]], 0..2, 1, 2, (0, 4), false), "a4 a2 | a4 a2");
        assert_eq!(moved(&[&[2, 6]], 0..1, 2, 2, (0, 0), false), "b2 a2 b4");
        // Alt+drag copies.
        assert_eq!(moved(&[&[2, 3, 2]], 0..1, 2, 3, (0, 7), true), "a2 b3 c2 b3");
        assert_eq!(moved(&[&[4]], 0..1, 0, 1, (0, 2), true), "a2 a2");
        // Rows that would land past the last layer are dropped.
        assert_eq!(moved(&[&[3], &[3]], 0..2, 0, 3, (1, 0), true), "a3 | a3");
    }
}
