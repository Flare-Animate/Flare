//! Adapter between the AnimationCraft document model (`flarelib/model.rs`, crate `ac-core`)
//! and the minnamate document (`flarelib/mm_doc.rs`), step 3 of docs/UNIFICATION_PLAN.md.
//!
//! This file belongs to `minnamate-core` (tnztools/minnamate_core.rs, `mod bridge`), never to
//! ac-core's mod list: it is the one place that sees both models, and mm_* files must stay out
//! of the ac crates. `minnamate-core` therefore depends on `ac-core`.
//!
//! Bridged: scenes (ac `scene` + `more_scenes` <-> mm `scenes`), layers (name, kind, visible,
//! locked, outline, color, parent), keyframes <-> frames (start, span, label), library symbols
//! (name, kind, timeline; ac ids <-> mm names), symbol instances (symbol, matrix, loop mode,
//! first frame, instance name, color effect), shapes (matrix, solid fill, solid stroke, path),
//! groups, classic and shape tweens (ease, rotation count, shape hints), size, fps, background.
//!
//! Lossy, always counted in the returned [`BridgeReport`] (never dropped silently):
//! - `ac.camera_layer`: Camera layers are skipped (mm has no camera layer).
//! - `ac.element.{text,bitmap,canvas,particles}`: not converted. mm text/bitmap likewise
//!   (`mm.element.*`), as is `mm.element.unsupported`.
//! - Z (close) comes back as a line to the subpath start (mm has no Z): same outline, other segs.
//! - `ac.element.extras`: 3D, filters, blend, mesh, hidden flag, cache/export-as-bitmap, tint.
//! - `ac.shape.extras`: gradient (variable width and art params are not checked); cubic
//!   segments are approximated by two quads (`ac.shape.cubic`).
//! - `mm.shape.paint`: mm gradient/bitmap fills, and fills/strokes beyond the first used.
//! - `ac.symbol.renamed`: same-named ac symbols get unique mm names (`Name~2`); instances follow.
//! - `ac.keyframe.leading_blank`: a blank mm keyframe inserted at 0 before an ac layer's first keyframe.
//! - `mm.layer.dangling_parent`: out-of-range `parentLayerIndex` dropped.
//! - `ac.shape.extras` also: width profile, warp, primitive, dash/hairline/art stroke style (cap/join are mapped).
//! - `ac.layer.extras`: layer colour alpha, anim_parent, row_height, collapsed, layer filters.
//! - `ac.keyframe.extras`: script, sound, camera options, prop eases, ease curve, sync/snap,
//!   playback offset/reversal, tween boundary, orient-to-path.
//! - `ac.tween.motion` / `mm.tween.motion`: motion tweens (the two Motion types differ);
//!   the span becomes a plain keyframe. `mm.tween.unsupported`.
//! - Rotation: mm `Rotate::Auto` and XFL "none" are both 0 turns in ac (the same thing, not lossy).
//! - `ac.loop_mode`: ac reverse loop modes become `Loop`.
//! - `ac.instance.pivot` / `mm.instance.pivot`: mm's transformation point has no ac field.
//! - `ac.document.extras`: guides, swatches, assistants, perspective, camera mode, folders
//!   (empty ones), linkage, swf ids, warnings, bitmaps, sounds. `mm.document.media`: media, sounds.
//! - `mm.extra`: mm `Extra` XML/attributes the model doesn't read (other than the ones above).
//! - `ac.element.alpha`: alpha is folded into the instance color transform, so it comes back
//!   as `color_fx` (same rendering, different field).

use crate::doc as mm;
use ac_core::geom::{Matrix as AcMatrix, Path as AcPath, Pt, Seg as AcSeg};
use ac_core::model as ac;
use std::collections::{BTreeMap, HashMap};

/// What a conversion could not carry across: field key (see the module docs) -> occurrences.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BridgeReport {
    pub lossy: BTreeMap<&'static str, usize>,
}

