//! In-memory document model. Follows Animate's concepts (scenes, layers,
//! keyframes, symbols), but stores geometry in pixels, not XFL's twips.

use std::collections::HashMap;
use std::ops::Mul;

pub use crate::gradient::{Gradient, GradientKind, Spread};

pub type Point = (f32, f32);
/// Straight (not premultiplied) RGBA, each channel 0..=1.
pub type Rgba = [f32; 4];

#[derive(Clone, Debug, PartialEq)]
pub struct Document {
    pub width: f32,
    pub height: f32,
    pub frame_rate: f32,
    pub background: Rgba,
    /// Scenes, played one after another.
    pub scenes: Vec<Timeline>,
    /// Library symbols by name, including folders: `Folder/Name`.
    pub library: HashMap<String, Symbol>,
    pub extra: Extra,
}

impl Document {
    /// A new white document with one scene holding one empty layer, as Animate creates.
    pub fn new(width: f32, height: f32, frame_rate: f32) -> Self {
        Document {
            width,
            height,
            frame_rate,
            background: [1.0; 4],
            scenes: vec![Timeline::new("Scene 1")],
            library: HashMap::new(),
            extra: Extra::default(),
        }
    }

    /// Insert > Scene: a new empty scene after scene `after`, named "Scene N" as
    /// Animate names them. Returns where it went.
    pub fn insert_scene(&mut self, after: usize) -> usize {
        let taken = |name: &String| self.scenes.iter().any(|s| &s.name == name);
        let name = (1..).map(|n| format!("Scene {n}")).find(|n| !taken(n)).unwrap_or_default();
        let at = (after + 1).min(self.scenes.len());
        self.scenes.insert(at, Timeline::new(&name));
        at
    }
}

/// The parts of an XFL element the model doesn't read (layer colors, frame labels
/// and scripts, instance names, filters, the media list...), kept so that saving
/// writes them back.
// ponytail: kept as they were, so references in them can go stale when the model changes
// around them. Layer edits keep parentLayerIndex in step (edit.rs); model others as they bite.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Extra {
    /// (qualified name, value)
    pub attrs: Vec<(String, String)>,
    /// Child elements as XML text: those before the first child the model reads, then the rest.
    pub before: Vec<String>,
    pub after: Vec<String>,
}

impl Extra {
    pub fn get(&self, name: &str) -> Option<&str> {
        self.attrs.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
    }

