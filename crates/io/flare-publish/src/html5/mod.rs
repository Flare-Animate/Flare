//! Publishes a Flare document as one self-contained HTML5 page that plays it on a canvas.
//!
//! Like the SWF, the page is *baked*: each frame is evaluated with the editor's own evaluator,
//! so tweens become per-frame matrices and symbols are flattened into their shapes. Each distinct
//! shape is written once as SVG path data, each distinct frame once, and the order the frames play
//! in as a list of indexes, so a drawing held for many frames costs one entry. The page draws with
//! the browser's `Path2D` and 2D canvas; it needs no library and no network. Its settings are the
//! ones Animate's HTML5 Canvas publish settings call Loop timeline, Center stage and Make
//! responsive. It has been checked by reading its data back and by running its player against a
//! stand-in canvas, not in every browser.

use std::collections::HashMap;
use std::fmt::Write as _;

use flare_core::ShapeId;
use flare_document::{Cap, Document, Join, PathSegment, Shape};
use serde_json::{Value, json};

use crate::{Diagnostic, PublishError, Severity};

const PLAYER: &str = include_str!("player.js");

/// The most frames a page plays, so a long document cannot make an unbounded page.
const MAX_FRAMES: u64 = 100_000;
/// The most the page's data may take, in bytes.
pub const MAX_PAGE_BYTES: usize = 64 * 1024 * 1024;

/// What the page does; the names are Animate's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Html5Options {
    /// The page's title.
    pub title: String,
    /// Play again from the first frame after the last, instead of stopping on it.
    pub loop_timeline: bool,
    /// Put the stage in the middle of the browser window.
    pub center_stage: bool,
    /// Scale the stage to fit the browser window, keeping its proportions.
    pub responsive: bool,
}

impl Default for Html5Options {
    fn default() -> Self {
        Self {
            title: "Flare animation".to_owned(),
            loop_timeline: true,
            center_stage: true,
            responsive: false,
        }
    }
}

#[derive(Debug)]
pub struct PublishedHtml {
    pub html: String,
    /// Frames played, scene after scene.
    pub frames: u32,
    pub diagnostics: Vec<Diagnostic>,
}

/// A number the page reads back, kept short.
fn n(value: f64) -> Value {
    let rounded = (value * 10_000.0).round() / 10_000.0;
    // A whole number is written as one, so `1.0` does not take three characters.
    if rounded.fract() == 0.0 && rounded.abs() < 1.0e15 {
        json!(rounded as i64)
    } else {
        json!(rounded)
    }
}

fn color(c: flare_core::Rgba8) -> Value {
    json!([c.r, c.g, c.b, c.a])
}

/// The shape's outline as SVG path data.
fn path_data(shape: &Shape) -> String {
    let mut d = String::new();
    let point = |d: &mut String, p: &flare_core::DocumentPoint| {
        let v = p.as_vec();
        let _ = write!(d, " {} {}", n(v.x), n(v.y));
    };
    for segment in shape.path() {
        match segment {
            PathSegment::MoveTo(p) => {
                d.push('M');
                point(&mut d, p);
            }
            PathSegment::LineTo(p) => {
                d.push('L');
                point(&mut d, p);
            }
            PathSegment::CubicTo(a, b, c) => {
                d.push('C');
                point(&mut d, a);
                point(&mut d, b);
                point(&mut d, c);
            }
            PathSegment::Close => d.push('Z'),
        }
    }
    d
}

fn cap(cap: Cap) -> &'static str {
    match cap {
        Cap::Butt => "butt",
        Cap::Round => "round",
        Cap::Square => "square",
    }
}

fn join(join: Join) -> &'static str {
    match join {
        Join::Round => "round",
        Join::Bevel => "bevel",
        Join::Miter => "miter",
    }
}

/// What the page needs to paint a shape, or `None` for one that paints nothing.
fn describe(shape: &Shape) -> Option<Value> {
    let stroke = shape.stroke();
    if shape.fill_is_invisible() && stroke.is_none() {
        return None;
    }
    let mut out = json!({ "d": path_data(shape) });
    if let Some(g) = shape.gradient().filter(|g| !g.is_invisible()) {
        let (from, to) = (g.from().as_vec(), g.to().as_vec());
        out["grad"] = json!([
            n(from.x),
            n(from.y),
            n(to.x),
            n(to.y),
            color(g.start()),
            color(g.end())
        ]);
    } else if shape.gradient().is_none() && shape.fill().a > 0 {
        out["fill"] = color(shape.fill());
    }
    if let Some(stroke) = stroke {
        out["stroke"] = json!([
            n(stroke.width()),
            color(stroke.color()),
            cap(stroke.cap()),
            join(stroke.join())
        ]);
    }
    Some(out)
}

fn matrix(m: glamx::DAffine2) -> [Value; 6] {
    m.to_cols_array().map(n)
}

fn escape_html(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c if c.is_control() && c != '\n' => out.push(' '),
            c => out.push(c),
        }
    }
    out
}

