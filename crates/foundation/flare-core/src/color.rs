//! Color values.

/// Straight (non-premultiplied) sRGB color with 8-bit channels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rgba8 {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba8 {
    pub const WHITE: Self = Self::new(255, 255, 255, 255);
    pub const TRANSPARENT: Self = Self::new(0, 0, 0, 0);

    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub const fn opaque(r: u8, g: u8, b: u8) -> Self {
        Self::new(r, g, b, 255)
    }

    /// A color from `RRGGBB` or `RRGGBBAA` hex, with or without a leading `#`; `None` for
    /// anything else. The one parser the command line, the MCP server and effect settings share.
    pub fn from_hex(text: &str) -> Option<Self> {
        let hex = text.trim().trim_start_matches('#');
        if !(hex.len() == 6 || hex.len() == 8) || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let byte = |i: usize| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok();
        let alpha = if hex.len() == 8 { byte(6)? } else { 255 };
        Some(Self::new(byte(0)?, byte(2)?, byte(4)?, alpha))
    }

    pub const fn to_array(self) -> [u8; 4] {
        [self.r, self.g, self.b, self.a]
    }
}

#[cfg(test)]
mod tests {
    use super::Rgba8;

    #[test]
    fn hex_with_or_without_hash_and_alpha_and_nothing_else() {
        assert_eq!(Rgba8::from_hex("#FF8000"), Some(Rgba8::opaque(255, 128, 0)));
        assert_eq!(
            Rgba8::from_hex("ff800040"),
            Some(Rgba8::new(255, 128, 0, 64))
        );
        assert_eq!(Rgba8::from_hex(" #000000 "), Some(Rgba8::opaque(0, 0, 0)));
        for bad in ["", "#12345", "GG0000", "#1234567", "é00000", "#+12345"] {
            assert_eq!(Rgba8::from_hex(bad), None, "{bad:?}");
        }
    }
}
