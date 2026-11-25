//! Settings given as text, as the command line and the MCP server take them: `name=value`, a
//! number for a number and `RRGGBB` or `RRGGBBAA` (a leading `#` is fine) for a color.

use flare_core::Rgba8;

use crate::{FxError, ParamKind, ParamSpec, Value};

/// Most `name=value` tokens read at once.
const MAX_TOKENS: usize = 32;

/// Splits `name=value` tokens into pairs, trimmed.
pub fn parse_fields<S: AsRef<str>>(tokens: &[S]) -> Result<Vec<(String, String)>, FxError> {
    if tokens.len() > MAX_TOKENS {
        return Err(FxError::TooManyFields(MAX_TOKENS));
    }
    tokens
        .iter()
        .map(|token| {
            let token = token.as_ref();
            match token.split_once('=') {
                Some((name, value)) if !name.trim().is_empty() => {
                    Ok((name.trim().to_owned(), value.trim().to_owned()))
                }
                _ => Err(FxError::BadField(token.to_owned())),
            }
        })
        .collect()
}

/// A color as `RRGGBB` or `RRGGBBAA`, with or without a leading `#`.
pub fn parse_color(text: &str) -> Option<Rgba8> {
    Rgba8::from_hex(text)
}

/// The value `text` gives the setting `spec`, checked against its range.
pub(crate) fn value_from_text(spec: &ParamSpec, text: &str) -> Result<Value, FxError> {
    let value = match spec.kind {
        ParamKind::Number { .. } => {
            let number: f32 = text.trim().parse().map_err(|_| FxError::NotANumber {
                name: spec.name,
                value: text.to_owned(),
            })?;
            Value::Number(number)
        }
        ParamKind::Color { .. } => {
            Value::Color(parse_color(text).ok_or_else(|| FxError::BadColor {
                name: spec.name,
                value: text.to_owned(),
            })?)
        }
    };
    value.checked(spec)
}
