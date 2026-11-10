//! Import of other art tools' pen profiles: the named brush settings a person has kept in
//! Photoshop (`.abr`) or Krita (`.kpp` under `bundle`). What comes back is a [`Vec`] of
//! `flare_tools`' [`BrushPreset`]s, already sanitized, in the file's own order.
//!
//! A Photoshop or Krita brush is described by its maker as a stamped tip with on-disk bitmaps and
//! descriptor dynamics; Flare's brush paints a filled outline that answers pressure, speed and
//! flow. Only the parts that map over are carried: the name, the nib's angle and roundness, and
//! the pressure response. A tip that is a picture has nothing to map to, so a sampled `.abr` set
//! keeps its presets' names and their shape settings while the stamps themselves stay behind; a
//! format whose brushes are only pictures (GIMP `.gbr`, MyPaint `.myb`) says so rather than
//! guessing.

mod abr;
mod kpp;

use std::path::Path;

/// A whole profile collection does not come in bigger than this, the same bound as any other
/// foreign file.
pub const MAX_OWNED_BYTES: u64 = 64 * 1024 * 1024;

/// What went wrong reading a pen-profile file. Every word of it names the file's format, so a
/// person knows which of their tools' brushes did or did not arrive.
#[derive(Debug, thiserror::Error)]
pub enum BrushProfileError {
    /// The bytes are the named format, but a part of it Flare has no reader for yet.
    #[error("{0}")]
    Unsupported(String),
    /// Not any brush profile format at all.
    #[error("not a pen profile file: {0}")]
    NotRecognized(String),
    /// The file ends before its tables do.
    #[error("{0}")]
    Truncated(String),
    /// A constraint no real file breaks is broken.
    #[error("{0}")]
    Corrupt(String),
    /// Beyond [`MAX_OWNED_BYTES`].
    #[error("the file is larger than {MAX_OWNED_BYTES} bytes")]
    TooLarge,
}

/// Where [`import_profile_bytes`] sends the bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileKind {
    /// Photoshop 6 to CS's legacy `.abr` (versions 1 and 2).
    Abr,
    /// Krita's brush preset file.
    Kpp,
    /// Krita's brush bundle: a zip of `.kpp` files.
    Bundle,
}

/// Which of the kinds `path` names, by its extension.
pub fn profile_kind(path: &Path) -> Option<ProfileKind> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    match extension.as_str() {
        "abr" => Some(ProfileKind::Abr),
        "kpp" => Some(ProfileKind::Kpp),
        "bundle" => Some(ProfileKind::Bundle),
        _ => None,
    }
}

/// Reads whichever profile file `name` names out of `bytes`.
pub fn import_profile_bytes(name: &Path, bytes: &[u8]) -> Result<Vec<BrushPresetEntry>, ...> {
    unimplemented!()
}

/// The brushes of a file, in the file's order, plus anything about them a person should see.
///
/// This is a separate type rather than a bare [`Vec`] of presets because a profile file says more
/// than its brushes: how many of them do not map (say, tips without settings), so what a person
/// gets is said, not guessed at.
#[derive(Clone, Debug, PartialEq)]
pub struct ProfileSet {
    /// The file's presets, already [`BrushPreset::sanitized`].
    pub presets: Vec<BrushPreset>,
    /// Notes of what the file held that the import did not bring, one per kind of thing.
    pub notes: Vec<String>,
}

/// The names for a brush a file does not give: how Flare's shelf wants them.
const PLACEHOLDER_PREFIX: &str = "Brush";

/// Passes over a byte reader with a plain u16 at the front; most profile tables start that way.
mod shared {
    /// `bytes` is at least `needed` long.
    pub const fn enough(bytes: &[u8], needed: usize) -> bool {
        bytes.len() >= needed
    }

    /// An unsigned big-endian integer of `2` bytes.
    pub const fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
        let hi = *bytes.get(at)?;
        let lo = *bytes.get(at + 1)?;
        Some((u16::from(hi) << 8) | u16::from(lo))
    }

    /// A signed big-endian integer of `4` bytes.
    pub const fn i32_at(bytes: &[u8], at: usize) -> Option<i32> {
        let b0 = *bytes.get(at)?;
        let b1 = *bytes.get(at + 1)?;
        let b2 = *bytes.get(at + 2)?;
        let b3 = *bytes.get(at + 3)?;
        Some(i32::from_be_bytes([b0, b1, b2, b3]))
    }
}

/// Strips control characters and keeps a name inside what a shelf can keep, in English when the
/// file gives none.
pub(crate) fn safe_name(name: Option<&str>, fallback: usize) -> String {
    let chars: String = name
        .unwrap_or("")
        .chars()
        .filter(|c| !c.is_control())
        .take(40)
        .collect();
    if chars.trim().is_empty() {
        format!("{PLACEHOLDER_PREFIX} {fallback}")
    } else {
        chars.trim().to_owned()
    }
}
