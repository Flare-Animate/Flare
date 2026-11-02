//! Filters and the Display options (blend mode, visibility, bitmap caching) of
//! movie clips, buttons and text fields (docs/animate-ui.md §7.2.4). They stay in
//! the element's kept XFL, its `Extra`: the `<filters>` element and the
//! `blendMode`, `isVisible` and `cacheAsBitmap` attributes, so whatever the model
//! doesn't read of them saves back as it was. This reads and writes them, tweens
//! them for display.rs, draws them for render.rs and turns them into PlaceObject3
//! fields for swf_export.rs.

use std::array::from_fn;

use roxmltree::Node;
use tiny_skia::{self as sk, Pixmap};

use crate::doc::{ColorTransform, Extra, Rgba};
use crate::xfl::hex_color;
use crate::xfl_write::{escape, hex};

/// The filters of the Filters section's add (+) menu, in its order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterKind {
    DropShadow,
    Blur,
    Glow,
    Bevel,
    GradientGlow,
    GradientBevel,
    AdjustColor,
}

impl FilterKind {
    pub const ALL: [FilterKind; 7] = [
        FilterKind::DropShadow,
        FilterKind::Blur,
        FilterKind::Glow,
        FilterKind::Bevel,
        FilterKind::GradientGlow,
        FilterKind::GradientBevel,
        FilterKind::AdjustColor,
    ];

    /// As the add menu and the filter's card name it.
    pub fn name(self) -> &'static str {
        match self {
            FilterKind::DropShadow => "Drop Shadow",
            FilterKind::Blur => "Blur",
            FilterKind::Glow => "Glow",
            FilterKind::Bevel => "Bevel",
            FilterKind::GradientGlow => "Gradient Glow",
            FilterKind::GradientBevel => "Gradient Bevel",
            FilterKind::AdjustColor => "Adjust Color",
        }
    }

    fn tag(self) -> &'static str {
        match self {
            FilterKind::DropShadow => "DropShadowFilter",
            FilterKind::Blur => "BlurFilter",
            FilterKind::Glow => "GlowFilter",
            FilterKind::Bevel => "BevelFilter",
            FilterKind::GradientGlow => "GradientGlowFilter",
            FilterKind::GradientBevel => "GradientBevelFilter",
            FilterKind::AdjustColor => "AdjustColorFilter",
        }
    }

    /// The XFL attributes the model reads for this kind, besides `isEnabled`. The
    /// rest stay in `Filter::extra`.
    fn attrs(self) -> &'static [&'static str] {
        match self {
            FilterKind::DropShadow => {
                &["blurX", "blurY", "quality", "strength", "angle", "distance", "color", "alpha", "knockout", "inner", "hideObject"]
            }
            FilterKind::Blur => &["blurX", "blurY", "quality"],
            FilterKind::Glow => &["blurX", "blurY", "quality", "strength", "color", "alpha", "knockout", "inner"],
            FilterKind::Bevel => &[
                "blurX", "blurY", "quality", "strength", "angle", "distance", "highlightColor", "highlightAlpha", "shadowColor",
                "shadowAlpha", "knockout", "type",
            ],
            FilterKind::GradientGlow | FilterKind::GradientBevel => {
                &["blurX", "blurY", "quality", "strength", "angle", "distance", "knockout", "type"]
            }
            FilterKind::AdjustColor => &["brightness", "contrast", "saturation", "hue"],
        }
    }

    fn gradient(self) -> bool {
        matches!(self, FilterKind::GradientGlow | FilterKind::GradientBevel)
    }
}

/// Where a filter draws: inside the object, outside it, or both. Bevel and the
/// gradient filters call it Type; drop shadows and glows are inner or outer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Inner,
    Outer,
    Full,
}

/// One filter: every kind's settings in one record, as Animate's scripting has
/// it. Each kind reads the ones its card shows.
#[derive(Clone, Debug, PartialEq)]
pub struct Filter {
    pub kind: FilterKind,
    pub enabled: bool,
    /// Blur X and Y in pixels. Filters scale with the view, not with the object.
    pub blur: [f32; 2],
    /// 1 is 100 %.
    pub strength: f32,
    /// Blur passes: 1 Low, 2 Medium, 3 High.
    pub quality: u8,
    /// Degrees clockwise from the x axis, and pixels: where shadows and bevels fall.
    pub angle: f32,
    pub distance: f32,
    /// A drop shadow's or glow's colour; a bevel's highlight.
    pub color: Rgba,
    /// A bevel's shadow colour.
    pub shadow: Rgba,
    pub knockout: bool,
    pub hide_object: bool,
    pub side: Side,
    /// The gradient filters' stops: position 0..=1 and colour.
    pub gradient: Vec<(f32, Rgba)>,
    /// Adjust Color's brightness, contrast, saturation (-100..=100) and hue (-180..=180).
    pub adjust: [f32; 4],
    /// The filter element's attributes the model doesn't read.
    pub extra: Vec<(String, String)>,
}

const BLACK: Rgba = [0.0, 0.0, 0.0, 1.0];
const WHITE: Rgba = [1.0; 4];

impl Filter {
    /// A filter as the add menu makes it.
    // ponytail: 4 px blur and distance, as docs/animate-ui.md reads Animate 2024's cards (Flash 8
    // to CS6 seeded 5). Check on an install.
    pub fn new(kind: FilterKind) -> Filter {
        // Only settings the kind has, so the rest read back as they were.
        let mut f = Filter::unset(kind);
        if kind.attrs().contains(&"blurX") {
            f.blur = [4.0; 2];
        }
        if kind.attrs().contains(&"distance") {
            f.distance = 4.0;
        }
        f
    }

