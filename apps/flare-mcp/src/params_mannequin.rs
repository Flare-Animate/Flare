//! Parameters of the reference mannequin's tools.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PlaceMannequin {
    /// A whole figure to start from, as list_mannequin's "example" shows it: {"version": 1,
    /// "body", "proportions", "pose": {"joints", "lift"}, "view": {"yaw", "pitch", "fov"}}.
    pub figure: Option<serde_json::Value>,
    /// male or female.
    pub body: Option<String>,
    /// A built-in pose's id (list_mannequin lists them).
    pub preset: Option<String>,
    /// Joints to turn, by id, as [bend, twist, side] in degrees within each joint's limits
    /// (list_mannequin lists them); at most 64. Applied after the preset.
    pub joints: Option<BTreeMap<String, [f64; 3]>>,
    /// Hands and feet to pull to points, by id (left_wrist, right_wrist, left_ankle,
    /// right_ankle), as [x, y, z]: x to the figure's left, y up from the ground, z to its
    /// front, the figure one unit tall; the arm or leg bends to follow. Applied after joints.
    pub reach: Option<BTreeMap<String, [f64; 3]>>,
    /// Degrees the figure is turned to its left.
    pub yaw: Option<f64>,
    /// Degrees the view looks down on it, -89 to 89.
    pub pitch: Option<f64>,
    /// Field of view, 0 (flat) to 90 degrees.
    pub fov: Option<f64>,
    /// Where the figure's middle goes on the stage; the stage's middle by default.
    pub x: Option<f64>,
    pub y: Option<f64>,
    /// How tall it stands in stage units; four fifths of the stage's height by default.
    pub height: Option<f64>,
    /// The frame its keyframe starts on; it lasts to the end of the scene. Defaults to 0.
    pub frame: Option<u32>,
    /// The new layer's name (and the picture's in the library); defaults to "Mannequin".
    pub name: Option<String>,
    /// An existing layer's id to place it on instead of a new layer: into its keyframe at
    /// `frame`, beside the figures already there, or a keyframe of its own there when the layer
    /// shows nothing at that frame. Locked, hidden, folder, mask, guide and camera layers are
    /// refused.
    pub layer: Option<String>,
    /// With `layer`: a keyframe of its own from `frame`, cutting short the one shown there, so
    /// each stretch of frames of one layer can hold another figure. Defaults to false.
    pub new_keyframe: Option<bool>,
    /// Fail unless the project is at this revision (from get_info).
    pub expected_revision: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PictureMannequin {
    /// A whole figure to start from, as list_mannequin's "example" shows it.
    pub figure: Option<serde_json::Value>,
    /// male or female.
    pub body: Option<String>,
    /// A built-in pose's id.
    pub preset: Option<String>,
    /// Joints to turn, by id, as [bend, twist, side] in degrees.
    pub joints: Option<BTreeMap<String, [f64; 3]>>,
    /// Hands and feet to pull to points, as place_mannequin takes them.
    pub reach: Option<BTreeMap<String, [f64; 3]>>,
    pub yaw: Option<f64>,
    pub pitch: Option<f64>,
    pub fov: Option<f64>,
    /// The picture's size in pixels, each 16 to 4000; 600 by 800 by default.
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// Draw the guide lines on the body (default true).
    pub guides: Option<bool>,
    /// File name only: letters, digits, '.', '_' or '-'. ".png" is added when missing.
    pub name: String,
}
