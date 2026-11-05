//! `name=value` fields: how the command line and MCP name a component's values. Each token is
//! split at its first `=`; the component parser then checks every name and value.

use crate::{GameError, limits::MAX_FIELDS};

/// Splits each `name=value` token at its first `=`. A token with no `=`, an empty name, or more
/// than [`MAX_FIELDS`] tokens is refused. The value may be empty.
pub fn parse_fields<S: AsRef<str>>(tokens: &[S]) -> Result<Vec<(String, String)>, GameError> {
    if tokens.len() > MAX_FIELDS {
        return Err(GameError::BadInput(format!(
            "a component takes at most {MAX_FIELDS} fields"
        )));
    }
    tokens
        .iter()
        .map(|token| {
            let token = token.as_ref();
            match token.split_once('=') {
                Some((name, value)) if !name.trim().is_empty() => {
                    Ok((name.trim().to_owned(), value.trim().to_owned()))
                }
                _ => Err(GameError::BadInput(format!(
                    "{token:?} is not a name=value field"
                ))),
            }
        })
        .collect()
}

#[cfg(test)]
#[path = "fields_tests.rs"]
mod tests;
