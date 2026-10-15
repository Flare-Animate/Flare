//! Document model mirroring Animate's DOM: Document → Timeline → Layer →
//! Keyframe (DOMFrame) → Element (shape or symbol instance), plus a Library
//! of symbols that each own a timeline.

use crate::deform::{project_path, Perspective, Transform3D, Warp};
use crate::geom::{morph_paths, pt, Matrix, Path, Pt};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
pub use crate::bitmap::{BitmapAsset, BitmapId, Image};
pub use crate::fx::{BevelType, BlendMode, EffectValue, Filter, FxGroup};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Color { r, g, b, a: 255 }
    }
    pub fn from_hex(s: &str) -> Option<Color> {
        let s = s.trim_start_matches('#');
        let v = u32::from_str_radix(s, 16).ok()?;
        match s.len() {
            6 => Some(Color::rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)),
            8 => Some(Color { r: (v >> 24) as u8, g: (v >> 16) as u8, b: (v >> 8) as u8, a: v as u8 }),
            _ => None,
        }
    }
    pub fn to_hex(self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }
    pub fn lerp(self, o: Color, t: f32) -> Color {
        let l = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
        Color { r: l(self.r, o.r), g: l(self.g, o.g), b: l(self.b, o.b), a: l(self.a, o.a) }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stroke {
    pub color: Color,
    pub width: f32,
}

/// A vector shape. Demo uses Animate's *Object Drawing* model (each shape is its
/// own object). *Merge Drawing* (shapes that cut/merge, Flash's edge + fill-style
/// planar map) is the big engine milestone — see CLAUDE.md.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Shape {
    pub path: Path,
    pub fill: Option<Color>,
    pub stroke: Option<Stroke>,
    /// Gradient fill; overrides `fill` (which stays as the fallback colour).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gradient: Option<Gradient>,
    /// Variable stroke width (Width tool / Fluid Brush): (t along path 0..1, width).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width_profile: Option<Vec<(f32, f32)>>,
    /// Asset Warp pins (non-destructive: `path` is the warped result).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warp: Option<Warp>,
    /// Rectangle / Oval Primitive: parameters stay editable; `path` follows them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primitive: Option<Primitive>,
    /// Stroke style: dashes, caps and joins.
    #[serde(default, skip_serializing_if = "StrokeStyle::is_default")]
    pub stroke_style: StrokeStyle,
    /// Bitmap fill (a library bitmap, tiled); overrides fill and gradient.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bitmap_fill: Option<BitmapFill>,
    /// Shape armature (bones inside this shape, bound with the Bind tool).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rig: Option<crate::rig::ShapeRig>,
}

/// Rectangle / Oval Primitive parameters (in the shape's own coordinates).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Primitive {
    /// Corner radii: top-left, top-right, bottom-right, bottom-left (negative = inset).
    Rect { x: f32, y: f32, w: f32, h: f32, radius: [f32; 4] },
    /// Angles in degrees (start = end: full oval); inner radius in % (0 = solid).
    Oval { cx: f32, cy: f32, rx: f32, ry: f32, start: f32, end: f32, inner: f32, close: bool },
}

