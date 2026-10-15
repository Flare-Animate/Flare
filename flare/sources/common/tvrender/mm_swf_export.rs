//! Exports a document as SWF, playable in Ruffle. Movie clips become sprites
//! with their own timelines. Graphic symbols and classic tweens are baked
//! frame by frame, as Animate's own SWF publish does. Shapes map one to one,
//! since SWF uses the same two-sided edge model as XFL.

use std::collections::{BTreeSet, HashMap};

use swf::{
    CharacterId, Color, Compression, Fixed8, Fixed16, FillStyle, Header, LineCapStyle, LineJoinStyle, LineStyle,
    PlaceObject, PlaceObjectAction, Point, PointDelta, Rectangle, RemoveObject, ShapeFlag, ShapeRecord, ShapeStyles,
    Sprite, StyleChangeData, Tag, Twips,
};
use swf::{GradientInterpolation, GradientRecord, GradientSpread};

use crate::Result;
use crate::display::{Item, Placed, display_list};
use crate::doc::{self, Cap, Document, Join, Paint, Seg, Shape, Symbol, SymbolKind, Text};
use crate::filters::{self, Effects};
use crate::text;

/// The SWF file, plus the features that couldn't be exported.
pub fn export(doc: &Document) -> Result<(Vec<u8>, BTreeSet<String>)> {
    let mut ex = Exporter { doc, defs: Vec::new(), ids: HashMap::new(), masks: HashMap::new(), skipped: BTreeSet::new() };
    let mut lists = Vec::new();
    for scene in &doc.scenes {
        for f in 0..scene.frame_count() {
            lists.push(display_list(doc, scene, f, &mut ex.skipped));
        }
    }
    let main = ex.timeline(&lists)?;

    let [r, g, b, _] = rgba(doc.background);
    let mut tags = vec![Tag::SetBackgroundColor(Color { r, g, b, a: 255 })];
    tags.extend(ex.defs);
    tags.extend(main);
    let header = Header {
        compression: Compression::Zlib,
        // 11 for PlaceObject3's visibility flag.
        version: 11,
        stage_size: Rectangle { x_min: Twips::ZERO, x_max: twips(doc.width), y_min: Twips::ZERO, y_max: twips(doc.height) },
        frame_rate: Fixed8::from_f32(doc.frame_rate),
        num_frames: lists.len().try_into().map_err(|_| "over 65535 frames")?,
    };
    let mut out = Vec::new();
    swf::write_swf(&header, &tags, &mut out)?;
    Ok((out, ex.skipped))
}

struct Exporter<'a> {
    doc: &'a Document,
    /// Character definitions, each after the ones it uses.
    defs: Vec<Tag<'static>>,
    /// Character ids by (address of the shape or symbol, exported as a button).
    ids: HashMap<(usize, bool), CharacterId>,
    /// Sprites holding a mask of several items, by what they place where (color doesn't mask).
    masks: HashMap<Vec<(CharacterId, swf::Matrix, Option<u16>)>, CharacterId>,
    skipped: BTreeSet<String>,
}

/// What sits at a depth: its character, matrix and color, and for a mask the
/// depth of the last thing it clips.
type Slot = (CharacterId, swf::Matrix, swf::ColorTransform, Option<u16>);