    /// `kind` with each setting at what a file means by leaving it out.
    // ponytail: XFL's defaults aren't published; these are the CS-era ones (5 px, 100 %, 45°, Low).
    // Saving writes every setting, so only files from elsewhere depend on them.
    fn unset(kind: FilterKind) -> Filter {
        let bevel = matches!(kind, FilterKind::Bevel | FilterKind::GradientBevel);
        Filter {
            kind,
            enabled: true,
            blur: [5.0; 2],
            strength: 1.0,
            quality: 1,
            angle: 45.0,
            distance: 5.0,
            color: match kind {
                FilterKind::Glow => [1.0, 0.0, 0.0, 1.0],
                FilterKind::Bevel => WHITE,
                _ => BLACK,
            },
            shadow: BLACK,
            knockout: false,
            hide_object: false,
            side: if bevel { Side::Inner } else { Side::Outer },
            // ponytail: the gradient filters' default stops are our reading of Animate's bars.
            gradient: match kind {
                FilterKind::GradientGlow => vec![(0.0, [1.0, 1.0, 1.0, 0.0]), (1.0, BLACK)],
                FilterKind::GradientBevel => vec![(0.0, WHITE), (0.5, [1.0, 1.0, 1.0, 0.0]), (1.0, BLACK)],
                _ => Vec::new(),
            },
            adjust: [0.0; 4],
            extra: Vec::new(),
        }
    }
}

// ---- XFL ----------------------------------------------------------------------------------

/// An element's filters (enabled or not) in the order they apply, from its kept `<filters>`.
// ponytail: elements in `<filters>` other than the seven kinds are dropped once the list is set
// again; Animate writes only these.
pub fn filters(extra: &Extra) -> Vec<Filter> {
    let Some(xml) = extra.before.iter().chain(&extra.after).find(|x| is_filters(x)) else { return Vec::new() };
    let Ok(xml) = roxmltree::Document::parse(xml) else { return Vec::new() };
    xml.root_element().children().filter_map(read).collect()
}

/// Replaces an element's filters; none removes its `<filters>`.
pub fn set_filters(extra: &mut Extra, filters: &[Filter]) {
    let xml = (!filters.is_empty()).then(|| format!("<filters>{}</filters>", filters.iter().map(write).collect::<String>()));
    for list in [&mut extra.before, &mut extra.after] {
        if let Some(i) = list.iter().position(|x| is_filters(x)) {
            match xml {
                Some(xml) => list[i] = xml,
                None => drop(list.remove(i)),
            }
            return;
        }
    }
    // Next to what the model writes: an instance's matrix and colour, a text field's runs.
    if let Some(xml) = xml {
        extra.after.insert(0, xml);
    }
}

fn is_filters(xml: &str) -> bool {
    xml.strip_prefix("<filters").is_some_and(|rest| rest.starts_with(['>', '/', ' ', '\t', '\r', '\n']))
}

fn read(n: Node) -> Option<Filter> {
    let kind = FilterKind::ALL.into_iter().find(|k| n.is_element() && k.tag() == n.tag_name().name())?;
    let mut f = Filter::unset(kind);
    for a in n.attributes() {
        let (name, v) = (a.name(), a.value());
        if a.namespace().is_some() || !(name == "isEnabled" || kind.attrs().contains(&name)) {
            f.extra.push((n.document().input_text()[a.range_qname()].into(), v.into()));
            continue;
        }
        let num = |slot: &mut f32| *slot = v.parse().unwrap_or(*slot);
        let rgb = |slot: &mut Rgba| {
            if let Ok([r, g, b]) = hex_color(v) {
                (slot[0], slot[1], slot[2]) = (r, g, b);
            }
        };
        match name {
            "isEnabled" => f.enabled = v != "false",
            "blurX" => num(&mut f.blur[0]),
            "blurY" => num(&mut f.blur[1]),
            "quality" => f.quality = v.parse::<u8>().map_or(f.quality, |q| q.clamp(1, 3)),
            "strength" => num(&mut f.strength),
            "angle" => num(&mut f.angle),
            "distance" => num(&mut f.distance),
            "color" | "highlightColor" => rgb(&mut f.color),
            "alpha" | "highlightAlpha" => num(&mut f.color[3]),
            "shadowColor" => rgb(&mut f.shadow),
            "shadowAlpha" => num(&mut f.shadow[3]),
            "knockout" => f.knockout = v == "true",
            "hideObject" => f.hide_object = v == "true",
            "inner" => f.side = if v == "true" { Side::Inner } else { Side::Outer },
            "type" => {
                f.side = match v {
                    "inner" => Side::Inner,
                    "full" => Side::Full,
                    _ => Side::Outer,
                }
            }
            "brightness" => num(&mut f.adjust[0]),
            "contrast" => num(&mut f.adjust[1]),
            "saturation" => num(&mut f.adjust[2]),
            "hue" => num(&mut f.adjust[3]),
            _ => {}
        }
    }
    let stops: Vec<_> = n.children().filter(|c| c.has_tag_name("GradientEntry")).filter_map(stop).collect();
    if !stops.is_empty() {
        f.gradient = stops;
    }
    Some(f)
}

fn stop(n: Node) -> Option<(f32, Rgba)> {
    let [r, g, b] = hex_color(n.attribute("color").unwrap_or("#000000")).ok()?;
    let num = |name: &str, default: f32| n.attribute(name).and_then(|v| v.parse().ok()).unwrap_or(default);
    Some((num("ratio", 0.0), [r, g, b, num("alpha", 1.0)]))
}

/// One filter as XFL, every setting its kind reads written out.
fn write(f: &Filter) -> String {
    let tag = f.kind.tag();
    let mut xml = format!("<{tag}");
    for &name in f.kind.attrs() {
        if let Some(v) = value(f, name) {
            xml += &format!(" {name}=\"{v}\"");
        }
    }
    if !f.enabled {
        // ponytail: `isEnabled` is our guess at how XFL marks a filter turned off.
        xml += " isEnabled=\"false\"";
    }
    for (name, v) in &f.extra {
        xml += &format!(" {name}=\"{}\"", escape(v));
    }
    if !f.kind.gradient() {
        return xml + "/>";
    }
    xml += ">";
    for (ratio, [r, g, b, a]) in &f.gradient {
        xml += &format!("<GradientEntry color=\"{}\" alpha=\"{a}\" ratio=\"{ratio}\"/>", hex([*r, *g, *b]));
    }
    xml + &format!("</{tag}>")
}

