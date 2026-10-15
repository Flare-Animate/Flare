//! Reads XFL, the format behind `.fla` since CS5: a zip (or plain folder) where
//! `DOMDocument.xml` holds the stage and scenes and `LIBRARY/*.xml` holds one
//! symbol per file.

use std::collections::HashMap;
use std::fmt::Display;
use std::fs;
use std::io::Read;
use std::str::FromStr;

use roxmltree::Node;

use crate::Result;
use crate::doc::*;

// The attributes the model reads, by element. `Extra` keeps the rest, and the writer
// writes these from the model.
// ponytail: a value the model has no variant for (motionTweenRotate="none", loop="play
// once reverse", a camera layer...) reads as the default and is saved as that.
pub(crate) const DOCUMENT_ATTRS: &[&str] = &["width", "height", "frameRate", "backgroundColor"];
pub(crate) const SYMBOL_ATTRS: &[&str] = &["name", "symbolType"];
pub(crate) const TIMELINE_ATTRS: &[&str] = &["name"];
pub(crate) const LAYER_ATTRS: &[&str] = &["name", "layerType"];
pub(crate) const INSTANCE_ATTRS: &[&str] = &["libraryItemName", "symbolType", "loop", "firstFrame"];
pub(crate) const GRADIENT_ATTRS: &[&str] = &["spreadMethod", "interpolationMethod", "focalPointRatio"];
/// `bitmapSize` follows `size`, and `lineHeight` the font, so neither is kept to go stale.
pub(crate) const TEXT_RUN_ATTRS: &[&str] = &[
    "alignment", "letterSpacing", "lineSpacing", "size", "bitmapSize", "lineHeight", "face", "fillColor", "alpha", "bold",
    "italic",
];

/// A text element's attributes the model reads: its box, and whether it extends. Dynamic and
/// input text given a fixed box keep its height as it was, and every field its `lineType`.
pub(crate) fn text_attrs(kind: TextKind, fixed: bool) -> &'static [&'static str] {
    match (kind, fixed) {
        (TextKind::Dynamic | TextKind::Input, true) => &["left", "width", "autoExpand"],
        _ => &["left", "width", "height", "autoExpand"],
    }
}

/// The tween settings each kind of tween reads; another kind's stay in `extra`.
pub(crate) fn frame_attrs(tween: &Tween) -> &'static [&'static str] {
    match tween {
        Tween::Classic { .. } => {
            &["index", "duration", "tweenType", "acceleration", "motionTweenRotate", "motionTweenRotateTimes"]
        }
        Tween::Motion(_) => &["index", "duration", "tweenType", "motionTweenRotate", "motionTweenRotateTimes"],
        Tween::Shape { .. } => &["index", "duration", "tweenType", "acceleration", "shapeTweenBlend", "shapeHints"],
        _ => &["index", "duration", "tweenType"],
    }
}

/// Loads a `.fla` file, an XFL folder, or the `.xfl` / `DOMDocument.xml` inside one.
pub fn load(path: &std::path::Path) -> Result<Document> {
    if let Some(dir) = folder(path) {
        return read_document(&mut |name| Ok(fs::read_to_string(dir.join(name))?));
    }
    let mut zip = zip::ZipArchive::new(fs::File::open(path)?)?;
    read_document(&mut |name| {
        let mut text = String::new();
        zip.by_name(name)?.read_to_string(&mut text)?;
        Ok(text)
    })
}

/// The XFL folder that `path` is, or holds the `.xfl` or `DOMDocument.xml` of; None for a `.fla` file.
pub(crate) fn folder(path: &std::path::Path) -> Option<&std::path::Path> {
    let inside = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("xfl"))
        || path.file_name().is_some_and(|n| n == "DOMDocument.xml");
    if path.is_dir() { Some(path) } else if inside { path.parent() } else { None }
}

