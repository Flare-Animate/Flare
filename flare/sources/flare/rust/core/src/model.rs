//! Document model mirroring Animate's DOM: Document → Timeline → Layer →
//! Keyframe (DOMFrame) → Element (shape or symbol instance), plus a Library
//! of symbols that each own a timeline.

pub use crate::bitmap::{BitmapAsset, BitmapId, Image};
use crate::deform::{project_path, Perspective, Transform3D, Warp};
pub use crate::fx::{BevelType, BlendMode, Filter, FxGroup};
use crate::geom::{morph_paths, pt, Matrix, Path, Pt};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

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
            8 => Some(Color {
                r: (v >> 24) as u8,
                g: (v >> 16) as u8,
                b: (v >> 8) as u8,
                a: v as u8,
            }),
            _ => None,
        }
    }
    pub fn to_hex(self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }
    pub fn lerp(self, o: Color, t: f32) -> Color {
        let l = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
        Color {
            r: l(self.r, o.r),
            g: l(self.g, o.g),
            b: l(self.b, o.b),
            a: l(self.a, o.a),
        }
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
    Rect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        radius: [f32; 4],
    },
    /// Angles in degrees (start = end: full oval); inner radius in % (0 = solid).
    Oval {
        cx: f32,
        cy: f32,
        rx: f32,
        ry: f32,
        start: f32,
        end: f32,
        inner: f32,
        close: bool,
    },
}

