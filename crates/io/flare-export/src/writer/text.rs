//! Formatting the numbers and names XFL is made of.

use flare_core::{Rgba8, SymbolId};

// ---- text helpers ---------------------------------------------------------------------

/// A number with up to four decimals and no trailing zeros.
pub(super) fn num(value: f64) -> String {
    let text = format!("{value:.4}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text.is_empty() || text == "-0" {
        "0".to_owned()
    } else {
        text.to_owned()
    }
}

pub(super) fn hex(color: Rgba8) -> String {
    format!("#{:02X}{:02X}{:02X}", color.r, color.g, color.b)
}

pub(super) fn alpha(color: Rgba8) -> String {
    num(f64::from(color.a) / 255.0)
}

/// A file stem that is safe on every platform and cannot escape a directory.
pub(super) fn safe_stem(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, ' ' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = cleaned
        .trim_matches([' ', '.'])
        .chars()
        .take(80)
        .collect::<String>();
    if trimmed.is_empty() {
        "Symbol".to_owned()
    } else {
        trimmed
    }
}

/// The `symbolType` XFL gives a kind, or none: Animate leaves the attribute off a movie clip.
pub(super) fn xfl_symbol_type(kind: flare_document::SymbolKind) -> Option<&'static str> {
    match kind {
        flare_document::SymbolKind::Graphic => Some("graphic"),
        flare_document::SymbolKind::Button => Some("button"),
        flare_document::SymbolKind::MovieClip => None,
    }
}

pub(super) struct Names {
    /// Library names (unique) and file names, in symbol order.
    pub(super) library: Vec<(SymbolId, String, String)>,
    /// What each symbol is, for the instances of it.
    pub(super) kinds: Vec<(SymbolId, flare_document::SymbolKind)>,
}

impl Names {
    pub(super) fn library_name(&self, id: SymbolId) -> &str {
        self.library
            .iter()
            .find(|(s, _, _)| *s == id)
            .map_or("", |(_, name, _)| name)
    }

    pub(super) fn kind(&self, id: SymbolId) -> flare_document::SymbolKind {
        self.kinds
            .iter()
            .find(|(s, _)| *s == id)
            .map_or(flare_document::SymbolKind::Graphic, |(_, kind)| *kind)
    }
}

/// Whether `xml` is exactly one well-formed element with no DTD. Preserved source data
/// came from a file, so it is checked before being written into the output.
pub(super) fn well_formed(xml: &str) -> bool {
    use quick_xml::events::Event;
    let mut reader = quick_xml::Reader::from_str(xml);
    let (mut depth, mut roots) = (0_usize, 0_usize);
    loop {
        match reader.read_event() {
            Ok(Event::Start(_)) => {
                if depth == 0 {
                    roots += 1;
                }
                depth += 1;
            }
            Ok(Event::Empty(_)) => {
                if depth == 0 {
                    roots += 1;
                }
            }
            Ok(Event::End(_)) => match depth.checked_sub(1) {
                Some(d) => depth = d,
                None => return false,
            },
            Ok(Event::DocType(_) | Event::PI(_)) => return false,
            Ok(Event::Text(t)) if depth == 0 && !t.xml10_content().trim().is_empty() => {
                return false;
            }
            Ok(Event::Eof) => return depth == 0 && roots == 1,
            Err(_) => return false,
            Ok(_) => {}
        }
    }
}

// ---- document -------------------------------------------------------------------------

pub(super) fn frame_rate_text(rate: flare_core::FrameRate) -> String {
    if rate.denominator() == 1 {
        rate.numerator().to_string()
    } else {
        // Three decimals keep NTSC rates (29.970...) recognisable on re-import.
        num((f64::from(rate.numerator()) / f64::from(rate.denominator()) * 1000.0).round() / 1000.0)
    }
}