fn read_document(read: &mut dyn FnMut(&str) -> Result<String>) -> Result<Document> {
    let text = read("DOMDocument.xml")?;
    let xml = roxmltree::Document::parse(text.trim_start_matches('\u{feff}'))?;
    let root = xml.root_element();

    let mut library = HashMap::new();
    for include in list(root, "symbols") {
        let name = format!("LIBRARY/{}", attr(include, "href")?);
        let text = read(&name).map_err(|e| format!("{name}: {e}"))?;
        let xml = roxmltree::Document::parse(text.trim_start_matches('\u{feff}'))
            .map_err(|e| format!("{name}: {e}"))?;
        let item = xml.root_element();
        let timeline = child(item, "timeline")
            .and_then(|t| child(t, "DOMTimeline"))
            .ok_or_else(|| format!("{name}: no timeline"))?;
        let symbol = Symbol {
            kind: symbol_kind(item),
            timeline: timeline_of(timeline)?,
            extra: extra(item, SYMBOL_ATTRS, &["timeline"]),
        };
        library.insert(attr(item, "name")?.to_string(), symbol);
    }

    let [r, g, b] = hex_color(root.attribute("backgroundColor").unwrap_or("#FFFFFF"))?;
    // A document has a scene at least: everything that shows one takes the first.
    let scenes: Vec<Timeline> = list(root, "timelines").map(timeline_of).collect::<Result<_>>()?;
    if scenes.is_empty() {
        return Err("DOMDocument.xml: no timelines".into());
    }
    Ok(Document {
        width: num(root, "width", 550.0)?,
        height: num(root, "height", 400.0)?,
        frame_rate: num(root, "frameRate", 24.0)?,
        background: [r, g, b, 1.0],
        scenes,
        library,
        extra: extra(root, DOCUMENT_ATTRS, &["symbols", "timelines"]),
    })
}

fn timeline_of(n: Node) -> Result<Timeline> {
    Ok(Timeline {
        name: n.attribute("name").unwrap_or_default().into(),
        layers: list(n, "layers").map(layer).collect::<Result<_>>()?,
        extra: extra(n, TIMELINE_ATTRS, &["layers"]),
    })
}

fn layer(n: Node) -> Result<Layer> {
    Ok(Layer {
        name: n.attribute("name").unwrap_or_default().into(),
        kind: match n.attribute("layerType") {
            Some("guide") => LayerKind::Guide,
            Some("mask") => LayerKind::Mask,
            Some("folder") => LayerKind::Folder,
            _ => LayerKind::Normal,
        },
        frames: list(n, "frames").map(frame).collect::<Result<_>>()?,
        extra: extra(n, LAYER_ATTRS, &["frames"]),
    })
}

fn frame(n: Node) -> Result<Frame> {
    let duration = num(n, "duration", 1u32)?.max(1);
    // Assumptions to check against real Animate files: `acceleration` has the
    // sign of the Ease field (+ = out), and a missing rotate count means 1.
    let tween = match n.attribute("tweenType") {
        None => Tween::None,
        Some("motion") => Tween::Classic {
            ease: num::<f32>(n, "acceleration", 0.0)? / 100.0,
            rotate: match n.attribute("motionTweenRotate") {
                Some("clockwise") => Rotate::Cw(num(n, "motionTweenRotateTimes", 1)?),
                Some("counter-clockwise") => Rotate::Ccw(num(n, "motionTweenRotateTimes", 1)?),
                _ => Rotate::Auto,
            },
        },
        // Deliberate round-trip fidelity, not a gap: the motion XML stays in `extra` too and is
        // written back verbatim while the tween is as read. A tween the model can't read becomes
        // `Tween::Unsupported`, carrying its raw XML through save unchanged, rather than the whole
        // document failing to open or the tween being dropped or rewritten from a lossy reading.
        Some("motion object") => match crate::motion::read(n, duration) {
            Ok(m) => Tween::Motion(Box::new(m)),
            Err(_) => Tween::Unsupported("motion object".into()),
        },
        // ponytail: `shapeTweenBlend` is JSFL's name for the blend; unconfirmed in Animate's XFL.
        Some("shape") => Tween::Shape {
            ease: num::<f32>(n, "acceleration", 0.0)? / 100.0,
            angular: n.attribute("shapeTweenBlend") == Some("angular"),
            hints: n.attribute("shapeHints").and_then(|h| crate::shape_tween::read_hints(h).ok()).unwrap_or_default(),
        },
        Some(other) => Tween::Unsupported(other.into()),
    };
    Ok(Frame {
        index: num(n, "index", 0)?,
        duration,
        elements: list(n, "elements").map(element).collect::<Result<_>>()?,
        extra: extra(n, frame_attrs(&tween), &["elements"]),
        tween,
    })
}

