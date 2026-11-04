//! Reader for palette XML (`.tpl`, `.plt`): the version, the number of styles, and the style
//! pages with the style indices on each. Colours are not decoded yet.

use crate::error::ToonzError;
use crate::limits::MAX_PALETTE_ITEMS;
use crate::tree;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Palette {
    /// The `<version>` pair, e.g. `(71, 0)`.
    pub version: Option<(u32, u32)>,
    pub style_count: usize,
    pub pages: Vec<StylePage>,
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
    let style_count = root
        .child("styles")
        .map_or(0, |styles| styles.children_named("style").count());
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