/// JSON that is safe inside a `<script>` element: nothing in it can close the element or start
/// a comment, whatever the document's text held.
fn script_safe(json: &str) -> String {
    json.replace('<', "\\u003c")
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029")
}

/// Publishes `document`, scene after scene, as an HTML5 page.
pub fn publish_html5(
    document: &Document,
    options: &Html5Options,
) -> Result<PublishedHtml, PublishError> {
    let mut diagnostics = Vec::new();
    let total: u64 = document
        .scenes()
        .iter()
        .map(|s| s.timeline().frame_count().max(1))
        .sum();
    if total > MAX_FRAMES {
        return Err(PublishError::TooManyFrames(total));
    }
    if document.scenes().len() > 1 {
        diagnostics.push(Diagnostic {
            severity: Severity::Warning,
            message: "the page plays one timeline, so the scenes were played one after another"
                .into(),
        });
    }
    if !document.sounds().is_empty() {
        diagnostics.push(Diagnostic {
            severity: Severity::Warning,
            message: "HTML5 publishing does not write sounds yet, so the page is silent".into(),
        });
    }
    diagnostics.push(Diagnostic {
        severity: Severity::Info,
        message: "frames were baked: tweens became per-frame positions and symbols were flattened"
            .into(),
    });

    let mut shapes: Vec<Value> = Vec::new();
    let mut known: HashMap<(ShapeId, u64), Option<usize>> = HashMap::new();
    let mut frames: Vec<Value> = Vec::new();
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut seq: Vec<usize> = Vec::new();
    let mut index_of = |shape: &Shape, phase: u64, shapes: &mut Vec<Value>| -> Option<usize> {
        *known.entry((shape.id(), phase)).or_insert_with(|| {
            describe(shape).map(|value| {
                shapes.push(value);
                shapes.len() - 1
            })
        })
    };

    for scene in document.scenes() {
        let length = u32::try_from(scene.timeline().frame_count().max(1)).unwrap_or(u32::MAX);
        for frame in 0..length {
            let evaluated =
                flare_eval::evaluate_frame(document, scene.id(), frame).map_err(|e| {
                    PublishError::Evaluate {
                        frame: u32::try_from(seq.len()).unwrap_or(u32::MAX),
                        message: e.to_string(),
                    }
                })?;
            let mut drawn: Vec<Value> = Vec::new();
            for item in &evaluated.shapes {
                let Some(index) = index_of(&item.shape, item.phase, &mut shapes) else {
                    continue;
                };
                let m = matrix(item.transform);
                let mut entry = vec![json!(index)];
                entry.extend(m);
                if !item.clips.is_empty() {
                    entry.push(json!(item.clips));
                }
                drawn.push(Value::Array(entry));
            }
            let mut clips: Vec<Value> = Vec::new();
            for region in &evaluated.clips {
                let mut members: Vec<Value> = Vec::new();
                for member in region {
                    let Some(index) = index_of(&member.shape, member.phase, &mut shapes) else {
                        continue;
                    };
                    let mut entry = vec![json!(index)];
                    entry.extend(matrix(member.transform));
                    members.push(Value::Array(entry));
                }
                clips.push(Value::Array(members));
            }
            let built = json!({ "shapes": drawn, "clips": clips });
            let key = built.to_string();
            let at = *seen.entry(key).or_insert_with(|| {
                frames.push(built);
                frames.len() - 1
            });
            seq.push(at);
        }
    }

    let stage = document.stage();
    let rate = document.frame_rate();
    let fps = f64::from(rate.numerator()) / f64::from(rate.denominator());
    let data = json!({
        "w": stage.width(),
        "h": stage.height(),
        "fps": n(fps),
        "bg": color(stage.background()),
        "loop": options.loop_timeline,
        "responsive": options.responsive,
        "shapes": shapes,
        "frames": frames,
        "seq": seq,
    });
    let data = script_safe(&data.to_string());
    if data.len() > MAX_PAGE_BYTES {
        return Err(PublishError::PageTooLarge(MAX_PAGE_BYTES));
    }
    let background = stage.background();
    let centering = if options.center_stage {
        "display:flex;justify-content:center;align-items:center;"
    } else {
        ""
    };
    let html = format!(
        "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>{title}</title>\n\
         <style>html,body{{margin:0;height:100%;background:#{r:02x}{g:02x}{b:02x}}}\
         body{{{centering}}}canvas{{display:block}}</style>\n</head>\n<body>\n\
         <canvas id=\"stage\"></canvas>\n\
         <script id=\"flare-data\" type=\"application/json\">{data}</script>\n\
         <script>\n{PLAYER}</script>\n</body>\n</html>\n",
        title = escape_html(&options.title),
        r = background.r,
        g = background.g,
        b = background.b,
    );
    Ok(PublishedHtml {
        html,
        frames: u32::try_from(seq.len()).unwrap_or(u32::MAX),
        diagnostics,
    })
}