/// What `f` writes for attribute `name`; None leaves it out (a flag that's off).
fn value(f: &Filter, name: &str) -> Option<String> {
    let flag = |on: bool| on.then(|| "true".to_string());
    let rgb = |c: Rgba| hex([c[0], c[1], c[2]]);
    Some(match name {
        "blurX" => f.blur[0].to_string(),
        "blurY" => f.blur[1].to_string(),
        "quality" => f.quality.to_string(),
        "strength" => f.strength.to_string(),
        "angle" => f.angle.to_string(),
        "distance" => f.distance.to_string(),
        "color" | "highlightColor" => rgb(f.color),
        "alpha" | "highlightAlpha" => f.color[3].to_string(),
        "shadowColor" => rgb(f.shadow),
        "shadowAlpha" => f.shadow[3].to_string(),
        "knockout" => return flag(f.knockout),
        "hideObject" => return flag(f.hide_object),
        "inner" => return flag(f.side == Side::Inner),
        "type" => ["inner", "outer", "full"][f.side as usize].into(),
        "brightness" => f.adjust[0].to_string(),
        "contrast" => f.adjust[1].to_string(),
        "saturation" => f.adjust[2].to_string(),
        "hue" => f.adjust[3].to_string(),
        _ => return None,
    })
}

// ---- Display ------------------------------------------------------------------------------

/// Blending, from the Display section.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Blend {
    #[default]
    Normal,
    Layer,
    Darken,
    Multiply,
    Lighten,
    Screen,
    Overlay,
    HardLight,
    Add,
    Subtract,
    Difference,
    Invert,
    Alpha,
    Erase,
}

impl Blend {
    /// In the Blending menu's order, with XFL's `blendMode` value and the menu's name.
    pub const ALL: [(Blend, &'static str, &'static str); 14] = [
        (Blend::Normal, "normal", "Normal"),
        (Blend::Layer, "layer", "Layer"),
        (Blend::Darken, "darken", "Darken"),
        (Blend::Multiply, "multiply", "Multiply"),
        (Blend::Lighten, "lighten", "Lighten"),
        (Blend::Screen, "screen", "Screen"),
        (Blend::Overlay, "overlay", "Overlay"),
        (Blend::HardLight, "hardlight", "Hard Light"),
        (Blend::Add, "add", "Add"),
        (Blend::Subtract, "subtract", "Subtract"),
        (Blend::Difference, "difference", "Difference"),
        (Blend::Invert, "invert", "Invert"),
        (Blend::Alpha, "alpha", "Alpha"),
        (Blend::Erase, "erase", "Erase"),
    ];

    /// An element's, from its `blendMode`.
    pub fn of(extra: &Extra) -> Blend {
        let mode = extra.get("blendMode").unwrap_or("normal");
        Blend::ALL.iter().find(|b| b.1 == mode).map_or(Blend::Normal, |b| b.0)
    }
}

/// The Render setting's attributes: Cache as Bitmap and Export as Bitmap.
pub const CACHE_AS_BITMAP: &str = "cacheAsBitmap";
// ponytail: `exportAsBitmap` is our guess at how XFL marks Export as Bitmap, which we publish
// as Cache as Bitmap (Animate rasterizes the instance into the SWF).
pub const EXPORT_AS_BITMAP: &str = "exportAsBitmap";

/// What drawing and publishing need of an element's Display and Filters settings.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Effects {
    /// The enabled filters, in order.
    pub filters: Vec<Filter>,
    pub blend: Blend,
    /// Visible unchecked: published (as invisible) but not drawn.
    pub hidden: bool,
    /// Rendered as a bitmap: drawn as it is, published with Cache as Bitmap.
    pub cached: bool,
}

impl Effects {
    pub fn of(extra: &Extra) -> Effects {
        Effects {
            filters: filters(extra).into_iter().filter(|f| f.enabled).collect(),
            blend: Blend::of(extra),
            hidden: extra.get("isVisible") == Some("false"),
            cached: [CACHE_AS_BITMAP, EXPORT_AS_BITMAP].iter().any(|a| extra.get(a) == Some("true")),
        }
    }

    /// Whether it draws as it is, with no layer of its own to filter or blend.
    pub fn plain(&self) -> bool {
        self.filters.is_empty() && self.blend == Blend::Normal
    }
}

/// The filters a classic tween shows `t` of the way from keyframe `a`'s to `b`'s (their
/// elements' kept XFL): lists of the same kinds in the same order tween setting by setting;
/// None, holding `a`'s, otherwise. Switches (knockout, inner, type, quality, on) stay `a`'s.
// ponytail: Animate adds a filter missing at one end with no effect, so unmatched lists tween too.
// Pair them up here if that's missed.
pub(crate) fn tween(a: &Extra, b: &Extra, t: f32) -> Option<Vec<Filter>> {
    let (from, to) = (filters(a), filters(b));
    let matching = !from.is_empty() && from.len() == to.len() && from.iter().zip(&to).all(|(x, y)| x.kind == y.kind);
    matching.then(|| from.iter().zip(&to).map(|(x, y)| x.lerp(y, t)).collect())
}

impl Filter {
    fn lerp(&self, to: &Filter, t: f32) -> Filter {
        let mix = |x: f32, y: f32| x + (y - x) * t;
        let rgba = |x: Rgba, y: Rgba| from_fn(|i| mix(x[i], y[i]));
        let gradient = match self.gradient.len() == to.gradient.len() {
            true => self.gradient.iter().zip(&to.gradient).map(|(&(p, c), &(q, d))| (mix(p, q), rgba(c, d))).collect(),
            false => self.gradient.clone(),
        };
        Filter {
            blur: from_fn(|i| mix(self.blur[i], to.blur[i])),
            strength: mix(self.strength, to.strength),
            angle: mix(self.angle, to.angle),
            distance: mix(self.distance, to.distance),
            color: rgba(self.color, to.color),
            shadow: rgba(self.shadow, to.shadow),
            gradient,
            adjust: from_fn(|i| mix(self.adjust[i], to.adjust[i])),
            ..self.clone()
        }
    }
}

// ---- Drawing ------------------------------------------------------------------------------

/// Premultiplied RGBA in floats, row by row: an object's layer while it's filtered.
pub(crate) struct Image {
    w: usize,
    h: usize,
    px: Vec<[f32; 4]>,
}

