//! CPU reference renderer: draws one frame of a document into a pixmap.

use std::collections::BTreeSet;

use tiny_skia::{self as sk, FillRule, PathBuilder, Pixmap, Transform};

use crate::Result;
use crate::button::Live;
use crate::display::{Item, Placed, display_list, display_list_shown, layer_items};
use crate::doc::*;
use crate::filters::{self, Effects};
use crate::text;

/// Renders `frame` (0-based, counted across scenes) at `scale`. Also returns
/// the features it skipped, so gaps against Animate's own output are visible.
pub fn render(doc: &Document, frame: u32, scale: f32) -> Result<(Pixmap, BTreeSet<String>)> {
    render_with(doc, frame, scale, None, None)
}

/// `render` of a movie playing in `player`: its frame, each movie clip at the frame it has
/// reached rather than its first, and the buttons under `live`'s pointer hovered or pressed.
/// Test Movie draws with it.
pub fn render_playing(doc: &Document, player: &crate::player::Player, scale: f32, live: Option<Live>) -> Result<(Pixmap, BTreeSet<String>)> {
    render_with(doc, player.root.frame, scale, Some(crate::player::ClipFrames::new(player)), live)
}

/// `render` as a player shows it, the buttons under `live`'s pointer hovered or pressed (button.rs).
pub fn render_live(doc: &Document, frame: u32, scale: f32, live: Option<Live>) -> Result<(Pixmap, BTreeSet<String>)> {
    render_with(doc, frame, scale, None, live)
}

fn render_with<'a>(doc: &'a Document, frame: u32, scale: f32, clips: Option<crate::player::ClipFrames<'a>>, live: Option<Live>) -> Result<(Pixmap, BTreeSet<String>)> {
    let (mut scene, mut local) = (None, frame);
    for s in &doc.scenes {
        if local < s.frame_count() {
            scene = Some(s);
            break;
        }
        local -= s.frame_count();
    }
    let scene = scene.ok_or_else(|| format!("frame out of range: the document has {} frames", frame - local))?;

    let size = |v: f32| (v * scale).ceil() as u32;
    let mut pixmap = Pixmap::new(size(doc.width), size(doc.height)).ok_or("bad stage size")?;
    pixmap.fill(color(doc.background));
    let mut r = Renderer { doc, pixmap, skipped: BTreeSet::new(), shown_only: false, only: None, masking: false, zoom: scale, layers: Vec::new(), under: None, clips, live };
    r.timeline(scene, local, Transform::from_scale(scale, scale), ColorTransform::IDENTITY, 0);
    Ok((r.pixmap, r.skipped))
}

/// Renders `frame` of any timeline of `doc` (a scene's or a symbol's), placed
/// by `matrix`, onto a transparent `width` × `height` canvas at `scale`. The editor
/// draws symbols being edited in place, and Library previews, with it.
pub fn render_timeline(
    doc: &Document,
    tl: &Timeline,
    frame: u32,
    matrix: &Matrix,
    (width, height): (f32, f32),
    scale: f32,
) -> Result<(Pixmap, BTreeSet<String>)> {
    render_timeline_live(doc, tl, frame, matrix, (width, height), scale, None)
}

/// `render_timeline` with the buttons under `live`'s pointer hovered or pressed: Control > Enable
/// Simple Buttons on the stage.
pub fn render_timeline_live(
    doc: &Document,
    tl: &Timeline,
    frame: u32,
    matrix: &Matrix,
    (width, height): (f32, f32),
    scale: f32,
    live: Option<Live>,
) -> Result<(Pixmap, BTreeSet<String>)> {
    let size = |v: f32| (v * scale).ceil() as u32;
    let pixmap = Pixmap::new(size(width), size(height)).ok_or("bad canvas size")?;
    let zoom = scale * (matrix.a * matrix.d - matrix.b * matrix.c).abs().sqrt();
    let ts = Transform::from_scale(scale, scale).pre_concat(transform(matrix));
    // A scene's canvas shows over the stage, whose colour its blend modes mix with.
    let is_scene = doc.scenes.iter().any(|s| std::ptr::eq(s, tl));
    let stage = sk::Rect::from_xywh(0.0, 0.0, doc.width, doc.height).and_then(|r| r.transform(ts)).filter(|_| is_scene);
    // The editor draws with this, so the timeline's hidden layers stay hidden.
    let mut r = Renderer {
        doc,
        pixmap,
        skipped: BTreeSet::new(),
        shown_only: true,
        only: None,
        masking: false,
        zoom,
        layers: Vec::new(),
        under: stage.map(|stage| (doc.background, stage)),
        clips: None,
        live,
    };
    r.timeline(tl, frame, ts, ColorTransform::IDENTITY, 0);
    Ok((r.pixmap, r.skipped))
}