impl BridgeReport {
    fn lose(&mut self, key: &'static str) {
        *self.lossy.entry(key).or_default() += 1;
    }
    pub fn is_lossless(&self) -> bool {
        self.lossy.is_empty()
    }
    pub fn count(&self, key: &str) -> usize {
        self.lossy.get(key).copied().unwrap_or(0)
    }
}

fn to_mm_matrix(m: &AcMatrix) -> mm::Matrix {
    mm::Matrix { a: m.a, b: m.b, c: m.c, d: m.d, tx: m.tx, ty: m.ty }
}
fn to_ac_matrix(m: &mm::Matrix) -> AcMatrix {
    AcMatrix { a: m.a, b: m.b, c: m.c, d: m.d, tx: m.tx, ty: m.ty }
}
fn to_mm_rgba(c: ac::Color) -> mm::Rgba {
    [c.r as f32 / 255.0, c.g as f32 / 255.0, c.b as f32 / 255.0, c.a as f32 / 255.0]
}
fn to_ac_color(c: mm::Rgba) -> ac::Color {
    let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    ac::Color { r: b(c[0]), g: b(c[1]), b: b(c[2]), a: b(c[3]) }
}
fn p(pt: Pt) -> mm::Point {
    (pt.x, pt.y)
}
fn q(pt: mm::Point) -> Pt {
    Pt { x: pt.0, y: pt.1 }
}

// ---------------------------------------------------------------- ac -> mm

/// Converts an AnimationCraft document to a minnamate one.
pub fn ac_to_mm(src: &ac::Document) -> (mm::Document, BridgeReport) {
    let mut rep = BridgeReport::default();
    let mut doc = mm::Document::new(src.width, src.height, src.fps);
    doc.background = to_mm_rgba(src.background);
    // mm keys symbols by name: give each ac symbol a unique one (suffix `~2`, `~3`, ...) so two
    // same-named symbols never overwrite each other; instances use the id -> name map.
    let mut used: std::collections::HashSet<String> = src.library.iter().map(|s| s.name.clone()).collect();
    let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let mut unique: Vec<String> = Vec::new();
    for s in &src.library {
        if seen.insert(s.name.as_str()) {
            unique.push(s.name.clone());
        } else {
            let mut n = 2;
            let mut cand = format!("{}~{n}", s.name);
            while used.contains(&cand) {
                n += 1;
                cand = format!("{}~{n}", s.name);
            }
            used.insert(cand.clone());
            rep.lose("ac.symbol.renamed");
            unique.push(cand);
        }
    }
    let mut names: HashMap<ac::SymbolId, (String, mm::SymbolKind)> = HashMap::new();
    for (s, u) in src.library.iter().zip(&unique) {
        names.entry(s.id).or_insert_with(|| (u.clone(), mm_kind(s.kind)));
    }
    doc.scenes = std::iter::once(&src.scene).chain(&src.more_scenes).map(|t| timeline_to_mm(t, &names, &mut rep)).collect();
    for (s, uname) in src.library.iter().zip(&unique) {
        let kind = mm_kind(s.kind);
        if s.scale_grid.is_some() {
            rep.lose("ac.document.extras");
        }
        let timeline = timeline_to_mm(&s.timeline, &names, &mut rep);
        doc.library.insert(uname.clone(), mm::Symbol { kind, timeline: mm::Timeline { name: uname.clone(), ..timeline }, extra: Default::default() });
    }
    let d = &src.bitmaps;
    let has_other = !d.is_empty() || !src.sounds.is_empty() || !src.guides.is_empty() || src.swatches.is_some() || !src.assistants.is_empty() || !src.folders.is_empty() || !src.linkage.is_empty() || !src.swf_char_ids.is_empty() || !src.warnings.is_empty() || src.vanishing_point.is_some() || !src.camera_mode.is_stage();
    if has_other {
        rep.lose("ac.document.extras");
    }
    (doc, rep)
}

