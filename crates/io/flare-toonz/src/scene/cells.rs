//! One `<cell>` element: `row count level frameId increment`, as `TXshLevelColumn` writes it.

use crate::error::ToonzError;
use crate::limits::{MAX_FRAME_NUMBER, MAX_INCREMENT, MAX_ROWS};

use super::model::{CellRun, FrameId};

pub fn parse_cell_run(text: &str) -> Result<CellRun, ToonzError> {
    let mut parts = text.split_whitespace();
    let row = number::<u32>(parts.next(), "row")?;
    let count = number::<u32>(parts.next(), "count")?;
    let level = number::<u32>(parts.next(), "level")?;
    let frame = parse_frame(parts.next().unwrap_or_default())?;
    let increment = number::<i32>(parts.next(), "increment")?;
    if parts.next().is_some() {
        return Err(ToonzError::malformed("cell", "more than five fields"));
    }
    if count == 0 {
        return Err(ToonzError::malformed("cell", "a run covers no rows"));
    }
    if u64::from(row) + u64::from(count) > u64::from(MAX_ROWS) {
        return Err(ToonzError::Limit {
            what: "xsheet rows",
            max: MAX_ROWS as usize,
        });
    }
    if increment.unsigned_abs() > MAX_INCREMENT {
        return Err(ToonzError::Limit {
            what: "cell increment",
            max: MAX_INCREMENT as usize,
        });
    }
    // The writer only repeats a frame without a letter, so a lettered frame covers one row.
    if count > 1 && frame.letter.is_some() {
        return Err(ToonzError::malformed(
            "cell",
            "a lettered frame covers more than one row",
        ));
    }
    Ok(CellRun {
        row,
        count,
        level,
        frame,
        increment,
    })
}

fn number<T: std::str::FromStr>(field: Option<&str>, what: &str) -> Result<T, ToonzError> {
    let field = field.ok_or_else(|| ToonzError::malformed("cell", format!("no {what} field")))?;
    field
        .parse::<T>()
        .map_err(|_| ToonzError::malformed("cell", format!("{what} `{field}` is not a number")))
}

/// Parses a frame id such as `3` or `12a`. An empty id is refused.
fn parse_frame(text: &str) -> Result<FrameId, ToonzError> {
    let digits = text.bytes().take_while(u8::is_ascii_digit).count();
    let (number, rest) = text.split_at(digits);
    if number.is_empty() {
        return Err(ToonzError::malformed(
            "cell",
            format!("frame `{text}` has no number"),
        ));
    }
    let number: u32 = number
        .parse()
        .map_err(|_| ToonzError::malformed("cell", format!("frame `{text}` is too large")))?;
    if number > MAX_FRAME_NUMBER {
        return Err(ToonzError::Limit {
            what: "frame number",
            max: MAX_FRAME_NUMBER as usize,
        });
    }
    let mut rest_chars = rest.chars();
    let letter = match (rest_chars.next(), rest_chars.next()) {
        (None, _) => None,
        (Some(c), None) if c.is_ascii_alphabetic() => Some(c),
        _ => {
            return Err(ToonzError::malformed(
                "cell",
                format!("frame `{text}` has a bad suffix"),
            ));
        }
    };
    Ok(FrameId { number, letter })
}