/// `render_timeline` drawing layer `layer` of `tl` alone, its links to other layers still at
/// work: its mask, guide and parent. The editor's onion skin draws the stage layer by layer.
pub fn render_layer(
    doc: &Document,
    tl: &Timeline,
    frame: u32,
    layer: usize,
    matrix: &Matrix,
    (width, height): (f32, f32),
    scale: f32,
) -> Result<(Pixmap, BTreeSet<String>)> {
    let size = |v: f32| (v * scale).ceil() as u32;
    let pixmap = Pixmap::new(size(width), size(height)).ok_or("bad canvas size")?;
    let zoom = scale * (matrix.a * matrix.d - matrix.b * matrix.c).abs().sqrt();
    // ponytail: no stage colour under blends here: a layer's canvas goes over the layers below
    // it, which the colour would hide, so its blend modes mix with transparency.
    let mut r = Renderer { doc, pixmap, skipped: BTreeSet::new(), shown_only: true, only: Some(layer), masking: false, zoom, layers: Vec::new(), under: None, clips: None, live: None };
    r.timeline(tl, frame, Transform::from_scale(scale, scale).pre_concat(transform(matrix)), ColorTransform::IDENTITY, 0);
    Ok((r.pixmap, r.skipped))
}

struct Renderer<'a> {
    doc: &'a Document,
    pixmap: Pixmap,
    skipped: BTreeSet<String>,
    /// Leave out the top timeline's hidden layers.
    shown_only: bool,
    /// Draw only this layer of the top timeline.
    only: Option<usize>,
    /// Drawing a mask, where only fills count: Animate ignores a mask's strokes.
    masking: bool,
    /// The view's scale, which filters scale by (an object's own scale doesn't change them).
    zoom: f32,
    /// What `pixmap` replaced while it holds a filtered or blended object, each with its margin.
    layers: Vec<(Pixmap, u32)>,
    /// The stage's colour and where it is, when the canvas is shown over it later.
    under: Option<(Rgba, sk::Rect)>,
    /// Test Movie's clips, each at its own frame; without, each shows its first, as on the stage.
    clips: Option<crate::player::ClipFrames<'a>>,
    /// A player's pointer, which buttons respond to.
    live: Option<Live>,
}

impl<'a> Renderer<'a> {
    fn timeline(&mut self, tl: &'a Timeline, frame: u32, ts: Transform, cx: ColorTransform, depth: u32) {
        if depth > 32 {
            self.skipped.insert("clips nested over 32 deep".into());
            return;
        }
        let list = if self.shown_only && depth == 0 { display_list_shown } else { display_list };
        let list = list(self.doc, tl, frame, &mut self.skipped);
        let list = match self.only {
            Some(l) if depth == 0 => layer_items(list, l),
            _ => list,
        };
        self.items(list, ts, cx, depth);
    }

    fn items(&mut self, list: Vec<Placed<'a>>, ts: Transform, cx: ColorTransform, depth: u32) {
        for p in list {
            let (ts, cx) = (ts.pre_concat(transform(&p.matrix)), cx * p.color);
            // Visible unchecked: published, not drawn. A clip still plays, so Test Movie's count it.
            if p.effects.hidden {
                if let (Item::Clip(..), Some(c)) = (&p.item, &mut self.clips) {
                    c.enter(p.source);
                    c.leave();
                }
                continue;
            }
            // Filters and blend modes draw the object in a layer of its own, coloured as a whole.
            let layer = if p.effects.plain() { None } else { self.begin_layer(&p.effects).map(|margin| (margin, cx)) };
            let (ts, cx) = match layer {
                Some((margin, _)) => (ts.post_translate(margin, margin), ColorTransform::IDENTITY),
                None => (ts, cx),
            };
            match p.item {
                Item::Shape(s) => self.shape(s, ts, cx),
                Item::Text(t) => self.shape(&text::to_shape(t), ts, cx),
                Item::Clip(sym, kind) => {
                    let mut frame = self.clips.as_mut().map_or(0, |c| c.enter(p.source));
                    // A button shows Up, Over or Down, as the pointer has it.
                    if kind == SymbolKind::Button {
                        frame = crate::button::shown_frame(self.doc, sym, kind, ts, self.live);
                    }
                    self.timeline(&sym.timeline, frame, ts, cx, depth + 1);
                    if let Some(c) = &mut self.clips {
                        c.leave();
                    }
                }
                Item::Morph(s) => self.shape(&s, ts, cx),
                Item::Masked { mask, content } => self.masked(mask, content, ts, cx, depth),
                Item::Bitmap(b) => {
                    if !crate::bitmap::draw(&mut self.pixmap, b, ts, cx) {
                        self.skipped.insert(format!("bitmap {:?}, whose image can't be read", b.name));
                    }
                }
            }
            if let Some((_, cx)) = layer {
                self.end_layer(&p.effects, cx);
            }
        }
    }

