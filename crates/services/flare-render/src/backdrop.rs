//! What an exported picture shows where nothing is drawn: the stage colour, or nothing at all.

use flare_core::Rgba8;

/// What fills an exported picture where the animation draws nothing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Backdrop {
    /// The stage colour, as the stage shows it.
    #[default]
    Stage,
    /// Nothing: the picture is see-through there, so it can be laid over something else. Only a
    /// format with an alpha channel can keep it; the others refuse it with the reason.
    Transparent,
}

impl Backdrop {
    /// `Transparent` when `transparent` is set, else `Stage`.
    pub fn transparent_if(transparent: bool) -> Self {
        if transparent {
            Self::Transparent
        } else {
            Self::Stage
        }
    }

    pub fn is_transparent(self) -> bool {
        self == Self::Transparent
    }

    /// The colour a picture is drawn over for a stage coloured `stage`.
    pub fn over(self, stage: Rgba8) -> Rgba8 {
        match self {
            Self::Stage => stage,
            Self::Transparent => Rgba8::TRANSPARENT,
        }
    }

    /// The colour a movie is drawn over: the stage colour made opaque, as a movie frame has no
    /// partial background, or nothing.
    pub fn over_opaque(self, stage: Rgba8) -> Rgba8 {
        match self {
            Self::Stage => Rgba8::opaque(stage.r, stage.g, stage.b),
            Self::Transparent => Rgba8::TRANSPARENT,
        }
    }
}

/// Why `formats` (a plural: "JPEG pictures", "MP4 videos") cannot be exported with a transparent
/// background, and what can.
pub fn no_transparency(formats: &str) -> String {
    format!(
        "{formats} cannot have a transparent background. Choose PNG, GIF, APNG, animated WebP, WebM or a QuickTime movie, or turn off the transparent background."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stage_colour_is_kept_or_dropped() {
        let stage = Rgba8::new(10, 20, 30, 128);
        assert_eq!(Backdrop::Stage.over(stage), stage);
        assert_eq!(
            Backdrop::Stage.over_opaque(stage),
            Rgba8::opaque(10, 20, 30)
        );
        assert_eq!(Backdrop::Transparent.over(stage), Rgba8::TRANSPARENT);
        assert_eq!(Backdrop::Transparent.over_opaque(stage), Rgba8::TRANSPARENT);
        assert_eq!(Backdrop::transparent_if(true), Backdrop::Transparent);
        assert_eq!(Backdrop::transparent_if(false), Backdrop::default());
    }

    #[test]
    fn a_refusal_names_the_format_and_what_to_do() {
        let text = no_transparency("MP4 videos");
        assert!(text.starts_with("MP4 videos cannot"), "{text}");
        assert!(text.contains("PNG"), "{text}");
    }
}