impl Image {
    /// The part of `pm` that holds anything, grown by `reach` and kept inside `pm`, and
    /// where that part's corner is; None if `pm` is empty.
    pub(crate) fn content(pm: &Pixmap, (rx, ry): (f32, f32)) -> Option<(Image, i32, i32)> {
        let (w, h) = (pm.width() as usize, pm.height() as usize);
        let rows: Vec<usize> = (0..h).filter(|y| pm.data()[y * w * 4..(y + 1) * w * 4].chunks_exact(4).any(|p| p[3] > 0)).collect();
        let (&top, &bottom) = (rows.first()?, rows.last()?);
        let filled = |x: usize| rows.iter().any(|y| pm.data()[(y * w + x) * 4 + 3] > 0);
        let (left, right) = ((0..w).find(|&x| filled(x))?, (0..w).rfind(|&x| filled(x))?);
        // Never past `pm`'s size: a file's blur or distance can be any number.
        let (gx, gy) = (rx.ceil().min(w as f32) as usize, ry.ceil().min(h as f32) as usize);
        let (x0, y0) = (left.saturating_sub(gx), top.saturating_sub(gy));
        let (x1, y1) = ((right + 1 + gx).min(w), (bottom + 1 + gy).min(h));
        let px = (y0..y1).flat_map(|y| (x0..x1).map(move |x| (y * w + x) * 4)).map(|i| unit(&pm.data()[i..i + 4])).collect();
        Some((Image { w: x1 - x0, h: y1 - y0, px }, x0 as i32, y0 as i32))
    }

    pub(crate) fn to_pixmap(&self) -> Option<Pixmap> {
        let mut pm = Pixmap::new(self.w as u32, self.h as u32)?;
        for (out, &p) in pm.data_mut().chunks_exact_mut(4).zip(&self.px) {
            out.copy_from_slice(&bytes(p));
        }
        Some(pm)
    }

    /// Colours every pixel by `cx`, as a filtered object takes its colour effect after
    /// its filters (a tinted object's shadow is tinted too).
    pub(crate) fn color(&mut self, cx: ColorTransform) {
        if cx == ColorTransform::IDENTITY {
            return;
        }
        for p in self.px.iter_mut().filter(|p| p[3] > 0.0) {
            let c = cx.apply([p[0] / p[3], p[1] / p[3], p[2] / p[3], p[3]]);
            *p = premul(c);
        }
    }
}

fn unit(p: &[u8]) -> [f32; 4] {
    from_fn(|i| p[i] as f32 / 255.0)
}

/// Premultiplied bytes, colour never over alpha.
fn bytes(p: [f32; 4]) -> [u8; 4] {
    let a = p[3].clamp(0.0, 1.0);
    from_fn(|i| (if i == 3 { a } else { p[i].clamp(0.0, a) } * 255.0).round() as u8)
}

fn premul([r, g, b, a]: Rgba) -> [f32; 4] {
    [r * a, g * a, b * a, a]
}

fn scale(p: [f32; 4], k: f32) -> [f32; 4] {
    p.map(|v| v * k)
}

/// `a` over `b`.
fn over(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    from_fn(|i| a[i] + b[i] * (1.0 - a[3]))
}

/// How far `filters` can spread what they're given, in pixels at `zoom`, along x and y.
pub(crate) fn reach(filters: &[Filter], zoom: f32) -> (f32, f32) {
    filters.iter().filter(|f| f.kind != FilterKind::AdjustColor).fold((0.0, 0.0), |(x, y), f| {
        // A pass of a box blur spreads by half its width.
        let spread = |b: f32| b * zoom * f.quality.clamp(1, 3) as f32 / 2.0 + 1.0;
        let d = if matches!(f.kind, FilterKind::Blur | FilterKind::Glow) { 0.0 } else { f.distance * zoom };
        let (sin, cos) = f.angle.to_radians().sin_cos();
        (x + spread(f.blur[0]) + (cos * d).abs(), y + spread(f.blur[1]) + (sin * d).abs())
    })
}

/// Runs `filters` over `img` in order, their sizes scaled by `zoom`: the view's scale, never
/// the object's own.
pub(crate) fn apply(img: &mut Image, filters: &[Filter], zoom: f32) {
    let (w, h) = (img.w, img.h);
    for f in filters {
        let blur = f.blur.map(|b| b * zoom);
        let passes = f.quality.clamp(1, 3);
        if f.kind == FilterKind::Blur {
            for c in 0..4 {
                let mut plane: Vec<f32> = img.px.iter().map(|p| p[c]).collect();
                blur_plane(&mut plane, w, h, blur, passes);
                img.px.iter_mut().zip(plane).for_each(|(p, v)| p[c] = v);
            }
            continue;
        }
        if f.kind == FilterKind::AdjustColor {
            let m = adjust_matrix(f.adjust);
            img.px.iter_mut().for_each(|p| *p = recolor(*p, &m));
            continue;
        }
        // The rest work from the object's blurred alpha, cast `off` away (toward the shadow).
        let mut cast: Vec<f32> = img.px.iter().map(|p| p[3]).collect();
        blur_plane(&mut cast, w, h, blur, passes);
        let (sin, cos) = f.angle.to_radians().sin_cos();
        let d = if f.kind == FilterKind::Glow { 0.0 } else { f.distance * zoom };
        let off = (cos * d, sin * d);
        let lut = if f.kind.gradient() { gradient_lut(&f.gradient) } else { Vec::new() };
        let shade = |t: f32| lut[(t.clamp(0.0, 1.0) * 255.0).round() as usize];
        for y in 0..h {
            for x in 0..w {
                let src = img.px[y * w + x];
                let at = |dx: f32, dy: f32| sample(&cast, w, h, x as f32 + dx, y as f32 + dy);
                let fx = if matches!(f.kind, FilterKind::Bevel | FilterKind::GradientBevel) {
                    // Lit where the object rises toward the light, shaded where it falls away.
                    let v = ((at(-off.0, -off.1) - at(off.0, off.1)) * f.strength).clamp(-1.0, 1.0);
                    let px = match f.kind {
                        FilterKind::GradientBevel => shade((1.0 + v) / 2.0),
                        _ if v >= 0.0 => scale(premul(f.shadow), v),
                        _ => scale(premul(f.color), -v),
                    };
                    match f.side {
                        Side::Inner => scale(px, src[3]),
                        Side::Outer => scale(px, 1.0 - src[3]),
                        Side::Full => px,
                    }
                } else {
                    let a = at(-off.0, -off.1);
                    let paint = |t: f32| if lut.is_empty() { scale(premul(f.color), t.min(1.0)) } else { shade(t) };
                    let outer = paint(a * f.strength);
                    let inner = scale(paint((1.0 - a) * f.strength), src[3]);
                    match f.side {
                        Side::Outer => outer,
                        Side::Inner => inner,
                        Side::Full => from_fn(|i| outer[i] * (1.0 - src[3]) + inner[i]),
                    }
                };
                img.px[y * w + x] = compose(src, fx, f);
            }
        }
    }
}