    /// Sets attribute `name`, or removes it when `value` is None.
    pub fn set(&mut self, name: &str, value: Option<&str>) {
        match (self.attrs.iter_mut().find(|(k, _)| k == name), value) {
            (Some(slot), Some(v)) => slot.1 = v.to_string(),
            (None, Some(v)) => self.attrs.push((name.to_string(), v.to_string())),
            (_, None) => self.attrs.retain(|(k, _)| k != name),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Symbol {
    pub kind: SymbolKind,
    pub timeline: Timeline,
    pub extra: Extra,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SymbolKind {
    MovieClip,
    Graphic,
    Button,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Timeline {
    pub name: String,
    /// Top layer first, as in Animate's timeline panel.
    pub layers: Vec<Layer>,
    pub extra: Extra,
}

impl Timeline {
    /// One layer holding one blank keyframe, as Animate starts scenes and symbols.
    pub fn new(name: &str) -> Timeline {
        let frame = Frame { index: 0, duration: 1, tween: Tween::None, elements: Vec::new(), extra: Extra::default() };
        let layer = Layer { name: "Layer_1".into(), kind: LayerKind::Normal, frames: vec![frame], extra: Extra::default() };
        Timeline { name: name.into(), layers: vec![layer], extra: Extra::default() }
    }

    pub fn frame_count(&self) -> u32 {
        let ends = self.layers.iter().flat_map(|l| &l.frames).map(|f| f.index + f.duration);
        ends.max().unwrap_or(0)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    pub name: String,
    pub kind: LayerKind,
    /// Keyframes back to back from frame 0: each spans `duration` frames and
    /// starts where the previous one ends. `edit.rs` keeps it so.
    pub frames: Vec<Frame>,
    pub extra: Extra,
}

impl Layer {
    /// The keyframe whose span holds `frame`, as an index into `frames`.
    pub fn keyframe_at(&self, frame: u32) -> Option<usize> {
        self.frames.iter().position(|k| (k.index..k.index + k.duration).contains(&frame))
    }

    // The timeline's per-layer switches, kept as XFL's DOMLayer attributes.

    /// Shown on the stage while editing (a hidden layer still publishes, by default).
    pub fn visible(&self) -> bool {
        self.extra.get("visible") != Some("false")
    }

    pub fn locked(&self) -> bool {
        self.extra.get("locked") == Some("true")
    }

    /// Drawn as outlines in the layer's color while editing.
    pub fn outline(&self) -> bool {
        self.extra.get("outline") == Some("true")
    }

    pub fn set_visible(&mut self, on: bool) {
        self.extra.set("visible", (!on).then_some("false"));
    }

    pub fn set_locked(&mut self, on: bool) {
        self.extra.set("locked", on.then_some("true"));
    }

    pub fn set_outline(&mut self, on: bool) {
        self.extra.set("outline", on.then_some("true"));
    }

    /// The layer's outline color, if the document gives one.
    pub fn color(&self) -> Option<Rgba> {
        let v = u32::from_str_radix(self.extra.get("color")?.trim_start_matches('#'), 16).ok()?;
        let channel = |shift: u32| ((v >> shift) & 0xFF) as f32 / 255.0;
        Some([channel(16), channel(8), channel(0), 1.0])
    }

    pub fn set_color(&mut self, [r, g, b, _]: Rgba) {
        let byte = |c: f32| (c.clamp(0.0, 1.0) * 255.0).round() as u32;
        self.extra.set("color", Some(&format!("#{:02X}{:02X}{:02X}", byte(r), byte(g), byte(b))));
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LayerKind {
    Normal,
    Guide,
    Mask,
    Folder,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub index: u32,
    pub duration: u32,
    pub tween: Tween,
    pub elements: Vec<Element>,
    pub extra: Extra,
}

impl Frame {
    /// The keyframe's label and its type ("name", "comment" or "anchor"), kept in
    /// XFL's `name` and `labelType` attributes.
    pub fn label(&self) -> Option<(&str, &str)> {
        let name = self.extra.get("name").filter(|n| !n.is_empty())?;
        Some((name, self.extra.get("labelType").unwrap_or("name")))
    }

    /// Sets the label; an empty `name` removes it.
    pub fn set_label(&mut self, name: &str, kind: &str) {
        self.extra.attrs.retain(|(k, _)| k != "name" && k != "labelType");
        if !name.is_empty() {
            self.extra.attrs.push(("name".into(), name.into()));
            self.extra.attrs.push(("labelType".into(), kind.into()));
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Tween {
    None,
    /// Classic tween to the next keyframe. `ease` runs -1 (in) ..= 1 (out).
    Classic { ease: f32, rotate: Rotate },
    /// Motion tween: the span moves its one target by property keyframes (motion.rs).
    Motion(Box<crate::motion::Motion>),
    /// Shape tween to the next keyframe (shape_tween.rs), `ease` as a classic tween's.
    /// `angular` blends keeping corners and straight sides, else smoothly (Distributive).
    /// `hints` (a, b, c...) pin a point of this keyframe's outlines to one of the next's.
    Shape { ease: f32, angular: bool, hints: Vec<(Point, Point)> },
    /// Other tween kinds, not modelled; the frame's `extra` keeps their settings.
    Unsupported(String),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Rotate {
    Auto,
    Cw(u32),
    Ccw(u32),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Element {
    Shape(Shape),
    Instance(Instance),
    Group(Vec<Element>),
    Text(Text),
    /// Bitmaps, video and so on: the XFL element's name and its XML, saved back as is.
    Unsupported { name: String, xml: String },
}

/// A classic text field. `matrix` places the top left corner of its box; the
/// text sits inside the box's 2 px gutter (text.rs lays it out).
#[derive(Clone, Debug, PartialEq)]
pub struct Text {
    pub kind: TextKind,
    pub matrix: Matrix,
    /// Box width, gutters included: lines wrap to fit. None makes the box grow
    /// with the text, breaking lines only at newlines (Animate's extending field).
    pub width: Option<f32>,
    pub align: TextAlign,
    /// Extra space between lines, in pixels.
    pub line_spacing: f32,
    /// The characters, in stretches of one style. Newlines are '\n'.
    pub runs: Vec<TextRun>,
    pub extra: Extra,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextRun {
    pub text: String,
    /// As XFL names fonts: a PostScript name ("Arial-BoldMT") or a family ("Arial").
    pub face: String,
    /// In pixels (Animate's panels call them points).
    pub size: f32,
    pub color: Rgba,
    /// Asks for a bold or italic face on top of what `face` names.
    pub bold: bool,
    pub italic: bool,
    /// Extra space after each character, in pixels.
    pub letter_spacing: f32,
    /// What the model doesn't read of the run's `DOMTextAttrs` (kerning, margins, links...).
    pub extra: Extra,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextKind {
    Static,
    Dynamic,
    Input,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextAlign {
    Left,
    Center,
    Right,
    Justify,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Shape {
    pub matrix: Matrix,
    /// Edges refer to fills and strokes 1-based; 0 means none.
    pub fills: Vec<Paint>,
    pub strokes: Vec<Stroke>,
    pub edges: Vec<Edge>,
    pub extra: Extra,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Paint {
    Solid(Rgba),
    /// A linear or radial gradient (gradient.rs).
    Gradient(Gradient),
    /// Bitmap fills: the XFL element's name and its XML, saved back as is.
    Unsupported { name: String, xml: String },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Stroke {
    pub width: f32,
    pub paint: Paint,
    pub cap: Cap,
    pub join: Join,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cap {
    None,
    Round,
    Square,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Join {
    Miter,
    Round,
    Bevel,
}

/// Animate shapes are not closed paths: each edge names the fill on its left
/// (`fill0`) and right (`fill1`) side, and regions emerge from the edge graph.
#[derive(Clone, Debug, PartialEq)]
pub struct Edge {
    pub fill0: u32,
    pub fill1: u32,
    pub stroke: u32,
    pub paths: Vec<Path>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Path {
    pub start: Point,
    pub segs: Vec<Seg>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Seg {
    Line(Point),
    /// Control point, end point.
    Quad(Point, Point),
}

impl Seg {
    pub fn end(self) -> Point {
        match self {
            Seg::Line(p) | Seg::Quad(_, p) => p,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Instance {
    pub symbol: String,
    /// How this instance behaves; can differ from the library symbol's kind.
    pub kind: SymbolKind,
    pub matrix: Matrix,
    /// Transformation point in the symbol's coordinates; tweens move this point in a straight line.
    pub pivot: Point,
    pub color: ColorTransform,
    pub looping: Loop,
    pub first_frame: u32,
    pub extra: Extra,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Loop {
    Loop,
    PlayOnce,
    SingleFrame,
}

/// x' = a·x + c·y + tx, y' = b·x + d·y + ty
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Matrix {
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub tx: f32,
    pub ty: f32,
}

impl Matrix {
    pub const IDENTITY: Matrix = Matrix { a: 1.0, b: 0.0, c: 0.0, d: 1.0, tx: 0.0, ty: 0.0 };

    pub fn apply(&self, (x, y): Point) -> Point {
        (self.a * x + self.c * y + self.tx, self.b * x + self.d * y + self.ty)
    }
}

/// `parent * child`: the child's transform applies first.
impl Mul for Matrix {
    type Output = Matrix;

    fn mul(self, child: Matrix) -> Matrix {
        let (tx, ty) = self.apply((child.tx, child.ty));
        Matrix {
            a: self.a * child.a + self.c * child.b,
            b: self.b * child.a + self.d * child.b,
            c: self.a * child.c + self.c * child.d,
            d: self.b * child.c + self.d * child.d,
            tx,
            ty,
        }
    }
}

/// Per channel: c' = c·mult + add. Animate applies it to every color drawn,
/// not to the flattened group.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColorTransform {
    pub mult: Rgba,
    /// -1..=1 (XFL stores -255..=255).
    pub add: Rgba,
}

impl ColorTransform {
    pub const IDENTITY: ColorTransform = ColorTransform { mult: [1.0; 4], add: [0.0; 4] };

    pub fn apply(&self, c: Rgba) -> Rgba {
        std::array::from_fn(|i| (c[i] * self.mult[i] + self.add[i]).clamp(0.0, 1.0))
    }
}

/// `parent * child`: the child's transform applies first.
impl Mul for ColorTransform {
    type Output = ColorTransform;

    fn mul(self, child: ColorTransform) -> ColorTransform {
        ColorTransform {
            mult: std::array::from_fn(|i| child.mult[i] * self.mult[i]),
            add: std::array::from_fn(|i| child.add[i] * self.mult[i] + self.add[i]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scenes_go_in_after_the_current_one() {
        let mut doc = Document::new(550.0, 400.0, 24.0);
        assert_eq!(doc.insert_scene(0), 1);
        doc.scenes[0].name = "Intro".into();
        assert_eq!(doc.insert_scene(0), 1);
        assert_eq!(doc.insert_scene(9), 3);
        let names: Vec<_> = doc.scenes.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["Intro", "Scene 1", "Scene 2", "Scene 3"]);
        assert_eq!(doc.scenes[3], Timeline::new("Scene 3"));
    }
}
