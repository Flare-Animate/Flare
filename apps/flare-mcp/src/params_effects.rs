//! Parameters of the layer effect tools. Settings are `name: value` text, as the command line
//! takes them; a color is RRGGBB or RRGGBBAA.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::Deserialize;

/// An input of a node, by its number (0 is the first) or its name ("under").
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Input {
    Number(usize),
    Name(String),
}

impl Input {
    pub fn text(&self) -> String {
        match self {
            Self::Number(n) => n.to_string(),
            Self::Name(name) => name.clone(),
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct EffectsOf {
    /// Layer id; defaults to the first layer of the first scene.
    pub layer: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddEffect {
    /// Layer id; defaults to the first layer of the first scene.
    pub layer: Option<String>,
    /// drop-shadow, glow, blur, adjust, tint, opacity, snow or over (list_effect_kinds).
    pub kind: String,
    /// Settings by name with their values as text (for example "distance": "12").
    #[serde(default)]
    pub settings: BTreeMap<String, String>,
    pub expected_revision: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetEffect {
    pub layer: Option<String>,
    /// The node's number (the Layer node is 1, the Output 2).
    pub node: u32,
    /// Settings by name with their values as text. All are set or none.
    pub settings: BTreeMap<String, String>,
    pub expected_revision: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ConnectEffects {
    pub layer: Option<String>,
    /// The node whose output is taken.
    pub from: u32,
    /// The node that takes it.
    pub to: u32,
    /// The input of `to`, by number or name ("under"); the first when not given.
    pub input: Option<Input>,
    pub expected_revision: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DisconnectEffect {
    pub layer: Option<String>,
    pub to: u32,
    /// The input of `to`, by number or name; the first when not given.
    pub input: Option<Input>,
    pub expected_revision: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct EffectNode {
    pub layer: Option<String>,
    pub node: u32,
    pub expected_revision: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MoveEffect {
    pub layer: Option<String>,
    pub node: u32,
    /// Where the node goes in the graph view.
    pub x: f32,
    pub y: f32,
    pub expected_revision: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ClearEffects {
    pub layer: Option<String>,
    pub expected_revision: Option<u64>,
}