    /// Starts drawing into a fresh layer, bigger than the current one all round by a margin
    /// as wide as `fx`'s filters reach, so what's just off the canvas still casts into it.
    /// Returns the margin.
    fn begin_layer(&mut self, fx: &Effects) -> Option<f32> {
        let (rx, ry) = filters::reach(&fx.filters, self.zoom);
        // ponytail: every layer is canvas-sized (only the part holding something is filtered), and
        // reach is cut at 512 px. Size layers to the object's bounds if many filtered objects slow
        // the stage down.
        let margin = rx.max(ry).ceil().min(512.0) as u32;
        let layer = Pixmap::new(self.pixmap.width() + 2 * margin, self.pixmap.height() + 2 * margin)?;
        self.layers.push((std::mem::replace(&mut self.pixmap, layer), margin));
        Some(margin as f32)
    }

    /// Ends `begin_layer`'s layer: filters it, colours it by `cx` and blends it into the one below.
    fn end_layer(&mut self, fx: &Effects, cx: ColorTransform) {
        let Some((below, margin)) = self.layers.pop() else { return };
        let layer = std::mem::replace(&mut self.pixmap, below);
        let Some((mut img, x, y)) = filters::Image::content(&layer, filters::reach(&fx.filters, self.zoom)) else { return };
        filters::apply(&mut img, &fx.filters, self.zoom);
        img.color(cx);
        if let Some(pm) = img.to_pixmap() {
            let (x, y, layered) = (x - margin as i32, y - margin as i32, !self.layers.is_empty());
            if let Some(under) = self.under.filter(|_| !layered) {
                filters::put_under(&mut self.pixmap, &pm, x, y, fx.blend, under);
            }
            filters::composite(&mut self.pixmap, &pm, x, y, fx.blend, layered);
        }
    }

    /// `content` drawn on a layer of its own, kept where `mask` covers.
    // ponytail: two canvas-sized pixmaps per group; clip them to the mask's box if big canvases get slow.
    fn masked(&mut self, mask: Vec<Placed<'a>>, content: Vec<Placed<'a>>, ts: Transform, cx: ColorTransform, depth: u32) {
        let (w, h) = (self.pixmap.width(), self.pixmap.height());
        let (Some(coverage), Some(layer)) = (Pixmap::new(w, h), Pixmap::new(w, h)) else { return };
        let under = std::mem::replace(&mut self.pixmap, coverage);
        // A mask's filled shapes count, not their color or alpha: all of it draws opaque.
        let masking = std::mem::replace(&mut self.masking, true);
        self.items(mask, ts, ColorTransform { mult: [0.0; 4], add: [0.0, 0.0, 0.0, 1.0] }, depth);
        self.masking = masking;
        let coverage = std::mem::replace(&mut self.pixmap, layer);
        self.items(content, ts, cx, depth);
        let mut layer = std::mem::replace(&mut self.pixmap, under);
        layer.apply_mask(&sk::Mask::from_pixmap(coverage.as_ref(), sk::MaskType::Alpha));
        self.pixmap.draw_pixmap(0, 0, layer.as_ref(), &sk::PixmapPaint::default(), Transform::identity(), None);
    }