/// The object `src` with its filter's effect `fx`: under it outside, over it inside.
/// Knockout keeps the effect alone, cut out of the object; Hide object keeps it whole.
fn compose(src: [f32; 4], fx: [f32; 4], f: &Filter) -> [f32; 4] {
    match f.side {
        Side::Outer if f.knockout => scale(fx, 1.0 - src[3]),
        Side::Outer if f.hide_object => fx,
        Side::Outer => over(src, fx),
        _ if f.knockout || f.hide_object => fx,
        _ => over(fx, src),
    }
}

/// `plane` at (x, y), between pixel centres; nothing outside it.
fn sample(plane: &[f32], w: usize, h: usize, x: f32, y: f32) -> f32 {
    let (x0, y0) = (x.floor(), y.floor());
    let (tx, ty) = (x - x0, y - y0);
    let at = |x: f32, y: f32| {
        let inside = x >= 0.0 && y >= 0.0 && x < w as f32 && y < h as f32;
        if inside { plane[y as usize * w + x as usize] } else { 0.0 }
    };
    let top = at(x0, y0) * (1.0 - tx) + at(x0 + 1.0, y0) * tx;
    let bottom = at(x0, y0 + 1.0) * (1.0 - tx) + at(x0 + 1.0, y0 + 1.0) * tx;
    top * (1.0 - ty) + bottom * ty
}

/// `passes` box blurs `bx` wide along the rows and `by` along the columns: Animate's blur,
/// one pass at Low quality, three at High. Widths needn't be whole pixels.
fn blur_plane(plane: &mut [f32], w: usize, h: usize, [bx, by]: [f32; 2], passes: u8) {
    for _ in 0..passes {
        if bx > 1.0 {
            box_pass(plane, (w, 1), (h, w), bx);
        }
        if by > 1.0 {
            box_pass(plane, (h, w), (w, 1), by);
        }
    }
}

/// A box blur `width` wide along lines of `n` samples `step` apart, `lines` lines `stride`
/// apart. Pixel i covers [i, i + 1); each takes the mean of the line over the box around
/// its middle, nothing past the ends.
fn box_pass(plane: &mut [f32], (n, step): (usize, usize), (lines, stride): (usize, usize), width: f32) {
    if n == 0 {
        return;
    }
    let mut line = vec![0.0f32; n];
    let mut sum = vec![0.0f64; n + 1];
    for l in 0..lines {
        for i in 0..n {
            line[i] = plane[l * stride + i * step];
            sum[i + 1] = sum[i] + line[i] as f64;
        }
        let integral = |t: f32| {
            let t = t.clamp(0.0, n as f32);
            let i = (t as usize).min(n - 1);
            sum[i] + (t - i as f32) as f64 * line[i] as f64
        };
        for i in 0..n {
            let mid = i as f32 + 0.5;
            plane[l * stride + i * step] = ((integral(mid + width / 2.0) - integral(mid - width / 2.0)) / width as f64) as f32;
        }
    }
}

/// The gradient at 256 steps, premultiplied, mixed premultiplied.
fn gradient_lut(stops: &[(f32, Rgba)]) -> Vec<[f32; 4]> {
    let mut stops = stops.to_vec();
    stops.sort_by(|a, b| a.0.total_cmp(&b.0));
    (0..256)
        .map(|i| {
            let t = i as f32 / 255.0;
            match stops.iter().position(|s| s.0 >= t) {
                _ if stops.is_empty() => [0.0; 4],
                None => premul(stops[stops.len() - 1].1),
                Some(0) => premul(stops[0].1),
                Some(k) => {
                    let ((ta, a), (tb, b)) = (stops[k - 1], stops[k]);
                    let u = (t - ta) / (tb - ta).max(1e-6);
                    from_fn(|c| premul(a)[c] * (1.0 - u) + premul(b)[c] * u)
                }
            }
        })
        .collect()
}

/// Adjust Color as a 4×5 colour matrix (rows r, g, b, a; offsets in 0..=1): brightness, then
/// contrast, saturation and hue, each applied to what the one before made.
// ponytail: our own curves, not Animate's: brightness adds up to ±100/255, contrast scales about
// mid grey by 0..=2, saturation by 0..=2 and hue rotates, both as SVG's feColorMatrix. Fit them to
// Animate's output if they're visibly off.
pub(crate) fn adjust_matrix([brightness, contrast, saturation, hue]: [f32; 4]) -> [f32; 20] {
    let b = brightness / 255.0;
    let k = 1.0 + contrast / 100.0;
    let c = (1.0 - k) / 2.0;
    let s = 1.0 + saturation / 100.0;
    let (sin, cos) = hue.to_radians().sin_cos();
    let steps = [
        [1.0, 0.0, 0.0, 0.0, b, 0.0, 1.0, 0.0, 0.0, b, 0.0, 0.0, 1.0, 0.0, b, 0.0, 0.0, 0.0, 1.0, 0.0],
        [k, 0.0, 0.0, 0.0, c, 0.0, k, 0.0, 0.0, c, 0.0, 0.0, k, 0.0, c, 0.0, 0.0, 0.0, 1.0, 0.0],
        [
            0.213 + 0.787 * s, 0.715 - 0.715 * s, 0.072 - 0.072 * s, 0.0, 0.0,
            0.213 - 0.213 * s, 0.715 + 0.285 * s, 0.072 - 0.072 * s, 0.0, 0.0,
            0.213 - 0.213 * s, 0.715 - 0.715 * s, 0.072 + 0.928 * s, 0.0, 0.0,
            0.0, 0.0, 0.0, 1.0, 0.0,
        ],
        [
            0.213 + 0.787 * cos - 0.213 * sin, 0.715 - 0.715 * cos - 0.715 * sin, 0.072 - 0.072 * cos + 0.928 * sin, 0.0, 0.0,
            0.213 - 0.213 * cos + 0.143 * sin, 0.715 + 0.285 * cos + 0.140 * sin, 0.072 - 0.072 * cos - 0.283 * sin, 0.0, 0.0,
            0.213 - 0.213 * cos - 0.787 * sin, 0.715 - 0.715 * cos + 0.715 * sin, 0.072 + 0.928 * cos + 0.072 * sin, 0.0, 0.0,
            0.0, 0.0, 0.0, 1.0, 0.0,
        ],
    ];
    steps.into_iter().reduce(|done, next| then(&done, &next)).unwrap_or_default()
}

