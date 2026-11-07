//! One frame of the stage as an SVG picture: each shape a path with its fill, gradient, stroke and
//! masks, each picture a PNG inside the file, all in the order they draw. What SVG cannot say (a
//! dashed or dotted stroke is drawn solid) is listed in the notes, never dropped silently.

use std::collections::HashMap;
use std::fmt::Write as _;

use base64::Engine as _;
use flare_core::{BitmapId, Rgba8};
use flare_document::{Bitmap, PathSegment, Shape, StrokeStyle};
use flare_eval::{DrawnBitmap, DrawnShape, Frame};

/// An SVG file's text, and what the file leaves out of the frame.
#[derive(Debug, Clone, PartialEq)]
pub struct Svg {
    pub text: String,
    pub notes: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum SvgError {
    #[error("a picture in the frame could not be written as PNG: {0}")]
    Png(#[from] png::EncodingError),
}

/// Writes `frame` as an SVG of `width` by `height` over `background`.
pub fn frame_svg(
    frame: &Frame<'_>,
    width: u32,
    height: u32,
    background: Rgba8,
) -> Result<Svg, SvgError> {
    let mut body = String::new();
    let mut defs = String::new();
    let mut notes: Vec<String> = Vec::new();
    let mut gradients = 0usize;
    let mut pictures: HashMap<BitmapId, String> = HashMap::new();

    if background.a > 0 {
        writeln!(
            body,
            r#"<rect width="{width}" height="{height}" {}/>"#,
            paint("fill", background, 1.0)
        )
        .ok();
    }
    for (index, clip) in frame.clips.iter().enumerate() {
        writeln!(defs, r#"<clipPath id="clip{index}">"#).ok();
        for member in clip {
            let path = path_data(member.shape.path());
            writeln!(
                defs,
                r#"<path d="{path}" transform="{}"/>"#,
                matrix(&member.transform.to_cols_array())
            )
            .ok();
        }
        defs.push_str("</clipPath>\n");
    }

    let mut next_picture = 0;
    for (index, drawn) in frame.shapes.iter().enumerate() {
        while let Some(picture) = frame.bitmaps.get(next_picture) {
            if picture.after_shapes > index {
                break;
            }
            picture_element(picture, &mut body, &mut pictures)?;
            next_picture += 1;
        }
        shape_element(drawn, &mut body, &mut defs, &mut gradients, &mut notes);
    }
    for picture in &frame.bitmaps[next_picture..] {
        picture_element(picture, &mut body, &mut pictures)?;
    }

    let mut text = format!(
        concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink""#,
            r#" width="{width}" height="{height}" viewBox="0 0 {width} {height}">"#,
            "\n"
        ),
        width = width,
        height = height
    );
    if !defs.is_empty() {
        text.push_str("<defs>\n");
        text.push_str(&defs);
        text.push_str("</defs>\n");
    }
    text.push_str(&body);
    text.push_str("</svg>\n");
    notes.sort();
    notes.dedup();
    Ok(Svg { text, notes })
}

fn shape_element(
    drawn: &DrawnShape<'_>,
    body: &mut String,
    defs: &mut String,
    gradients: &mut usize,
    notes: &mut Vec<String>,
) {
    let shape: &Shape = &drawn.shape;
    let fill = match shape.gradient() {
        Some(gradient) => {
            let id = format!("grad{gradients}");
            *gradients += 1;
            let (from, to) = (gradient.from().as_vec(), gradient.to().as_vec());
            writeln!(
                defs,
                concat!(
                    r#"<linearGradient id="{id}" gradientUnits="userSpaceOnUse" "#,
                    r#"x1="{}" y1="{}" x2="{}" y2="{}">"#
                ),
                from.x,
                from.y,
                to.x,
                to.y,
                id = id
            )
            .ok();
            for (offset, color) in [(0, gradient.start()), (1, gradient.end())] {
                writeln!(
                    defs,
                    r#"<stop offset="{offset}" {}/>"#,
                    paint("stop-color", color, 1.0)
                )
                .ok();
            }
            defs.push_str("</linearGradient>\n");
            format!(r#"fill="url(#{id})""#)
        }
        None => paint("fill", shape.fill(), 1.0),
    };
    let stroke = match shape.stroke() {
        Some(stroke) => {
            if stroke.style() != StrokeStyle::Solid {
                notes.push(
                    "a dashed, dotted, stippled, hatched or otherwise patterned stroke is written as a solid line"
                        .to_owned(),
                );
            }
            let cap = match stroke.cap() {
                flare_document::Cap::Butt => "butt",
                flare_document::Cap::Round => "round",
                flare_document::Cap::Square => "square",
            };
            let join = match stroke.join() {
                flare_document::Join::Round => "round",
                flare_document::Join::Bevel => "bevel",
                flare_document::Join::Miter => "miter",
            };
            format!(
                r#" {} stroke-width="{}" stroke-linecap="{cap}" stroke-linejoin="{join}""#,
                paint("stroke", stroke.color(), stroke.opacity()),
                stroke.width()
            )
        }
        None => String::new(),
    };
    let element = format!(
        r#"<path d="{}" {fill}{stroke} transform="{}"/>"#,
        path_data(shape.path()),
        matrix(&drawn.transform.to_cols_array())
    );
    body.push_str(&wrap_in_masks(&element, &drawn.clips));
    body.push('\n');
}

fn picture_element(
    picture: &DrawnBitmap<'_>,
    body: &mut String,
    pictures: &mut HashMap<BitmapId, String>,
) -> Result<(), SvgError> {
    let href = match pictures.get(&picture.bitmap.id()) {
        Some(href) => href.clone(),
        None => {
            let href = png_data_uri(picture.bitmap)?;
            pictures.insert(picture.bitmap.id(), href.clone());
            href
        }
    };
    let element = format!(
        r#"<image width="{}" height="{}" xlink:href="{href}" opacity="{}" transform="{}"/>"#,
        picture.bitmap.width(),
        picture.bitmap.height(),
        picture.opacity,
        matrix(&picture.transform.to_cols_array())
    );
    body.push_str(&wrap_in_masks(&element, &picture.clips));
    body.push('\n');
    Ok(())
}

/// The element inside one group per mask it is clipped by, outermost first.
fn wrap_in_masks(element: &str, clips: &[usize]) -> String {
    let mut out = element.to_owned();
    for index in clips.iter().rev() {
        out = format!(r#"<g clip-path="url(#clip{index})">{out}</g>"#);
    }
    out
}

/// A `fill`, `stop-color` or `stroke` attribute for `color` with `opacity` on top of its alpha.
fn paint(attribute: &str, color: Rgba8, opacity: f64) -> String {
    let alpha = f64::from(color.a) / 255.0 * opacity.clamp(0.0, 1.0);
    if attribute == "fill" && color.a == 0 {
        return r#"fill="none""#.to_owned();
    }
    let hex = format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b);
    let opacity_attr = match attribute {
        "fill" => "fill-opacity",
        "stroke" => "stroke-opacity",
        _ => "stop-opacity",
    };
    format!(r#"{attribute}="{hex}" {opacity_attr}="{alpha}""#)
}

/// SVG path data for `segments`.
fn path_data(segments: &[PathSegment]) -> String {
    let point = |p: &flare_core::DocumentPoint| {
        let v = p.as_vec();
        format!("{} {}", v.x, v.y)
    };
    let mut out = String::new();
    for segment in segments {
        match segment {
            PathSegment::MoveTo(p) => write!(out, "M{} ", point(p)).ok(),
            PathSegment::LineTo(p) => write!(out, "L{} ", point(p)).ok(),
            PathSegment::CubicTo(a, b, c) => {
                write!(out, "C{} {} {} ", point(a), point(b), point(c)).ok()
            }
            PathSegment::Close => write!(out, "Z ").ok(),
        };
    }
    out.trim_end().to_owned()
}

/// `matrix(a b c d e f)` for an affine transform's six columns.
fn matrix(cols: &[f64; 6]) -> String {
    format!(
        "matrix({} {} {} {} {} {})",
        cols[0], cols[1], cols[2], cols[3], cols[4], cols[5]
    )
}

/// A picture's pixels as a PNG in a data URI.
fn png_data_uri(bitmap: &Bitmap) -> Result<String, SvgError> {
    let mut png_bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png_bytes, bitmap.width(), bitmap.height());
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header()?.write_image_data(bitmap.pixels())?;
    }
    Ok(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(png_bytes)
    ))
}
