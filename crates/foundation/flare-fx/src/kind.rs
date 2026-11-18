//! The kinds of node, their inputs and their settings with ranges and defaults.

use flare_core::Rgba8;

/// What a node does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EffectKind {
    /// The layer's own picture: where every graph starts. Exactly one, number 1.
    Layer,
    /// What is drawn in the layer's place. Exactly one, number 2.
    Output,
    /// Softens the picture.
    Blur,
    /// The picture over a soft, offset copy of its shape in one color.
    DropShadow,
    /// The picture over a soft halo of its shape in one color.
    Glow,
    /// Brightness, contrast, saturation and hue.
    Adjust,
    /// Mixes every color toward one color, keeping the shape.
    Tint,
    /// Fades the picture.
    Opacity,
    /// Falling snowflakes over what comes in, moving with the frame.
    Snow,
    /// One picture laid over another.
    Over,
}

/// What a number setting measures, which says how it is shown and whether it scales with zoom.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    /// Stage units; scaled to pixels when the graph runs.
    Length,
    /// Stage units per frame; scaled like a length.
    Speed,
    /// Degrees.
    Angle,
    /// Percent.
    Percent,
    /// A whole number of things.
    Count,
}

impl Unit {
    /// Whether a value in this unit is a size on the stage, which is scaled to the picture.
    pub fn scales(self) -> bool {
        matches!(self, Self::Length | Self::Speed)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ParamKind {
    Number {
        min: f32,
        max: f32,
        default: f32,
        unit: Unit,
    },
    Color {
        default: Rgba8,
    },
}

/// One setting of a node kind.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParamSpec {
    /// The setting's name in files, the command line and the MCP server.
    pub name: &'static str,
    pub kind: ParamKind,
}

const fn number(name: &'static str, min: f32, max: f32, default: f32, unit: Unit) -> ParamSpec {
    ParamSpec {
        name,
        kind: ParamKind::Number {
            min,
            max,
            default,
            unit,
        },
    }
}

const fn color(name: &'static str, default: Rgba8) -> ParamSpec {
    ParamSpec {
        name,
        kind: ParamKind::Color { default },
    }
}

use Unit::{Angle, Count, Length, Percent, Speed};

const BLUR: &[ParamSpec] = &[number("radius", 0.0, 200.0, 6.0, Length)];
const DROP_SHADOW: &[ParamSpec] = &[
    number("distance", 0.0, 500.0, 10.0, Length),
    number("angle", -180.0, 180.0, 45.0, Angle),
    number("blur", 0.0, 200.0, 8.0, Length),
    color("color", Rgba8::opaque(0, 0, 0)),
    number("opacity", 0.0, 100.0, 60.0, Percent),
];
const GLOW: &[ParamSpec] = &[
    number("radius", 0.0, 200.0, 12.0, Length),
    number("strength", 0.0, 400.0, 100.0, Percent),
    color("color", Rgba8::opaque(0xFF, 0xD0, 0x40)),
];
const ADJUST: &[ParamSpec] = &[
    number("brightness", -100.0, 100.0, 0.0, Percent),
    number("contrast", -100.0, 100.0, 0.0, Percent),
    number("saturation", -100.0, 100.0, 0.0, Percent),
    number("hue", -180.0, 180.0, 0.0, Angle),
];
const TINT: &[ParamSpec] = &[
    color("color", Rgba8::opaque(0x30, 0x60, 0xC0)),
    number("amount", 0.0, 100.0, 50.0, Percent),
];
const OPACITY: &[ParamSpec] = &[number("opacity", 0.0, 100.0, 50.0, Percent)];
const SNOW: &[ParamSpec] = &[
    number("flakes", 0.0, 2000.0, 300.0, Count),
    number("size", 0.5, 40.0, 5.0, Length),
    number("speed", 0.0, 100.0, 4.0, Speed),
    number("wind", -100.0, 100.0, 1.0, Speed),
    number("sway", 0.0, 100.0, 8.0, Length),
    number("seed", 0.0, 9999.0, 1.0, Count),
    color("color", Rgba8::WHITE),
    number("opacity", 0.0, 100.0, 90.0, Percent),
];

impl EffectKind {
    /// Every kind, the fixed two first.
    pub const ALL: [EffectKind; 10] = [
        Self::Layer,
        Self::Output,
        Self::Blur,
        Self::DropShadow,
        Self::Glow,
        Self::Adjust,
        Self::Tint,
        Self::Opacity,
        Self::Snow,
        Self::Over,
    ];

    /// The kinds a person can add.
    pub const ADDABLE: [EffectKind; 8] = [
        Self::DropShadow,
        Self::Glow,
        Self::Blur,
        Self::Adjust,
        Self::Tint,
        Self::Opacity,
        Self::Snow,
        Self::Over,
    ];

    /// The name in files, on the command line and in the MCP server.
    pub fn name(self) -> &'static str {
        match self {
            Self::Layer => "layer",
            Self::Output => "output",
            Self::Blur => "blur",
            Self::DropShadow => "drop-shadow",
            Self::Glow => "glow",
            Self::Adjust => "adjust",
            Self::Tint => "tint",
            Self::Opacity => "opacity",
            Self::Snow => "snow",
            Self::Over => "over",
        }
    }

    /// A kind by its name, in any case, with `_` or a space for `-`.
    pub fn parse(text: &str) -> Option<Self> {
        let wanted = text.trim().to_ascii_lowercase().replace(['_', ' '], "-");
        Self::ALL.into_iter().find(|kind| kind.name() == wanted)
    }

    /// The names of the kinds that can be added, for messages.
    pub fn name_list() -> String {
        Self::ADDABLE.map(Self::name).join(", ")
    }

    /// Whether this is one of the two nodes every graph has.
    pub fn is_fixed(self) -> bool {
        matches!(self, Self::Layer | Self::Output)
    }

    /// The names of the inputs, in order.
    pub fn inputs(self) -> &'static [&'static str] {
        match self {
            Self::Layer => &[],
            Self::Over => &["over", "under"],
            Self::Snow => &["under"],
            _ => &["in"],
        }
    }

    /// The input a node added to a chain takes the chain on: the picture it changes. Over lays
    /// something else over the chain, so the chain is what goes under.
    pub fn chain_input(self) -> Option<usize> {
        match self {
            Self::Layer => None,
            Self::Over => Some(1),
            _ => Some(0),
        }
    }

    /// Whether the picture this kind makes changes from frame to frame by itself.
    pub fn is_animated(self) -> bool {
        matches!(self, Self::Snow)
    }

    pub fn params(self) -> &'static [ParamSpec] {
        match self {
            Self::Blur => BLUR,
            Self::DropShadow => DROP_SHADOW,
            Self::Glow => GLOW,
            Self::Adjust => ADJUST,
            Self::Tint => TINT,
            Self::Opacity => OPACITY,
            Self::Snow => SNOW,
            Self::Layer | Self::Output | Self::Over => &[],
        }
    }

    /// The setting named `name`, with its place in the list.
    pub fn param(self, name: &str) -> Option<(usize, &'static ParamSpec)> {
        let wanted = name.trim().to_ascii_lowercase();
        self.params()
            .iter()
            .enumerate()
            .find(|(_, spec)| spec.name == wanted)
    }

    pub(crate) fn param_list(self) -> String {
        let names: Vec<&str> = self.params().iter().map(|p| p.name).collect();
        if names.is_empty() {
            "no settings".to_owned()
        } else {
            names.join(", ")
        }
    }
}