    fn shape(&mut self, s: &Shape, ts: Transform, cx: ColorTransform) {
        // Each fill's edges, gathered once: a traced bitmap (trace.rs) has thousands of fills.
        let mut edges: Vec<Vec<&Edge>> = vec![Vec::new(); s.fills.len() + 1];
        for e in &s.edges {
            edges.get_mut(e.fill0 as usize).into_iter().for_each(|v| v.push(e));
            if e.fill1 != e.fill0 {
                edges.get_mut(e.fill1 as usize).into_iter().for_each(|v| v.push(e));
            }
        }
        for (i, fill) in s.fills.iter().enumerate() {
            let image = crate::bitmap::fill_pixels(self.doc, fill, cx);
            let (Some(paint), Some(path)) = (self.paint(fill, cx, image.as_deref()), fill_path(edges[i + 1].iter().copied(), i as u32 + 1)) else {
                continue;
            };
            self.pixmap.fill_path(&path, &paint, FillRule::EvenOdd, ts, None);
        }
        if self.masking {
            return;
        }
        for e in &s.edges {
            let Some(style) = (e.stroke as usize).checked_sub(1).and_then(|i| s.strokes.get(i)) else { continue };
            let image = crate::bitmap::fill_pixels(self.doc, &style.paint, cx);
            let Some(paint) = self.paint(&style.paint, cx, image.as_deref()) else { continue };
            let mut pb = PathBuilder::new();
            for p in &e.paths {
                pb.move_to(p.start.0, p.start.1);
                p.segs.iter().for_each(|&seg| push(&mut pb, seg));
            }
            let Some(path) = pb.finish() else { continue };
            let stroke = sk::Stroke {
                width: style.width,
                miter_limit: 3.0,
                line_cap: match style.cap {
                    Cap::None => sk::LineCap::Butt,
                    Cap::Round => sk::LineCap::Round,
                    Cap::Square => sk::LineCap::Square,
                },
                line_join: match style.join {
                    Join::Miter => sk::LineJoin::Miter,
                    Join::Round => sk::LineJoin::Round,
                    Join::Bevel => sk::LineJoin::Bevel,
                },
                ..Default::default()
            };
            self.pixmap.stroke_path(&path, &paint, &stroke, ts, None);
        }
    }

    /// The paint drawing `p`, its colours through `cx`; a bitmap fill's `image` is its pixels (`bitmap::fill_pixels`).
    fn paint<'p>(&mut self, p: &Paint, cx: ColorTransform, image: Option<&'p Pixmap>) -> Option<sk::Paint<'p>> {
        match p {
            Paint::Solid(c) => {
                let mut paint = sk::Paint::default();
                paint.set_color(color(cx.apply(*c)));
                Some(paint)
            }
            Paint::Gradient(g) => crate::gradient::sk_paint(g, cx),
            Paint::Bitmap(f) => {
                let paint = image.zip(self.doc.bitmap(&f.name)).map(|(px, b)| crate::bitmap::sk_paint(f, b.smoothing, px));
                if paint.is_none() {
                    self.skipped.insert(format!("bitmap fill {:?}, whose bitmap is missing or can't be read", f.name));
                }
                paint
            }
            Paint::Unsupported { name, .. } => {
                self.skipped.insert(name.clone());
                None
            }
        }
    }
}

/// The outline of fill `id`: every edge with that fill on either side, chained
/// end to start into closed loops. Even-odd filling then ignores loop direction.
pub(crate) fn fill_path<'e>(edges: impl IntoIterator<Item = &'e Edge>, id: u32) -> Option<sk::Path> {
    let mut pieces = Vec::new();
    for e in edges.into_iter().filter(|e| e.fill0 == id || e.fill1 == id) {
        for p in &e.paths {
            let mut from = p.start;
            for &seg in &p.segs {
                // fill1 lies right of the edge's direction, fill0 left: flip fill0
                // pieces so the pieces around one region all run the same way.
                if e.fill1 == id {
                    pieces.push((from, seg));
                }
                if e.fill0 == id {
                    pieces.push(match seg {
                        Seg::Line(to) => (to, Seg::Line(from)),
                        Seg::Quad(c, to) => (to, Seg::Quad(c, from)),
                    });
                }
                from = seg.end();
            }
        }
    }
    let near = |a: Point, b: Point| (a.0 - b.0).abs() < 1e-3 && (a.1 - b.1).abs() < 1e-3;
    let mut pb = PathBuilder::new();
    // ponytail: O(n²) endpoint search. It runs from the end, where the loop being chained sits
    // when edges come loop by loop (as glyph outlines do), so text costs per glyph, not per
    // text. Index endpoints in a HashMap if dense brush shapes get slow.
    while let Some((start, seg)) = pieces.pop() {
        pb.move_to(start.0, start.1);
        push(&mut pb, seg);
        let mut end = seg.end();
        while let Some(i) = pieces.iter().rposition(|(from, _)| near(*from, end)) {
            let (_, seg) = pieces.swap_remove(i);
            push(&mut pb, seg);
            end = seg.end();
        }
        pb.close();
    }
    pb.finish()
}

fn push(pb: &mut PathBuilder, seg: Seg) {
    match seg {
        Seg::Line((x, y)) => pb.line_to(x, y),
        Seg::Quad((cx, cy), (x, y)) => pb.quad_to(cx, cy, x, y),
    }
}

fn transform(m: &Matrix) -> Transform {
    Transform::from_row(m.a, m.b, m.c, m.d, m.tx, m.ty)
}

fn color([r, g, b, a]: Rgba) -> sk::Color {
    sk::Color::from_rgba(r, g, b, a).unwrap_or(sk::Color::BLACK)
}