impl<'a> Exporter<'a> {
    /// Control tags that play `lists` (one display list per frame). Item `i`
    /// lives at depth `i + 1`; each frame only sends what changed.
    fn timeline(&mut self, lists: &[Vec<Placed<'a>>]) -> Result<Vec<Tag<'static>>> {
        let mut tags = Vec::new();
        let (mut prev, mut prev_fx): (Vec<Slot>, Vec<Effects>) = (Vec::new(), Vec::new());
        for list in lists {
            let (mut cur, mut fx) = (Vec::new(), Vec::new());
            self.slots(list, &mut cur, &mut fx)?;
            // ponytail: depth is list position, so a clip that shifts position (something
            // added below it) restarts. Stable per-layer depths fix that when it matters.
            for (i, &slot) in cur.iter().enumerate() {
                let depth = i as u16 + 1;
                // Filters, blend mode, caching and visibility go out (PlaceObject3) as they change.
                let fx_changed = prev_fx.get(i).map_or(fx[i] != Effects::default(), |p| *p != fx[i]);
                let action = match prev.get(i) {
                    Some(&old) if old == slot && !fx_changed => continue,
                    // A clip depth can't be taken off again, so a depth changing role starts afresh.
                    Some(&(.., clip)) if clip != slot.3 => {
                        tags.push(Tag::RemoveObject(RemoveObject { depth, character_id: None }));
                        PlaceObjectAction::Place(slot.0)
                    }
                    Some(&(old, ..)) if old == slot.0 => PlaceObjectAction::Modify,
                    Some(_) => PlaceObjectAction::Replace(slot.0),
                    None => PlaceObjectAction::Place(slot.0),
                };
                // A character new at its depth takes them afresh.
                let send_fx = fx_changed || (action != PlaceObjectAction::Modify && fx[i] != Effects::default());
                tags.push(place(depth, action, slot));
                if let (true, Some(Tag::PlaceObject(place))) = (send_fx, tags.last_mut()) {
                    filters::place(place, &fx[i]);
                }
            }
            for depth in cur.len()..prev.len() {
                tags.push(Tag::RemoveObject(RemoveObject { depth: depth as u16 + 1, character_id: None }));
            }
            tags.push(Tag::ShowFrame);
            (prev, prev_fx) = (cur, fx);
        }
        Ok(tags)
    }

    /// What `list` puts at each depth: a mask, then what it masks, clipped by it.
    fn slots(&mut self, list: &[Placed<'a>], out: &mut Vec<Slot>, fx: &mut Vec<Effects>) -> Result<()> {
        for p in list {
            match &p.item {
                Item::Masked { mask, content } => {
                    // An empty mask hides what it masks.
                    let Some((id, m)) = self.mask(mask)? else { continue };
                    let at = out.len();
                    out.push((id, m, color_transform(&doc::ColorTransform::IDENTITY), None));
                    fx.push(Effects::default());
                    self.slots(content, out, fx)?;
                    out[at].3 = Some(u16::try_from(out.len()).map_err(|_| "over 65535 depths")?);
                }
                _ => {
                    out.push((self.character(p)?, matrix(&p.matrix), color_transform(&p.color), None));
                    fx.push(p.effects.clone());
                }
            }
        }
        Ok(())
    }

    /// The character a mask clips with, and its matrix: a lone item as it is; several, as a
    /// sprite holding them, since a clip layer is one character.
    fn mask(&mut self, mask: &[Placed<'a>]) -> Result<Option<(CharacterId, swf::Matrix)>> {
        if let [p] = mask
            && !matches!(p.item, Item::Masked { .. })
        {
            return Ok(Some((self.character(p)?, matrix(&p.matrix))));
        }
        // ponytail: inside a mask, filters and blend modes don't count (a clip layer is its shape),
        // so its effects are dropped.
        let (mut slots, mut fx) = (Vec::new(), Vec::new());
        self.slots(mask, &mut slots, &mut fx)?;
        if slots.is_empty() {
            return Ok(None);
        }
        let key: Vec<_> = slots.iter().map(|&(id, m, _, clip)| (id, m, clip)).collect();
        let id = match self.masks.get(&key) {
            Some(&id) => id,
            None => {
                let id = self.new_id()?;
                let mut tags: Vec<_> = slots.iter().enumerate().map(|(i, &s)| place(i as u16 + 1, PlaceObjectAction::Place(s.0), s)).collect();
                tags.push(Tag::ShowFrame);
                self.defs.push(Tag::DefineSprite(Sprite { id, num_frames: 1, tags }));
                self.masks.insert(key, id);
                id
            }
        };
        Ok(Some((id, matrix(&doc::Matrix::IDENTITY))))
    }

    fn character(&mut self, p: &Placed<'a>) -> Result<CharacterId> {
        match p.item {
            Item::Shape(s) => self.shape(s),
            Item::Text(t) => self.text(t),
            Item::Clip(sym, kind) => self.sprite(sym, kind == SymbolKind::Button),
            Item::Masked { .. } => Err("a mask is placed by `slots`, not as a character".into()),
            // A shape tween's in-between: a shape of its own each frame, keyed past any address.
            Item::Morph(ref s) => {
                let (id, _) = self.id((usize::MAX - self.ids.len(), false))?;
                self.define_shape(id, s);
                Ok(id)
            }
        }
    }

    /// The id for `key`, and whether it still needs defining.
    fn id(&mut self, key: (usize, bool)) -> Result<(CharacterId, bool)> {
        if let Some(&id) = self.ids.get(&key) {
            return Ok((id, false));
        }
        let id = self.new_id()?;
        self.ids.insert(key, id);
        Ok((id, true))
    }

    /// The next character id.
    fn new_id(&self) -> Result<CharacterId> {
        Ok(CharacterId::try_from(self.ids.len() + self.masks.len() + 1).map_err(|_| "over 65535 shapes and symbols")?)
    }

    fn sprite(&mut self, sym: &'a Symbol, button: bool) -> Result<CharacterId> {
        let (id, new) = self.id((sym as *const Symbol as usize, button))?;
        if new {
            // ponytail: buttons export their Up frame only; DefineButton2 states and hit areas come later.
            let frames = if button { 1 } else { sym.timeline.frame_count().max(1) };
            let lists: Vec<_> = (0..frames).map(|f| display_list(self.doc, &sym.timeline, f, &mut self.skipped)).collect();
            let tags = self.timeline(&lists)?;
            let num_frames = frames.try_into().map_err(|_| "symbol over 65535 frames")?;
            self.defs.push(Tag::DefineSprite(Sprite { id, num_frames, tags }));
        }
        Ok(id)
    }

    fn shape(&mut self, s: &Shape) -> Result<CharacterId> {
        let (id, new) = self.id((s as *const Shape as usize, false))?;
        if new {
            self.define_shape(id, s);
        }
        Ok(id)
    }

    /// Text goes out as the shape of its glyphs.
    // ponytail: so it can't be selected, and dynamic and input text are fixed. DefineFont3 and
    // DefineEditText would keep them live.
    fn text(&mut self, t: &Text) -> Result<CharacterId> {
        let (id, new) = self.id((t as *const Text as usize, false))?;
        if new {
            self.define_shape(id, &text::to_shape(t));
        }
        Ok(id)
    }

    fn define_shape(&mut self, id: CharacterId, s: &Shape) {
        let fill_styles = s.fills.iter().map(|f| self.fill_style(f)).collect();
        let line_styles = s.strokes.iter().map(|st| {
            let cap = match st.cap {
                Cap::None => LineCapStyle::None,
                Cap::Round => LineCapStyle::Round,
                Cap::Square => LineCapStyle::Square,
            };
            LineStyle::new()
                .with_width(twips(st.width))
                .with_fill_style(self.fill_style(&st.paint))
                .with_start_cap(cap)
                .with_end_cap(cap)
                .with_join_style(match st.join {
                    Join::Miter => LineJoinStyle::Miter(Fixed8::from_f32(3.0)),
                    Join::Round => LineJoinStyle::Round,
                    Join::Bevel => LineJoinStyle::Bevel,
                })
        });
        let line_styles: Vec<_> = line_styles.collect();

        // Deltas run between rounded absolute positions, so rounding never drifts.
        let tw = |(x, y): doc::Point| (twips(x), twips(y));
        let delta = |a: (Twips, Twips), b: (Twips, Twips)| PointDelta::new(b.0 - a.0, b.1 - a.1);
        let mut records = Vec::new();
        let mut points = Vec::new();
        for e in &s.edges {
            for path in &e.paths {
                let mut pen = tw(path.start);
                records.push(ShapeRecord::StyleChange(Box::new(StyleChangeData {
                    move_to: Some(Point::new(pen.0, pen.1)),
                    fill_style_0: Some(e.fill0),
                    fill_style_1: Some(e.fill1),
                    line_style: Some(e.stroke),
                    new_styles: None,
                })));
                points.push(pen);
                for &seg in &path.segs {
                    let to = tw(seg.end());
                    records.push(match seg {
                        Seg::Line(_) => ShapeRecord::StraightEdge { delta: delta(pen, to) },
                        Seg::Quad(c, _) => {
                            let c = tw(c);
                            points.push(c);
                            ShapeRecord::CurvedEdge { control_delta: delta(pen, c), anchor_delta: delta(c, to) }
                        }
                    });
                    points.push(to);
                    pen = to;
                }
            }
        }

        // Control points included: a slightly loose box, never a clipped one.
        let edge_bounds = match points.first() {
            None => Rectangle::default(),
            Some(&(x, y)) => points.iter().fold(Rectangle { x_min: x, x_max: x, y_min: y, y_max: y }, |r, &(x, y)| {
                Rectangle { x_min: r.x_min.min(x), x_max: r.x_max.max(x), y_min: r.y_min.min(y), y_max: r.y_max.max(y) }
            }),
        };
        let pad = twips(s.strokes.iter().map(|st| st.width / 2.0).fold(0.0, f32::max));
        let shape_bounds = Rectangle {
            x_min: edge_bounds.x_min - pad,
            x_max: edge_bounds.x_max + pad,
            y_min: edge_bounds.y_min - pad,
            y_max: edge_bounds.y_max + pad,
        };
        let flags = if s.strokes.is_empty() { ShapeFlag::empty() } else { ShapeFlag::HAS_SCALING_STROKES };
        self.defs.push(Tag::DefineShape(Box::new(swf::Shape {
            version: 4,
            id,
            shape_bounds,
            edge_bounds,
            flags,
            styles: ShapeStyles { fill_styles, line_styles },
            shape: records,
        })));
    }

    fn fill_style(&mut self, p: &Paint) -> FillStyle {
        match p {
            Paint::Solid(c) => FillStyle::Color(color(*c)),
            Paint::Gradient(g) => gradient(g),
            // Keeps the style's slot, since edges refer to styles by index.
            Paint::Unsupported { name, .. } => {
                self.skipped.insert(name.clone());
                FillStyle::Color(Color { r: 0, g: 0, b: 0, a: 0 })
            }
        }
    }
}

fn color(c: doc::Rgba) -> Color {
    let [r, g, b, a] = rgba(c);
    Color { r, g, b, a }
}

/// A gradient fill. SWF's gradient square is XFL's in twips, so its matrix carries over.
// ponytail: SWF holds 15 stops, as many as Animate makes; a file with more loses the rest.
// Resample the ramp to 15 if hand-made files need them.
fn gradient(g: &doc::Gradient) -> FillStyle {
    let gradient = swf::Gradient {
        matrix: matrix(&g.matrix),
        spread: match g.spread {
            doc::Spread::Pad => GradientSpread::Pad,
            doc::Spread::Reflect => GradientSpread::Reflect,
            doc::Spread::Repeat => GradientSpread::Repeat,
        },
        interpolation: if g.linear_rgb { GradientInterpolation::LinearRgb } else { GradientInterpolation::Rgb },
        records: g.stops.iter().take(15).map(|&(t, c)| GradientRecord { ratio: (t.clamp(0.0, 1.0) * 255.0).round() as u8, color: color(c) }).collect(),
    };
    match g.kind {
        doc::GradientKind::Radial { focal } if focal != 0.0 => FillStyle::FocalGradient { gradient, focal_point: Fixed8::from_f32(focal) },
        doc::GradientKind::Radial { .. } => FillStyle::RadialGradient(gradient),
        doc::GradientKind::Linear => FillStyle::LinearGradient(gradient),
    }
}

/// PlaceObject2 putting `slot` at `depth`.
fn place(depth: u16, action: PlaceObjectAction, (_, matrix, color_transform, clip_depth): Slot) -> Tag<'static> {
    Tag::PlaceObject(Box::new(PlaceObject {
        version: 2,
        action,
        depth,
        matrix: Some(matrix),
        color_transform: Some(color_transform),
        ratio: None,
        name: None,
        clip_depth,
        class_name: None,
        filters: None,
        background_color: None,
        blend_mode: None,
        clip_actions: None,
        has_image: false,
        is_bitmap_cached: None,
        is_visible: None,
        amf_data: None,
    }))
}

fn twips(px: f32) -> Twips {
    Twips::new((px * 20.0).round() as i32)
}

fn rgba(c: doc::Rgba) -> [u8; 4] {
    c.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)
}

fn matrix(m: &doc::Matrix) -> swf::Matrix {
    swf::Matrix {
        a: Fixed16::from_f32(m.a),
        b: Fixed16::from_f32(m.b),
        c: Fixed16::from_f32(m.c),
        d: Fixed16::from_f32(m.d),
        tx: twips(m.tx),
        ty: twips(m.ty),
    }
}

fn color_transform(c: &doc::ColorTransform) -> swf::ColorTransform {
    let add = c.add.map(|v| (v * 255.0).round() as i16);
    swf::ColorTransform {
        r_multiply: Fixed8::from_f32(c.mult[0]),
        g_multiply: Fixed8::from_f32(c.mult[1]),
        b_multiply: Fixed8::from_f32(c.mult[2]),
        a_multiply: Fixed8::from_f32(c.mult[3]),
        r_add: add[0],
        g_add: add[1],
        b_add: add[2],
        a_add: add[3],
    }
}
