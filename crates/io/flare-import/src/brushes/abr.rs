//! Photoshop's legacy `.abr`: versions 1 and 2, a plain table of brush records with a size to
//! resync by. The descriptor formats (6 and later) are refused before this reader is reached;
//! this is the code path GIMP and Krita share.
//!
//! A record is `i16` type, `i32` size, and then the body the size covers: a sampled brush (type
//! 2) carries `misc`, `spacing` and, in version 2, a UCS-2BE name; a computed brush (type 1) is
//! opaque data GIMP and Krita skip. Only the name maps onto Flare's presets — a legacy tip holds
//! no nib geometry — so a set imports as named plain nibs, in the file's order, resyncing past
//! whatever a record holds.

use flare_tools::brush::BrushPreset;

use super::{BrushProfileError, PLACEHOLDER_PREFIX, ProfileSet, safe_name, shared};

pub(super) fn import(bytes: &[u8]) -> Result<ProfileSet, BrushProfileError> {
    let version =
        shared::u16_at(bytes, 0).ok_or_else(|| truncated("the file has no .abr header", 0))?;
    let count =
        shared::u16_at(bytes, 2).ok_or_else(|| truncated("the file has no .abr header", 2))?;
    match version {
        1 | 2 => {}
        6 | 7 | 9 | 10 => {
            return Err(BrushProfileError::Unsupported(format!(
                "Photoshop .abr version {version} keeps its brushes as descriptor blocks, which \
                 Flare's pen-profile import does not read yet; the legacy versions 1 and 2 do"
            )));
        }
        other => {
            return Err(BrushProfileError::Corrupt(format!(
                "not an .abr version Flare knows: {other}"
            )));
        }
    }

    let mut presets: Vec<BrushPreset> = Vec::new();
    let mut notes: Vec<String> = Vec::new();
    let mut unnamed = 0usize;
    let mut computed = 0usize;
    let mut at = 4usize;
    for _ in 0..count {
        let head = bytes
            .get(at..at + 6)
            .ok_or_else(|| truncated("the file ends inside a brush record", at))?;
        let kind =
            shared::u16_at(head, 0).ok_or_else(|| truncated("a record has no type", at))? as i16;
        let size = shared::i32_at(head, 2).ok_or_else(|| truncated("a record has no size", at))?;
        if size < 0 {
            return Err(BrushProfileError::Corrupt(format!(
                "brush record {kind} says it is {size} bytes long"
            )));
        }
        let end = at + 6 + size as usize;
        let body = bytes
            .get(at + 6..end)
            .ok_or_else(|| truncated("a brush record is longer than the file", at))?;
        match kind {
            // A sampled tip: misc, spacing, and a name in version 2. The stamps stay behind;
            // Flare's preset is the name and the pen response, which a legacy file does not
            // describe, so the default one is used.
            2 => {
                let name = if version == 2 {
                    sampled_name(body)?
                } else {
                    None
                };
                if name.is_none() {
                    unnamed += 1;
                }
                presets.push(BrushPreset {
                    name: safe_name(name.as_deref(), presets.len() + 1),
                    ..BrushPreset::default()
                });
            }
            // A computed brush: no pixels, no name, an opaque body GIMP and Krita also skip.
            // It is still one of the person's brushes, so it is kept under a placeholder name.
            1 => {
                computed += 1;
                presets.push(BrushPreset {
                    name: safe_name(None, presets.len() + 1),
                    ..BrushPreset::default()
                });
            }
            other => {
                notes.push(format!("brush {other} of an unknown kind was left out"));
            }
        }
        // The size is the anchor: whatever the body holds, the next record starts here.
        at = end;
    }
    if unnamed > 0 {
        notes.push(format!(
            "{unnamed} of the brushes have no name in the file and are kept under \
             {PLACEHOLDER_PREFIX} placeholder names"
        ));
    }
    if computed > 0 {
        notes.push(format!(
            "{computed} computed brushes carry no tip to draw with and are kept as plain nibs"
        ));
    }
    Ok(ProfileSet { presets, notes })
}

/// The name of a version-2 sampled record: after `misc` (`i32`) and `spacing` (`i16`), a `u32`
/// character count, then that many UCS-2BE characters, possibly ending in a null. A name is
/// display only, so a body that does not hold one whole is treated as unnamed rather than
/// refusing the brush.
fn sampled_name(body: &[u8]) -> Result<Option<String>, BrushProfileError> {
    let chars = shared::i32_at(body, 6).ok_or_else(|| truncated("a name has no length", 6))?;
    if chars < 0 {
        return Err(BrushProfileError::Corrupt(format!(
            "a brush name says it is {chars} characters long"
        )));
    }
    let chars = chars as usize;
    let span = chars.saturating_mul(2);
    let Some(raw) = body.get(10..10 + span) else {
        return Ok(None);
    };
    let mut name = String::new();
    let (pairs, _) = raw.as_chunks::<2>();
    for pair in pairs {
        let unit = u16::from(pair[0]) << 8 | u16::from(pair[1]);
        if unit == 0 {
            continue;
        }
        if let Some(c) = char::from_u32(u32::from(unit)) {
            name.push(c);
        }
    }
    Ok(Some(name))
}

fn truncated(what: &str, at: usize) -> BrushProfileError {
    BrushProfileError::Truncated(format!("{what} (at byte {at})"))
}