fn element(n: Node) -> Result<Element> {
    Ok(match n.tag_name().name() {
        "DOMShape" => Element::Shape(Shape {
            matrix: matrix(n)?,
            fills: list(n, "fills").map(paint).collect::<Result<_>>()?,
            strokes: list(n, "strokes").map(stroke).collect::<Result<_>>()?,
            edges: list(n, "edges").map(edge).collect::<Result<_>>()?,
            extra: extra(n, &[], &["matrix", "fills", "strokes", "edges"]),
        }),
        "DOMSymbolInstance" => Element::Instance(instance(n)?),
        // ponytail: a group's own attributes aren't kept. Give Group an Extra if they matter.
        "DOMGroup" => Element::Group(list(n, "members").map(element).collect::<Result<_>>()?),
        "DOMStaticText" | "DOMDynamicText" | "DOMInputText" => Element::Text(text(n)?),
        other => Element::Unsupported { name: other.into(), xml: source(n) },
    })
}

/// A classic text field. Assumptions to check against real Animate files:
/// `width` spans the whole box, gutters included, and `left` shifts the box along
/// the matrix's x axis (folded into the matrix here, so it's saved as 0).
fn text(n: Node) -> Result<Text> {
    let kind = match n.tag_name().name() {
        "DOMDynamicText" => TextKind::Dynamic,
        "DOMInputText" => TextKind::Input,
        _ => TextKind::Static,
    };
    // Extending text grows with its characters; any other kind has the width it was given.
    // ponytail: and wraps there, where Animate keeps single-line and no-wrap dynamic and input
    // text on one line. Model the line type if such fields need to show true.
    let fixed = n.attribute("autoExpand") != Some("true");
    let mut matrix = matrix(n)?;
    let left: f32 = num(n, "left", 0.0)?;
    (matrix.tx, matrix.ty) = (matrix.tx + matrix.a * left, matrix.ty + matrix.b * left);
    // ponytail: paragraph settings come with every run; the first run's hold for the whole
    // field, so paragraphs aligned differently come out alike. Model paragraphs if that bites.
    let first = list(n, "textRuns").next().and_then(text_attrs_of);
    Ok(Text {
        kind,
        matrix,
        width: if fixed { Some(num(n, "width", 0.0)?) } else { None },
        align: match first.and_then(|a| a.attribute("alignment")) {
            Some("center") => TextAlign::Center,
            Some("right") => TextAlign::Right,
            Some("justify") => TextAlign::Justify,
            _ => TextAlign::Left,
        },
        line_spacing: first.map_or(Ok(0.0), |a| num(a, "lineSpacing", 0.0))?,
        runs: list(n, "textRuns").map(text_run).collect::<Result<_>>()?,
        extra: extra(n, text_attrs(kind, fixed), &["matrix", "textRuns"]),
    })
}

fn text_run(n: Node) -> Result<TextRun> {
    let a = text_attrs_of(n);
    let get = |name: &str, default: f32| a.map_or(Ok(default), |a| num(a, name, default));
    let [r, g, b] = hex_color(a.and_then(|a| a.attribute("fillColor")).unwrap_or("#000000"))?;
    let characters = child(n, "characters").and_then(|c| c.text()).unwrap_or_default();
    Ok(TextRun {
        // Animate ends paragraphs with &#xD;.
        text: characters.replace("\r\n", "\n").replace('\r', "\n"),
        face: a.and_then(|a| a.attribute("face")).unwrap_or("Times New Roman").into(),
        size: get("size", 12.0)?,
        color: [r, g, b, get("alpha", 1.0)?],
        bold: a.and_then(|a| a.attribute("bold")) == Some("true"),
        italic: a.and_then(|a| a.attribute("italic")) == Some("true"),
        letter_spacing: get("letterSpacing", 0.0)?,
        extra: a.map_or_else(Extra::default, |a| extra(a, TEXT_RUN_ATTRS, &[])),
    })
}