fn timeline_to_mm(t: &ac::Timeline, names: &HashMap<ac::SymbolId, (String, mm::SymbolKind)>, rep: &mut BridgeReport) -> mm::Timeline {
    let mut layers = Vec::new();
    // ac layer index -> mm layer index (camera layers are skipped), for `parent`.
    let mut remap: HashMap<usize, usize> = HashMap::new();
    for (i, l) in t.layers.iter().enumerate() {
        if l.kind == ac::LayerKind::Camera {
            rep.lose("ac.camera_layer");
            continue;
        }
        remap.insert(i, layers.len());
        layers.push(layer_to_mm(l, names, rep));
    }
    for (i, l) in t.layers.iter().enumerate() {
        if let (Some(par), Some(&mi)) = (l.parent, remap.get(&i)) {
            match remap.get(&par) {
                Some(mp) => layers[mi].extra.set("parentLayerIndex", Some(&mp.to_string())),
                None => rep.lose("ac.layer.extras"),
            }
        }
    }
    mm::Timeline { name: t.name.clone(), layers, extra: Default::default() }
}

fn layer_to_mm(l: &ac::Layer, names: &HashMap<ac::SymbolId, (String, mm::SymbolKind)>, rep: &mut BridgeReport) -> mm::Layer {
    if !l.bones.is_empty() || l.anim_parent.is_some() || l.row_height != 100 || l.collapsed || !l.filters.is_empty() || l.color.a != 255 {
        rep.lose("ac.layer.extras");
    }
    let kind = match l.kind {
        ac::LayerKind::Normal | ac::LayerKind::Camera => mm::LayerKind::Normal,
        ac::LayerKind::Guide => mm::LayerKind::Guide,
        ac::LayerKind::Mask => mm::LayerKind::Mask,
        ac::LayerKind::Folder => mm::LayerKind::Folder,
    };
    let mut frames = Vec::new();
    let starts: Vec<u32> = l.keyframes.keys().copied().collect();
    // mm frames run back to back from 0: a leading gap becomes a blank keyframe.
    if let Some(&first) = starts.first() {
        if first > 0 {
            rep.lose("ac.keyframe.leading_blank");
            frames.push(mm::Frame { index: 0, duration: first, tween: mm::Tween::None, elements: vec![], extra: Default::default() });
        }
    }
    for (n, (&start, kf)) in l.keyframes.iter().enumerate() {
        let end = starts.get(n + 1).copied().unwrap_or(l.end.max(start.saturating_add(1)));
        let next = starts.get(n + 1).and_then(|s| l.keyframes.get(s));
        let mut frame = mm::Frame { index: start, duration: end.saturating_sub(start).max(1), tween: tween_to_mm(kf, next, rep), elements: vec![], extra: Default::default() };
        frame.elements = kf.elements.iter().filter_map(|e| element_to_mm(e, names, rep)).collect();
        if let Some(label) = &kf.label {
            frame.set_label(label, kf.label_kind.id());
        }
        if kf.script.is_some() || kf.sound.is_some() || kf.camera.is_some() || kf.prop_eases.is_some() || kf.ease_curve.is_some() || kf.sync || !kf.snap || kf.playback_offset != 0 || kf.playback_reversed || kf.tween_boundary.is_some() || kf.orient_to_path {
            rep.lose("ac.keyframe.extras");
        }
        frames.push(frame);
    }
    let mut out = mm::Layer { name: l.name.clone(), kind, frames, extra: Default::default() };
    out.set_visible(l.visible);
    out.set_locked(l.locked);
    out.set_outline(l.outline);
    out.set_color([l.color.r as f32 / 255.0, l.color.g as f32 / 255.0, l.color.b as f32 / 255.0, 1.0]);
    out
}

