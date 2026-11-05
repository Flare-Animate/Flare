//! How a stroke's line is broken up: solid, or a repeating pattern of dashes and dots.

use std::fmt;

/// The pattern a stroke is drawn in. Lengths scale with the stroke's width, so a thick dotted line
/// has bigger dots, as in Animate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StrokeStyle {
    #[default]
    Solid,
    /// Long dashes with gaps about as long.
    Dashed,
    /// Round dots one width apart (they read as dots with a round cap).
    Dotted,
    /// Fine, tightly spaced specks.
    Stipple,
}

impl StrokeStyle {
    pub const ALL: [Self; 4] = [Self::Solid, Self::Dashed, Self::Dotted, Self::Stipple];

    /// The lowercase name used in files, on the command line and by the MCP server.
    pub fn name(self) -> &'static str {
        match self {
            Self::Solid => "solid",
            Self::Dashed => "dashed",
            Self::Dotted => "dotted",
            Self::Stipple => "stipple",
        }
    }

    /// Reads a name, ignoring case and surrounding space.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        Self::ALL
            .into_iter()
            .find(|s| s.name().eq_ignore_ascii_case(text))
    }

    /// Alternating on and off lengths for a line `width` wide, or `None` for a solid line.
    /// Every length is positive and finite for a positive finite width.
    pub fn dashes(self, width: f64) -> Option<[f64; 2]> {
        // A hairline would give invisible, huge-count dashes; keep the pattern readable.
        let w = if width.is_finite() {
            width.max(1.0)
        } else {
            1.0
        };
        match self {
            Self::Solid => None,
            Self::Dashed => Some([4.0 * w, 3.0 * w]),
            // Zero-length dashes become dots when the cap is round, and nothing when it is butt,
            // so the on length is a small positive one.
            Self::Dotted => Some([0.01 * w, 2.0 * w]),
            Self::Stipple => Some([0.01 * w, 0.8 * w]),
        }
    }
}

impl fmt::Display for StrokeStyle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    #[test]
    fn names_round_trip_and_junk_is_refused() {
        for s in StrokeStyle::ALL {
            assert_eq!(StrokeStyle::parse(s.name()), Some(s));
            assert_eq!(
                StrokeStyle::parse(&format!(" {} ", s.name().to_uppercase())),
                Some(s)
            );
        }
        assert_eq!(StrokeStyle::parse("wavy"), None);
        assert_eq!(StrokeStyle::parse(""), None);
    }

    #[test]
    fn only_solid_has_no_pattern_and_patterns_scale_with_width() {
        assert_eq!(StrokeStyle::Solid.dashes(4.0), None);
        for s in &StrokeStyle::ALL[1..] {
            let thin = s.dashes(2.0).expect("pattern");
            let thick = s.dashes(8.0).expect("pattern");
            assert!(thin.iter().all(|v| *v > 0.0 && v.is_finite()));
            assert!((thick[1] / thin[1] - 4.0).abs() < 1e-9);
            // Hostile widths still give a usable pattern.
            assert!(s.dashes(f64::NAN).expect("pattern")[1] > 0.0);
            assert!(s.dashes(0.0).expect("pattern")[1] > 0.0);
        }
    }
}