impl Primitive {
    pub fn path(&self) -> Path {
        match *self {
            Primitive::Rect { x, y, w, h, radius } => Path::rounded_rect4(x, y, w, h, radius),
            Primitive::Oval { cx, cy, rx, ry, start, end, inner, close } => Path::oval_arc(cx, cy, rx, ry, start, end, inner, close),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Dash {
    #[default]
    Solid,
    Dashed,
    Dotted,
    /// Wobbly, hand-drawn line.
    Ragged,
    /// Scattered dots across the stroke's width.
    Stipple,
    /// Short strokes across the line.
    Hatched,
    // ---- OpenToonz stroke styles (generated in `strokes.rs`) ----
    /// Slanted hairs on alternating sides (OpenToonz "Herringbone").
    Herringbone,
    /// Linked rings.
    Chain,
    /// Scattered little circles (OpenToonz "Circlets").
    Circlets,
    /// Pen strokes across the line (OpenToonz "Dashes").
    Scratchy,
    /// Twisted strands with an outline.
    Rope,
    /// Translucent creased band (OpenToonz "Tulle").
    Tulle,
    /// Three coloured stripes woven together (OpenToonz "Plait").
    Plait,
    /// Many short sketchy strands (OpenToonz "Fuzz").
    Fuzz,
    /// Circles along the line in two colours.
    Bubbles,
    /// Saw-tooth triangles (OpenToonz "Jagged").
    Jagged,
    /// Random zigzag.
    Zigzag,
    /// Sine-wave ribbon.
    Wave,
    /// Curling ribbon (OpenToonz "Curl").
    Curl,
}

impl Dash {
    pub const ALL: [Dash; 19] = [
        Dash::Solid,
        Dash::Dashed,
        Dash::Dotted,
        Dash::Ragged,
        Dash::Stipple,
        Dash::Hatched,
        Dash::Herringbone,
        Dash::Chain,
        Dash::Circlets,
        Dash::Scratchy,
        Dash::Rope,
        Dash::Tulle,
        Dash::Plait,
        Dash::Fuzz,
        Dash::Bubbles,
        Dash::Jagged,
        Dash::Zigzag,
        Dash::Wave,
        Dash::Curl,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Dash::Solid => "Solid",
            Dash::Dashed => "Dashed",
            Dash::Dotted => "Dotted",
            Dash::Ragged => "Ragged",
            Dash::Stipple => "Stipple",
            Dash::Hatched => "Hatched",
            Dash::Herringbone => "Herringbone",
            Dash::Chain => "Chain",
            Dash::Circlets => "Circlets",
            Dash::Scratchy => "Scratchy",
            Dash::Rope => "Rope",
            Dash::Tulle => "Tulle",
            Dash::Plait => "Plait",
            Dash::Fuzz => "Fuzz",
            Dash::Bubbles => "Bubbles",
            Dash::Jagged => "Jagged",
            Dash::Zigzag => "Zigzag",
            Dash::Wave => "Wave",
            Dash::Curl => "Curl",
        }
    }
    /// Styles .fla files can store (the rest come from OpenToonz).
    pub fn in_fla(self) -> bool {
        matches!(self, Dash::Solid | Dash::Dashed | Dash::Dotted | Dash::Ragged | Dash::Stipple | Dash::Hatched)
    }
    /// Script name (`selection.set_stroke_style`).
    pub fn id(self) -> String {
        self.name().to_ascii_lowercase()
    }
    pub fn from_id(s: &str) -> Option<Dash> {
        Dash::ALL.into_iter().find(|d| d.id() == s.to_ascii_lowercase())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Cap {
    None,
    #[default]
    Round,
    Square,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Join {
    Miter,
    #[default]
    Round,
    Bevel,
}

/// Properties ▸ Fill and stroke ▸ Style / Cap / Join.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct StrokeStyle {
    pub dash: Dash,
    pub cap: Cap,
    pub join: Join,
    /// Dash length and gap, in stroke widths (Dashed).
    #[serde(default = "default_dash")]
    pub dash_len: f32,
    #[serde(default = "default_dash")]
    pub gap: f32,
    /// Parameters of the OpenToonz styles (`None` = the style's default; see
    /// `strokes::params`).
    #[serde(default, skip_serializing_if = "ArtParams::is_default")]
    pub art: ArtParams,
}

/// Parameters and extra colours of a generated stroke style.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ArtParams {
    #[serde(default)]
    pub p: [Option<f32>; 5],
    /// Second/third colours (Rope outline, Plait stripes, Bubbles); `None`
    /// derives one from the stroke colour.
    #[serde(default)]
    pub colors: [Option<Color>; 2],
}

impl ArtParams {
    pub fn is_default(&self) -> bool {
        *self == ArtParams::default()
    }
}
fn default_dash() -> f32 {
    3.0
}

impl Default for StrokeStyle {
    fn default() -> Self {
        StrokeStyle { dash: Dash::Solid, cap: Cap::Round, join: Join::Round, dash_len: 3.0, gap: 3.0, art: ArtParams::default() }
    }
}

impl StrokeStyle {
    pub fn is_default(&self) -> bool {
        self.dash == Dash::Solid && self.cap == Cap::Round && self.join == Join::Round
    }
}

/// Fill a shape with a library bitmap (tiled, in the shape's coordinates).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct BitmapFill {
    pub bitmap: crate::bitmap::BitmapId,
    pub matrix: Matrix,
}

impl Shape {
    pub fn new(path: Path, fill: Option<Color>, stroke: Option<Stroke>) -> Shape {
        Shape { path, fill, stroke, ..Default::default() }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GradientKind {
    Linear,
    Radial,
}

/// Gradient fill. `matrix` maps gradient space (linear: x from -1 to 1;
/// radial: unit circle) into the shape's local coordinates, like Flash's
/// gradient matrix. The Gradient Transform tool edits it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Gradient {
    pub kind: GradientKind,
    pub stops: Vec<(f32, Color)>,
    pub matrix: Matrix,
    /// Color panel ▸ Flow: what happens past the ends.
    #[serde(default, skip_serializing_if = "Spread::is_pad")]
    pub spread: Spread,
    /// Radial gradients: the focal point along the gradient's x axis, −1…1 of
    /// the radius (Animate's focal point; 0 = centred).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub focal: f32,
}

fn is_zero(v: &f32) -> bool {
    *v == 0.0
}

/// Gradient overflow (Animate's Extend / Reflect / Repeat).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Spread {
    /// Extend the end colours.
    #[default]
    Pad,
    Reflect,
    Repeat,
}

impl Spread {
    pub const ALL: [Spread; 3] = [Spread::Pad, Spread::Reflect, Spread::Repeat];
    fn is_pad(&self) -> bool {
        *self == Spread::Pad
    }
    pub fn name(self) -> &'static str {
        match self {
            Spread::Pad => "Extend",
            Spread::Reflect => "Reflect",
            Spread::Repeat => "Repeat",
        }
    }
}

impl Gradient {
    /// A gradient fitted to a box (Animate's default placement).
    pub fn fit(kind: GradientKind, stops: Vec<(f32, Color)>, lo: Pt, hi: Pt) -> Gradient {
        let c = pt((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0);
        let (hw, hh) = ((hi.x - lo.x) / 2.0, (hi.y - lo.y) / 2.0);
        let s = match kind {
            GradientKind::Linear => Matrix::scale(hw.max(1.0), hw.max(1.0)),
            GradientKind::Radial => {
                let r = (hw * hw + hh * hh).sqrt().max(1.0);
                Matrix::scale(r, r)
            }
        };
        Gradient { kind, stops, matrix: Matrix::translate(c.x, c.y).concat(&s), spread: Spread::Pad, focal: 0.0 }
    }
    pub fn lerp(&self, o: &Gradient, t: f32) -> Gradient {
        if self.kind != o.kind || self.stops.len() != o.stops.len() {
            return if t < 0.5 { self.clone() } else { o.clone() };
        }
        Gradient {
            kind: self.kind,
            stops: self.stops.iter().zip(&o.stops).map(|(a, b)| (a.0 + (b.0 - a.0) * t, a.1.lerp(b.1, t))).collect(),
            matrix: self.matrix.interpolate(&o.matrix, t, 0),
            spread: self.spread,
            focal: self.focal + (o.focal - self.focal) * t,
        }
    }
}

/// A colour swatch, or a gradient one (its matrix is not used: the gradient
/// is fitted to whatever it fills).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Swatch {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    pub color: Color,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gradient: Option<Gradient>,
}

impl Swatch {
    pub fn solid(name: impl Into<String>, color: Color) -> Swatch {
        Swatch { name: name.into(), color, gradient: None }
    }
    pub fn gradient(name: impl Into<String>, kind: GradientKind, stops: Vec<(f32, Color)>) -> Swatch {
        let color = stops.first().map(|s| s.1).unwrap_or(Color::rgb(0, 0, 0));
        Swatch { name: name.into(), color, gradient: Some(Gradient { kind, stops, matrix: Matrix::IDENTITY, spread: Spread::Pad, focal: 0.0 }) }
    }
}

pub type SymbolId = u32;

/// A library item's folder ("" = the library root) and its own name.
pub fn split_item_path(name: &str) -> (&str, &str) {
    match name.rfind('/') {
        Some(i) => (&name[..i], &name[i + 1..]),
        None => ("", name),
    }
}

/// `folder/leaf`, or `leaf` at the root.
pub fn join_item_path(folder: &str, leaf: &str) -> String {
    if folder.is_empty() { leaf.to_string() } else { format!("{folder}/{leaf}") }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SymbolKind {
    /// Timeline is synced to the parent timeline (Animate "Graphic").
    Graphic,
    /// Independent timeline (Animate "Movie Clip").
    MovieClip,
    Button,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoopMode {
    Loop,
    PlayOnce,
    SingleFrame,
    /// Play backwards from the first frame, then hold frame 1.
    PlayReverseOnce,
    /// Play backwards from the first frame, wrapping to the last.
    LoopReverse,
}

impl LoopMode {
    pub const ALL: [LoopMode; 5] = [LoopMode::Loop, LoopMode::PlayOnce, LoopMode::SingleFrame, LoopMode::PlayReverseOnce, LoopMode::LoopReverse];
    pub fn name(self) -> &'static str {
        match self {
            LoopMode::Loop => "Loop",
            LoopMode::PlayOnce => "Play Once",
            LoopMode::SingleFrame => "Single Frame",
            LoopMode::PlayReverseOnce => "Play Reverse Once",
            LoopMode::LoopReverse => "Loop Reverse",
        }
    }
    /// Lower-case script name ("loop", "play_once", "single_frame", "play_reverse_once", "loop_reverse").
    pub fn id(self) -> &'static str {
        match self {
            LoopMode::Loop => "loop",
            LoopMode::PlayOnce => "play_once",
            LoopMode::SingleFrame => "single_frame",
            LoopMode::PlayReverseOnce => "play_reverse_once",
            LoopMode::LoopReverse => "loop_reverse",
        }
    }
    pub fn from_id(s: &str) -> Option<LoopMode> {
        let k = s.to_ascii_lowercase().replace([' ', '-'], "_");
        LoopMode::ALL.into_iter().find(|m| m.id() == k)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Instance {
    pub symbol: SymbolId,
    pub loop_mode: LoopMode,
    pub first_frame: u32,
}

impl Instance {
    /// The symbol frame shown `local` frames after the instance's keyframe
    /// (graphics follow the parent; movie clips run on the global clock).
    pub fn symbol_frame(&self, kind: SymbolKind, local: u32, global: u32, dur: u32) -> u32 {
        let dur = dur.max(1);
        match kind {
            SymbolKind::MovieClip => global % dur,
            SymbolKind::Button => 0,
            SymbolKind::Graphic => {
                // widened so huge imported First Frame values wrap exactly (Flare fix)
                let raw = u64::from(self.first_frame) + u64::from(local);
                match self.loop_mode {
                    LoopMode::Loop => (raw % u64::from(dur)) as u32,
                    LoopMode::PlayOnce => raw.min(u64::from(dur - 1)) as u32,
                    LoopMode::SingleFrame => self.first_frame.min(dur - 1),
                    LoopMode::PlayReverseOnce => self.first_frame.min(dur - 1).saturating_sub(local),
                    LoopMode::LoopReverse => (self.first_frame.min(dur - 1) as i64 - local as i64).rem_euclid(dur as i64) as u32,
                }
            }
        }
    }
}

/// Static text (Animate's Text tool, "Static Text" mode).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextEl {
    pub text: String,
    pub size: f32,
    pub color: Color,
    #[serde(default)]
    pub font: crate::text::FontFace,
    #[serde(default)]
    pub align: crate::text::Align,
    #[serde(default)]
    pub letter_spacing: f32,
    /// System font family (falls back to `font` if it isn't installed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub family: Option<String>,
    /// Font weight 100–900 (400 regular, 700 bold); faked when the family lacks it.
    #[serde(default = "w400", skip_serializing_if = "is_w400")]
    pub weight: u16,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub italic: bool,
    /// Paragraph and character options (line spacing, indent, margins, fixed width, baseline shift, kerning).
    #[serde(default, skip_serializing_if = "crate::text::Paragraph::is_default")]
    pub para: crate::text::Paragraph,
    /// Static / Dynamic / Input and the field options.
    #[serde(default, skip_serializing_if = "crate::text::TextField::is_default")]
    pub field: crate::text::TextField,
}

fn w400() -> u16 {
    400
}
fn is_w400(w: &u16) -> bool {
    *w == 400
}

impl TextEl {
    /// What the text shows: bullets for password input fields.
    pub fn shown(&self) -> std::borrow::Cow<'_, str> {
        if self.field.password && self.field.kind == crate::text::TextKind::Input {
            std::borrow::Cow::Owned(self.text.chars().map(|c| if c == '\n' { c } else { '•' }).collect())
        } else {
            std::borrow::Cow::Borrowed(&self.text)
        }
    }
    pub fn outline(&self) -> Path {
        let text = self.shown();
        if self.weight == 400 && !self.italic {
            return crate::text::text_path_para(&text, self.size, self.family.as_deref(), self.font, self.align, self.letter_spacing, &self.para);
        }
        crate::text::text_path_styled_para(&text, self.size, self.family.as_deref(), self.font, self.weight, self.italic, self.align, self.letter_spacing, &self.para)
    }
    /// The text box (top-left at the element's origin): width × height.
    pub fn box_size(&self) -> (f32, f32) {
        crate::text::box_size(&self.shown(), self.size, self.family.as_deref(), self.font, self.letter_spacing, &self.para)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
// Elements live in Vecs on keyframes; boxing Shape would only add indirection.
#[allow(clippy::large_enum_variant)]
pub enum ElementKind {
    Shape(Shape),
    Instance(Instance),
    Text(TextEl),
    /// Modify › Group (Ctrl+G): child elements in the group's coordinate space.
    Group(Vec<Element>),
    /// A library bitmap placed on stage (drawn at 0,0 … width,height).
    Bitmap(BitmapRef),
    /// Merge Drawing: the layer's raw shape, where drawing cuts and merges
    /// (Flash's planar shape). At most one per keyframe, kept at the bottom.
    Canvas(crate::merge::MergeShape),
    /// A particle emitter (Particle tool): particles sprayed over time from
    /// the keyframe where it appears.
    Particles(Box<crate::particles::Emitter>),
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct BitmapRef {
    pub bitmap: crate::bitmap::BitmapId,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Element {
    pub name: String,
    pub matrix: Matrix,
    /// Color effect: alpha multiplier 0..1
    pub alpha: f32,
    /// Color effect: tint (color, amount 0..1)
    pub tint: Option<(Color, f32)>,
    pub kind: ElementKind,
    /// 3D Rotation / 3D Translation (perspective-projected at render time).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub three_d: Option<Transform3D>,
    /// Properties › Filters (applied to everything the element draws).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub filters: Vec<crate::fx::Filter>,
    #[serde(default, skip_serializing_if = "is_normal_blend")]
    pub blend: crate::fx::BlendMode,
    /// Color effect › Advanced: per-channel multiplier and offset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color_fx: Option<ColorXform>,
    /// Asset Warp on a bitmap, symbol or group: pins (element coordinates)
    /// deform a mesh over what the element draws (OpenToonz plastic-style).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mesh: Option<crate::deform::MeshWarp>,
}

/// Animate's Advanced colour effect: `out = in × mul + add` per channel
/// (r, g, b, a). Multipliers are fractions (1 = 100%), offsets −255…255.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ColorXform {
    pub mul: [f32; 4],
    pub add: [f32; 4],
}

impl Default for ColorXform {
    fn default() -> Self {
        ColorXform::IDENTITY
    }
}

impl ColorXform {
    pub const IDENTITY: ColorXform = ColorXform { mul: [1.0; 4], add: [0.0; 4] };
    pub fn apply(&self, c: Color) -> Color {
        let ch = |v: u8, i: usize| (v as f32 * self.mul[i] + self.add[i]).round().clamp(0.0, 255.0) as u8;
        Color { r: ch(c.r, 0), g: ch(c.g, 1), b: ch(c.b, 2), a: ch(c.a, 3) }
    }
    /// `self` applied after `inner`.
    pub fn after(&self, inner: &ColorXform) -> ColorXform {
        let mut o = ColorXform::IDENTITY;
        for i in 0..4 {
            o.mul[i] = self.mul[i] * inner.mul[i];
            o.add[i] = self.mul[i] * inner.add[i] + self.add[i];
        }
        o
    }
    pub fn compose(outer: Option<ColorXform>, inner: Option<ColorXform>) -> Option<ColorXform> {
        match (outer, inner) {
            (Some(o), Some(i)) => Some(o.after(&i)),
            (o, i) => o.or(i),
        }
    }
    pub fn lerp(a: Option<ColorXform>, b: Option<ColorXform>, t: f32) -> Option<ColorXform> {
        if a.is_none() && b.is_none() {
            return None;
        }
        let (a, b) = (a.unwrap_or_default(), b.unwrap_or_default());
        let mut o = a;
        for i in 0..4 {
            o.mul[i] = a.mul[i] + (b.mul[i] - a.mul[i]) * t;
            o.add[i] = a.add[i] + (b.add[i] - a.add[i]) * t;
        }
        Some(o)
    }
    pub fn is_identity(&self) -> bool {
        *self == ColorXform::IDENTITY
    }
}

fn is_normal_blend(b: &crate::fx::BlendMode) -> bool {
    *b == crate::fx::BlendMode::Normal
}

impl Element {
    fn base(name: &str, matrix: Matrix, kind: ElementKind) -> Self {
        Element { name: name.into(), matrix, alpha: 1.0, tint: None, kind, three_d: None, filters: vec![], blend: Default::default(), color_fx: None, mesh: None }
    }
    pub fn group(children: Vec<Element>) -> Self {
        Element::base("", Matrix::IDENTITY, ElementKind::Group(children))
    }
    /// A particle emitter at the origin (place it with `matrix`).
    pub fn particles(e: crate::particles::Emitter) -> Self {
        Element { name: String::new(), ..Element::group(vec![]) }.with_kind(ElementKind::Particles(Box::new(e)))
    }
    fn with_kind(mut self, kind: ElementKind) -> Self {
        self.kind = kind;
        self
    }
    pub fn bitmap(name: &str, bitmap: crate::bitmap::BitmapId, matrix: Matrix) -> Self {
        Element::base(name, matrix, ElementKind::Bitmap(BitmapRef { bitmap }))
    }
    /// Instance name (Properties › Object), used by frame scripts. Only
    /// symbol instances and text keep one in Animate; we allow any element.
    pub fn instance_name(&self) -> Option<&str> {
        (!self.name.is_empty()).then_some(self.name.as_str())
    }
    pub fn shape(name: &str, shape: Shape) -> Self {
        Element::base(name, Matrix::IDENTITY, ElementKind::Shape(shape))
    }
    pub fn text(text: &str, size: f32, color: Color, at: Pt) -> Self {
        Element::base("Text", Matrix::translate(at.x, at.y), ElementKind::Text(TextEl { text: text.into(), size, color, font: Default::default(), align: Default::default(), letter_spacing: 0.0, family: None, weight: 400, italic: false, para: Default::default(), field: Default::default() }))
    }
    pub fn instance(name: &str, symbol: SymbolId, matrix: Matrix) -> Self {
        Element::base(name, matrix, ElementKind::Instance(Instance { symbol, loop_mode: LoopMode::Loop, first_frame: 0 }))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Tween {
    None,
    /// Matrix + color-effect interpolation (Animate "Classic Tween").
    Classic { ease: f32, rotate_turns: i32 },
    /// Path + color morph (Animate "Shape Tween").
    Shape { ease: f32 },
    /// Property keyframes on a span (Animate "Motion Tween"); the keys live
    /// in `Keyframe::motion`, the span needs no closing keyframe.
    Motion { ease: f32 },
}

impl Tween {
    pub fn ease(&self) -> f32 {
        match *self {
            Tween::None => 0.0,
            Tween::Classic { ease, .. } | Tween::Shape { ease } | Tween::Motion { ease } => ease,
        }
    }
    pub fn is_motion(&self) -> bool {
        matches!(self, Tween::Motion { .. })
    }
}

/// Animate's simple ease: -100 = ease in, +100 = ease out (quadratic).
pub fn apply_ease(t: f32, ease: f32) -> f32 {
    let e = (ease / 100.0).clamp(-1.0, 1.0);
    let curved = if e >= 0.0 { 1.0 - (1.0 - t) * (1.0 - t) } else { t * t };
    t + (curved - t) * e.abs()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Keyframe {
    pub elements: Vec<Element>,
    pub tween: Tween,
    pub label: Option<String>,
    /// Frame script (JavaScript, CreateJS-style API) run when playback enters this frame.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script: Option<String>,
    /// Classic tween along a motion guide: rotate with the path's direction.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub orient_to_path: bool,
    /// Custom/preset ease for this span (overrides the −100…100 simple ease).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ease_curve: Option<crate::motion::Ease>,
    /// Shape hints (a, b, c…) for shape tweens, in the first shape's local
    /// coordinates; hint i here matches hint i on the next keyframe.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shape_hints: Vec<Pt>,
    /// Property keyframes of a motion tween span.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motion: Option<Box<crate::motion::Motion>>,
    /// Properties ▸ Sound: the sound this keyframe starts (or stops).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sound: Option<crate::sound::FrameSound>,
    /// Frame label type: Name (gotoAndPlay target), Comment (author note,
    /// not published) or Anchor (a named anchor; published like Name).
    #[serde(default, skip_serializing_if = "LabelKind::is_name")]
    pub label_kind: LabelKind,
    /// Camera layer only: shake / follow options for this span.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera: Option<Box<CameraOpts>>,
    /// Classic tween: separate eases for position, rotation, scale, colour, filters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prop_eases: Option<Box<crate::motion::PropEases>>,
    /// Classic tween "Sync": graphic instances keep counting frames across
    /// the following keyframes of the tween instead of restarting.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub sync: bool,
    /// Classic tween "Snap": on a motion guide the transformation point snaps
    /// to the path (on by default); off keeps the object's offset from it.
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub snap: bool,
    /// Graphic-symbol clock at this keyframe: graphic instances play from
    /// here (plus their First Frame), so a keyframe inserted inside a span
    /// keeps nested animation running instead of restarting it.
    /// (Ported from Flare's Studio core.)
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub playback_offset: u32,
    /// A reversed static exposure runs graphic symbols backwards as well.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub playback_reversed: bool,
    /// A tween piece that keeps the original curve over part of its source
    /// time (made when a tween is split or partly copied with
    /// `frames::set_retain_tween_curves(true)`; Flare's Studio behaviour).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tween_boundary: Option<Box<TweenBoundary>>,
}

fn is_zero_u32(v: &u32) -> bool {
    *v == 0
}

/// An authored tween curve retained across clipping, splitting and reversal.
/// The owning keyframe's elements stay the live start of the curve; this holds
/// its end, and which part of the source time `[time_start, time_end)` the
/// span shows (backwards when `reversed`). Ported from Flare's Studio core.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TweenBoundary {
    /// Length of the original tween in frames.
    pub duration: u32,
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub time_start: f32,
    pub time_end: f32,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub reversed: bool,
    /// The original tween's end keyframe content.
    pub elements: Vec<Element>,
    pub shape_hints: Vec<Pt>,
    /// Sparse corrections made by editing poses inside the piece.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pose_edits: Vec<crate::tween_edit::TweenPoseEdit>,
}

fn is_zero_f32(v: &f32) -> bool {
    *v == 0.0
}

impl TweenBoundary {
    /// Source time (frames into the original tween) shown at `local` frames
    /// into a span of `span_len` frames.
    pub fn source_time(&self, local: u32, span_len: u32) -> f32 {
        let index = if self.reversed { span_len.saturating_sub(1).saturating_sub(local) } else { local };
        self.time_start + index as f32 * ((self.time_end - self.time_start) / span_len.max(1) as f32)
    }
}

fn yes() -> bool {
    true
}
fn is_true(b: &bool) -> bool {
    *b
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LabelKind {
    #[default]
    Name,
    Comment,
    Anchor,
}

impl LabelKind {
    pub const ALL: [LabelKind; 3] = [LabelKind::Name, LabelKind::Comment, LabelKind::Anchor];
    pub fn is_name(&self) -> bool {
        *self == LabelKind::Name
    }
    pub fn name(self) -> &'static str {
        match self {
            LabelKind::Name => "Name",
            LabelKind::Comment => "Comment",
            LabelKind::Anchor => "Anchor",
        }
    }
    pub fn id(self) -> &'static str {
        match self {
            LabelKind::Name => "name",
            LabelKind::Comment => "comment",
            LabelKind::Anchor => "anchor",
        }
    }
    pub fn from_id(s: &str) -> Option<LabelKind> {
        LabelKind::ALL.into_iter().find(|k| k.id() == s.to_ascii_lowercase())
    }
}

impl Keyframe {
    pub fn blank() -> Self {
        Keyframe::with(vec![])
    }
    pub fn with(elements: Vec<Element>) -> Self {
        Keyframe { elements, tween: Tween::None, label: None, script: None, orient_to_path: false, ease_curve: None, shape_hints: vec![], motion: None, sound: None, label_kind: LabelKind::Name, prop_eases: None, sync: false, snap: true, camera: None, playback_offset: 0, playback_reversed: false, tween_boundary: None }
    }
    /// The graphic-symbol clock `local` frames into this keyframe's span.
    pub fn playback_frame(&self, local: u32) -> u32 {
        if self.playback_reversed {
            self.playback_offset.saturating_sub(local)
        } else {
            self.playback_offset.saturating_add(local)
        }
    }
    /// The label as a script/publish target (comments are author notes only).
    pub fn target_label(&self) -> Option<&str> {
        self.label.as_deref().filter(|_| self.label_kind != LabelKind::Comment)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayerKind {
    Normal,
    /// Not exported. With child layers it is a motion guide: classic tweens on
    /// the children follow the first stroke on it.
    Guide,
    Folder,
    /// The shapes on a mask layer cut out (clip) its child layers.
    Mask,
    /// Camera layer: its keyframes hold the camera (pan/zoom/rotate), tweenable.
    Camera,
}

/// An IK bone linking two objects on the same layer (element indices are
/// the same in every keyframe of the layer). Points are in local coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bone {
    pub parent: usize,
    pub child: usize,
    /// Where the bone starts, in the parent's local space.
    pub head: Pt,
    /// The joint, in the parent's local space…
    pub tail_parent: Pt,
    /// …and the same joint in the child's local space.
    pub tail_child: Pt,
    /// Joint: Rotation ▸ Enable. Off: the joint stays as it is while posing.
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub rotate: bool,
    /// Joint: Rotation ▸ Constrain: how far (degrees) the child may turn
    /// from `rest`, as (min ≤ 0, max ≥ 0). `None` = free.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<(f32, f32)>,
    /// The child's rotation relative to its parent when the bone was made (degrees).
    #[serde(default)]
    pub rest: f32,
    /// Pinned joint: while posing, the joint stays at this point (layer coordinates).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pin: Option<Pt>,
}

impl Bone {
    /// The child's rotation relative to the parent (degrees, −180…180).
    pub fn relative_angle(els: &[Element], parent: usize, child: usize) -> Option<f32> {
        let (p, c) = (els.get(parent)?.matrix, els.get(child)?.matrix);
        let a = c.b.atan2(c.a).to_degrees() - p.b.atan2(p.a).to_degrees();
        Some(wrap_deg(a))
    }
    /// Clamp a joint turn of `d` degrees from relative angle `rel` to the limits.
    pub fn allowed_turn(&self, rel: f32, d: f32) -> f32 {
        if !self.rotate {
            return 0.0;
        }
        match self.limit {
            Some((lo, hi)) => {
                let off = wrap_deg(rel - self.rest);
                let to = (off + d).clamp(lo.min(0.0), hi.max(0.0));
                to - off
            }
            None => d,
        }
    }
}

/// An angle in degrees wrapped to −180…180.
pub fn wrap_deg(a: f32) -> f32 {
    let mut a = a % 360.0;
    if a > 180.0 {
        a -= 360.0;
    } else if a <= -180.0 {
        a += 360.0;
    }
    a
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub name: String,
    pub color: Color,
    pub visible: bool,
    pub locked: bool,
    pub outline: bool,
    pub kind: LayerKind,
    /// Keyframes by start frame (0-based). A keyframe spans until the next one.
    pub keyframes: BTreeMap<u32, Keyframe>,
    /// Number of frames the layer occupies (exclusive end).
    pub end: u32,
    /// IK armature (Bone tool).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bones: Vec<Bone>,
    /// Folder, mask or guide layer this layer belongs to (index in the same
    /// timeline, always above this layer) — XFL's `parentLayerIndex`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<usize>,
    /// Folder collapsed in the Timeline panel.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub collapsed: bool,
    /// Layer parenting: this layer's objects are in the coordinate space of
    /// the first object on that layer, and follow it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anim_parent: Option<usize>,
    /// Timeline row height in percent (Layer Properties › Layer height: 100, 200, 300).
    #[serde(default = "default_row_height", skip_serializing_if = "is_default_row_height")]
    pub row_height: u16,
}

fn default_row_height() -> u16 {
    100
}
fn is_default_row_height(h: &u16) -> bool {
    *h == 100
}

const LAYER_COLORS: [Color; 6] = [
    Color::rgb(0x4F, 0xFF, 0x4F),
    Color::rgb(0xFF, 0x4F, 0xFF),
    Color::rgb(0x4F, 0xFF, 0xFF),
    Color::rgb(0xFF, 0x80, 0x00),
    Color::rgb(0x99, 0x66, 0xFF),
    Color::rgb(0xFF, 0xFF, 0x4F),
];

impl Layer {
    pub fn new(name: &str, index: usize) -> Self {
        let mut keyframes = BTreeMap::new();
        keyframes.insert(0, Keyframe::blank());
        Layer {
            name: name.into(),
            color: LAYER_COLORS[index % LAYER_COLORS.len()],
            visible: true,
            locked: false,
            outline: false,
            kind: LayerKind::Normal,
            keyframes,
            end: 1,
            bones: vec![],
            parent: None,
            collapsed: false,
            anim_parent: None,
            row_height: 100,
        }
    }

    /// The keyframe governing `frame`: (start, keyframe, next keyframe start if any).
    pub fn span_at(&self, frame: u32) -> Option<(u32, &Keyframe, Option<u32>)> {
        if frame >= self.end {
            return None;
        }
        let (start, kf) = self.keyframes.range(..=frame).next_back()?;
        let next = self.keyframes.range(frame + 1..).next().map(|(s, _)| *s);
        Some((*start, kf, next))
    }

    /// Exclusive end of the span starting at `start`.
    pub fn span_end(&self, start: u32) -> u32 {
        self.keyframes.range(start + 1..).next().map(|(s, _)| *s).unwrap_or(self.end)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Timeline {
    pub name: String,
    /// Index 0 is the top layer (as listed in the Timeline panel).
    pub layers: Vec<Layer>,
}

impl Timeline {
    pub fn new(name: &str) -> Self {
        Timeline { name: name.into(), layers: vec![Layer::new("Layer 1", 0)] }
    }
    pub fn frame_count(&self) -> u32 {
        self.layers.iter().map(|l| l.end).max().unwrap_or(1).max(1)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Symbol {
    pub id: SymbolId,
    pub name: String,
    pub kind: SymbolKind,
    pub timeline: Timeline,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Document {
    pub width: f32,
    pub height: f32,
    pub fps: f32,
    pub background: Color,
    pub scene: Timeline,
    pub library: Vec<Symbol>,
    pub next_symbol_id: SymbolId,
    /// 3D: perspective angle (degrees) and vanishing point (default: stage centre).
    #[serde(default = "default_perspective")]
    pub perspective_angle: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vanishing_point: Option<Pt>,
    /// Imported bitmaps (library items alongside symbols).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bitmaps: Vec<crate::bitmap::BitmapAsset>,
    /// Ruler guides (View › Guides), in stage coordinates.
    #[serde(default, skip_serializing_if = "Guides::is_empty")]
    pub guides: Guides,
    /// Scenes after the first (`scene`); the movie plays them in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub more_scenes: Vec<Timeline>,
    /// Imported sounds (library items).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sounds: Vec<crate::sound::SoundAsset>,
    /// How the camera is authored: Stage (the stage is the camera frame, as
    /// in Animate's camera tool) or Free (a VCam: the camera is a frame you
    /// move around the scene; output still renders through it).
    #[serde(default, skip_serializing_if = "CameraMode::is_stage")]
    pub camera_mode: CameraMode,
    /// The document's swatches (Animate keeps them in the .fla); None = the
    /// app's own set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub swatches: Option<Vec<Swatch>>,
    /// Library folders, as paths ("Characters/Hero"), including empty ones.
    /// Items live in a folder by their name: "Characters/Hero/Head" is the
    /// item "Head" in "Characters/Hero" (Animate's XFL convention).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub folders: Vec<String>,
    /// Linkage (Export for runtime): a symbol's class name in published code.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub linkage: std::collections::BTreeMap<SymbolId, String>,
    /// Drawing assistants and symmetry (OpenToonz): guides and replicators
    /// that shape freehand strokes, in stage coordinates.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assistants: Vec<crate::assist::Assistant>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Guides {
    /// Vertical guides at these x positions.
    pub x: Vec<f32>,
    /// Horizontal guides at these y positions.
    pub y: Vec<f32>,
}

impl Guides {
    pub fn is_empty(&self) -> bool {
        self.x.is_empty() && self.y.is_empty()
    }
}

fn default_perspective() -> f32 {
    55.0
}

impl Default for Document {
    fn default() -> Self {
        Document {
            width: 550.0,
            height: 400.0,
            fps: 24.0,
            background: Color::rgb(255, 255, 255),
            scene: Timeline::new("Scene 1"),
            library: vec![],
            next_symbol_id: 1,
            perspective_angle: 55.0,
            vanishing_point: None,
            bitmaps: vec![],
            guides: Guides::default(),
            more_scenes: vec![],
            sounds: vec![],
            camera_mode: CameraMode::Stage,
            assistants: vec![],
            swatches: None,
            folders: vec![],
            linkage: Default::default(),
        }
    }
}

/// Camera authoring mode (see `Document::camera_mode`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CameraMode {
    #[default]
    Stage,
    Free,
}

impl CameraMode {
    pub fn is_stage(&self) -> bool {
        *self == CameraMode::Stage
    }
}

/// Per-span camera options (on the Camera layer's keyframes): shake and
/// following an object, as VCam users scripted them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CameraOpts {
    /// Shake amount in px (0 = none) and speed in shakes per second.
    #[serde(default)]
    pub shake: f32,
    #[serde(default = "default_shake_speed")]
    pub shake_speed: f32,
    /// Follow the object with this instance name (its centre); the camera
    /// keyframes still set zoom and rotation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub follow: Option<String>,
    /// Follow lag in frames (0 = locked on; larger = smoother, later).
    #[serde(default)]
    pub follow_lag: u32,
    /// Offset of the view centre from the followed object, in px.
    #[serde(default)]
    pub follow_offset: Pt,
}

fn default_shake_speed() -> f32 {
    8.0
}

impl Default for CameraOpts {
    fn default() -> Self {
        CameraOpts { shake: 0.0, shake_speed: default_shake_speed(), follow: None, follow_lag: 0, follow_offset: Pt::default() }
    }
}

impl Document {
    /// Lengths of the movie clips (with more than one frame) showing at
    /// movie frame `frame`, at any depth.
    pub fn movie_clip_lengths(&self, frame: u32) -> Vec<u32> {
        fn walk(doc: &Document, tl: &Timeline, frame: u32, global: u32, depth: u32, out: &mut Vec<u32>) {
            if depth > 16 {
                return;
            }
            for l in &tl.layers {
                let Some((start, kf, _)) = l.span_at(frame) else { continue };
                fn els(doc: &Document, e: &[Element], local: u32, global: u32, depth: u32, out: &mut Vec<u32>) {
                    for el in e {
                        match &el.kind {
                            ElementKind::Instance(i) => {
                                if let Some(sym) = doc.symbol(i.symbol) {
                                    let dur = sym.timeline.frame_count();
                                    if sym.kind == SymbolKind::MovieClip && dur > 1 && !out.contains(&dur) {
                                        out.push(dur);
                                    }
                                    walk(doc, &sym.timeline, i.symbol_frame(sym.kind, local, global, dur), global, depth + 1, out);
                                }
                            }
                            ElementKind::Group(ch) => els(doc, ch, local, global, depth, out),
                            _ => {}
                        }
                    }
                }
                els(doc, &kf.elements, frame - start, global, depth, out);
            }
        }
        let (si, local) = self.locate_frame(frame);
        let mut out = vec![];
        if let Some(tl) = self.scene_at(si) {
            walk(self, tl, local, frame, 0, &mut out);
        }
        out
    }

    /// A copy with every floating merge selection merged back (for saving).
    pub fn with_floats_committed(&self) -> Document {
        fn fix(els: &mut [Element]) {
            for el in els {
                match &mut el.kind {
                    ElementKind::Canvas(c) => {
                        c.commit_float();
                    }
                    ElementKind::Group(ch) => fix(ch),
                    _ => {}
                }
            }
        }
        let mut d = self.clone();
        let mut tls: Vec<&mut Timeline> = std::iter::once(&mut d.scene).chain(d.more_scenes.iter_mut()).collect();
        tls.extend(d.library.iter_mut().map(|s| &mut s.timeline));
        for t in tls {
            for l in &mut t.layers {
                for kf in l.keyframes.values_mut() {
                    fix(&mut kf.elements);
                    if let Some(b) = &mut kf.tween_boundary {
                        fix(&mut b.elements);
                    }
                }
            }
        }
        d
    }
    /// Number of scenes (at least one).
    pub fn scene_count(&self) -> usize {
        1 + self.more_scenes.len()
    }
    pub fn scene_at(&self, i: usize) -> Option<&Timeline> {
        if i == 0 {
            Some(&self.scene)
        } else {
            self.more_scenes.get(i - 1)
        }
    }
    pub fn scene_at_mut(&mut self, i: usize) -> Option<&mut Timeline> {
        if i == 0 {
            Some(&mut self.scene)
        } else {
            self.more_scenes.get_mut(i - 1)
        }
    }
    pub fn scenes(&self) -> impl Iterator<Item = &Timeline> {
        std::iter::once(&self.scene).chain(self.more_scenes.iter())
    }
    pub fn scenes_mut(&mut self) -> impl Iterator<Item = &mut Timeline> {
        std::iter::once(&mut self.scene).chain(self.more_scenes.iter_mut())
    }
    /// Frames in the whole movie (all scenes).
    pub fn total_frames(&self) -> u32 {
        self.scenes().map(|t| t.frame_count()).sum::<u32>().max(1)
    }
    /// Movie frame → (scene index, frame in that scene). Past the end: the last frame.
    pub fn locate_frame(&self, frame: u32) -> (usize, u32) {
        let mut f = frame;
        let n = self.scene_count();
        for (i, t) in self.scenes().enumerate() {
            let c = t.frame_count();
            if f < c || i + 1 == n {
                return (i, f.min(c - 1));
            }
            f -= c;
        }
        (0, 0)
    }
    /// First movie frame of scene `i`.
    pub fn scene_start(&self, i: usize) -> u32 {
        self.scenes().take(i).map(|t| t.frame_count()).sum()
    }
    pub fn symbol(&self, id: SymbolId) -> Option<&Symbol> {
        self.library.iter().find(|s| s.id == id)
    }
    pub fn bitmap(&self, id: crate::bitmap::BitmapId) -> Option<&crate::bitmap::BitmapAsset> {
        self.bitmaps.iter().find(|b| b.id == id)
    }
    pub fn sound(&self, id: crate::sound::SoundId) -> Option<&crate::sound::SoundAsset> {
        self.sounds.iter().find(|s| s.id == id)
    }
    pub fn next_bitmap_id(&self) -> crate::bitmap::BitmapId {
        self.bitmaps.iter().map(|b| b.id).max().unwrap_or(0) + 1
    }
    pub fn symbol_mut(&mut self, id: SymbolId) -> Option<&mut Symbol> {
        self.library.iter_mut().find(|s| s.id == id)
    }
    pub fn add_symbol(&mut self, name: &str, kind: SymbolKind, timeline: Timeline) -> SymbolId {
        let id = self.next_symbol_id;
        self.next_symbol_id += 1;
        self.library.push(Symbol { id, name: name.into(), kind, timeline });
        id
    }
    pub fn perspective(&self) -> Perspective {
        Perspective { angle_deg: self.perspective_angle, vanishing: self.vanishing_point.unwrap_or(pt(self.width / 2.0, self.height / 2.0)), stage_width: self.width }
    }
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("document serialises")
    }
    pub fn from_json(s: &str) -> Result<Document, serde_json::Error> {
        serde_json::from_str(s)
    }
}

// ---------------------------------------------------------------- evaluation

/// One flattened, ready-to-draw shape.
#[derive(Clone, Debug)]
pub struct DrawItem {
    pub path: Path,
    pub fill: Option<Color>,
    pub stroke: Option<Stroke>,
    pub matrix: Matrix,
    pub alpha: f32,
    pub tint: Option<(Color, f32)>,
    pub outline: Option<Color>,
    /// (layer index, element index) in the timeline being evaluated — top level only.
    pub src: Option<(usize, usize)>,
    /// Gradient fill in the item's local space.
    pub gradient: Option<Gradient>,
    /// Effect groups this item belongs to, outermost first (filters/blend
    /// render all items of a group into one offscreen layer).
    pub fx: Vec<Arc<FxGroup>>,
    /// Mask layers clipping this item (intersected).
    pub clips: Vec<Arc<Clip>>,
    /// Bitmap drawn into `path` (a 0,0,w,h rectangle) when set.
    pub image: Option<Arc<Image>>,
    /// Advanced colour effect (composed through nested symbols).
    pub cx: Option<ColorXform>,
    pub stroke_style: StrokeStyle,
    /// Bitmap fill: the image and its placement in the item's space.
    pub pattern: Option<(Arc<Image>, Matrix)>,
}

/// The shapes of a mask layer, in the same space as item matrices.
#[derive(Debug)]
pub struct Clip {
    pub shapes: Vec<(Path, Matrix)>,
}

impl DrawItem {
    pub fn new(path: Path, matrix: Matrix) -> DrawItem {
        DrawItem { path, fill: None, stroke: None, matrix, alpha: 1.0, tint: None, outline: None, src: None, gradient: None, fx: vec![], clips: vec![], image: None, cx: None, stroke_style: StrokeStyle { dash: Dash::Solid, cap: Cap::Round, join: Join::Round, dash_len: 3.0, gap: 3.0, art: ArtParams::default() }, pattern: None }
    }
}

impl DrawItem {
    pub fn hit(&self, p: Pt) -> bool {
        let Some(inv) = self.matrix.invert() else { return false };
        let lp = inv.apply(p);
        let scale = self.matrix.decompose().scale_x.abs().max(0.01);
        let tol = self.stroke.map(|s| s.width / 2.0).unwrap_or(0.0) + 3.0 / scale;
        if !self.clips.is_empty() && !self.clips.iter().all(|c| c.shapes.iter().any(|(sp, m)| m.invert().map(|i| sp.hit(i.apply(p), true, 0.0)).unwrap_or(false))) {
            return false; // masked out here
        }
        if let Some(img) = &self.image {
            // bitmaps: only where there are pixels (a transparent paint
            // bitmap must not hide what is under it)
            let (cx, cy) = (lp.x.floor() as i64, lp.y.floor() as i64);
            let r = (2.0 / scale).ceil().max(1.0) as i64;
            for y in (cy - r)..=(cy + r) {
                for x in (cx - r)..=(cx + r) {
                    if x >= 0 && y >= 0 && (x as u32) < img.width && (y as u32) < img.height && img.rgba[((y as u32 * img.width + x as u32) * 4 + 3) as usize] > 16 {
                        return true;
                    }
                }
            }
            return false;
        }
        self.path.hit(lp, self.fill.is_some() || self.gradient.is_some(), tol)
    }
}

/// Elements of a layer at `frame`, with tweens resolved.
/// Returns (elements, frames since keyframe start).
pub fn layer_elements_at(layer: &Layer, frame: u32) -> Option<(Vec<Element>, u32)> {
    let (start, kf, next) = layer.span_at(frame)?;
    let local = frame - start;
    let span_len = next.unwrap_or(layer.end).min(layer.end) - start;
    let props_of = |tween: Tween| if matches!(tween, Tween::Classic { .. }) { kf.prop_eases.as_deref() } else { None };
    let tweened = if let Some(b) = kf.tween_boundary.as_ref().filter(|_| kf.tween != Tween::None) {
        // A retained piece of a longer tween: evaluate the original curve.
        let st = b.source_time(local, span_len);
        match kf.tween {
            Tween::Motion { ease } => Some(motion_elements(kf, st, b.duration, ease)),
            tween if st > 0.0 => {
                let t = crate::motion::PropT::at(st / b.duration.max(1) as f32, tween.ease(), kf.ease_curve.as_ref(), props_of(tween));
                let n = kf.shape_hints.len().min(b.shape_hints.len());
                let hints = (n > 0).then(|| (&kf.shape_hints[..n], &b.shape_hints[..n]));
                Some(interpolate_elements(&kf.elements, &b.elements, t, tween, hints))
            }
            _ => None,
        }
    } else {
        match (kf.tween, next) {
            (Tween::Motion { ease }, _) => Some(motion_elements(kf, local as f32, span_len, ease)),
            (Tween::None, _) | (_, None) => None,
            (tween, Some(next_start)) if frame > start => {
                let to = &layer.keyframes[&next_start];
                let t = crate::motion::PropT::at(local as f32 / (next_start - start) as f32, tween.ease(), kf.ease_curve.as_ref(), props_of(tween));
                let n = kf.shape_hints.len().min(to.shape_hints.len());
                let hints = (n > 0).then(|| (&kf.shape_hints[..n], &to.shape_hints[..n]));
                Some(interpolate_elements(&kf.elements, &to.elements, t, tween, hints))
            }
            _ => None,
        }
    };
    let mut els = tweened.unwrap_or_else(|| kf.elements.clone());
    if let Some(b) = &kf.tween_boundary {
        crate::tween_edit::apply(&b.pose_edits, b.source_time(local, span_len), &mut els);
    }
    sync_first_frames(layer, start, &mut els);
    if !layer.bones.is_empty() {
        apply_bones(&layer.bones, &mut els);
    }
    Some((els, kf.playback_frame(local)))
}

/// Classic tween Sync: an instance in a keyframe reached by a chain of synced
/// classic tweens continues counting from where the previous keyframe's
/// instance of the same symbol was, instead of restarting at its first frame.
fn sync_first_frames(layer: &Layer, start: u32, els: &mut [Element]) {
    let mut chain = vec![start];
    let mut s = start;
    while let Some((&ps, pk)) = layer.keyframes.range(..s).next_back() {
        if !(pk.sync && matches!(pk.tween, Tween::Classic { .. })) {
            break;
        }
        chain.push(ps);
        s = ps;
    }
    if chain.len() < 2 {
        return;
    }
    chain.reverse();
    let first = |e: &Element| match &e.kind {
        ElementKind::Instance(i) => Some((i.symbol, i.first_frame)),
        _ => None,
    };
    let mut ff: Vec<Option<(SymbolId, u32)>> = layer.keyframes[&chain[0]].elements.iter().map(first).collect();
    for w in chain.windows(2) {
        let step = w[1] - w[0];
        let next: Vec<Option<(SymbolId, u32)>> = layer.keyframes[&w[1]].elements.iter().map(first).collect();
        ff = next
            .iter()
            .enumerate()
            .map(|(i, n)| match (n, ff.get(i).copied().flatten()) {
                (Some((sym, _)), Some((psym, pf))) if *sym == psym => Some((psym, pf + step)),
                (n, _) => *n,
            })
            .collect();
    }
    for (e, f) in els.iter_mut().zip(ff) {
        if let (ElementKind::Instance(i), Some((sym, frame))) = (&mut e.kind, f) {
            if i.symbol == sym {
                i.first_frame = frame;
            }
        }
    }
}

/// The motion tween's target (the first element) posed at `local` frame.
pub fn motion_elements(kf: &Keyframe, local: f32, len: u32, simple: f32) -> Vec<Element> {
    let mut els = kf.elements.clone();
    let (Some(m), Some(el)) = (&kf.motion, els.first_mut()) else { return els };
    let base = crate::motion::Pose::of(el, m.pivot);
    let mut pose = m.pose_at(&base, local, len, simple, kf.ease_curve.as_ref());
    if kf.orient_to_path {
        if let (Some(a), Some(a0)) = (m.path_angle(local), m.path_angle(0.0)) {
            pose.rotation += a - a0;
        }
    }
    pose.apply(el, m.pivot);
    m.apply_extra(el, local, len, simple, kf.ease_curve.as_ref());
    els
}

/// Forward kinematics: move each bone's child so its joint sits on the
/// parent's joint (parents first). Rotations come from the keyframes/tweens.
pub fn apply_bones(bones: &[Bone], els: &mut [Element]) {
    let depth = |b: &Bone| -> usize {
        let mut d = 0;
        let mut p = b.parent;
        while let Some(pb) = bones.iter().find(|x| x.child == p) {
            d += 1;
            p = pb.parent;
            if d > 64 {
                break;
            }
        }
        d
    };
    let mut order: Vec<&Bone> = bones.iter().collect();
    order.sort_by_key(|b| depth(b));
    for b in order {
        if b.parent >= els.len() || b.child >= els.len() {
            continue;
        }
        let target = els[b.parent].matrix.apply(b.tail_parent);
        let cur = els[b.child].matrix.apply(b.tail_child);
        els[b.child].matrix.tx += target.x - cur.x;
        els[b.child].matrix.ty += target.y - cur.y;
    }
}

fn interpolate_elements(from: &[Element], to: &[Element], pt_: crate::motion::PropT, tween: Tween, hints: Option<(&[Pt], &[Pt])>) -> Vec<Element> {
    let t = pt_.pos;
    from.iter()
        .enumerate()
        .map(|(i, a)| {
            let Some(b) = to.get(i) else { return a.clone() };
            let turns = if let Tween::Classic { rotate_turns, .. } = tween { rotate_turns } else { 0 };
            let mut out = a.clone();
            out.matrix = a.matrix.interpolate_split(&b.matrix, pt_.pos, pt_.rot, pt_.scale, turns);
            let tc = pt_.color;
            out.alpha = a.alpha + (b.alpha - a.alpha) * tc;
            out.tint = match (a.tint, b.tint) {
                (Some((ca, ta)), Some((cb, tb))) => Some((ca.lerp(cb, tc), ta + (tb - ta) * tc)),
                (Some((c, ta)), None) => Some((c, ta * (1.0 - tc))),
                (None, Some((c, tb))) => Some((c, tb * tc)),
                (None, None) => None,
            };
            out.three_d = Transform3D::lerp(a.three_d, b.three_d, pt_.rot);
            out.color_fx = ColorXform::lerp(a.color_fx, b.color_fx, tc);
            out.filters = Filter::lerp_list(&a.filters, &b.filters, pt_.filters);
            // shape armatures: poses tween with classic and shape tweens
            if let (ElementKind::Shape(sa), ElementKind::Shape(sb)) = (&a.kind, &b.kind) {
                if let (Some(ra), Some(rb), ElementKind::Shape(so)) = (&sa.rig, &sb.rig, &mut out.kind) {
                    so.rig = Some(crate::rig::ShapeRig::lerp(ra, rb, t));
                }
            }
            if let (Tween::Shape { .. }, ElementKind::Canvas(ca), ElementKind::Canvas(cb)) = (tween, &a.kind, &b.kind) {
                out.kind = ElementKind::Canvas(crate::merge::morph(ca, cb, t));
            }
            if let (Tween::Shape { .. }, ElementKind::Shape(sa), ElementKind::Shape(sb)) = (tween, &a.kind, &b.kind) {
                out.kind = ElementKind::Shape(Shape {
                    gradient: match (&sa.gradient, &sb.gradient) {
                        (Some(x), Some(y)) => Some(x.lerp(y, t)),
                        (x, y) => if t < 0.5 { x.clone() } else { y.clone() },
                    },
                    width_profile: if t < 0.5 { sa.width_profile.clone() } else { sb.width_profile.clone() },
                    warp: None,
                    primitive: None,
                    stroke_style: if t < 0.5 { sa.stroke_style } else { sb.stroke_style },
                    bitmap_fill: if t < 0.5 { sa.bitmap_fill } else { sb.bitmap_fill },
                    rig: match (&sa.rig, &sb.rig) {
                        (Some(x), Some(y)) => Some(crate::rig::ShapeRig::lerp(x, y, t)),
                        _ => None,
                    },
                    path: match hints.filter(|_| i == 0) {
                        Some((ha, hb)) => crate::geom::morph_paths_hinted(&sa.path, &sb.path, t, ha, hb),
                        None => morph_paths(&sa.path, &sb.path, t),
                    },
                    fill: match (sa.fill, sb.fill) {
                        (Some(x), Some(y)) => Some(x.lerp(y, t)),
                        (x, y) => if t < 0.5 { x } else { y },
                    },
                    stroke: match (sa.stroke, sb.stroke) {
                        (Some(x), Some(y)) => Some(Stroke { color: x.color.lerp(y.color, t), width: x.width + (y.width - x.width) * t }),
                        (x, y) => if t < 0.5 { x } else { y },
                    },
                });
            }
            out
        })
        .collect()
}

pub struct EvalCtx<'a> {
    pub doc: &'a Document,
    /// Global frame (used by movie clips, which run on their own clock).
    pub global_frame: u32,
    /// A live button: (top-level layer, element) and the state frame it shows.
    pub button: Option<((usize, usize), u32)>,
    /// Top-level element being evaluated (set by `evaluate` at depth 0).
    pub top: std::cell::Cell<Option<(usize, usize)>>,
}

impl<'a> EvalCtx<'a> {
    pub fn new(doc: &'a Document, global_frame: u32) -> Self {
        EvalCtx { doc, global_frame, button: None, top: Default::default() }
    }
}

fn shape_items(s: &Shape, m: Matrix, a: f32, tint: Option<(Color, f32)>, outline: Option<Color>, out: &mut Vec<DrawItem>) {
    let item = |path: Path, fill: Option<Color>, stroke: Option<Stroke>, gradient: Option<Gradient>| DrawItem { fill, stroke, alpha: a, tint, outline, gradient, ..DrawItem::new(path, m) };
    match (&s.width_profile, s.stroke) {
        // Variable-width stroke: drawn as a filled outline.
        (Some(profile), Some(st)) if !profile.is_empty() => {
            out.push(item(s.path.clone(), s.fill, None, s.gradient.clone()));
            let scaled: Vec<(f32, f32)> = profile.iter().map(|(t, w)| (*t, w * st.width)).collect();
            out.push(item(crate::deform::variable_stroke(&s.path, &scaled), Some(st.color), None, None));
        }
        _ => out.push(item(s.path.clone(), s.fill, s.stroke, s.gradient.clone())),
    }
}

/// Is a layer shown, taking hidden folders above it into account?
pub fn layer_shown(tl: &Timeline, li: usize) -> bool {
    let mut cur = Some(li);
    let mut guard = 0;
    while let Some(i) = cur {
        let Some(l) = tl.layers.get(i) else { return true };
        if !l.visible && (i == li || l.kind == LayerKind::Folder) {
            return false;
        }
        cur = l.parent.filter(|p| *p != i);
        guard += 1;
        if guard > 64 {
            break;
        }
    }
    true
}

/// The mask or guide layer directly governing `li` (skipping folders).
pub fn layer_effect_parent(tl: &Timeline, li: usize, kind: LayerKind) -> Option<usize> {
    let p = tl.layers.get(li)?.parent?;
    (tl.layers.get(p)?.kind == kind).then_some(p)
}

/// Evaluate a timeline at `frame` into draw items, bottom layer first.
pub fn evaluate(ctx: &EvalCtx, tl: &Timeline, frame: u32, parent: &Matrix, alpha: f32, depth: u32, out: &mut Vec<DrawItem>) {
    if depth > 16 {
        return; // recursive symbol guard
    }
    let mut masks: HashMap<usize, Option<Arc<Clip>>> = HashMap::new();
    for (li, layer) in tl.layers.iter().enumerate().rev() {
        if layer.kind != LayerKind::Normal || !layer_shown(tl, li) {
            continue;
        }
        let Some((elements, local)) = posed_elements(ctx, tl, li, frame) else { continue };
        // Motion Blur needs where each blurred object was a frame earlier.
        let earlier = (frame > 0 && elements.iter().any(|e| e.filters.iter().any(|f| matches!(f, Filter::MotionBlur { .. }))))
            .then(|| posed_elements(ctx, tl, li, frame - 1).map(|(e, _)| e))
            .flatten();
        let outline = layer.outline.then_some(layer.color);
        let layer_first = out.len();
        for (ei, el) in elements.iter().enumerate() {
            let first = out.len();
            if depth == 0 {
                ctx.top.set(Some((li, ei)));
            }
            eval_element(ctx, el, parent, alpha, local, outline, depth, out);
            if let Some(prev) = earlier.as_ref().and_then(|e| e.get(ei)) {
                set_group_motion(&mut out[first..], parent, &prev.matrix, &el.matrix);
            }
            if depth == 0 {
                for it in &mut out[first..] {
                    it.src = Some((li, ei));
                }
            }
        }
        if let Some(m) = layer_effect_parent(tl, li, LayerKind::Mask).filter(|m| tl.layers[*m].visible) {
            let clip = masks.entry(m).or_insert_with(|| mask_clip(ctx, tl, m, frame, parent, depth)).clone();
            match clip {
                Some(c) => {
                    for it in &mut out[layer_first..] {
                        it.clips.push(c.clone());
                    }
                }
                // an empty mask frame hides everything it masks (as in Animate)
                None => out.truncate(layer_first),
            }
        }
    }
}

/// A layer's objects at `frame` as drawn: tweens resolved, layer parenting and
/// the classic motion guide applied.
fn posed_elements(ctx: &EvalCtx, tl: &Timeline, li: usize, frame: u32) -> Option<(Vec<Element>, u32)> {
    let layer = &tl.layers[li];
    let (mut elements, local) = layer_elements_at(layer, frame)?;
    if layer.anim_parent.is_some() {
        let pm = layer_parent_matrix(tl, li, frame);
        for el in &mut elements {
            el.matrix = pm.concat(&el.matrix);
        }
    }
    if let Some(g) = layer_effect_parent(tl, li, LayerKind::Guide) {
        apply_motion_guide(ctx, tl, li, g, frame, &mut elements);
    }
    Some((elements, local))
}

/// Give the object's own effect group (the outermost group its items gained
/// in `eval_element`) its motion since the previous frame, in stage space.
fn set_group_motion(items: &mut [DrawItem], parent: &Matrix, prev: &Matrix, cur: &Matrix) {
    let Some(g) = items.first().and_then(|it| it.fx.first()).filter(|g| g.filters.iter().any(|f| matches!(f, Filter::MotionBlur { .. }))).cloned() else { return };
    let (now, then) = (parent.concat(cur), parent.concat(prev));
    let Some(inv) = now.invert() else { return };
    let motion = then.concat(&inv);
    if motion == Matrix::IDENTITY {
        return;
    }
    let mut ng = (*g).clone();
    ng.motion = Some(motion);
    let ng = Arc::new(ng);
    for it in items.iter_mut() {
        if it.fx.first().is_some_and(|f| Arc::ptr_eq(f, &g)) {
            it.fx[0] = ng.clone();
        }
    }
}

/// Layer parenting: the transform that layer `li`'s objects inherit at
/// `frame` (the parent layer's first object, through its own parents).
pub fn layer_parent_matrix(tl: &Timeline, li: usize, frame: u32) -> Matrix {
    let mut m = Matrix::IDENTITY;
    let mut cur = tl.layers.get(li).and_then(|l| l.anim_parent);
    let mut guard = 0;
    while let (Some(p), true) = (cur, guard < 32) {
        let Some(pl) = tl.layers.get(p) else { break };
        let pm = layer_elements_at(pl, frame).and_then(|(els, _)| els.first().map(|e| e.matrix)).unwrap_or(Matrix::IDENTITY);
        m = pm.concat(&m);
        cur = pl.anim_parent;
        guard += 1;
    }
    m
}

/// Shapes on mask layer `m` at `frame`, as a clip.
fn mask_clip(ctx: &EvalCtx, tl: &Timeline, m: usize, frame: u32, parent: &Matrix, depth: u32) -> Option<Arc<Clip>> {
    let (els, local) = layer_elements_at(&tl.layers[m], frame)?;
    let mut items = Vec::new();
    for el in &els {
        eval_element(ctx, el, parent, 1.0, local, None, depth, &mut items);
    }
    let shapes: Vec<(Path, Matrix)> = items.into_iter().map(|it| (it.path, it.matrix)).collect();
    (!shapes.is_empty()).then(|| Arc::new(Clip { shapes }))
}

/// Draw items for one element (recursing into groups and symbols).
#[allow(clippy::too_many_arguments)]
pub fn eval_element(ctx: &EvalCtx, el: &Element, parent: &Matrix, alpha: f32, local: u32, outline: Option<Color>, depth: u32, out: &mut Vec<DrawItem>) {
    let m = parent.concat(&el.matrix);
    let a = alpha * el.alpha;
    let first = out.len();
    match &el.kind {
        ElementKind::Shape(s) => {
            let posed;
            let s = match &s.rig {
                Some(r) if !r.bones.is_empty() => {
                    posed = Shape { path: r.deform(&s.path), ..s.clone() };
                    &posed
                }
                _ => s,
            };
            let art = crate::strokes::is_artistic(s.stroke_style.dash) && s.stroke.is_some();
            if art {
                shape_items(&Shape { stroke: None, ..s.clone() }, m, a, el.tint, outline, out);
            } else {
                shape_items(s, m, a, el.tint, outline, out);
            }
            let pattern = s.bitmap_fill.and_then(|b| ctx.doc.bitmap(b.bitmap).and_then(|asset| asset.image()).map(|img| (img, b.matrix)));
            for it in &mut out[first..] {
                it.stroke_style = s.stroke_style;
                if it.fill.is_some() || it.gradient.is_some() {
                    it.pattern = pattern.clone();
                }
            }
            if art {
                // ragged / stipple / hatched: generated geometry, drawn solid
                let solid = StrokeStyle { dash: Dash::Solid, ..s.stroke_style };
                for (path, fill, stroke) in crate::strokes::styled(&s.path, s.stroke.unwrap(), &s.stroke_style, s.width_profile.as_deref()) {
                    out.push(DrawItem { fill, stroke, alpha: a, tint: el.tint, outline, stroke_style: solid, ..DrawItem::new(path, m) });
                }
            }
        }
        ElementKind::Text(t) => {
            if t.field.border {
                // Show border around text: white box, black hairline, 2 px gutter
                let (w, h) = t.box_size();
                let r = Path::rect(-2.0, -2.0, w + 4.0, h + 4.0);
                out.push(DrawItem { fill: Some(Color::rgb(255, 255, 255)), stroke: Some(Stroke { color: Color::rgb(0, 0, 0), width: 1.0 }), alpha: a, tint: el.tint, outline, ..DrawItem::new(r, m) });
            }
            out.push(DrawItem { fill: Some(t.color), alpha: a, tint: el.tint, outline, ..DrawItem::new(t.outline(), m) })
        }
        ElementKind::Group(children) => {
            for c in children {
                eval_element(ctx, c, &m, a, local, outline, depth, out);
            }
            for it in &mut out[first..] {
                if it.tint.is_none() {
                    it.tint = el.tint;
                }
            }
        }
        ElementKind::Canvas(c) => {
            for part in std::iter::once(c).chain(c.float.as_deref()) {
                for r in &part.fills {
                    let pattern = r.bitmap.and_then(|b| ctx.doc.bitmap(b.bitmap).and_then(|asset| asset.image()).map(|img| (img, b.matrix)));
                    // a bitmap-filled region still needs a fill to be drawn (grey if the bitmap is missing)
                    let fill = if r.bitmap.is_some() { r.fill.or(Some(Color::rgb(128, 128, 128))) } else { r.fill };
                    out.push(DrawItem { fill, gradient: r.gradient.clone(), pattern, alpha: a, tint: el.tint, outline, ..DrawItem::new(r.path(), m) });
                }
                for st in &part.strokes {
                    match &st.width {
                        // variable width: a filled outline, as for drawing objects
                        Some(prof) => {
                            let scaled: Vec<(f32, f32)> = prof.iter().map(|(t, w)| (*t, w * st.stroke.width)).collect();
                            out.push(DrawItem { fill: Some(st.stroke.color), alpha: a, tint: el.tint, outline, ..DrawItem::new(crate::deform::variable_stroke(&st.path(), &scaled), m) });
                        }
                        None => out.push(DrawItem { stroke: Some(st.stroke), alpha: a, tint: el.tint, outline, ..DrawItem::new(st.path(), m) }),
                    }
                }
            }
        }
        ElementKind::Bitmap(b) => {
            if let Some(asset) = ctx.doc.bitmap(b.bitmap) {
                let path = Path::rect(0.0, 0.0, asset.width as f32, asset.height as f32);
                let image = asset.image();
                // outline mode / missing pixels: draw the frame only
                let fill = image.is_none().then_some(Color::rgb(200, 200, 200));
                out.push(DrawItem { fill, alpha: a, tint: el.tint, outline, image, ..DrawItem::new(path, m) });
            }
        }
        ElementKind::Particles(e) => e.eval(ctx, &m, a, el.tint, local, outline, depth, out),
        ElementKind::Instance(inst) => {
            let Some(sym) = ctx.doc.symbol(inst.symbol) else { return };
            let dur = sym.timeline.frame_count();
            let f = match sym.kind {
                // Up, or Over/Down while live; the Hit frame (4th) never shows
                SymbolKind::Button => match ctx.button {
                    Some((src, state)) if ctx.top.get() == Some(src) => state.min(2).min(dur.saturating_sub(1)),
                    _ => 0,
                },
                kind => inst.symbol_frame(kind, local, ctx.global_frame, dur),
            };
            evaluate(ctx, &sym.timeline, f, &m, a, depth + 1, out);
            for it in &mut out[first..] {
                if it.tint.is_none() {
                    it.tint = el.tint;
                }
                if outline.is_some() {
                    it.outline = outline;
                }
            }
        }
    }
    if el.color_fx.is_some() {
        for it in &mut out[first..] {
            it.cx = ColorXform::compose(el.color_fx, it.cx);
        }
    }
    // 3D: project everything this object produced.
    if let Some(td) = el.three_d.filter(|t| !t.is_identity()) {
        let center = m.apply(td.center);
        let persp = ctx.doc.perspective();
        for it in &mut out[first..] {
            it.path = project_path(&it.path, &it.matrix, &td, center, &persp);
            if let Some(g) = &mut it.gradient {
                g.matrix = it.matrix.concat(&g.matrix);
            }
            it.image = None; // projected bitmaps draw as their outline for now
            it.matrix = Matrix::IDENTITY;
        }
    }
    // Asset Warp mesh (bitmaps, symbols, groups): the drawn layer is deformed.
    let mesh = match (&el.mesh, &el.kind) {
        (Some(w), ElementKind::Bitmap(_) | ElementKind::Instance(_) | ElementKind::Group(_)) if !w.pins.is_empty() && outline.is_none() => {
            let inv = m.invert();
            let local_bounds = inv.and_then(|inv| {
                out[first..]
                    .iter()
                    .filter_map(|it| it.path.transformed(&inv.concat(&it.matrix)).bounds())
                    .reduce(|a, b| (pt(a.0.x.min(b.0.x), a.0.y.min(b.0.y)), pt(a.1.x.max(b.1.x), a.1.y.max(b.1.y))))
            });
            local_bounds.map(|(lo, hi)| Arc::new(w.map(lo, hi, &m)))
        }
        _ => None,
    };
    // Filters and blend modes: everything this element drew is one layer.
    if ((!el.filters.is_empty() || el.blend != BlendMode::Normal) && outline.is_none()) || mesh.is_some() {
        let mut g = FxGroup::new(el.filters.clone(), el.blend);
        g.time = local as f32 / ctx.doc.fps.max(1.0);
        g.mesh = mesh;
        let fx = Arc::new(g);
        for it in &mut out[first..] {
            it.fx.insert(0, fx.clone());
        }
    }
}

/// Local-space centre of an element (its default transformation point).
pub fn element_local_center(ctx: &EvalCtx, el: &Element) -> Pt {
    let mut items = Vec::new();
    let mut plain = el.clone();
    plain.matrix = Matrix::IDENTITY;
    plain.three_d = None;
    plain.filters.clear();
    eval_element(ctx, &plain, &Matrix::IDENTITY, 1.0, 0, None, 1, &mut items);
    items
        .iter()
        .filter_map(|it| it.path.transformed(&it.matrix).bounds())
        .reduce(|a, b| (pt(a.0.x.min(b.0.x), a.0.y.min(b.0.y)), pt(a.1.x.max(b.1.x), a.1.y.max(b.1.y))))
        .map(|(lo, hi)| pt((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0))
        .unwrap_or_default()
}

/// Classic motion guide: during a classic tween, elements on a layer under a
/// guide layer travel along the guide's first shape (Animate snaps the
/// transformation point to the path at both keyframes).
fn apply_motion_guide(ctx: &EvalCtx, tl: &Timeline, li: usize, g: usize, frame: u32, els: &mut [Element]) {
    let layer = &tl.layers[li];
    let Some((start, kf, next)) = layer.span_at(frame) else { return };
    let Tween::Classic { ease, .. } = kf.tween else { return };
    let snap = kf.snap;
    // the tween's end: the next keyframe, or a retained piece's original end
    let (to_els, t) = match (&kf.tween_boundary, next) {
        (Some(b), _) => {
            let len = next.unwrap_or(layer.end).min(layer.end) - start;
            let st = b.source_time(frame - start, len);
            (&b.elements, crate::motion::PropT::at(st / b.duration.max(1) as f32, ease, kf.ease_curve.as_ref(), kf.prop_eases.as_deref()).pos)
        }
        (None, Some(next)) => (
            &layer.keyframes[&next].elements,
            if frame >= next { 1.0 } else { crate::motion::PropT::at((frame - start) as f32 / (next - start) as f32, ease, kf.ease_curve.as_ref(), kf.prop_eases.as_deref()).pos },
        ),
        (None, None) => return,
    };
    let Some((gels, _)) = layer_elements_at(&tl.layers[g], frame) else { return };
    let Some(guide) = gels.iter().find_map(|e| match &e.kind {
        ElementKind::Shape(s) => Some(s.path.transformed(&e.matrix)),
        _ => None,
    }) else {
        return;
    };
    for (i, el) in els.iter_mut().enumerate() {
        let (Some(a), Some(b)) = (kf.elements.get(i), to_els.get(i)) else { continue };
        let c = element_local_center(ctx, a);
        let (pa, pb) = (a.matrix.apply(c), b.matrix.apply(c));
        let (Some((ta, _)), Some((tb, _))) = (crate::deform::nearest_t(&guide, pa), crate::deform::nearest_t(&guide, pb)) else { continue };
        let tt = ta + (tb - ta) * t;
        let Some(mut p) = crate::deform::point_at(&guide, tt) else { continue };
        if !snap {
            // keep each keyframe's offset from the path, blended along the tween
            if let (Some(qa), Some(qb)) = (crate::deform::point_at(&guide, ta), crate::deform::point_at(&guide, tb)) {
                p.x += (pa.x - qa.x) + ((pb.x - qb.x) - (pa.x - qa.x)) * t;
                p.y += (pa.y - qa.y) + ((pb.y - qb.y) - (pa.y - qa.y)) * t;
            }
        }
        if kf.orient_to_path {
            let dir = |u: f32| {
                let (p0, p1) = (crate::deform::point_at(&guide, (u - 0.002).max(0.0)), crate::deform::point_at(&guide, (u + 0.002).min(1.0)));
                match (p0, p1) {
                    (Some(p0), Some(p1)) => (p1.y - p0.y).atan2(p1.x - p0.x),
                    _ => 0.0,
                }
            };
            let da = dir(tt) - dir(ta);
            let cw = el.matrix.apply(c);
            el.matrix = Matrix::translate(cw.x, cw.y).concat(&Matrix::rotate(da)).concat(&Matrix::translate(-cw.x, -cw.y)).concat(&el.matrix);
        }
        let cur = el.matrix.apply(c);
        el.matrix.tx += p.x - cur.x;
        el.matrix.ty += p.y - cur.y;
    }
}

/// Camera view matrix of a timeline at `frame`, if it has a visible camera layer.
pub fn camera_view(doc: &Document, tl: &Timeline, frame: u32) -> Option<Matrix> {
    camera_element(doc, tl, frame).map(|c| camera_matrix_to_view(doc, &c.matrix))
}

/// The camera object at `frame` (tweened), with follow and shake applied:
/// its matrix is the framing, its colour effect / filters apply to the view.
pub fn camera_element(doc: &Document, tl: &Timeline, frame: u32) -> Option<Element> {
    let layer = tl.layers.iter().find(|l| l.kind == LayerKind::Camera && l.visible)?;
    let (els, _) = layer_elements_at(layer, frame)?;
    let mut cam = els.first()?.clone();
    let (_, kf, _) = layer.span_at(frame)?;
    if let Some(o) = kf.camera.as_deref() {
        if let Some(name) = o.follow.as_deref().filter(|n| !n.is_empty()) {
            // centre on the object, averaged over the lag window
            let lag = o.follow_lag.min(24);
            let pts: Vec<Pt> = (0..=lag).filter_map(|k| frame.checked_sub(k)).filter_map(|f| named_center(doc, tl, f, name)).collect();
            if !pts.is_empty() {
                let n = pts.len() as f32;
                let c = pt(pts.iter().map(|p| p.x).sum::<f32>() / n, pts.iter().map(|p| p.y).sum::<f32>() / n);
                cam.matrix.tx = c.x + o.follow_offset.x;
                cam.matrix.ty = c.y + o.follow_offset.y;
            }
        }
        if o.shake > 0.0 {
            let t = frame as f32 * o.shake_speed.max(0.01) / doc.fps.max(1.0);
            cam.matrix.tx += o.shake * smooth_noise(t, 17.3);
            cam.matrix.ty += o.shake * smooth_noise(t, 91.7);
        }
    }
    Some(cam)
}

/// Deterministic smooth noise in −1…1 (cosine-interpolated hashed values).
fn smooth_noise(t: f32, seed: f32) -> f32 {
    let h = |i: f32| {
        let x = ((i + seed) * 12.9898).sin() * 43758.547;
        (x - x.floor()) * 2.0 - 1.0
    };
    let i = t.floor();
    let u = t - i;
    let w = (1.0 - (u * std::f32::consts::PI).cos()) / 2.0;
    h(i) * (1.0 - w) + h(i + 1.0) * w
}

/// Stage-space centre of the top-level object named `name` at `frame`
/// (its drawn bounds, so tweens, parenting and motion paths count).
pub fn named_center(doc: &Document, tl: &Timeline, frame: u32, name: &str) -> Option<Pt> {
    let (li, ei) = tl.layers.iter().enumerate().filter(|(_, l)| l.kind != LayerKind::Camera).find_map(|(li, l)| {
        let (els, _) = layer_elements_at(l, frame)?;
        els.iter().position(|e| e.name == name).map(|ei| (li, ei))
    })?;
    let ctx = EvalCtx::new(doc, frame);
    let mut items = vec![];
    evaluate(&ctx, tl, frame, &Matrix::IDENTITY, 1.0, 0, &mut items);
    let mut b: Option<(Pt, Pt)> = None;
    for it in items.iter().filter(|it| it.src == Some((li, ei))) {
        if let Some((lo, hi)) = it.path.transformed(&it.matrix).bounds() {
            b = Some(match b {
                None => (lo, hi),
                Some((a, c)) => (pt(a.x.min(lo.x), a.y.min(lo.y)), pt(c.x.max(hi.x), c.y.max(hi.y))),
            });
        }
    }
    b.map(|(lo, hi)| pt((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0))
}

/// The camera's colour effect and filters apply to everything it sees.
pub fn apply_camera_effects(items: &mut [DrawItem], cam: &Element) {
    let fx = (!cam.filters.is_empty() || cam.blend != BlendMode::Normal).then(|| Arc::new(FxGroup::new(cam.filters.clone(), cam.blend)));
    for it in items {
        it.alpha *= cam.alpha;
        if it.tint.is_none() {
            it.tint = cam.tint;
        }
        if cam.color_fx.is_some() {
            it.cx = ColorXform::compose(cam.color_fx, it.cx);
        }
        if let Some(f) = &fx {
            it.fx.insert(0, f.clone());
        }
    }
}

/// The camera element's matrix is translate(centre)·rotate(r)·scale(zoom);
/// the view maps that framing onto the stage.
pub fn camera_matrix_to_view(doc: &Document, cam: &Matrix) -> Matrix {
    let d = cam.decompose();
    let zoom = d.scale_x.abs().max(0.01);
    Matrix::translate(doc.width / 2.0, doc.height / 2.0)
        .concat(&Matrix::scale(zoom, zoom))
        .concat(&Matrix::rotate(-d.skew_y))
        .concat(&Matrix::translate(-cam.tx, -cam.ty))
}

/// Default camera: centred on the stage, zoom 100%, no rotation.
pub fn default_camera(doc: &Document) -> Element {
    let mut e = Element::shape("Camera", Shape::default());
    e.matrix = Matrix::translate(doc.width / 2.0, doc.height / 2.0);
    e
}

pub fn apply_view(items: &mut [DrawItem], view: &Matrix) {
    let mut done: HashMap<*const Clip, Arc<Clip>> = HashMap::new();
    let mut moved: HashMap<*const FxGroup, Arc<FxGroup>> = HashMap::new();
    let inv = view.invert();
    for it in items {
        it.matrix = view.concat(&it.matrix);
        // Motion-blur deltas live in the items' space: carry them into the view.
        for g in &mut it.fx {
            let has_motion = g.motion.is_some() && inv.is_some();
            if has_motion || g.mesh.is_some() {
                let k = Arc::as_ptr(g);
                let n = moved
                    .entry(k)
                    .or_insert_with(|| {
                        let mut ng = (**g).clone();
                        if let (Some(m), Some(inv)) = (g.motion, inv) {
                            ng.motion = Some(view.concat(&m).concat(&inv));
                        }
                        if let Some(mesh) = &g.mesh {
                            ng.mesh = Some(Arc::new(mesh.transformed(view)));
                        }
                        Arc::new(ng)
                    })
                    .clone();
                *g = n;
            }
        }
        for c in &mut it.clips {
            let k = Arc::as_ptr(c);
            let n = done.entry(k).or_insert_with(|| Arc::new(Clip { shapes: c.shapes.iter().map(|(p, m)| (p.clone(), view.concat(m))).collect() })).clone();
            *c = n;
        }
    }
}

/// The whole movie at `frame`: scenes play one after another.
pub fn evaluate_scene(doc: &Document, frame: u32) -> Vec<DrawItem> {
    evaluate_scene_clock(doc, frame, frame)
}

/// Like `evaluate_scene`, with movie clips at their own clock `global`
/// (they keep playing while a script holds the main timeline still).
pub fn evaluate_scene_clock(doc: &Document, frame: u32, global: u32) -> Vec<DrawItem> {
    let (si, local) = doc.locate_frame(frame);
    let scene = doc.scene_at(si).unwrap_or(&doc.scene);
    let ctx = EvalCtx::new(doc, global);
    let mut out = Vec::new();
    evaluate(&ctx, scene, local, &Matrix::IDENTITY, 1.0, 0, &mut out);
    if let Some(cam) = camera_element(doc, scene, local) {
        apply_view(&mut out, &camera_matrix_to_view(doc, &cam.matrix));
        apply_camera_effects(&mut out, &cam);
    }
    out
}