impl Primitive {
    pub fn path(&self) -> Path {
        match *self {
            Primitive::Rect { x, y, w, h, radius } => Path::rounded_rect4(x, y, w, h, radius),
            Primitive::Oval {
                cx,
                cy,
                rx,
                ry,
                start,
                end,
                inner,
                close,
            } => Path::oval_arc(cx, cy, rx, ry, start, end, inner, close),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Dash {
    #[default]
    Solid,
    Dashed,
    Dotted,
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
}
fn default_dash() -> f32 {
    3.0
}

impl Default for StrokeStyle {
    fn default() -> Self {
        StrokeStyle {
            dash: Dash::Solid,
            cap: Cap::Round,
            join: Join::Round,
            dash_len: 3.0,
            gap: 3.0,
        }
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
        Shape {
            path,
            fill,
            stroke,
            ..Default::default()
        }
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
        Gradient {
            kind,
            stops,
            matrix: Matrix::translate(c.x, c.y).concat(&s),
        }
    }
    pub fn lerp(&self, o: &Gradient, t: f32) -> Gradient {
        if self.kind != o.kind || self.stops.len() != o.stops.len() {
            return if t < 0.5 { self.clone() } else { o.clone() };
        }
        Gradient {
            kind: self.kind,
            stops: self
                .stops
                .iter()
                .zip(&o.stops)
                .map(|(a, b)| (a.0 + (b.0 - a.0) * t, a.1.lerp(b.1, t)))
                .collect(),
            matrix: self.matrix.interpolate(&o.matrix, t, 0),
        }
    }
}

pub type SymbolId = u32;

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
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Instance {
    pub symbol: SymbolId,
    pub loop_mode: LoopMode,
    pub first_frame: u32,
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
}

impl TextEl {
    pub fn outline(&self) -> Path {
        crate::text::text_path_family(
            &self.text,
            self.size,
            self.family.as_deref(),
            self.font,
            self.align,
            self.letter_spacing,
        )
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
    pub const IDENTITY: ColorXform = ColorXform {
        mul: [1.0; 4],
        add: [0.0; 4],
    };
    pub fn apply(&self, c: Color) -> Color {
        let ch = |v: u8, i: usize| {
            (v as f32 * self.mul[i] + self.add[i])
                .round()
                .clamp(0.0, 255.0) as u8
        };
        Color {
            r: ch(c.r, 0),
            g: ch(c.g, 1),
            b: ch(c.b, 2),
            a: ch(c.a, 3),
        }
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
        Element {
            name: name.into(),
            matrix,
            alpha: 1.0,
            tint: None,
            kind,
            three_d: None,
            filters: vec![],
            blend: Default::default(),
            color_fx: None,
        }
    }
    pub fn group(children: Vec<Element>) -> Self {
        Element::base("", Matrix::IDENTITY, ElementKind::Group(children))
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
        Element::base(
            "Text",
            Matrix::translate(at.x, at.y),
            ElementKind::Text(TextEl {
                text: text.into(),
                size,
                color,
                font: Default::default(),
                align: Default::default(),
                letter_spacing: 0.0,
                family: None,
            }),
        )
    }
    pub fn instance(name: &str, symbol: SymbolId, matrix: Matrix) -> Self {
        Element::base(
            name,
            matrix,
            ElementKind::Instance(Instance {
                symbol,
                loop_mode: LoopMode::Loop,
                first_frame: 0,
            }),
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Tween {
    None,
    /// Matrix + color-effect interpolation (Animate "Classic Tween").
    Classic {
        ease: f32,
        rotate_turns: i32,
    },
    /// Path + color morph (Animate "Shape Tween").
    Shape {
        ease: f32,
    },
    /// Property keyframes on a span (Animate "Motion Tween"); the keys live
    /// in `Keyframe::motion`, the span needs no closing keyframe.
    Motion {
        ease: f32,
    },
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
    let curved = if e >= 0.0 {
        1.0 - (1.0 - t) * (1.0 - t)
    } else {
        t * t
    };
    t + (curved - t) * e.abs()
}

/// An authored curve retained across clipping, splitting and reversal. Elements
/// on the owning key remain the live authored source, not a hidden snapshot.
/// The half-open time domain is independent of the visible timeline duration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TweenBoundary {
    pub duration: u32,
    #[serde(default, skip_serializing_if = "is_zero_time")]
    pub time_start: f32,
    pub time_end: f32,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub reversed: bool,
    pub elements: Vec<Element>,
    pub shape_hints: Vec<Pt>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pose_edits: Vec<crate::tween_edit::TweenPoseEdit>,
}

fn is_zero_time(value: &f32) -> bool {
    *value == 0.0
}

impl TweenBoundary {
    pub fn source_time(&self, local: u32, span_len: u32) -> f32 {
        let index = if self.reversed {
            span_len.saturating_sub(1).saturating_sub(local)
        } else {
            local
        };
        self.time_start
            + index as f32 * ((self.time_end - self.time_start) / span_len.max(1) as f32)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Keyframe {
    pub elements: Vec<Element>,
    pub tween: Tween,
    pub label: Option<String>,
    /// Graphic-symbol clock at this keyframe. Splitting or sampling a span must
    /// preserve its playback position independently of the tween's local time.
    #[serde(default, skip_serializing_if = "is_zero_frame")]
    pub playback_offset: u32,
    /// A reversed static exposure runs graphic symbols backwards as well.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub playback_reversed: bool,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tween_boundary: Option<Box<TweenBoundary>>,
}

impl Keyframe {
    pub fn playback_frame(&self, local: u32) -> u32 {
        if self.playback_reversed {
            self.playback_offset.saturating_sub(local)
        } else {
            self.playback_offset.saturating_add(local)
        }
    }

    pub fn blank() -> Self {
        Keyframe::with(vec![])
    }
    pub fn with(elements: Vec<Element>) -> Self {
        Keyframe {
            elements,
            tween: Tween::None,
            label: None,
            playback_offset: 0,
            playback_reversed: false,
            script: None,
            orient_to_path: false,
            ease_curve: None,
            shape_hints: vec![],
            motion: None,
            tween_boundary: None,
        }
    }
}

fn is_zero_frame(frame: &u32) -> bool {
    *frame == 0
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
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub name: String,
    pub color: Color,
    pub visible: bool,
    pub locked: bool,
    pub outline: bool,
    /// Timeline display height, as a percentage of the standard row. This travels
    /// with the layer through history, clipboard and project serialization.
    #[serde(default = "default_row_height")]
    pub row_height: u16,
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
}

const LAYER_COLORS: [Color; 6] = [
    Color::rgb(0x4F, 0xFF, 0x4F),
    Color::rgb(0xFF, 0x4F, 0xFF),
    Color::rgb(0x4F, 0xFF, 0xFF),
    Color::rgb(0xFF, 0x80, 0x00),
    Color::rgb(0x99, 0x66, 0xFF),
    Color::rgb(0xFF, 0xFF, 0x4F),
];

fn default_row_height() -> u16 {
    100
}

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
            row_height: 100,
            kind: LayerKind::Normal,
            keyframes,
            end: 1,
            bones: vec![],
            parent: None,
            collapsed: false,
            anim_parent: None,
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
        self.keyframes
            .range(start + 1..)
            .next()
            .map(|(s, _)| *s)
            .unwrap_or(self.end)
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
        Timeline {
            name: name.into(),
            layers: vec![Layer::new("Layer 1", 0)],
        }
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
        }
    }
}

impl Document {
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
        let mut tls: Vec<&mut Timeline> = std::iter::once(&mut d.scene)
            .chain(d.more_scenes.iter_mut())
            .collect();
        tls.extend(d.library.iter_mut().map(|s| &mut s.timeline));
        for t in tls {
            for l in &mut t.layers {
                for kf in l.keyframes.values_mut() {
                    fix(&mut kf.elements);
                    if let Some(boundary) = &mut kf.tween_boundary {
                        fix(&mut boundary.elements);
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
    pub fn next_bitmap_id(&self) -> crate::bitmap::BitmapId {
        self.bitmaps.iter().map(|b| b.id).max().unwrap_or(0) + 1
    }
    pub fn symbol_mut(&mut self, id: SymbolId) -> Option<&mut Symbol> {
        self.library.iter_mut().find(|s| s.id == id)
    }
    pub fn add_symbol(&mut self, name: &str, kind: SymbolKind, timeline: Timeline) -> SymbolId {
        let id = self.next_symbol_id;
        self.next_symbol_id += 1;
        self.library.push(Symbol {
            id,
            name: name.into(),
            kind,
            timeline,
        });
        id
    }
    pub fn perspective(&self) -> Perspective {
        Perspective {
            angle_deg: self.perspective_angle,
            vanishing: self
                .vanishing_point
                .unwrap_or(pt(self.width / 2.0, self.height / 2.0)),
            stage_width: self.width,
        }
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
        DrawItem {
            path,
            fill: None,
            stroke: None,
            matrix,
            alpha: 1.0,
            tint: None,
            outline: None,
            src: None,
            gradient: None,
            fx: vec![],
            clips: vec![],
            image: None,
            cx: None,
            stroke_style: StrokeStyle {
                dash: Dash::Solid,
                cap: Cap::Round,
                join: Join::Round,
                dash_len: 3.0,
                gap: 3.0,
            },
            pattern: None,
        }
    }
}

impl DrawItem {
    pub fn hit(&self, p: Pt) -> bool {
        let Some(inv) = self.matrix.invert() else {
            return false;
        };
        let lp = inv.apply(p);
        let scale = self.matrix.decompose().scale_x.abs().max(0.01);
        let tol = self.stroke.map(|s| s.width / 2.0).unwrap_or(0.0) + 3.0 / scale;
        if !self.clips.is_empty()
            && !self.clips.iter().all(|c| {
                c.shapes.iter().any(|(sp, m)| {
                    m.invert()
                        .map(|i| sp.hit(i.apply(p), true, 0.0))
                        .unwrap_or(false)
                })
            })
        {
            return false; // masked out here
        }
        self.path.hit(
            lp,
            self.fill.is_some() || self.gradient.is_some() || self.image.is_some(),
            tol,
        )
    }
}

/// Elements of a layer at `frame`, with tweens resolved.
/// Returns (elements, graphic-symbol playback frame). Tween evaluation uses the
/// timeline-local frame; graphic playback keeps its phase across span edits.
pub fn layer_elements_at(layer: &Layer, frame: u32) -> Option<(Vec<Element>, u32)> {
    let (start, kf, next) = layer.span_at(frame)?;
    let local = frame - start;
    let span_len = next.unwrap_or(layer.end).min(layer.end) - start;
    let tweened = if let Some(boundary) = kf
        .tween_boundary
        .as_ref()
        .filter(|_| kf.tween != Tween::None)
    {
        let source_time = boundary.source_time(local, span_len);
        match kf.tween {
            Tween::Motion { ease } => {
                Some(motion_elements(kf, source_time, boundary.duration, ease))
            }
            tween if source_time > 0.0 => {
                let t = crate::motion::tween_progress(
                    source_time / boundary.duration.max(1) as f32,
                    tween.ease(),
                    kf.ease_curve.as_ref(),
                );
                let n = kf.shape_hints.len().min(boundary.shape_hints.len());
                let hints = (n > 0).then(|| (&kf.shape_hints[..n], &boundary.shape_hints[..n]));
                Some(interpolate_elements(
                    &kf.elements,
                    &boundary.elements,
                    t,
                    tween,
                    hints,
                ))
            }
            _ => None,
        }
    } else {
        match (kf.tween, next) {
            (Tween::Motion { ease }, _) => {
                let len = next.unwrap_or(layer.end).min(layer.end) - start;
                Some(motion_elements(kf, local as f32, len, ease))
            }
            (Tween::None, _) | (_, None) => None,
            (tween, Some(next_start)) if frame > start => {
                let to = &layer.keyframes[&next_start];
                let t = crate::motion::tween_progress(
                    local as f32 / (next_start - start) as f32,
                    tween.ease(),
                    kf.ease_curve.as_ref(),
                );
                let n = kf.shape_hints.len().min(to.shape_hints.len());
                let hints = (n > 0).then(|| (&kf.shape_hints[..n], &to.shape_hints[..n]));
                Some(interpolate_elements(
                    &kf.elements,
                    &to.elements,
                    t,
                    tween,
                    hints,
                ))
            }
            _ => None,
        }
    };
    let mut els = tweened.unwrap_or_else(|| kf.elements.clone());
    if let Some(boundary) = &kf.tween_boundary {
        crate::tween_edit::apply(
            &boundary.pose_edits,
            boundary.source_time(local, span_len),
            &mut els,
        );
    }
    if !layer.bones.is_empty() {
        apply_bones(&layer.bones, &mut els);
    }
    Some((els, kf.playback_frame(local)))
}

/// The motion tween's target (the first element) posed at `local` frame.
pub fn motion_elements(kf: &Keyframe, local: f32, len: u32, simple: f32) -> Vec<Element> {
    let mut els = kf.elements.clone();
    let (Some(m), Some(el)) = (&kf.motion, els.first_mut()) else {
        return els;
    };
    let base = crate::motion::Pose::of(el, m.pivot);
    let mut pose = m.pose_at(&base, local, len, simple, kf.ease_curve.as_ref());
    if kf.orient_to_path {
        if let (Some(a), Some(a0)) = (m.path_angle(local), m.path_angle(0.0)) {
            pose.rotation += a - a0;
        }
    }
    pose.apply(el, m.pivot);
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

fn interpolate_elements(
    from: &[Element],
    to: &[Element],
    t: f32,
    tween: Tween,
    hints: Option<(&[Pt], &[Pt])>,
) -> Vec<Element> {
    from.iter()
        .enumerate()
        .map(|(i, a)| {
            let Some(b) = to.get(i) else { return a.clone() };
            let turns = if let Tween::Classic { rotate_turns, .. } = tween {
                rotate_turns
            } else {
                0
            };
            let mut out = a.clone();
            out.matrix = a.matrix.interpolate(&b.matrix, t, turns);
            out.alpha = a.alpha + (b.alpha - a.alpha) * t;
            out.tint = match (a.tint, b.tint) {
                (Some((ca, ta)), Some((cb, tb))) => Some((ca.lerp(cb, t), ta + (tb - ta) * t)),
                (Some((c, ta)), None) => Some((c, ta * (1.0 - t))),
                (None, Some((c, tb))) => Some((c, tb * t)),
                (None, None) => None,
            };
            out.three_d = Transform3D::lerp(a.three_d, b.three_d, t);
            out.color_fx = ColorXform::lerp(a.color_fx, b.color_fx, t);
            out.filters = Filter::lerp_list(&a.filters, &b.filters, t);
            // shape armatures: poses tween with classic and shape tweens
            if let (ElementKind::Shape(sa), ElementKind::Shape(sb)) = (&a.kind, &b.kind) {
                if let (Some(ra), Some(rb), ElementKind::Shape(so)) =
                    (&sa.rig, &sb.rig, &mut out.kind)
                {
                    so.rig = Some(crate::rig::ShapeRig::lerp(ra, rb, t));
                }
            }
            if let (Tween::Shape { .. }, ElementKind::Canvas(ca), ElementKind::Canvas(cb)) =
                (tween, &a.kind, &b.kind)
            {
                out.kind = ElementKind::Canvas(crate::merge::morph(ca, cb, t));
            }
            if let (Tween::Shape { .. }, ElementKind::Shape(sa), ElementKind::Shape(sb)) =
                (tween, &a.kind, &b.kind)
            {
                out.kind = ElementKind::Shape(Shape {
                    gradient: match (&sa.gradient, &sb.gradient) {
                        (Some(x), Some(y)) => Some(x.lerp(y, t)),
                        (x, y) => {
                            if t < 0.5 {
                                x.clone()
                            } else {
                                y.clone()
                            }
                        }
                    },
                    width_profile: if t < 0.5 {
                        sa.width_profile.clone()
                    } else {
                        sb.width_profile.clone()
                    },
                    warp: None,
                    primitive: None,
                    stroke_style: if t < 0.5 {
                        sa.stroke_style
                    } else {
                        sb.stroke_style
                    },
                    bitmap_fill: if t < 0.5 {
                        sa.bitmap_fill
                    } else {
                        sb.bitmap_fill
                    },
                    rig: match (&sa.rig, &sb.rig) {
                        (Some(x), Some(y)) => Some(crate::rig::ShapeRig::lerp(x, y, t)),
                        _ => None,
                    },
                    path: match hints.filter(|_| i == 0) {
                        Some((ha, hb)) => {
                            crate::geom::morph_paths_hinted(&sa.path, &sb.path, t, ha, hb)
                        }
                        None => morph_paths(&sa.path, &sb.path, t),
                    },
                    fill: match (sa.fill, sb.fill) {
                        (Some(x), Some(y)) => Some(x.lerp(y, t)),
                        (x, y) => {
                            if t < 0.5 {
                                x
                            } else {
                                y
                            }
                        }
                    },
                    stroke: match (sa.stroke, sb.stroke) {
                        (Some(x), Some(y)) => Some(Stroke {
                            color: x.color.lerp(y.color, t),
                            width: x.width + (y.width - x.width) * t,
                        }),
                        (x, y) => {
                            if t < 0.5 {
                                x
                            } else {
                                y
                            }
                        }
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
        EvalCtx {
            doc,
            global_frame,
            button: None,
            top: Default::default(),
        }
    }
}

fn shape_items(
    s: &Shape,
    m: Matrix,
    a: f32,
    tint: Option<(Color, f32)>,
    outline: Option<Color>,
    out: &mut Vec<DrawItem>,
) {
    let item = |path: Path,
                fill: Option<Color>,
                stroke: Option<Stroke>,
                gradient: Option<Gradient>| DrawItem {
        fill,
        stroke,
        alpha: a,
        tint,
        outline,
        gradient,
        ..DrawItem::new(path, m)
    };
    match (&s.width_profile, s.stroke) {
        // Variable-width stroke: drawn as a filled outline.
        (Some(profile), Some(st)) if !profile.is_empty() => {
            out.push(item(s.path.clone(), s.fill, None, s.gradient.clone()));
            let scaled: Vec<(f32, f32)> = profile.iter().map(|(t, w)| (*t, w * st.width)).collect();
            out.push(item(
                crate::deform::variable_stroke(&s.path, &scaled),
                Some(st.color),
                None,
                None,
            ));
        }
        _ => out.push(item(s.path.clone(), s.fill, s.stroke, s.gradient.clone())),
    }
}

/// Is a layer shown, taking hidden folders above it into account?
pub fn layer_shown(tl: &Timeline, li: usize) -> bool {
    let mut cur = Some(li);
    let mut guard = 0;
    while let Some(i) = cur {
        let Some(l) = tl.layers.get(i) else {
            return true;
        };
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
pub fn evaluate(
    ctx: &EvalCtx,
    tl: &Timeline,
    frame: u32,
    parent: &Matrix,
    alpha: f32,
    depth: u32,
    out: &mut Vec<DrawItem>,
) {
    if depth > 16 {
        return; // recursive symbol guard
    }
    let mut masks: HashMap<usize, Option<Arc<Clip>>> = HashMap::new();
    for (li, layer) in tl.layers.iter().enumerate().rev() {
        if layer.kind != LayerKind::Normal || !layer_shown(tl, li) {
            continue;
        }
        let Some((mut elements, local)) = layer_elements_at(layer, frame) else {
            continue;
        };
        if layer.anim_parent.is_some() {
            let pm = layer_parent_matrix(tl, li, frame);
            for el in &mut elements {
                el.matrix = pm.concat(&el.matrix);
            }
        }
        if let Some(g) = layer_effect_parent(tl, li, LayerKind::Guide) {
            apply_motion_guide(ctx, tl, li, g, frame, &mut elements);
        }
        let outline = layer.outline.then_some(layer.color);
        let layer_first = out.len();
        for (ei, el) in elements.iter().enumerate() {
            let first = out.len();
            if depth == 0 {
                ctx.top.set(Some((li, ei)));
            }
            eval_element(ctx, el, parent, alpha, local, outline, depth, out);
            if depth == 0 {
                for it in &mut out[first..] {
                    it.src = Some((li, ei));
                }
            }
        }
        if let Some(m) =
            layer_effect_parent(tl, li, LayerKind::Mask).filter(|m| tl.layers[*m].visible)
        {
            let clip = masks
                .entry(m)
                .or_insert_with(|| mask_clip(ctx, tl, m, frame, parent, depth))
                .clone();
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

/// Layer parenting: the transform that layer `li`'s objects inherit at
/// `frame` (the parent layer's first object, through its own parents).
pub fn layer_parent_matrix(tl: &Timeline, li: usize, frame: u32) -> Matrix {
    let mut m = Matrix::IDENTITY;
    let mut cur = tl.layers.get(li).and_then(|l| l.anim_parent);
    let mut guard = 0;
    while let (Some(p), true) = (cur, guard < 32) {
        let Some(pl) = tl.layers.get(p) else { break };
        let pm = layer_elements_at(pl, frame)
            .and_then(|(els, _)| els.first().map(|e| e.matrix))
            .unwrap_or(Matrix::IDENTITY);
        m = pm.concat(&m);
        cur = pl.anim_parent;
        guard += 1;
    }
    m
}

/// Shapes on mask layer `m` at `frame`, as a clip.
fn mask_clip(
    ctx: &EvalCtx,
    tl: &Timeline,
    m: usize,
    frame: u32,
    parent: &Matrix,
    depth: u32,
) -> Option<Arc<Clip>> {
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
pub fn eval_element(
    ctx: &EvalCtx,
    el: &Element,
    parent: &Matrix,
    alpha: f32,
    local: u32,
    outline: Option<Color>,
    depth: u32,
    out: &mut Vec<DrawItem>,
) {
    let m = parent.concat(&el.matrix);
    let a = alpha * el.alpha;
    let first = out.len();
    match &el.kind {
        ElementKind::Shape(s) => {
            let posed;
            let s = match &s.rig {
                Some(r) if !r.bones.is_empty() => {
                    posed = Shape {
                        path: r.deform(&s.path),
                        ..s.clone()
                    };
                    &posed
                }
                _ => s,
            };
            shape_items(s, m, a, el.tint, outline, out);
            let pattern = s.bitmap_fill.and_then(|b| {
                ctx.doc
                    .bitmap(b.bitmap)
                    .and_then(|asset| asset.image())
                    .map(|img| (img, b.matrix))
            });
            for it in &mut out[first..] {
                it.stroke_style = s.stroke_style;
                if it.fill.is_some() || it.gradient.is_some() {
                    it.pattern = pattern.clone();
                }
            }
        }
        ElementKind::Text(t) => out.push(DrawItem {
            fill: Some(t.color),
            alpha: a,
            tint: el.tint,
            outline,
            ..DrawItem::new(t.outline(), m)
        }),
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
                    out.push(DrawItem {
                        fill: r.fill,
                        gradient: r.gradient.clone(),
                        alpha: a,
                        tint: el.tint,
                        outline,
                        ..DrawItem::new(r.path(), m)
                    });
                }
                for st in &part.strokes {
                    out.push(DrawItem {
                        stroke: Some(st.stroke),
                        alpha: a,
                        tint: el.tint,
                        outline,
                        ..DrawItem::new(st.path(), m)
                    });
                }
            }
        }
        ElementKind::Bitmap(b) => {
            if let Some(asset) = ctx.doc.bitmap(b.bitmap) {
                let path = Path::rect(0.0, 0.0, asset.width as f32, asset.height as f32);
                let image = asset.image();
                // outline mode / missing pixels: draw the frame only
                let fill = image.is_none().then_some(Color::rgb(200, 200, 200));
                out.push(DrawItem {
                    fill,
                    alpha: a,
                    tint: el.tint,
                    outline,
                    image,
                    ..DrawItem::new(path, m)
                });
            }
        }
        ElementKind::Instance(inst) => {
            let Some(sym) = ctx.doc.symbol(inst.symbol) else {
                return;
            };
            let dur = sym.timeline.frame_count();
            let f = match sym.kind {
                SymbolKind::MovieClip => ctx.global_frame % dur,
                // Up, or Over/Down while live; the Hit frame (4th) never shows
                SymbolKind::Button => match ctx.button {
                    Some((src, state)) if ctx.top.get() == Some(src) => {
                        state.min(2).min(dur.saturating_sub(1))
                    }
                    _ => 0,
                },
                SymbolKind::Graphic => {
                    let raw = u64::from(inst.first_frame) + u64::from(local);
                    match inst.loop_mode {
                        LoopMode::Loop => (raw % u64::from(dur)) as u32,
                        LoopMode::PlayOnce => raw.min(u64::from(dur - 1)) as u32,
                        LoopMode::SingleFrame => inst.first_frame.min(dur - 1),
                    }
                }
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
    // Filters and blend modes: everything this element drew is one layer.
    if (!el.filters.is_empty() || el.blend != BlendMode::Normal) && outline.is_none() {
        let fx = Arc::new(FxGroup {
            filters: el.filters.clone(),
            blend: el.blend,
        });
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
        .reduce(|a, b| {
            (
                pt(a.0.x.min(b.0.x), a.0.y.min(b.0.y)),
                pt(a.1.x.max(b.1.x), a.1.y.max(b.1.y)),
            )
        })
        .map(|(lo, hi)| pt((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0))
        .unwrap_or_default()
}

/// Classic motion guide: during a classic tween, elements on a layer under a
/// guide layer travel along the guide's first shape (Animate snaps the
/// transformation point to the path at both keyframes).
fn apply_motion_guide(
    ctx: &EvalCtx,
    tl: &Timeline,
    li: usize,
    g: usize,
    frame: u32,
    els: &mut [Element],
) {
    let layer = &tl.layers[li];
    let Some((start, kf, next)) = layer.span_at(frame) else {
        return;
    };
    let Tween::Classic { ease, .. } = kf.tween else {
        return;
    };
    let Some((gels, _)) = layer_elements_at(&tl.layers[g], frame) else {
        return;
    };
    let Some(guide) = gels.iter().find_map(|e| match &e.kind {
        ElementKind::Shape(s) => Some(s.path.transformed(&e.matrix)),
        _ => None,
    }) else {
        return;
    };
    let (to_elements, t) = if let Some(boundary) = &kf.tween_boundary {
        let len = next.unwrap_or(layer.end).min(layer.end) - start;
        let source_time = boundary.source_time(frame - start, len);
        (
            &boundary.elements,
            apply_ease(source_time / boundary.duration.max(1) as f32, ease),
        )
    } else {
        let Some(next) = next else {
            return;
        };
        (
            &layer.keyframes[&next].elements,
            apply_ease((frame - start) as f32 / (next - start) as f32, ease),
        )
    };
    for (i, el) in els.iter_mut().enumerate() {
        let (Some(a), Some(b)) = (kf.elements.get(i), to_elements.get(i)) else {
            continue;
        };
        let c = element_local_center(ctx, a);
        let (pa, pb) = (a.matrix.apply(c), b.matrix.apply(c));
        let (Some((ta, _)), Some((tb, _))) = (
            crate::deform::nearest_t(&guide, pa),
            crate::deform::nearest_t(&guide, pb),
        ) else {
            continue;
        };
        let tt = ta + (tb - ta) * t;
        let Some(p) = crate::deform::point_at(&guide, tt) else {
            continue;
        };
        if kf.orient_to_path {
            let dir = |u: f32| {
                let (p0, p1) = (
                    crate::deform::point_at(&guide, (u - 0.002).max(0.0)),
                    crate::deform::point_at(&guide, (u + 0.002).min(1.0)),
                );
                match (p0, p1) {
                    (Some(p0), Some(p1)) => (p1.y - p0.y).atan2(p1.x - p0.x),
                    _ => 0.0,
                }
            };
            let da = dir(tt) - dir(ta);
            let cw = el.matrix.apply(c);
            el.matrix = Matrix::translate(cw.x, cw.y)
                .concat(&Matrix::rotate(da))
                .concat(&Matrix::translate(-cw.x, -cw.y))
                .concat(&el.matrix);
        }
        let cur = el.matrix.apply(c);
        el.matrix.tx += p.x - cur.x;
        el.matrix.ty += p.y - cur.y;
    }
}

/// Camera view matrix of a timeline at `frame`, if it has a visible camera layer.
pub fn camera_view(doc: &Document, tl: &Timeline, frame: u32) -> Option<Matrix> {
    let layer = tl
        .layers
        .iter()
        .find(|l| l.kind == LayerKind::Camera && l.visible)?;
    let (els, _) = layer_elements_at(layer, frame)?;
    let cam = els.first()?.matrix;
    Some(camera_matrix_to_view(doc, &cam))
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
    for it in items {
        it.matrix = view.concat(&it.matrix);
        for c in &mut it.clips {
            let k = Arc::as_ptr(c);
            let n = done
                .entry(k)
                .or_insert_with(|| {
                    Arc::new(Clip {
                        shapes: c
                            .shapes
                            .iter()
                            .map(|(p, m)| (p.clone(), view.concat(m)))
                            .collect(),
                    })
                })
                .clone();
            *c = n;
        }
    }
}

/// The whole movie at `frame`: scenes play one after another.
pub fn evaluate_scene(doc: &Document, frame: u32) -> Vec<DrawItem> {
    let (si, local) = doc.locate_frame(frame);
    let scene = doc.scene_at(si).unwrap_or(&doc.scene);
    let ctx = EvalCtx::new(doc, frame);
    let mut out = Vec::new();
    evaluate(&ctx, scene, local, &Matrix::IDENTITY, 1.0, 0, &mut out);
    if let Some(v) = camera_view(doc, scene, local) {
        apply_view(&mut out, &v);
    }
    out
}
