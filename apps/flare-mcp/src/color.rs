//! Parsing the strings agents send for colors and file names.

use flare_core::Rgba8;

/// `RRGGBB` or `RRGGBBAA` hex, with or without a leading `#`.
pub fn parse_color(text: &str) -> Result<Rgba8, String> {
    let bad = || format!("bad color {text:?}, expected RRGGBB or RRGGBBAA");
    let hex = text.trim_start_matches('#');
    let byte = |i: usize| {
        hex.get(i..i + 2)
            .and_then(|pair| u8::from_str_radix(pair, 16).ok())
            .ok_or_else(bad)
    };
    match hex.len() {
        6 => Ok(Rgba8::opaque(byte(0)?, byte(2)?, byte(4)?)),
        8 => Ok(Rgba8::new(byte(0)?, byte(2)?, byte(4)?, byte(6)?)),
        _ => Err(bad()),
    }
}

/// A file name that cannot escape the export directory, with `.png` added when missing.
pub fn safe_file_name(name: &str) -> Result<String, String> {
    let ok = !name.is_empty()
        && name.len() <= 100
        && !name.starts_with(['.', '-'])
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    if !ok {
        return Err("name must be 1-100 characters of letters, digits, '.', '_' or '-', not starting with '.' or '-'".to_owned());
    }
    Ok(if name.to_ascii_lowercase().ends_with(".png") {
        name.to_owned()
    } else {
        format!("{name}.png")
    })
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn colors_parse_with_or_without_hash_and_alpha() {
        assert_eq!(
            parse_color("#ff8000").expect("rgb"),
            Rgba8::opaque(255, 128, 0)
        );
        assert_eq!(
            parse_color("ff800080").expect("rgba"),
            Rgba8::new(255, 128, 0, 128)
        );
        for bad in ["", "#fff", "gg0000", "ff80", "ff8000ff00", "é12345"] {
            assert!(parse_color(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn file_names_cannot_escape_and_get_a_png_extension() {
        assert_eq!(safe_file_name("frame").expect("ok"), "frame.png");
        assert_eq!(safe_file_name("Shot.PNG").expect("ok"), "Shot.PNG");
        for bad in [
            "",
            "../x",
            "a/b",
            ".hidden",
            "-flag",
            "a b",
            &"x".repeat(101),
        ] {
            assert!(safe_file_name(bad).is_err(), "{bad:?}");
        }
    }
}
