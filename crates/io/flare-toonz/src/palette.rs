//! Reader for palette XML (`.tpl`, `.plt`): the version, the number of styles, and the style
//! pages with the style indices on each. Colours are not decoded yet.

use crate::error::ToonzError;
use crate::limits::MAX_PALETTE_ITEMS;
use crate::tree;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Palette {
    /// Every style in file order; a style's id is its position here.
    pub styles: Vec<Style>,
    /// The `<version>` pair, e.g. `(71, 0)`.
    pub version: Option<(u32, u32)>,
    pub style_count: usize,
    pub pages: Vec<StylePage>,
}

/// One `<style>`: its name, its tag id (3 is a solid colour) and, for a solid colour, the colour.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Style {
    pub name: String,
    pub tag_id: u32,
    /// Red, green, blue, alpha (Toonz calls the last `m`).
    pub color: Option<[u8; 4]>,
}

/// The tag id of `TSolidColorStyle` in OpenToonz (`tsimplecolorstyles.cpp`).
pub const SOLID_COLOR_TAG: u32 = 3;

/// Reads a style's text the way `TColorStyle::load` does: optional `_flags`, `|global name` and
/// `@original name` tokens, the name, the tag id (the name itself when it is a number), then the
/// data, which for a solid colour is four numbers.
pub(crate) fn parse_style(text: &str) -> Style {
    let mut tokens = tokenize(text).into_iter().peekable();
    let numeric = |t: &str| t.chars().next().is_some_and(|c| c.is_ascii_digit());
    let mut name = tokens.next().unwrap_or_default();
    if name.len() == 2 && name.starts_with('_') && name.as_bytes()[1].is_ascii_digit() {
        name = tokens.next().unwrap_or_default();
    }
    if name.starts_with('|') {
        name = tokens.next().unwrap_or_default();
        if name.starts_with('@') {
            name = tokens.next().unwrap_or_default();
        }
    }
    let tag_id = if numeric(&name) {
        let id = name.parse().unwrap_or(0);
        name = "color".to_owned();
        id
    } else {
        tokens.next().and_then(|t| t.parse().ok()).unwrap_or(0)
    };
    let channels: Vec<u8> = tokens
        .take(4)
        .filter_map(|t| t.parse::<i64>().ok())
        .map(|v| v.clamp(0, 255) as u8)
        .collect();
    let color = (tag_id == SOLID_COLOR_TAG && channels.len() == 4)
        .then(|| [channels[0], channels[1], channels[2], channels[3]]);
    Style {
        name,
        tag_id,
        color,
    }
}

/// Splits on whitespace, keeping a double-quoted name together.
fn tokenize(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
        } else if c == '"' {
            chars.next();
            out.push(chars.by_ref().take_while(|c| *c != '"').collect());
        } else {
            let mut word = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_whitespace() {
                    break;
                }
                word.push(c);
                chars.next();
            }
            out.push(word);
        }
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StylePage {
    pub name: String,
    pub style_ids: Vec<u32>,
}

pub fn parse_palette(input: &str) -> Result<Palette, ToonzError> {
    let root = tree::parse(input)?;
    if root.name != "palette" {
        return Err(ToonzError::UnknownFormat("root element is not <palette>"));
    }
    let version = root
        .child("version")
        .map(|v| parse_version(&v.text))
        .transpose()?;
    let styles: Vec<Style> = root
        .child("styles")
        .map(|styles| {
            styles
                .children_named("style")
                .map(|s| parse_style(&s.text))
                .collect()
        })
        .unwrap_or_default();
    let style_count = styles.len();
    if style_count > MAX_PALETTE_ITEMS {
        return Err(ToonzError::Limit {
            what: "palette styles",
            max: MAX_PALETTE_ITEMS,
        });
    }
    let mut pages = Vec::new();
    let mut indices_seen = 0usize;
    if let Some(stylepages) = root.child("stylepages") {
        for page in stylepages.children_named("page") {
            let mut style_ids = Vec::new();
            if let Some(indices) = page.child("indices") {
                for field in indices.text.split_whitespace() {
                    indices_seen += 1;
                    if indices_seen > MAX_PALETTE_ITEMS {
                        return Err(ToonzError::Limit {
                            what: "palette style indices",
                            max: MAX_PALETTE_ITEMS,
                        });
                    }
                    let id = field.parse::<u32>().map_err(|_| {
                        ToonzError::malformed("palette", format!("style index `{field}`"))
                    })?;
                    style_ids.push(id);
                }
            }
            pages.push(StylePage {
                name: page
                    .child("name")
                    .map_or_else(String::new, |n| n.text.clone()),
                style_ids,
            });
        }
    }
    Ok(Palette {
        styles,
        version,
        style_count,
        pages,
    })
}

fn parse_version(text: &str) -> Result<(u32, u32), ToonzError> {
    let mut parts = text.split_whitespace();
    let mut next = || -> Result<u32, ToonzError> {
        parts
            .next()
            .ok_or_else(|| ToonzError::malformed("palette", "version needs two numbers"))?
            .parse::<u32>()
            .map_err(|_| ToonzError::malformed("palette", "version is not numeric"))
    };
    let major = next()?;
    let minor = next()?;
    Ok((major, minor))
}