/// A `<DOMTextRun>`'s `<DOMTextAttrs>`.
fn text_attrs_of<'a, 'i>(run: Node<'a, 'i>) -> Option<Node<'a, 'i>> {
    child(run, "textAttrs").and_then(|a| child(a, "DOMTextAttrs"))
}

/// The paint inside a `<FillStyle>` or a stroke's `<fill>`.
fn paint(n: Node) -> Result<Paint> {
    let Some(p) = n.first_element_child() else {
        return Ok(Paint::Unsupported { name: "empty fill".into(), xml: String::new() });
    };
    Ok(match p.tag_name().name() {
        "SolidColor" => {
            let [r, g, b] = hex_color(p.attribute("color").unwrap_or("#000000"))?;
            Paint::Solid([r, g, b, num(p, "alpha", 1.0)?])
        }
        "LinearGradient" | "RadialGradient" => Paint::Gradient(gradient(p)?),
        other => Paint::Unsupported { name: other.into(), xml: source(p) },
    })
}

/// A `<LinearGradient>` or `<RadialGradient>`: its matrix and `<GradientEntry>` stops.
// ponytail: a GradientEntry's attributes other than color, alpha and ratio aren't kept; give
// stops an Extra if Animate writes others.
fn gradient(n: Node) -> Result<Gradient> {
    let kind = match n.tag_name().name() {
        "RadialGradient" => GradientKind::Radial { focal: num(n, "focalPointRatio", 0.0)? },
        _ => GradientKind::Linear,
    };
    let mut stops = Vec::new();
    for e in n.children().filter(|c| c.tag_name().name() == "GradientEntry") {
        let [r, g, b] = hex_color(e.attribute("color").unwrap_or("#000000"))?;
        stops.push((num(e, "ratio", 0.0)?, [r, g, b, num(e, "alpha", 1.0)?]));
    }
    Ok(Gradient {
        kind,
        matrix: matrix(n)?,
        stops,
        spread: match n.attribute("spreadMethod") {
            Some("reflect") => Spread::Reflect,
            Some("repeat") => Spread::Repeat,
            _ => Spread::Pad,
        },
        linear_rgb: n.attribute("interpolationMethod") == Some("linearRGB"),
        extra: extra(n, GRADIENT_ATTRS, &["matrix", "GradientEntry"]),
    })
}

fn stroke(n: Node) -> Result<Stroke> {
    // Dashed, dotted, ragged etc. carry the same attributes; they draw solid for now.
    // ponytail: and save as SolidStroke, losing the style and its scaleMode, miterLimit
    // and hinting. Keep the stroke element's XML like Element::Unsupported if that bites.
    let s = n.first_element_child().ok_or("empty StrokeStyle")?;
    Ok(Stroke {
        width: num(s, "weight", 1.0)?,
        paint: match child(s, "fill") {
            Some(f) => paint(f)?,
            None => Paint::Solid([0.0, 0.0, 0.0, 1.0]),
        },
        cap: match s.attribute("caps") {
            Some("none") => Cap::None,
            Some("square") => Cap::Square,
            _ => Cap::Round,
        },
        join: match s.attribute("joints") {
            Some("miter") => Join::Miter,
            Some("bevel") => Join::Bevel,
            _ => Join::Round,
        },
    })
}

fn edge(n: Node) -> Result<Edge> {
    Ok(Edge {
        fill0: num(n, "fillStyle0", 0)?,
        fill1: num(n, "fillStyle1", 0)?,
        stroke: num(n, "strokeStyle", 0)?,
        paths: match n.attribute("edges") {
            Some(s) => parse_edges(s).map_err(|e| format!("edges {s:?}: {e}"))?,
            None => Vec::new(),
        },
    })
}