fn tween_to_mm(kf: &ac::Keyframe, next: Option<&ac::Keyframe>, rep: &mut BridgeReport) -> mm::Tween {
    match kf.tween {
        ac::Tween::None => mm::Tween::None,
        ac::Tween::Classic { ease, rotate_turns } => {
            let rotate = match rotate_turns {
                0 => mm::Rotate::Auto,
                n if n > 0 => mm::Rotate::Cw(n as u32),
                n => mm::Rotate::Ccw(n.unsigned_abs()),
            };
            mm::Tween::Classic { ease: ease / 100.0, rotate }
        }
        ac::Tween::Shape { ease } => {
            let nh = next.map(|n| n.shape_hints.as_slice()).unwrap_or(&[]);
            let hints = kf.shape_hints.iter().zip(nh).map(|(a, b)| (p(*a), p(*b))).collect();
            mm::Tween::Shape { ease: ease / 100.0, angular: false, hints }
        }
        ac::Tween::Motion { .. } => {
            rep.lose("ac.tween.motion");
            mm::Tween::None
        }
    }
}

fn element_to_mm(e: &ac::Element, names: &HashMap<ac::SymbolId, (String, mm::SymbolKind)>, rep: &mut BridgeReport) -> Option<mm::Element> {
    if e.three_d.is_some() || !e.filters.is_empty() || e.blend != ac::BlendMode::Normal || e.mesh.is_some() || !e.visible || e.cache_as_bitmap || e.export_as_bitmap || e.tint.is_some() {
        rep.lose("ac.element.extras");
    }
    if !e.name.is_empty() && !matches!(e.kind, ac::ElementKind::Instance(_)) {
        rep.lose("ac.element.extras");
    }
    let m = to_mm_matrix(&e.matrix);
    match &e.kind {
        ac::ElementKind::Shape(s) => Some(mm::Element::Shape(shape_to_mm(s, m, rep))),
        ac::ElementKind::Instance(i) => {
            let Some((name, kind)) = names.get(&i.symbol) else {
                rep.lose("ac.element.dangling_symbol");
                return None;
            };
            let looping = match i.loop_mode {
                ac::LoopMode::Loop => mm::Loop::Loop,
                ac::LoopMode::PlayOnce => mm::Loop::PlayOnce,
                ac::LoopMode::SingleFrame => mm::Loop::SingleFrame,
                ac::LoopMode::PlayReverseOnce | ac::LoopMode::LoopReverse => {
                    rep.lose("ac.loop_mode");
                    mm::Loop::Loop
                }
            };
            let xf = ac::ColorXform::compose(Some(ac::ColorXform { mul: [1.0, 1.0, 1.0, e.alpha], add: [0.0; 4] }), e.color_fx).unwrap_or_default();
            let color = mm::ColorTransform { mult: xf.mul, add: xf.add.map(|v| v / 255.0) };
            let mut extra = mm::Extra::default();
            if !e.name.is_empty() {
                extra.set("name", Some(&e.name));
            }
            Some(mm::Element::Instance(mm::Instance {
                symbol: name.clone(),
                kind: *kind,
                matrix: m,
                pivot: (0.0, 0.0),
                color,
                looping,
                first_frame: i.first_frame,
                extra,
            }))
        }
        ac::ElementKind::Group(children) => Some(mm::Element::Group(children.iter().filter_map(|c| element_to_mm(c, names, rep)).collect())),
        ac::ElementKind::Text(_) => {
            rep.lose("ac.element.text");
            None
        }
        ac::ElementKind::Bitmap(_) => {
            rep.lose("ac.element.bitmap");
            None
        }
        ac::ElementKind::Canvas(_) => {
            rep.lose("ac.element.canvas");
            None
        }
        ac::ElementKind::Particles(_) => {
            rep.lose("ac.element.particles");
            None
        }
    }
}

