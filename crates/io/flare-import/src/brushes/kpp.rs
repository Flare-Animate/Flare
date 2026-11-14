//! Krita's `.kpp` paint-op preset. A `.kpp` file is a zip holding an XML description (and a
//! thumbnail and a stamp this import leaves behind), and the same XML arrives as plain text
//! when a person unpacks one. Both are read here with a tolerant scan: Krita writes far more
//! than Flare's pen profile holds, so the reader looks for the few things that map — the name,
//! the angle, the tip's ratio, the opacity and the size-response curve — and says what stayed
//! behind rather than guessing at the rest.

use std::io::Read;

use flare_tools::brush::BrushPreset;

use super::{BrushProfileError, ProfileSet};

/// The most text one preset's XML may be. A real one is a few kilobytes.
const MAX_TEXT_BYTES: usize = 1024 * 1024;

/// The window of text searched after an option's name for its value.
const OPTION_WINDOW: usize = 600;

pub(super) fn import(name: Option<&str>, bytes: &[u8]) -> Result<ProfileSet, BrushProfileError> {
    let text = text_of(bytes)?;
    if !is_krita(&text) {
        return Err(BrushProfileError::NotRecognized(
            "the file holds no Krita preset marker".to_owned(),
        ));
    }
    let mut preset = BrushPreset {
        name: crate::brushes::safe_name(
            preset_name(&text)
                .or_else(|| name.map(|n| n.to_owned()))
                .as_deref(),
            1,
        ),
        ..BrushPreset::default()
    };
    let mut notes: Vec<String> = Vec::new();

    if let Some(angle) = option_number(&text, "Angle") {
        preset.angle = clamp(angle, -360.0, 360.0) as f32;
    }
    if let Some(ratio) = option_number(&text, "Ratio").or_else(|| option_number(&text, "ratio")) {
        // Krita's ratio is the tip's width over height; Flare's flatness is the height over
        // the width, as a percentage of a round tip.
        if ratio > 0.0 {
            preset.flatness = clamp(100.0 / ratio, 1.0, 10_000.0) as f32;
        }
    }
    if let Some(opacity) = option_number(&text, "Opacity") {
        preset.flow = clamp(opacity, 0.0, 1.0) as f32;
    }
    match curve_of(&text) {
        Curve::Points(points) => {
            preset = preset.with_pressure_points(Some(points));
        }
        Curve::None => notes.push(
            "the preset's size response is not a drawn curve, so Flare's own pressure curve is \
             used"
                .to_owned(),
        ),
    }
    notes.push(
        "the preset's texture, colour dynamics and stamp stay in Krita; only the nib and the \
         pen response came over"
            .to_owned(),
    );
    Ok(ProfileSet {
        presets: vec![preset.sanitized()],
        notes,
    })
}

/// The XML of a `.kpp`: either the file is the zip Krita writes, or it is the XML itself.
fn text_of(bytes: &[u8]) -> Result<String, BrushProfileError> {
    if bytes.len() >= 4 && bytes[..4] == [0x50, 0x4b, 0x03, 0x04] {
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))
            .map_err(|error| BrushProfileError::Corrupt(format!("the preset zip: {error}")))?;
        for index in 0..archive.len() {
            let mut member = archive
                .by_index(index)
                .map_err(|error| BrushProfileError::Corrupt(format!("the preset zip: {error}")))?;
            let name = member
                .name()
                .map_err(|error| BrushProfileError::Corrupt(format!("the preset zip: {error}")))?
                .into_owned();
            let is_xml = name.ends_with(".image.xml") || name.ends_with(".xml");
            if !is_xml {
                continue;
            }
            let mut text = String::new();
            let read = member
                .by_ref()
                .take(MAX_TEXT_BYTES as u64)
                .read_to_string(&mut text)
                .map_err(|error| BrushProfileError::Corrupt(format!("the preset XML: {error}")))?;
            if read > 0 {
                return Ok(text);
            }
        }
        Err(BrushProfileError::Corrupt(
            "the preset zip holds no XML description".to_owned(),
        ))
    } else {
        let end = bytes.len().min(MAX_TEXT_BYTES);
        Ok(String::from_utf8_lossy(&bytes[..end]).into_owned())
    }
}

/// A Krita preset names itself, its paint op or its maker somewhere in the text.
fn is_krita(text: &str) -> bool {
    let lowered = text.to_ascii_lowercase();
    lowered.contains("paintoppreset") || lowered.contains("krita")
}

/// The preset's own name: the first `name="…"` in the text.
fn preset_name(text: &str) -> Option<String> {
    quoted_after(text, "name=\"", 0)
}

/// A number an option holds. Krita writes options as `name="Angle"` blocks whose value is an
/// attribute (`a="30"`, `ratio="2"`) or XML text, so the scan takes the first number in the
/// window after the name, in quotes first.
fn option_number(text: &str, option: &str) -> Option<f64> {
    let at = text.find(option)?;
    let start = at + option.len();
    let end = start.saturating_add(OPTION_WINDOW).min(text.len());
    let window = text.get(start..end).unwrap_or("");
    if let Some(quoted) = quoted_number(window) {
        return Some(quoted);
    }
    bare_number(window)
}

/// The first `d.d` in double quotes in `text`.
fn quoted_number(text: &str) -> Option<f64> {
    for (at, _) in text.match_indices('"') {
        let rest = text.get(at + 1..)?;
        let end = rest.find('"')?;
        if let Some(value) = rest.get(..end).and_then(|t| t.parse::<f64>().ok()) {
            return Some(value);
        }
    }
    None
}

/// The first `d.d` standing as XML text in `text`.
fn bare_number(text: &str) -> Option<f64> {
    for (at, _) in text.match_indices('<') {
        let rest = text.get(at + 1..)?;
        let end = rest.find('>')?;
        if let Some(value) = rest.get(..end).and_then(|t| t.trim().parse::<f64>().ok()) {
            return Some(value);
        }
    }
    None
}

/// The text between `marker` and the next quote, searched from `from`.
fn quoted_after(text: &str, marker: &str, from: usize) -> Option<String> {
    let at = text.get(from..)?.find(marker)? + from + marker.len();
    let rest = text.get(at..)?;
    let end = rest.find('"')?;
    Some(rest.get(..end)?.to_owned())
}

/// What the preset's curve holds. Krita writes a curve as `x:y` points separated by `;`.
fn curve_of(text: &str) -> Curve {
    let mut rest = text;
    while let Some(at) = rest.find("<curve") {
        let after = rest.get(at..).unwrap_or("");
        let close = after.find("</curve>").unwrap_or(after.len());
        let body = after.get(..close).unwrap_or("");
        let body = body.split_once('>').map(|(_, rest)| rest).unwrap_or(body);
        let points = curve_points(body);
        if points.len() >= 2 {
            return Curve::Points(points);
        }
        rest = rest.get(at + 6..).unwrap_or("");
    }
    Curve::None
}

enum Curve {
    Points(Vec<(f32, f32)>),
    None,
}

/// The `x:y` pairs of one curve's body, in order, trimmed to Flare's curve size upstream.
fn curve_points(body: &str) -> Vec<(f32, f32)> {
    let mut points = Vec::new();
    for part in body.split(';') {
        let Some((x, y)) = part.trim().split_once(':') else {
            continue;
        };
        if let (Ok(x), Ok(y)) = (x.trim().parse::<f32>(), y.trim().parse::<f32>()) {
            points.push((x, y));
        }
    }
    points
}

fn clamp(value: f64, low: f64, high: f64) -> f64 {
    if value.is_nan() {
        low
    } else {
        value.clamp(low, high)
    }
}