fn instance(n: Node) -> Result<Instance> {
    let pivot = match child(n, "transformationPoint").and_then(|t| child(t, "Point")) {
        Some(p) => (num(p, "x", 0.0)?, num(p, "y", 0.0)?),
        None => (0.0, 0.0),
    };
    let color = match child(n, "color").and_then(|c| child(c, "Color")) {
        Some(c) => color_transform(c)?,
        None => ColorTransform::IDENTITY,
    };
    Ok(Instance {
        symbol: attr(n, "libraryItemName")?.into(),
        kind: symbol_kind(n),
        matrix: matrix(n)?,
        pivot,
        color,
        looping: match n.attribute("loop") {
            Some("play once") => Loop::PlayOnce,
            Some("single frame") => Loop::SingleFrame,
            _ => Loop::Loop,
        },
        first_frame: num(n, "firstFrame", 0)?,
        extra: extra(n, INSTANCE_ATTRS, &["matrix", "transformationPoint", "color"]),
    })
}

fn symbol_kind(n: Node) -> SymbolKind {
    match n.attribute("symbolType") {
        Some("graphic") => SymbolKind::Graphic,
        Some("button") => SymbolKind::Button,
        _ => SymbolKind::MovieClip,
    }
}

fn matrix(n: Node) -> Result<Matrix> {
    let Some(m) = child(n, "matrix").and_then(|m| child(m, "Matrix")) else { return Ok(Matrix::IDENTITY) };
    Ok(Matrix {
        a: num(m, "a", 1.0)?,
        b: num(m, "b", 0.0)?,
        c: num(m, "c", 0.0)?,
        d: num(m, "d", 1.0)?,
        tx: num(m, "tx", 0.0)?,
        ty: num(m, "ty", 0.0)?,
    })
}

fn color_transform(n: Node) -> Result<ColorTransform> {
    let mut cx = ColorTransform::IDENTITY;
    for (i, ch) in ["red", "green", "blue", "alpha"].into_iter().enumerate() {
        cx.mult[i] = num(n, &format!("{ch}Multiplier"), 1.0)?;
        cx.add[i] = num::<f32>(n, &format!("{ch}Offset"), 0.0)? / 255.0;
    }
    if let Some(tint) = n.attribute("tintColor") {
        let amount: f32 = num(n, "tintMultiplier", 0.0)?;
        for (i, c) in hex_color(tint)?.into_iter().enumerate() {
            cx.mult[i] = 1.0 - amount;
            cx.add[i] = c * amount;
        }
    }
    let brightness: f32 = num(n, "brightness", 0.0)?;
    if brightness != 0.0 {
        for i in 0..3 {
            cx.mult[i] = 1.0 - brightness.abs();
            cx.add[i] = brightness.max(0.0);
        }
    }
    Ok(cx)
}

/// Parses an `<Edge edges="...">` string: `!` move, `|` or `/` line, `[` or `]`
/// quadratic curve (control, end), `S<n>` selection marker (ignored).
pub fn parse_edges(s: &str) -> Result<Vec<Path>> {
    let mut paths: Vec<Path> = Vec::new();
    let mut rest = s;
    while let Some(c) = rest.chars().next() {
        rest = &rest[c.len_utf8()..];
        if c.is_whitespace() {
            continue;
        }
        if c == 'S' {
            rest = rest.trim_start_matches(|c: char| c.is_ascii_digit());
            continue;
        }
        if c == '!' {
            paths.push(Path { start: point(&mut rest)?, segs: Vec::new() });
            continue;
        }
        let seg = match c {
            '|' | '/' => Seg::Line(point(&mut rest)?),
            '[' | ']' => Seg::Quad(point(&mut rest)?, point(&mut rest)?),
            _ => return Err(format!("unexpected {c:?}").into()),
        };
        paths.last_mut().ok_or("segment before first move")?.segs.push(seg);
    }
    paths.retain(|p| !p.segs.is_empty());
    Ok(paths)
}

fn point(rest: &mut &str) -> Result<Point> {
    let mut coord = || -> Result<f32> {
        let s = rest.trim_start();
        let end = s.find(|c: char| !(c.is_ascii_hexdigit() || "#.-".contains(c))).unwrap_or(s.len());
        let (token, tail) = s.split_at(end);
        *rest = tail;
        Ok((twips(token)? / 20.0) as f32)
    };
    Ok((coord()?, coord()?))
}