fn shape_to_mm(s: &ac::Shape, matrix: mm::Matrix, rep: &mut BridgeReport) -> mm::Shape {
    let ss = &s.stroke_style;
    let dflt = ac::StrokeStyle { cap: ss.cap, join: ss.join, ..ac::StrokeStyle::default() };
    if s.gradient.is_some() || s.width_profile.is_some() || s.warp.is_some() || s.primitive.is_some() || *ss != dflt {
        rep.lose("ac.shape.extras");
    }
    let mut fills = Vec::new();
    let mut strokes = Vec::new();
    if let Some(f) = s.fill {
        fills.push(mm::Paint::Solid(to_mm_rgba(f)));
    }
    if let Some(st) = s.stroke {
        strokes.push(mm::Stroke { width: st.width, paint: mm::Paint::Solid(to_mm_rgba(st.color)), cap: match s.stroke_style.cap {
            ac::Cap::None => mm::Cap::None,
            ac::Cap::Round => mm::Cap::Round,
            ac::Cap::Square => mm::Cap::Square,
        }, join: match s.stroke_style.join {
            ac::Join::Miter => mm::Join::Miter,
            ac::Join::Round => mm::Join::Round,
            ac::Join::Bevel => mm::Join::Bevel,
        } });
    }
    let (fill0, fill1, stroke) = (0, fills.len() as u32, strokes.len() as u32);
    let mut paths: Vec<mm::Path> = Vec::new();
    let mut cur: Option<(mm::Path, mm::Point)> = None; // path, subpath start
    let mut last = (0.0, 0.0);
    let flush = |cur: &mut Option<(mm::Path, mm::Point)>, paths: &mut Vec<mm::Path>| {
        if let Some((path, _)) = cur.take() {
            if !path.segs.is_empty() {
                paths.push(path);
            }
        }
    };
    for seg in &s.path.segs {
        match *seg {
            AcSeg::M(a) => {
                flush(&mut cur, &mut paths);
                cur = Some((mm::Path { start: p(a), segs: vec![] }, p(a)));
                last = p(a);
            }
            AcSeg::L(a) => {
                if let Some((path, _)) = cur.as_mut() {
                    path.segs.push(mm::Seg::Line(p(a)));
                }
                last = p(a);
            }
            AcSeg::Q(c, a) => {
                if let Some((path, _)) = cur.as_mut() {
                    path.segs.push(mm::Seg::Quad(p(c), p(a)));
                }
                last = p(a);
            }
            AcSeg::C(c1, c2, a) => {
                rep.lose("ac.shape.cubic");
                // Split at t = 0.5 (de Casteljau) and approximate each half by one quad.
                let (p0, p1, p2, p3) = (last, p(c1), p(c2), p(a));
                let mid = |a: mm::Point, b: mm::Point| ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
                let (m01, m12, m23) = (mid(p0, p1), mid(p1, p2), mid(p2, p3));
                let (m012, m123) = (mid(m01, m12), mid(m12, m23));
                let m = mid(m012, m123);
                let quad_ctl = |a: mm::Point, c1: mm::Point, c2: mm::Point, b: mm::Point| {
                    ((3.0 * (c1.0 + c2.0) - a.0 - b.0) / 4.0, (3.0 * (c1.1 + c2.1) - a.1 - b.1) / 4.0)
                };
                if let Some((path, _)) = cur.as_mut() {
                    path.segs.push(mm::Seg::Quad(quad_ctl(p0, m01, m012, m), m));
                    path.segs.push(mm::Seg::Quad(quad_ctl(m, m123, m23, p3), p3));
                }
                last = p3;
            }
            AcSeg::Z => {
                if let Some((path, start)) = cur.as_mut() {
                    if last != *start {
                        path.segs.push(mm::Seg::Line(*start));
                    }
                    last = *start;
                }
            }
        }
    }
    flush(&mut cur, &mut paths);
    let edges = if paths.is_empty() { vec![] } else { vec![mm::Edge { fill0, fill1, stroke, paths }] };
    mm::Shape { matrix, fills, strokes, edges, extra: Default::default() }
}

