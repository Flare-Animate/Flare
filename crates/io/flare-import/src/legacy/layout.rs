//! What the stream names of a legacy `.fla` say about it.
//!
//! Two naming styles exist in the files checked: numbered (`Page 1`, `Symbol 2`, `Media 1`) and
//! stamped with a number that looks like a Unix time (`P 1 1678559673`, `S 1 ...`, `M 1 ...`).

use std::fmt;

/// How the streams of a document are named.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Era {
    /// `Page N`, `Symbol N`, `Media N`.
    Numbered,
    /// `P N stamp`, `S N stamp`, `M N stamp`.
    Stamped,
    /// Both styles, or none of the known streams, so nothing to tell them by.
    Unknown,
}

/// The kind of a stream other than `Contents`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StreamKind {
    Page,
    Symbol,
    Media,
}

/// Classifies a stream name, with the style it is written in.
pub(crate) fn classify(name: &str) -> Option<(StreamKind, Era)> {
    let mut parts = name.split(' ');
    let head = parts.next()?;
    let number = parts.next()?;
    if number.is_empty() || !number.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let rest: Vec<&str> = parts.collect();
    let kind = |numbered: &str, stamped: &str| {
        if head == numbered && rest.is_empty() {
            Some(Era::Numbered)
        } else if head == stamped
            && rest.len() == 1
            && !rest[0].is_empty()
            && rest[0].bytes().all(|b| b.is_ascii_digit())
        {
            Some(Era::Stamped)
        } else {
            None
        }
    };
    if let Some(era) = kind("Page", "P") {
        return Some((StreamKind::Page, era));
    }
    if let Some(era) = kind("Symbol", "S") {
        return Some((StreamKind::Symbol, era));
    }
    kind("Media", "M").map(|era| (StreamKind::Media, era))
}

/// A summary of what a legacy `.fla` holds, from its stream names alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyInfo {
    pub era: Era,
    /// The two-byte number at the start of `Contents`, the file format's version.
    pub format_version: Option<u16>,
    /// Streams holding a scene's stage (`Page N` or `P N stamp`).
    pub pages: usize,
    /// Library symbols (`Symbol N` or `S N stamp`).
    pub symbols: usize,
    /// Library media (`Media N` or `M N stamp`).
    pub media: usize,
    pub other_streams: usize,
}

impl LegacyInfo {
    pub(crate) fn from_names<'a>(
        names: impl Iterator<Item = &'a str>,
        format_version: Option<u16>,
    ) -> Self {
        let (mut numbered, mut stamped) = (false, false);
        let mut info = Self {
            era: Era::Unknown,
            format_version,
            pages: 0,
            symbols: 0,
            media: 0,
            other_streams: 0,
        };
        for name in names {
            match classify(name) {
                Some((kind, era)) => {
                    match era {
                        Era::Numbered => numbered = true,
                        _ => stamped = true,
                    }
                    match kind {
                        StreamKind::Page => info.pages += 1,
                        StreamKind::Symbol => info.symbols += 1,
                        StreamKind::Media => info.media += 1,
                    }
                }
                None if name != "Contents" => info.other_streams += 1,
                None => {}
            }
        }
        info.era = match (numbered, stamped) {
            (true, false) => Era::Numbered,
            (false, true) => Era::Stamped,
            _ => Era::Unknown,
        };
        info
    }
}

impl fmt::Display for LegacyInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let style = match self.era {
            Era::Numbered => "numbered streams",
            Era::Stamped => "time-stamped streams",
            Era::Unknown => "unrecognised stream names",
        };
        write!(f, "a legacy binary FLA (")?;
        match self.format_version {
            Some(v) => write!(f, "format {v:#06x}, ")?,
            None => write!(f, "format unknown, ")?,
        }
        write!(
            f,
            "{style}) with {} scene(s), {} symbol(s) and {} media item(s)",
            self.pages, self.symbols, self.media
        )
    }
}