/// An edge coordinate in twips: decimal, or `#` + signed 32-bit fixed point
/// in hex (24-bit integer part, 8-bit fraction).
fn twips(token: &str) -> Result<f64> {
    let Some(hex) = token.strip_prefix('#') else { return Ok(token.parse::<f64>().map_err(|e| format!("{token:?}: {e}"))?) };
    let (int, frac) = hex.split_once('.').unwrap_or((hex, ""));
    let bits = u32::from_str_radix(&format!("{int:0>6}{frac:0<2}"), 16).map_err(|e| format!("{token:?}: {e}"))?;
    Ok(bits as i32 as f64 / 256.0)
}

pub(crate) fn hex_color(s: &str) -> Result<[f32; 3]> {
    let v = u32::from_str_radix(s.trim_start_matches('#'), 16).map_err(|e| format!("color {s:?}: {e}"))?;
    Ok([v >> 16, v >> 8, v].map(|c| (c & 0xFF) as f32 / 255.0))
}

/// What the model doesn't read of `n`: attributes other than `attrs`, and child
/// elements other than `children`.
fn extra(n: Node, attrs: &[&str], children: &[&str]) -> Extra {
    let text = n.document().input_text();
    let mut extra = Extra::default();
    for a in n.attributes().filter(|a| a.namespace().is_some() || !attrs.contains(&a.name())) {
        extra.attrs.push((text[a.range_qname()].into(), a.value().into()));
    }
    // Prefixes declared here, which kept XML may use; the writer declares the default and `xsi`.
    let inherited = |ns: &roxmltree::Namespace| n.parent_element().is_some_and(|p| p.namespaces().any(|q| q == ns));
    for ns in n.namespaces().filter(|ns| !inherited(ns)) {
        if let Some(prefix) = ns.name().filter(|&p| p != "xsi") {
            extra.attrs.push((format!("xmlns:{prefix}"), ns.uri().into()));
        }
    }
    let mut read = false;
    for c in n.children().filter(Node::is_element) {
        if children.contains(&c.tag_name().name()) {
            read = true;
        } else {
            (if read { &mut extra.after } else { &mut extra.before }).push(source(c));
        }
    }
    extra
}

/// An element's XML, as it appears in the file.
fn source(n: Node) -> String {
    n.document().input_text()[n.range()].into()
}

fn child<'a, 'i>(n: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
    n.children().find(|c| c.tag_name().name() == name)
}

/// Element children of `n`'s `<wrapper>`, e.g. the `<DOMLayer>`s in `<layers>`.
fn list<'a, 'i>(n: Node<'a, 'i>, wrapper: &str) -> impl Iterator<Item = Node<'a, 'i>> + use<'a, 'i> {
    child(n, wrapper).into_iter().flat_map(|w| w.children().filter(Node::is_element))
}

fn attr<'a>(n: Node<'a, '_>, name: &str) -> Result<&'a str> {
    n.attribute(name).ok_or_else(|| format!("<{}> has no {name}", n.tag_name().name()).into())
}