/// `first`, then `next`, as one matrix.
fn then(first: &[f32; 20], next: &[f32; 20]) -> [f32; 20] {
    from_fn(|i| {
        let (r, c) = (i / 5, i % 5);
        let dot: f32 = (0..4).map(|k| next[r * 5 + k] * first[k * 5 + c]).sum();
        if c == 4 { dot + next[r * 5 + 4] } else { dot }
    })
}

fn recolor(p: [f32; 4], m: &[f32; 20]) -> [f32; 4] {
    if p[3] <= 0.0 {
        return p;
    }
    let c = [p[0] / p[3], p[1] / p[3], p[2] / p[3], p[3]];
    let out: [f32; 4] = from_fn(|r| ((0..4).map(|k| m[r * 5 + k] * c[k]).sum::<f32>() + m[r * 5 + 4]).clamp(0.0, 1.0));
    premul(out)
}

/// Draws `src` onto `dst` with its corner at (x, y), blended by `blend`. Alpha and Erase
/// act on the layer they're drawn in, which Animate asks to be the parent's Layer; drawn
/// straight onto the stage (`layered` false) they draw as Normal.
// ponytail: Alpha acts on the whole box `src` fills, transparent corners included, where Animate
// leaves what its shapes don't cover alone.
pub(crate) fn composite(dst: &mut Pixmap, src: &Pixmap, x: i32, y: i32, blend: Blend, layered: bool) {
    use sk::BlendMode as M;
    let mode = match blend {
        Blend::Normal | Blend::Layer => M::SourceOver,
        Blend::Darken => M::Darken,
        Blend::Multiply => M::Multiply,
        Blend::Lighten => M::Lighten,
        Blend::Screen => M::Screen,
        Blend::Overlay => M::Overlay,
        Blend::HardLight => M::HardLight,
        Blend::Add => M::Plus,
        Blend::Difference => M::Difference,
        Blend::Alpha if layered => M::DestinationIn,
        Blend::Erase if layered => M::DestinationOut,
        Blend::Alpha | Blend::Erase => M::SourceOver,
        // What's below, less the object, never under black.
        Blend::Subtract => {
            return blend_by(dst, src, (x, y), |d, s, _| from_fn(|i| if i == 3 { s[3] + d[3] * (1.0 - s[3]) } else { (d[i] - s[i]).max(0.0) }));
        }
        // What's below, inverted as far as the object is opaque.
        Blend::Invert => return blend_by(dst, src, (x, y), |d, s, _| from_fn(|i| if i == 3 { d[3] } else { s[3] * (d[3] - d[i]) + (1.0 - s[3]) * d[i] })),
    };
    let paint = sk::PixmapPaint { blend_mode: mode, ..Default::default() };
    dst.draw_pixmap(x, y, src.as_ref(), &paint, sk::Transform::identity(), None);
}

/// Before `composite` blends `src` straight onto a canvas that's shown over the stage later
/// (the editor's): puts the stage's `color` under the canvas where `src` lands inside `stage`,
/// so the blend mixes with the stage, as on Animate's, and not with nothing.
pub(crate) fn put_under(dst: &mut Pixmap, src: &Pixmap, x: i32, y: i32, blend: Blend, (color, stage): (Rgba, sk::Rect)) {
    // Drawn over what's below, it makes no difference.
    if matches!(blend, Blend::Normal | Blend::Layer | Blend::Alpha | Blend::Erase) {
        return;
    }
    let inside = |(px, py): (i32, i32)| {
        let (cx, cy) = (px as f32 + 0.5, py as f32 + 0.5);
        cx >= stage.left() && cx < stage.right() && cy >= stage.top() && cy < stage.bottom()
    };
    blend_by(dst, src, (x, y), |d, _, at| if inside(at) { over(d, premul(color)) } else { d });
}

/// Blends `src` onto `dst` at `at` pixel by pixel: `f(below, src, where on dst)`, premultiplied.
fn blend_by(dst: &mut Pixmap, src: &Pixmap, (x, y): (i32, i32), f: impl Fn([f32; 4], [f32; 4], (i32, i32)) -> [f32; 4]) {
    let (dw, dh, sw) = (dst.width() as i32, dst.height() as i32, src.width() as i32);
    for (k, s) in src.data().chunks_exact(4).enumerate() {
        let (dx, dy) = (x + k as i32 % sw, y + k as i32 / sw);
        if s[3] == 0 || dx < 0 || dy < 0 || dx >= dw || dy >= dh {
            continue;
        }
        let i = ((dy * dw + dx) * 4) as usize;
        let d = &mut dst.data_mut()[i..i + 4];
        d.copy_from_slice(&bytes(f(unit(d), unit(s), (dx, dy))));
    }
}

// ---- SWF ----------------------------------------------------------------------------------