// ---------------------------------------------------------------- mm -> ac

/// Converts a minnamate document to an AnimationCraft one.
pub fn mm_to_ac(src: &mm::Document) -> (ac::Document, BridgeReport) {
    let mut rep = BridgeReport::default();
    let mut doc = ac::Document { width: src.width, height: src.height, fps: src.frame_rate, background: to_ac_color(src.background), ..Default::default() };
    let mut names: Vec<&String> = src.library.keys().collect();
    names.sort();
    let ids: HashMap<String, ac::SymbolId> = names.iter().enumerate().map(|(i, n)| ((*n).clone(), i as ac::SymbolId + 1)).collect();
    let kinds: HashMap<String, ac::SymbolKind> = src.library.iter().map(|(n, s)| (n.clone(), sym_kind(s.kind))).collect();
    let ctx = Ctx { ids: &ids, kinds: &kinds };
    let mut scenes = src.scenes.iter().map(|t| timeline_to_ac(t, &ctx, &mut rep));
    doc.scene = scenes.next().unwrap_or_else(|| ac::Timeline::new("Scene 1"));
    doc.more_scenes = scenes.collect();
    for name in names {
        let s = &src.library[name];
        if !s.extra.attrs.is_empty() || !s.extra.before.is_empty() || !s.extra.after.is_empty() {
            rep.lose("mm.extra");
        }
        doc.library.push(ac::Symbol { id: ids[name], name: name.clone(), kind: sym_kind(s.kind), timeline: timeline_to_ac(&s.timeline, &ctx, &mut rep), scale_grid: None });
    }
    doc.next_symbol_id = ids.len() as ac::SymbolId + 1;
    if !src.media.is_empty() || !src.sounds.is_empty() {
        rep.lose("mm.document.media");
    }
    (doc, rep)
}

struct Ctx<'a> {
    ids: &'a HashMap<String, ac::SymbolId>,
    kinds: &'a HashMap<String, ac::SymbolKind>,
}

fn mm_kind(k: ac::SymbolKind) -> mm::SymbolKind {
    match k {
        ac::SymbolKind::Graphic => mm::SymbolKind::Graphic,
        ac::SymbolKind::MovieClip => mm::SymbolKind::MovieClip,
        ac::SymbolKind::Button => mm::SymbolKind::Button,
    }
}

fn sym_kind(k: mm::SymbolKind) -> ac::SymbolKind {
    match k {
        mm::SymbolKind::MovieClip => ac::SymbolKind::MovieClip,
        mm::SymbolKind::Graphic => ac::SymbolKind::Graphic,
        mm::SymbolKind::Button => ac::SymbolKind::Button,
    }
}

fn timeline_to_ac(t: &mm::Timeline, ctx: &Ctx, rep: &mut BridgeReport) -> ac::Timeline {
    if !t.extra.attrs.is_empty() || !t.extra.before.is_empty() || !t.extra.after.is_empty() {
        rep.lose("mm.extra");
    }
    let mut layers: Vec<ac::Layer> = t.layers.iter().enumerate().map(|(i, l)| layer_to_ac(l, i, ctx, rep)).collect();
    let n = layers.len();
    for (i, l) in layers.iter_mut().enumerate() {
        if matches!(l.parent, Some(p) if p >= n || p == i) {
            l.parent = None;
            rep.lose("mm.layer.dangling_parent");
        }
    }
    ac::Timeline { name: t.name.clone(), layers }
}