fn num<T: FromStr>(n: Node, name: &str, default: T) -> Result<T>
where
    T::Err: Display,
{
    match n.attribute(name) {
        None => Ok(default),
        Some(v) => v.parse().map_err(|e| format!("<{}> {name}={v:?}: {e}", n.tag_name().name()).into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn twips_formats() {
        assert_eq!(twips("-12.5").unwrap(), -12.5);
        assert_eq!(twips("#A0.8").unwrap(), 160.5);
        assert_eq!(twips("#FFFE70").unwrap(), -400.0);
        assert_eq!(twips("#FFFFFF.80").unwrap(), -0.5);
    }

    fn parse_element(xml: &str) -> Element {
        let xml = roxmltree::Document::parse(xml).unwrap_or_else(|e| panic!("{e}: {xml}"));
        element(xml.root_element()).unwrap()
    }

    #[test]
    fn static_text() {
        let xml = r##"<DOMStaticText left="-1.5" width="146.95" height="31.1" isSelectable="false">
  <matrix>
    <Matrix a="2" d="2" tx="100" ty="50"/>
  </matrix>
  <textRuns>
    <DOMTextRun>
      <characters>Hello&#xD;wor</characters>
      <textAttrs>
        <DOMTextAttrs alignment="center" aliasText="false" letterSpacing="1.5" lineSpacing="4" size="24" bitmapSize="480" face="Arial-BoldMT" fillColor="#FF0000" alpha="0.5"/>
      </textAttrs>
    </DOMTextRun>
    <DOMTextRun>
      <characters>ld &amp; more</characters>
      <textAttrs>
        <DOMTextAttrs size="12" bitmapSize="240" face="TimesNewRomanPSMT" italic="true"/>
      </textAttrs>
    </DOMTextRun>
  </textRuns>
</DOMStaticText>"##;
        let Element::Text(t) = parse_element(xml) else { panic!("not text") };
        assert_eq!(t.kind, TextKind::Static);
        assert_eq!(t.width, Some(146.95), "no autoExpand: a fixed width");
        assert_eq!(t.matrix, Matrix { a: 2.0, d: 2.0, tx: 97.0, ty: 50.0, ..Matrix::IDENTITY }, "left runs along the matrix's x axis");
        assert_eq!((t.align, t.line_spacing), (TextAlign::Center, 4.0));
        assert_eq!(t.extra.attrs, [("isSelectable".to_string(), "false".to_string())]);
        let bold_red = TextRun {
            text: "Hello\nwor".into(),
            face: "Arial-BoldMT".into(),
            size: 24.0,
            color: [1.0, 0.0, 0.0, 0.5],
            bold: false,
            italic: false,
            letter_spacing: 1.5,
            extra: Extra { attrs: vec![("aliasText".into(), "false".into())], ..Extra::default() },
        };
        let italic = TextRun {
            text: "ld & more".into(),
            face: "TimesNewRomanPSMT".into(),
            size: 12.0,
            color: [0.0, 0.0, 0.0, 1.0],
            italic: true,
            letter_spacing: 0.0,
            extra: Extra::default(),
            ..bold_red.clone()
        };
        assert_eq!(t.runs, [bold_red, italic]);

        let extending = r#"<DOMStaticText width="50" height="20" autoExpand="true"><textRuns/></DOMStaticText>"#;
        let Element::Text(t) = parse_element(extending) else { panic!("not text") };
        assert_eq!((t.width, t.matrix, t.runs.len()), (None, Matrix::IDENTITY, 0));
        // Dynamic and input text have a fixed box unless they extend; its height and the line type stay as they were.
        let kept = |attrs: &[(&str, &str)]| attrs.iter().map(|&(k, v)| (k.to_string(), v.to_string())).collect::<Vec<_>>();
        let password = r#"<DOMInputText width="50" height="20" lineType="password"/>"#;
        let Element::Text(t) = parse_element(password) else { panic!("not text") };
        assert_eq!((t.kind, t.width), (TextKind::Input, Some(50.0)));
        assert_eq!(t.extra.attrs, kept(&[("height", "20"), ("lineType", "password")]));
        let dynamic = r#"<DOMDynamicText width="50" height="20" autoExpand="true" lineType="multiline"/>"#;
        let Element::Text(t) = parse_element(dynamic) else { panic!("not text") };
        assert_eq!((t.kind, t.width), (TextKind::Dynamic, None));
        assert_eq!(t.extra.attrs, kept(&[("lineType", "multiline")]));
    }

    #[test]
    fn a_document_needs_a_scene() {
        let xml = r#"<DOMDocument xmlns="http://ns.adobe.com/xfl/2008/" width="550" height="400"><timelines/></DOMDocument>"#;
        assert!(read_document(&mut |_| Ok(xml.into())).is_err());
    }

    #[test]
    fn edge_commands() {
        let paths = parse_edges("!0 0S2|20 40[0 0 #14 0!5 5").unwrap();
        assert_eq!(
            paths,
            [Path { start: (0.0, 0.0), segs: vec![Seg::Line((1.0, 2.0)), Seg::Quad((0.0, 0.0), (1.0, 0.0))] }]
        );
    }
}