/// Makes `p` a PlaceObject3 that sets `fx`: filters, blend mode, bitmap caching, visibility.
pub(crate) fn place(p: &mut swf::PlaceObject, fx: &Effects) {
    p.version = 3;
    p.filters = Some(fx.filters.iter().map(swf_filter).collect());
    p.blend_mode = Some(match fx.blend {
        Blend::Normal => swf::BlendMode::Normal,
        Blend::Layer => swf::BlendMode::Layer,
        Blend::Darken => swf::BlendMode::Darken,
        Blend::Multiply => swf::BlendMode::Multiply,
        Blend::Lighten => swf::BlendMode::Lighten,
        Blend::Screen => swf::BlendMode::Screen,
        Blend::Overlay => swf::BlendMode::Overlay,
        Blend::HardLight => swf::BlendMode::HardLight,
        Blend::Add => swf::BlendMode::Add,
        Blend::Subtract => swf::BlendMode::Subtract,
        Blend::Difference => swf::BlendMode::Difference,
        Blend::Invert => swf::BlendMode::Invert,
        Blend::Alpha => swf::BlendMode::Alpha,
        Blend::Erase => swf::BlendMode::Erase,
    });
    p.is_bitmap_cached = Some(fx.cached);
    p.is_visible = Some(!fx.hidden);
}

