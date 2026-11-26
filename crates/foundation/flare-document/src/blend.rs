//! How a layer's picture is laid over the layers under it: the blend modes of a painting
//! program's layers (Photoshop, Krita, Clip Studio and Procreate share these names), plus Add.

/// How a layer is mixed with what is under it. Normal lays it on top; the others combine its
/// colors with those below, as the same modes do in a painting program.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum LayerBlend {
    #[default]
    Normal,
    Multiply,
    Screen,
    Overlay,
    Darken,
    Lighten,
    ColorDodge,
    ColorBurn,
    HardLight,
    SoftLight,
    Difference,
    Exclusion,
    Hue,
    Saturation,
    Color,
    Luminosity,
    /// The colors added together (Linear Dodge in Photoshop), for glows and light.
    Add,
}

impl LayerBlend {
    /// In the order a painting program's list has them: darkening, lightening, contrast,
    /// comparing, then the color components.
    pub const ALL: [LayerBlend; 17] = [
        Self::Normal,
        Self::Darken,
        Self::Multiply,
        Self::ColorBurn,
        Self::Lighten,
        Self::Screen,
        Self::ColorDodge,
        Self::Add,
        Self::Overlay,
        Self::SoftLight,
        Self::HardLight,
        Self::Difference,
        Self::Exclusion,
        Self::Hue,
        Self::Saturation,
        Self::Color,
        Self::Luminosity,
    ];

    /// The word for it in a file, on a command line and in a message key.
    pub fn name(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Multiply => "multiply",
            Self::Screen => "screen",
            Self::Overlay => "overlay",
            Self::Darken => "darken",
            Self::Lighten => "lighten",
            Self::ColorDodge => "color-dodge",
            Self::ColorBurn => "color-burn",
            Self::HardLight => "hard-light",
            Self::SoftLight => "soft-light",
            Self::Difference => "difference",
            Self::Exclusion => "exclusion",
            Self::Hue => "hue",
            Self::Saturation => "saturation",
            Self::Color => "color",
            Self::Luminosity => "luminosity",
            Self::Add => "add",
        }
    }

    /// Its name in English, for messages that are not in a person's language.
    pub fn label(self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::Multiply => "Multiply",
            Self::Screen => "Screen",
            Self::Overlay => "Overlay",
            Self::Darken => "Darken",
            Self::Lighten => "Lighten",
            Self::ColorDodge => "Color Dodge",
            Self::ColorBurn => "Color Burn",
            Self::HardLight => "Hard Light",
            Self::SoftLight => "Soft Light",
            Self::Difference => "Difference",
            Self::Exclusion => "Exclusion",
            Self::Hue => "Hue",
            Self::Saturation => "Saturation",
            Self::Color => "Color",
            Self::Luminosity => "Luminosity",
            Self::Add => "Add",
        }
    }

    /// The mode a word names, ignoring case, spaces and underscores (`Color Dodge`,
    /// `color_dodge` and `color-dodge` are the same); `linear-dodge` is Add.
    pub fn parse(word: &str) -> Option<Self> {
        let word: String = word
            .trim()
            .chars()
            .map(|c| match c {
                ' ' | '_' => '-',
                c => c.to_ascii_lowercase(),
            })
            .collect();
        if word == "linear-dodge" {
            return Some(Self::Add);
        }
        Self::ALL.into_iter().find(|b| b.name() == word)
    }

    pub fn is_normal(self) -> bool {
        self == Self::Normal
    }
}
