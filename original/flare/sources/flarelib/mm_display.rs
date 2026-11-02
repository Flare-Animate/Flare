//! Playback rules: what is on stage at a given frame of a timeline. The
//! renderer and the exporters all read the document through this.

use std::collections::BTreeSet;
use std::f32::consts::{PI, TAU};

use crate::doc::*;
use crate::filters::Effects;
use crate::hit::ElementRef;

/// Something on stage, with its transform relative to the timeline's origin.
#[derive(Clone)]
pub struct Placed<'a> {
    pub item: Item<'a>,
    pub matrix: Matrix,
    pub color: ColorTransform,
    /// The element of the listed timeline that put this here. Graphic symbols
    /// and groups are flattened in, so one element can place several items.
    pub source: ElementRef,
    /// A movie clip's, button's or text field's filters and Display settings.
    pub effects: Effects,
}

#[derive(Clone)]
pub enum Item<'a> {
    Shape(&'a Shape),
    /// Drawn as its glyphs' shape (text.rs).
    Text(&'a Text),
    /// A movie clip or button: it runs its own timeline.
    Clip(&'a Symbol, SymbolKind),
    /// A shape tween's in-between, made for this frame (shape_tween.rs).
    Morph(Box<Shape>),
    /// A library bitmap, its pixels from (0, 0) to its size (bitmap.rs).
    Bitmap(&'a BitmapItem),
    /// A mask layer at work: `content`, the items of the layers it masks, shows only where
    /// `mask`, its own items, covers. The mask itself isn't drawn. Placed at the identity: the
    /// items carry their own transforms.
    Masked { mask: Vec<Placed<'a>>, content: Vec<Placed<'a>> },
}

/// Bottom-to-top contents of `frame` of `tl`. Graphic symbols and groups are
/// flattened in, since they play in step with their parent. Features that
/// can't be shown yet are added to `skipped`.
pub fn display_list<'a>(doc: &'a Document, tl: &'a Timeline, frame: u32, skipped: &mut BTreeSet<String>) -> Vec<Placed<'a>> {
    list(doc, tl, frame, skipped, false)
}

/// `display_list` without the layers hidden in the timeline (their eye turned
/// off), as the editor's stage shows them. Publishing still includes them.
pub fn display_list_shown<'a>(doc: &'a Document, tl: &'a Timeline, frame: u32, skipped: &mut BTreeSet<String>) -> Vec<Placed<'a>> {
    list(doc, tl, frame, skipped, true)
}

/// `list` with each masked group replaced by what it masks: what there is to hit and
/// to measure, as a mask draws nothing itself.
pub fn unmasked(list: Vec<Placed<'_>>) -> Vec<Placed<'_>> {
    let mut out = Vec::new();
    for p in list {
        match p.item {
            Item::Masked { content, .. } => out.extend(unmasked(content)),
            _ => out.push(p),
        }
    }
    out
}

/// What layer `l` of the listed timeline puts in `list`, clipped by its mask if
/// that's at work: one layer of the stage, as onion skinning draws them.
pub fn layer_items(list: Vec<Placed<'_>>, l: usize) -> Vec<Placed<'_>> {
    let mut out = Vec::new();
    for p in list {
        match p.item {
            Item::Masked { mask, content } => {
                let content: Vec<_> = content.into_iter().filter(|q| q.source.layer == l).collect();
                if !content.is_empty() {
                    out.push(Placed { item: Item::Masked { mask, content }, matrix: p.matrix, color: p.color, source: p.source, effects: p.effects });
                }
            }
            _ if p.source.layer == l => out.push(p),
            _ => {}
        }
    }
    out
}

fn list<'a>(doc: &'a Document, tl: &'a Timeline, frame: u32, skipped: &mut BTreeSet<String>, shown_only: bool) -> Vec<Placed<'a>> {
    let source = ElementRef { layer: 0, keyframe: 0, element: 0 };
    let mut lister = Lister { doc, skipped, out: Vec::new(), source, shown_only };
    lister.timeline(tl, frame, Matrix::IDENTITY, ColorTransform::IDENTITY, 0);
    lister.out
}

struct Lister<'a, 's> {
    doc: &'a Document,
    skipped: &'s mut BTreeSet<String>,
    out: Vec<Placed<'a>>,
    /// The top-level element being listed.
    source: ElementRef,
    /// Skip the listed timeline's hidden layers.
    shown_only: bool,
}

impl<'a> Lister<'a, '_> {
    fn timeline(&mut self, tl: &'a Timeline, frame: u32, m: Matrix, cx: ColorTransform, depth: u32) {
        if depth > 32 {
            self.skipped.insert("graphics nested over 32 deep".into());
            return;
        }
        // Published, a mask always clips. The editor's stage shows a mask at work once it's
        // locked, and clips each masked layer once that's locked too (Show Masking locks both);
        // unlocked, each shows as it is, the mask's shape and all.
        let editing = self.shown_only && depth == 0;
        let at_work = |l: usize| !editing || tl.layers[l].locked();
        // The masked layers listed so far: their mask, and where their items start in `out`.
        let mut masked: Option<(usize, usize)> = None;
        for (l, layer) in tl.layers.iter().enumerate().rev() {
            if editing && !layer.visible() {
                continue;
            }
            match layer.kind {
                LayerKind::Normal | LayerKind::Mask => {}
                // A guide shows on the stage of the timeline being edited, and never publishes.
                LayerKind::Guide if editing => {}
                LayerKind::Guide | LayerKind::Folder => continue,
            }
            if let Some(mask) = tl.mask_of(l).filter(|&mask| at_work(mask) && at_work(l))
                && masked.is_none_or(|(by, _)| by != mask)
            {
                masked = Some((mask, self.out.len()));
            }
            // A mask at work gathers its items, and its masked layers', into a group: empty, it hides them.
            let (start, mask) = (self.out.len(), layer.kind == LayerKind::Mask && at_work(l));
            let Some(i) = layer.keyframe_at(frame) else {
                if mask {
                    self.mask(l, start, &mut masked, depth, &at_work);
                }
                continue;
            };
            let key = &layer.frames[i];
            let offset = frame - key.index;
            if let Tween::Unsupported(kind) = &key.tween {
                self.skipped.insert(format!("{kind} tweens"));
            }
            // Along a motion guide, and after the layer this one's parented to.
            let (tweened, rig) = crate::layer_motion::motion(tl, l, i, offset);
            let m = m * rig;
            // A shape tween shows an in-between of its own; a motion tween moves its text too.
            if let Some(shape) = crate::shape_tween::at(key, layer.frames.get(i + 1), offset) {
                if depth == 0 {
                    self.source = ElementRef { layer: l, keyframe: i, element: 0 };
                }
                self.out.push(Placed { item: Item::Morph(Box::new(shape)), matrix: m, color: cx, source: self.source, effects: Effects::default() });
            } else {
                let moved = crate::motion::text_at(key, offset);
                for (e, el) in key.elements.iter().enumerate() {
                    if depth == 0 {
                        self.source = ElementRef { layer: l, keyframe: i, element: e };
                    }
                    // A tween only starts from a keyframe holding one instance, and replaces it.
                    match (&tweened, el) {
                        (Some(inst), _) => self.instance(inst, offset, m, cx, depth),
                        (None, Element::Text(t)) if let Some(at) = moved => {
                            self.out.push(Placed { item: Item::Text(t), matrix: m * at, color: cx, source: self.source, effects: Effects::of(&t.extra) })
                        }
                        (None, _) => self.element(el, offset, m, cx, depth),
                    }
                }
            }
            if mask {
                self.mask(l, start, &mut masked, depth, &at_work);
            }
        }
    }

    /// Mask layer `l`'s items, listed from `start` on, become a masked group with
    /// the items its masked layers listed before them. On the stage, a masked layer
    /// that isn't `at_work` (unlocked) over ones that are shows as it is, between groups.
    fn mask(&mut self, l: usize, start: usize, masked: &mut Option<(usize, usize)>, depth: u32, at_work: &dyn Fn(usize) -> bool) {
        let mask = self.out.split_off(start);
        let content = match masked.take() {
            Some((by, from)) if by == l => self.out.split_off(from),
            other => {
                *masked = other;
                Vec::new()
            }
        };
        let source = if depth == 0 { ElementRef { layer: l, keyframe: 0, element: 0 } } else { self.source };
        let clipped = |p: &Placed| depth > 0 || at_work(p.source.layer);
        let mut content = content.into_iter().peekable();
        while let Some(p) = content.next() {
            if !clipped(&p) {
                self.out.push(p);
                continue;
            }
            let mut run = vec![p];
            while let Some(q) = content.next_if(clipped) {
                run.push(q);
            }
            let item = Item::Masked { mask: mask.clone(), content: run };
            self.out.push(Placed { item, matrix: Matrix::IDENTITY, color: ColorTransform::IDENTITY, source, effects: Effects::default() });
        }
    }

    fn element(&mut self, el: &'a Element, offset: u32, m: Matrix, cx: ColorTransform, depth: u32) {
        match el {
            Element::Shape(s) => self.out.push(Placed {
                item: Item::Shape(s),
                matrix: m * s.matrix,
                color: cx,
                source: self.source,
                effects: Effects::default(),
            }),
            Element::Text(t) => self.out.push(Placed {
                item: Item::Text(t),
                matrix: m * t.matrix,
                color: cx,
                source: self.source,
                effects: Effects::of(&t.extra),
            }),
            Element::Instance(i) => self.instance(i, offset, m, cx, depth),
            Element::Group(members) => members.iter().for_each(|el| self.element(el, offset, m, cx, depth)),
            Element::Bitmap(b) => match self.doc.bitmap(&b.name) {
                Some(item) => self.out.push(Placed { item: Item::Bitmap(item), matrix: m * b.matrix, color: cx, source: self.source, effects: Effects::default() }),
                None => {
                    self.skipped.insert(format!("missing bitmap {:?}", b.name));
                }
            },
            Element::Unsupported { name, .. } => {
                self.skipped.insert(name.clone());
            }
        }
    }

    /// `offset` is how many frames ago the instance's keyframe started.
    fn instance(&mut self, inst: &Instance, offset: u32, m: Matrix, cx: ColorTransform, depth: u32) {
        let Some(sym) = self.doc.library.get(&inst.symbol) else {
            self.skipped.insert(format!("missing symbol {:?}", inst.symbol));
            return;
        };
        let (m, cx) = (m * inst.matrix, cx * inst.color);
        if inst.kind != SymbolKind::Graphic {
            let effects = Effects::of(&inst.extra);
            self.out.push(Placed { item: Item::Clip(sym, inst.kind), matrix: m, color: cx, source: self.source, effects });
            return;
        }
        self.timeline(&sym.timeline, graphic_frame(inst, offset, &sym.timeline), m, cx, depth + 1);
    }
}

/// The frame of `symbol` a graphic instance shows `offset` frames into its keyframe.
pub(crate) fn graphic_frame(inst: &Instance, offset: u32, symbol: &Timeline) -> u32 {
    let len = symbol.frame_count().max(1);
    match inst.looping {
        Loop::Loop => (inst.first_frame + offset) % len,
        Loop::PlayOnce => (inst.first_frame + offset).min(len - 1),
        Loop::SingleFrame => inst.first_frame,
    }
}

/// The in-between instance `offset` frames into `key`'s classic or motion tween,
/// or None to show the keyframe as is.
pub fn tween_at(key: &Frame, next: Option<&Frame>, offset: u32) -> Option<Instance> {
    if let Some(Element::Instance(inst)) = crate::motion::target_at(key, offset) {
        return Some(inst);
    }
    let Tween::Classic { ease, rotate } = key.tween else { return None };
    // Like Animate, a broken tween (no next keyframe, or not one matching symbol each side) holds.
    let next = next.filter(|n| n.index == key.index + key.duration)?;
    let ([Element::Instance(a)], [Element::Instance(b)]) = (&key.elements[..], &next.elements[..]) else {
        return None;
    };
    if a.symbol != b.symbol {
        return None;
    }
    let t = offset as f32 / key.duration as f32;
    let t = t + ease * t * (1.0 - t);
    let mut inst = lerp_instance(a, b, t, rotate);
    // Filters tween too, kept in the in-between's XFL as the keyframes keep theirs.
    if let Some(list) = crate::filters::tween(&a.extra, &b.extra, t) {
        crate::filters::set_filters(&mut inst.extra, &list);
    }
    Some(inst)
}

/// Classic tween in-between: scale and skew angles interpolate separately and
/// the transformation point moves in a straight line, as in Animate.
fn lerp_instance(a: &Instance, b: &Instance, t: f32, rotate: Rotate) -> Instance {
    let lerp = |x: f32, y: f32| x + (y - x) * t;
    let turn = |from: f32, to: f32| {
        let d = (to - from).rem_euclid(TAU);
        from + t * match rotate {
            Rotate::Auto => if d > PI { d - TAU } else { d },
            Rotate::Cw(n) => d + TAU * n as f32,
            Rotate::Ccw(n) => (if d > 0.0 { d - TAU } else { 0.0 }) - TAU * n as f32,
        }
    };
    let decompose = |m: &Matrix| (m.a.hypot(m.b), m.c.hypot(m.d), (-m.c).atan2(m.d), m.b.atan2(m.a));
    let (sa, sb) = (decompose(&a.matrix), decompose(&b.matrix));
    let (sx, sy, skew_x, skew_y) = (lerp(sa.0, sb.0), lerp(sa.1, sb.1), turn(sa.2, sb.2), turn(sa.3, sb.3));

    let pivot = (lerp(a.pivot.0, b.pivot.0), lerp(a.pivot.1, b.pivot.1));
    let (pa, pb) = (a.matrix.apply(a.pivot), b.matrix.apply(b.pivot));
    let mut matrix = Matrix {
        a: sx * skew_y.cos(),
        b: sx * skew_y.sin(),
        c: -sy * skew_x.sin(),
        d: sy * skew_x.cos(),
        tx: 0.0,
        ty: 0.0,
    };
    let at = matrix.apply(pivot);
    (matrix.tx, matrix.ty) = (lerp(pa.0, pb.0) - at.0, lerp(pa.1, pb.1) - at.1);

    let color = ColorTransform {
        mult: std::array::from_fn(|i| lerp(a.color.mult[i], b.color.mult[i])),
        add: std::array::from_fn(|i| lerp(a.color.add[i], b.color.add[i])),
    };
    Instance { matrix, pivot, color, ..a.clone() }
}