fn swf_filter(f: &Filter) -> swf::Filter {
    use swf::{Fixed8, Fixed16};
    let color = |c: Rgba| {
        let [r, g, b, a] = c.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8);
        swf::Color { r, g, b, a }
    };
    let (blur_x, blur_y) = (Fixed16::from_f32(f.blur[0]), Fixed16::from_f32(f.blur[1]));
    // SWF's angles are radians; its strength saturates just under 128 (12800 %).
    let (angle, distance, strength) = (Fixed16::from_f32(f.angle.to_radians()), Fixed16::from_f32(f.distance), Fixed8::from_f32(f.strength));
    let passes = f.quality.clamp(1, 3);
    let (inner, full) = (f.side == Side::Inner, f.side == Side::Full);
    match f.kind {
        FilterKind::DropShadow => {
            use swf::DropShadowFilterFlags as F;
            let mut flags = F::from_passes(passes);
            flags.set(F::INNER_SHADOW, inner);
            flags.set(F::KNOCKOUT, f.knockout);
            flags.set(F::COMPOSITE_SOURCE, !f.hide_object);
            let shadow = swf::DropShadowFilter { color: color(f.color), blur_x, blur_y, angle, distance, strength, flags };
            swf::Filter::DropShadowFilter(Box::new(shadow))
        }
        FilterKind::Blur => {
            swf::Filter::BlurFilter(Box::new(swf::BlurFilter { blur_x, blur_y, flags: swf::BlurFilterFlags::from_passes(passes) }))
        }
        FilterKind::Glow => {
            use swf::GlowFilterFlags as F;
            let mut flags = F::from_passes(passes) | F::COMPOSITE_SOURCE;
            flags.set(F::INNER_GLOW, inner);
            flags.set(F::KNOCKOUT, f.knockout);
            swf::Filter::GlowFilter(Box::new(swf::GlowFilter { color: color(f.color), blur_x, blur_y, strength, flags }))
        }
        FilterKind::Bevel => {
            use swf::BevelFilterFlags as F;
            let mut flags = F::from_passes(passes) | F::COMPOSITE_SOURCE;
            flags.set(F::INNER_SHADOW, inner);
            flags.set(F::ON_TOP, full);
            flags.set(F::KNOCKOUT, f.knockout);
            let (shadow_color, highlight_color) = (color(f.shadow), color(f.color));
            let bevel = swf::BevelFilter { shadow_color, highlight_color, blur_x, blur_y, angle, distance, strength, flags };
            swf::Filter::BevelFilter(Box::new(bevel))
        }
        FilterKind::GradientGlow | FilterKind::GradientBevel => {
            use swf::GradientFilterFlags as F;
            let mut flags = F::from_passes(passes) | F::COMPOSITE_SOURCE;
            flags.set(F::INNER_SHADOW, inner);
            flags.set(F::ON_TOP, full);
            flags.set(F::KNOCKOUT, f.knockout);
            let ratio = |t: f32| (t.clamp(0.0, 1.0) * 255.0).round() as u8;
            let colors = f.gradient.iter().map(|&(t, c)| swf::GradientRecord { ratio: ratio(t), color: color(c) }).collect();
            let g = Box::new(swf::GradientFilter { colors, blur_x, blur_y, angle, distance, strength, flags });
            if f.kind == FilterKind::GradientGlow { swf::Filter::GradientGlowFilter(g) } else { swf::Filter::GradientBevelFilter(g) }
        }
        FilterKind::AdjustColor => {
            let mut matrix = adjust_matrix(f.adjust);
            // A colour matrix filter's offsets run 0..=255.
            (0..4).for_each(|r| matrix[r * 5 + 4] *= 255.0);
            swf::Filter::ColorMatrixFilter(Box::new(swf::ColorMatrixFilter { matrix }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extra_with(xml: &str) -> Extra {
        Extra { after: vec![xml.into()], ..Extra::default() }
    }

    #[test]
    fn xfl_filters_read_and_write_back() {
        let xml = r##"<filters>
            <DropShadowFilter blurX="10" blurY="12" strength="0.5" quality="3" angle="44.9999999772279" distance="7" color="#336699" alpha="0.75" knockout="true" hideObject="true" isEnabled="false" mystery="&quot;x&quot;"/>
            <BlurFilter/>
            <GlowFilter inner="true" color="#00FF00"/>
            <BevelFilter type="full" highlightColor="#FFFF00" shadowAlpha="0.5" angle="10"/>
            <GradientGlowFilter type="inner"><GradientEntry color="#FF0000" alpha="0" ratio="0"/><GradientEntry color="#0000FF" ratio="0.6"/></GradientGlowFilter>
            <GradientBevelFilter/>
            <AdjustColorFilter brightness="-25" contrast="10" saturation="-100" hue="90"/>
            <ConvolutionFilter/>
        </filters>"##;
        let extra = extra_with(xml);
        let list = filters(&extra);
        let kinds: Vec<_> = list.iter().map(|f| f.kind).collect();
        assert_eq!(kinds, FilterKind::ALL, "in file order; unknown elements skipped");
        let shadow = &list[0];
        assert_eq!((shadow.blur, shadow.strength, shadow.quality, shadow.distance), ([10.0, 12.0], 0.5, 3, 7.0));
        assert_eq!(shadow.color, [0.2, 0.4, 0.6, 0.75]);
        assert!(shadow.knockout && shadow.hide_object && !shadow.enabled && shadow.side == Side::Outer);
        assert_eq!(shadow.extra, [("mystery".to_string(), "\"x\"".to_string())]);
        assert_eq!(list[1], Filter::unset(FilterKind::Blur), "left out means the defaults");
        assert_eq!((list[2].side, list[2].color), (Side::Inner, [0.0, 1.0, 0.0, 1.0]));
        assert_eq!((list[3].side, list[3].color, list[3].shadow[3], list[3].angle), (Side::Full, [1.0, 1.0, 0.0, 1.0], 0.5, 10.0));
        assert_eq!(list[4].gradient, [(0.0, [1.0, 0.0, 0.0, 0.0]), (0.6, [0.0, 0.0, 1.0, 1.0])]);
        assert_eq!(list[4].side, Side::Inner);
        assert_eq!(list[6].adjust, [-25.0, 10.0, -100.0, 90.0]);

        // Written back, every setting reads the same; the unread attribute survives.
        let mut saved = extra.clone();
        set_filters(&mut saved, &list);
        assert_eq!(filters(&saved), list);
        assert!(saved.after[0].contains(r#"mystery="&quot;x&quot;""#), "{}", saved.after[0]);
        assert_eq!(saved.after.len(), 1, "replaced in place");

        // Effects keep the enabled ones; none at all removes the element.
        assert_eq!(Effects::of(&saved).filters.len(), 6);
        set_filters(&mut saved, &[]);
        assert_eq!((saved.after.len(), filters(&saved).len()), (0, 0));
        set_filters(&mut saved, &[Filter::new(FilterKind::Glow)]);
        assert_eq!(filters(&saved), [Filter::new(FilterKind::Glow)]);
    }

    #[test]
    fn display_attributes() {
        let mut extra = Extra::default();
        assert_eq!(Effects::of(&extra), Effects::default());
        assert!(Effects::of(&extra).plain());
        extra.set("blendMode", Some("hardlight"));
        extra.set("isVisible", Some("false"));
        extra.set(EXPORT_AS_BITMAP, Some("true"));
        let fx = Effects::of(&extra);
        assert_eq!((fx.blend, fx.hidden, fx.cached, fx.plain()), (Blend::HardLight, true, true, false));
    }

    fn image(px: Vec<[f32; 4]>, w: usize) -> Image {
        Image { w, h: px.len() / w, px }
    }

    #[test]
    fn box_blur_keeps_the_total_and_spreads_it() {
        let mut line = vec![0.0; 21];
        line[10] = 1.0;
        blur_plane(&mut line, 21, 1, [5.0, 0.0], 1);
        assert!((line.iter().sum::<f32>() - 1.0).abs() < 1e-5);
        assert_eq!(line[10], 0.2);
        assert_eq!((line[7], line[12], line[13]), (0.0, 0.2, 0.0), "five pixels wide");
        // A fractional width takes part of the pixels at its ends (2 px each way for 4), and
        // each pass spreads as far again.
        let mut line = vec![0.0; 21];
        line[10] = 1.0;
        blur_plane(&mut line, 21, 1, [4.0, 0.0], 3);
        assert!((line.iter().sum::<f32>() - 1.0).abs() < 1e-5 && line[4] > 0.0 && line[3] == 0.0, "{line:?}");
    }

    #[test]
    fn adjust_color_matrix() {
        let near = |p: [f32; 4], q: [f32; 4]| p.iter().zip(q).all(|(a, b)| (a - b).abs() < 1e-4);
        let red = [1.0, 0.0, 0.0, 1.0];
        assert!(near(recolor(red, &adjust_matrix([0.0; 4])), red));
        let grey = recolor(red, &adjust_matrix([0.0, 0.0, -100.0, 0.0]));
        assert!(near(grey, [grey[0], grey[0], grey[0], 1.0]), "no saturation is grey: {grey:?}");
        assert!(near(recolor([0.2, 0.4, 0.6, 1.0], &adjust_matrix([0.0, -100.0, 0.0, 0.0])), [0.5, 0.5, 0.5, 1.0]));
        let brighter = recolor([0.2, 0.2, 0.2, 0.5], &adjust_matrix([51.0, 0.0, 0.0, 0.0]));
        assert!(near(brighter, [0.3, 0.3, 0.3, 0.5]), "premultiplied in and out, 51/255 brighter: {brighter:?}");
        let turned = recolor(red, &adjust_matrix([0.0, 0.0, 0.0, 120.0]));
        assert!(turned[1] > turned[0] && turned[1] > turned[2], "a third of a turn takes red toward green: {turned:?}");
    }

    #[test]
    fn subtract_and_invert_blend_by_hand() {
        let mut dst = Pixmap::new(2, 1).unwrap();
        dst.fill(sk::Color::from_rgba8(200, 100, 50, 255));
        let mut src = Pixmap::new(1, 1).unwrap();
        src.fill(sk::Color::from_rgba8(100, 100, 100, 255));
        composite(&mut dst, &src, 0, 0, Blend::Subtract, false);
        composite(&mut dst, &src, 1, 0, Blend::Invert, false);
        assert_eq!(&dst.data()[..4], [100, 0, 0, 255]);
        assert_eq!(&dst.data()[4..], [55, 155, 205, 255]);
    }

    #[test]
    fn shadows_fall_away_from_the_light() {
        // A 5 × 5 opaque square in a 25 × 25 transparent image, a shadow 6 px down and right.
        let w = 25;
        let px = (0..w * w).map(|i| if (10..15).contains(&(i % w)) && (10..15).contains(&(i / w)) { [1.0; 4] } else { [0.0; 4] });
        let mut img = image(px.collect(), w);
        let shadow = Filter { blur: [0.0; 2], distance: 6.0 * 2f32.sqrt(), ..Filter::new(FilterKind::DropShadow) };
        apply(&mut img, &[shadow], 1.0);
        let at = |x: usize, y: usize| img.px[y * w + x];
        assert_eq!(at(12, 12), [1.0; 4], "the object stays on top");
        assert!(at(19, 19)[3] > 0.99 && at(19, 19)[0] < 0.01, "black shadow below right: {:?}", at(19, 19));
        assert_eq!(at(8, 8), [0.0; 4], "nothing above left");
    }
}