fn layer_to_ac(l: &mm::Layer, index: usize, ctx: &Ctx, rep: &mut BridgeReport) -> ac::Layer {
    let kind = match l.kind {
        mm::LayerKind::Normal => ac::LayerKind::Normal,
        mm::LayerKind::Guide => ac::LayerKind::Guide,
        mm::LayerKind::Mask => ac::LayerKind::Mask,
        mm::LayerKind::Folder => ac::LayerKind::Folder,
    };
    let mut out = ac::Layer::new(&l.name, index);
    out.kind = kind;
    out.visible = l.visible();
    out.locked = l.locked();
    out.outline = l.outline();
    if let Some(c) = l.color() {
        out.color = to_ac_color(c);
    }
    out.parent = l.extra.get("parentLayerIndex").and_then(|v| v.parse().ok());
    for (k, _) in &l.extra.attrs {
        if !matches!(k.as_str(), "visible" | "locked" | "outline" | "color" | "parentLayerIndex") {
            rep.lose("mm.extra");
        }
    }
    if !l.extra.before.is_empty() || !l.extra.after.is_empty() {
        rep.lose("mm.extra");
    }
    out.keyframes.clear();
    out.end = 0;
    for (n, f) in l.frames.iter().enumerate() {
        let next = l.frames.get(n + 1);
        let mut kf = ac::Keyframe::with(f.elements.iter().filter_map(|e| element_to_ac(e, ctx, rep)).collect());
        match &f.tween {
            mm::Tween::None => {}
            mm::Tween::Classic { ease, rotate } => {
                let rotate_turns = match rotate {
                    mm::Rotate::Auto => 0,
                    mm::Rotate::Cw(n) => *n as i32,
                    mm::Rotate::Ccw(n) => -(*n as i32),
                };
                kf.tween = ac::Tween::Classic { ease: ease * 100.0, rotate_turns };
            }
            mm::Tween::Shape { ease, hints, .. } => {
                kf.tween = ac::Tween::Shape { ease: ease * 100.0 };
                kf.shape_hints = hints.iter().map(|h| q(h.0)).collect();
            }
            mm::Tween::Motion(_) => rep.lose("mm.tween.motion"),
            mm::Tween::Unsupported(_) => rep.lose("mm.tween.unsupported"),
        }
        // The closing hints live on the next keyframe.
        if let (mm::Tween::Shape { hints, .. }, Some(_)) = (&f.tween, next) {
            if !hints.is_empty() {
                out.keyframes.entry(f.index.saturating_add(f.duration)).or_insert_with(|| ac::Keyframe::blank()).shape_hints = hints.iter().map(|h| q(h.1)).collect();
            }
        }
        if let Some((name, kind)) = f.label() {
            kf.label = Some(name.to_string());
            kf.label_kind = ac::LabelKind::from_id(kind).unwrap_or(ac::LabelKind::Name);
        }
        for (k, _) in &f.extra.attrs {
            if !matches!(k.as_str(), "name" | "labelType") {
                rep.lose("mm.extra");
            }
        }
        if !f.extra.before.is_empty() || !f.extra.after.is_empty() {
            rep.lose("mm.extra");
        }
        // A pre-created closing keyframe (for hints) is merged, not replaced.
        if let Some(pre) = out.keyframes.remove(&f.index) {
            kf.shape_hints = if kf.shape_hints.is_empty() { pre.shape_hints } else { kf.shape_hints };
        }
        out.keyframes.insert(f.index, kf);
        out.end = out.end.max(f.index.saturating_add(f.duration));
    }
    out.end = out.end.max(1);
    if out.keyframes.is_empty() {
        out.keyframes.insert(0, ac::Keyframe::blank());
    }
    // Drop a hint-only keyframe that landed past the end.
    out.keyframes.retain(|s, _| *s < out.end);
    out
}

