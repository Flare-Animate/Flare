//! A sprite sheet: the frames of the first scene laid out in a grid on one PNG, with a JSON
//! atlas in the "hash" shape that game engines read (frame rectangles, source sizes, durations
//! and the frame rate). Frames are drawn at the stage's size over a transparent background so a
//! game composites them itself.

use std::fmt::Write as _;

use flare_core::Rgba8;
use flare_document::Document;

/// Most frames on one sheet.
pub const MAX_SHEET_FRAMES: u32 = 2000;
/// Longest side of the sheet in pixels (the usual texture limit).
pub const MAX_SHEET_SIDE: u64 = 16_384;

/// The sheet and its atlas, ready to write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpriteSheet {
    pub png: Vec<u8>,
    pub json: String,
    pub width: u32,
    pub height: u32,
    pub frames: u32,
}

/// Renders frames `start..=end` (the whole scene when `end` is `None`) into a grid of `columns`
/// (about square when `None`). `image_name` is what the atlas says the PNG is called.
pub fn sprite_sheet(
    document: &Document,
    start: u32,
    end: Option<u32>,
    columns: Option<u32>,
    image_name: &str,
) -> Result<SpriteSheet, String> {
    let scene = document
        .scenes()
        .first()
        .ok_or("The document has no scene to export.")?;
    let length = scene.timeline().frame_count().max(1);
    let last = end.map_or(length - 1, u64::from);
    if u64::from(start) > last {
        return Err(format!(
            "The first frame {start} is after the last, {last}."
        ));
    }
    if last >= length {
        return Err(format!(
            "Frame {last} is past the end; the scene has {length} frames."
        ));
    }
    let count = u32::try_from(last - u64::from(start) + 1).unwrap_or(u32::MAX);
    if count > MAX_SHEET_FRAMES {
        return Err(format!(
            "{count} frames is more than a sheet holds ({MAX_SHEET_FRAMES})."
        ));
    }
    let columns = columns.unwrap_or_else(|| f64::from(count).sqrt().ceil() as u32);
    if columns == 0 || columns > count {
        return Err(format!("{columns} columns does not fit {count} frames."));
    }
    let rows = count.div_ceil(columns);
    let stage = document.stage();
    let (fw, fh) = (stage.width(), stage.height());
    let (sheet_w, sheet_h) = (
        u64::from(fw) * u64::from(columns),
        u64::from(fh) * u64::from(rows),
    );
    if sheet_w > MAX_SHEET_SIDE || sheet_h > MAX_SHEET_SIDE {
        return Err(format!(
            "The sheet would be {sheet_w} by {sheet_h} pixels; the limit is {MAX_SHEET_SIDE} on a side."
        ));
    }
    let (sheet_w, sheet_h) = (sheet_w as u32, sheet_h as u32);
    let mut pixels = vec![0u8; sheet_w as usize * sheet_h as usize * 4];
    let duration = (1000.0 / document.frame_rate().per_second()).round();
    if !duration.is_finite() || duration <= 0.0 {
        return Err("The frame rate must be above zero to export a sprite sheet.".to_owned());
    }
    let mut json = String::from("{\n  \"frames\": {\n");
    for i in 0..count {
        let frame = start + i;
        let drawn = flare_eval::evaluate_frame(document, scene.id(), frame)
            .map_err(|e| format!("Cannot draw frame {}: {e}", frame + 1))?;
        let image = flare_render::render(&drawn, fw, fh, Rgba8::TRANSPARENT)
            .map_err(|e| format!("Cannot render frame {}: {e}", frame + 1))?;
        let (x, y) = ((i % columns) * fw, (i / columns) * fh);
        for row in 0..fh as usize {
            let from = row * fw as usize * 4;
            let to = ((y as usize + row) * sheet_w as usize + x as usize) * 4;
            pixels[to..to + fw as usize * 4]
                .copy_from_slice(&image.rgba()[from..from + fw as usize * 4]);
        }
        let comma = if i + 1 < count { "," } else { "" };
        let _ = writeln!(
            json,
            "    \"frame_{frame:05}\": {{\"frame\": {{\"x\": {x}, \"y\": {y}, \"w\": {fw}, \"h\": {fh}}}, \"rotated\": false, \"trimmed\": false, \"spriteSourceSize\": {{\"x\": 0, \"y\": 0, \"w\": {fw}, \"h\": {fh}}}, \"sourceSize\": {{\"w\": {fw}, \"h\": {fh}}}, \"duration\": {duration}}}{comma}"
        );
    }
    let _ = write!(
        json,
        "  }},\n  \"meta\": {{\"app\": \"Flare\", \"image\": {}, \"format\": \"RGBA8888\", \"size\": {{\"w\": {sheet_w}, \"h\": {sheet_h}}}, \"scale\": \"1\", \"frameRate\": {}, \"frameCount\": {count}, \"columns\": {columns}}}\n}}\n",
        quote(image_name),
        document.frame_rate().per_second()
    );
    let mut png = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png, sheet_w, sheet_h);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .map_err(|e| format!("Cannot write the PNG: {e}"))?;
        writer
            .write_image_data(&pixels)
            .map_err(|e| format!("Cannot write the PNG: {e}"))?;
    }
    Ok(SpriteSheet {
        png,
        json,
        width: sheet_w,
        height: sheet_h,
        frames: count,
    })
}

/// `text` as a JSON string.
pub(crate) fn quote(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
#[path = "sprites_tests.rs"]
mod tests;
