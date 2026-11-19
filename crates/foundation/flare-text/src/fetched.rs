//! Fonts that arrive while the program runs: the Google Fonts families a front end downloads
//! because text names them and the machine lacks them.

use skrifa::raw::TableProvider;
use skrifa::string::StringId;
use skrifa::{FontRef, MetadataProvider};

use crate::google::{self, GoogleFamily, MAX_FAMILY_FILES, MAX_FONT_BYTES};
use crate::{TextEngine, TextError};

impl TextEngine {
    /// Whether text can be set in the family `name` here, without downloading anything.
    pub fn has_family(&mut self, name: &str) -> bool {
        self.fonts.collection.family_id(name).is_some()
    }

    /// The Google Fonts families text set with the family list `list` would download first:
    /// those named before the first one this machine has (or a generic one such as
    /// `sans-serif`), since nothing after that is reached.
    pub fn missing_google_families(&mut self, list: &str) -> Vec<&'static GoogleFamily> {
        let mut missing: Vec<&'static GoogleFamily> = Vec::new();
        for name in google::names(list) {
            if google::is_generic(name) || self.has_family(name) {
                break;
            }
            if let Some(family) = google::family_named(name)
                && !missing.contains(&family)
            {
                missing.push(family);
            }
        }
        missing
    }

    /// Makes `family` available from the bytes of its files, as downloaded. Every file is
    /// checked before any is added: it must be a font no larger than [`MAX_FONT_BYTES`] whose
    /// family is the one asked for, so a wrong or damaged download changes nothing.
    pub fn add_google_family(
        &mut self,
        family: &GoogleFamily,
        files: Vec<Vec<u8>>,
    ) -> Result<(), TextError> {
        check_google_family(family, &files)?;
        for bytes in files {
            self.register_font(bytes);
        }
        Ok(())
    }
}

/// Whether `files` are the fonts of `family`: one to [`MAX_FAMILY_FILES`] of them, each a font no
/// larger than [`MAX_FONT_BYTES`] that names itself the family. A downloader checks this before
/// it keeps what it downloaded, so a wrong answer (an error page, a cut-off file) is not kept.
pub fn check_google_family(family: &GoogleFamily, files: &[Vec<u8>]) -> Result<(), TextError> {
    if files.is_empty() || files.len() > MAX_FAMILY_FILES {
        return Err(TextError::NotAFont);
    }
    for bytes in files {
        if bytes.len() > MAX_FONT_BYTES {
            return Err(TextError::FontTooLarge);
        }
        let names = family_names(bytes).ok_or(TextError::NotAFont)?;
        if !names.iter().any(|n| n.eq_ignore_ascii_case(family.name())) {
            return Err(TextError::WrongFamily {
                wanted: family.name().to_owned(),
                found: names.into_iter().next().unwrap_or_default(),
            });
        }
    }
    Ok(())
}

/// The family names a font file gives itself (the typographic one first, which is what the
/// font database files it under), or `None` when the bytes are not a font that can be drawn.
fn family_names(bytes: &[u8]) -> Option<Vec<String>> {
    let font = FontRef::new(bytes).ok()?;
    // A font with no outlines cannot set text; reading these tables fails on a damaged file.
    font.maxp().ok()?;
    font.head().ok()?;
    let mut names: Vec<String> = [StringId::TYPOGRAPHIC_FAMILY_NAME, StringId::FAMILY_NAME]
        .into_iter()
        .flat_map(|id| font.localized_strings(id).english_or_first())
        .map(|name| name.chars().collect::<String>())
        .filter(|name| !name.is_empty())
        .collect();
    names.dedup();
    (!names.is_empty()).then_some(names)
}