fn element_to_ac(e: &mm::Element, ctx: &Ctx, rep: &mut BridgeReport) -> Option<ac::Element> {
    match e {
        mm::Element::Shape(s) => {
            let (shape, matrix) = shape_to_ac(s, rep);
            let mut el = ac::Element::shape("", shape);
            el.matrix = matrix;
            Some(el)
        }
        mm::Element::Instance(i) => {
            let Some(&id) = ctx.ids.get(&i.symbol) else {
                rep.lose("mm.element.unsupported");
                return None;
            };
            if i.pivot != (0.0, 0.0) {
                rep.lose("mm.instance.pivot");
            }
            let mut el = ac::Element::instance(i.extra.get("name").unwrap_or(""), id, to_ac_matrix(&i.matrix));
            if let ac::ElementKind::Instance(inst) = &mut el.kind {
                inst.first_frame = i.first_frame;
                inst.loop_mode = match i.looping {
                    mm::Loop::Loop => ac::LoopMode::Loop,
                    mm::Loop::PlayOnce => ac::LoopMode::PlayOnce,
                    mm::Loop::SingleFrame => ac::LoopMode::SingleFrame,
                };
            }
            let _ = ctx.kinds;
            let xf = ac::ColorXform { mul: i.color.mult, add: i.color.add.map(|v| v * 255.0) };
            if !xf.is_identity() {
                el.color_fx = Some(xf);
            }
            for (k, _) in &i.extra.attrs {
                if k != "name" {
                    rep.lose("mm.extra");
                }
            }
            Some(el)
        }
        mm::Element::Group(children) => Some(ac::Element::group(children.iter().filter_map(|c| element_to_ac(c, ctx, rep)).collect())),
        mm::Element::Text(_) => {
            rep.lose("mm.element.text");
            None
        }
        mm::Element::Bitmap(_) => {
            rep.lose("mm.element.bitmap");
            None
        }
        mm::Element::Unsupported { .. } => {
            rep.lose("mm.element.unsupported");
            None
        }
    }
}

fn shape_to_ac(s: &mm::Shape, rep: &mut BridgeReport) -> (ac::Shape, AcMatrix) {
    let used_fill = s.edges.iter().flat_map(|e| [e.fill0, e.fill1]).find(|&f| f > 0);
    let used_stroke = s.edges.iter().map(|e| e.stroke).find(|&f| f > 0);
    let solid = |p: &mm::Paint, rep: &mut BridgeReport| match p {
        mm::Paint::Solid(c) => Some(to_ac_color(*c)),
        _ => {
            rep.lose("mm.shape.paint");
            None
        }
    };
    let fill = used_fill.and_then(|i| s.fills.get(i as usize - 1)).and_then(|p| solid(p, rep));
    let mut stroke = None;
    let mut out_style = ac::StrokeStyle::default();
    if let Some(st) = used_stroke.and_then(|i| s.strokes.get(i as usize - 1)) {
        stroke = solid(&st.paint, rep).map(|color| ac::Stroke { color, width: st.width });
        out_style.cap = match st.cap {
            mm::Cap::None => ac::Cap::None,
            mm::Cap::Round => ac::Cap::Round,
            mm::Cap::Square => ac::Cap::Square,
        };
        out_style.join = match st.join {
            mm::Join::Miter => ac::Join::Miter,
            mm::Join::Round => ac::Join::Round,
            mm::Join::Bevel => ac::Join::Bevel,
        };
    }
    if s.fills.len() > 1 || s.strokes.len() > 1 {
        rep.lose("mm.shape.paint");
    }
    let mut segs = Vec::new();
    let mut end: Option<mm::Point> = None;
    for path in s.edges.iter().flat_map(|e| &e.paths) {
        if end != Some(path.start) {
            segs.push(AcSeg::M(q(path.start)));
        }
        for seg in &path.segs {
            match *seg {
                mm::Seg::Line(a) => segs.push(AcSeg::L(q(a))),
                mm::Seg::Quad(c, a) => segs.push(AcSeg::Q(q(c), q(a))),
            }
        }
        end = Some(path.segs.last().map_or(path.start, |s| s.end()));
    }
    if !s.extra.attrs.is_empty() || !s.extra.before.is_empty() || !s.extra.after.is_empty() {
        rep.lose("mm.extra");
    }
    (ac::Shape { path: AcPath { segs }, fill, stroke, stroke_style: out_style, ..Default::default() }, to_ac_matrix(&s.matrix))
}
