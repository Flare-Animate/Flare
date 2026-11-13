//! A figure asked for by name and number, as the command line and the MCP server ask: a figure
//! file to start from, then a body, a built-in pose, a pose file, single joints and the view,
//! each laid over what came before. Every value is checked, never brought into range, so a
//! mistyped angle is refused with its reason instead of quietly changed.

use serde_json::{Value, json};

use crate::body::{Body, Proportion};
use crate::figure::Mannequin;
use crate::grab::ENDS;
use crate::math::{Angles, Vec3};
use crate::pose::{MAX_LIFT, Pose};
use crate::skeleton::Joint;
use crate::view::{MAX_FOV, MAX_PITCH};
use crate::wire::{WireError, presets};

/// Most joints one request may set or pull one by one.
pub const MAX_JOINT_SETTINGS: usize = 64;
/// The furthest a point to reach for may be from the middle of the ground, each way.
pub const MAX_REACH: f64 = 4.0;

/// What a figure should be; anything left out is the figure file's, else the default.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Request {
    /// A whole figure, as `Mannequin::to_json` writes it.
    pub figure: Option<String>,
    /// `male` or `female`.
    pub body: Option<String>,
    /// A built-in pose's id.
    pub preset: Option<String>,
    /// A pose, as `Pose::to_json` writes it.
    pub pose: Option<String>,
    /// Joints set one by one, by id, as bend, twist and side in degrees.
    pub joints: Vec<(String, [f64; 3])>,
    /// Hands and feet pulled to points, by id, as x (to the figure's left), y (up from the
    /// ground) and z (to its front) in figure units, the default figure one unit tall; the arm
    /// or leg bends to follow. Applied after the joints.
    pub reach: Vec<(String, [f64; 3])>,
    pub yaw: Option<f64>,
    pub pitch: Option<f64>,
    pub fov: Option<f64>,
}

impl Request {
    pub fn build(&self) -> Result<Mannequin, WireError> {
        let mut figure = match &self.figure {
            Some(text) => Mannequin::from_json(text)?,
            None => Mannequin::default(),
        };
        if let Some(name) = &self.body {
            figure.body =
                Body::from_id(name).ok_or_else(|| WireError::UnknownBody(name.clone()))?;
        }
        if let Some(id) = &self.preset {
            figure.pose = presets()
                .iter()
                .find(|p| p.id == *id)
                .map(|p| p.pose)
                .ok_or_else(|| WireError::UnknownPreset(id.clone()))?;
        }
        if let Some(text) = &self.pose {
            figure.pose = Pose::from_json(text)?;
        }
        if self.joints.len() + self.reach.len() > MAX_JOINT_SETTINGS {
            return Err(WireError::TooLarge);
        }
        for (name, angles) in &self.joints {
            let joint =
                Joint::from_id(name).ok_or_else(|| WireError::UnknownJoint(name.clone()))?;
            figure
                .pose
                .set_checked(joint, Angles::from_array(*angles))?;
        }
        for (name, point) in &self.reach {
            let joint =
                Joint::from_id(name).ok_or_else(|| WireError::UnknownJoint(name.clone()))?;
            if joint.reach_chain().is_none() {
                return Err(WireError::NotAnEnd(name.clone()));
            }
            let inside = point.iter().all(|v| v.is_finite() && v.abs() <= MAX_REACH);
            let [x, y, z] = *point;
            if !(inside && figure.reach_point(joint, Vec3::new(x, y, z))) {
                return Err(WireError::OutOfReach {
                    joint: joint.id(),
                    point: *point,
                });
            }
        }
        let view = &mut figure.view;
        let (yaw, pitch, fov) = (
            self.yaw.unwrap_or(view.yaw()),
            self.pitch.unwrap_or(view.pitch()),
            self.fov.unwrap_or(view.fov()),
        );
        if !(yaw.is_finite()
            && pitch.is_finite()
            && pitch.abs() <= MAX_PITCH
            && fov.is_finite()
            && (0.0..=MAX_FOV).contains(&fov))
        {
            return Err(WireError::View);
        }
        view.set_yaw(yaw);
        view.set_pitch(pitch);
        view.set_fov(fov);
        Ok(figure)
    }
}

/// Reads `name=bend,twist,side` (degrees), the command line's way of setting one joint.
pub fn parse_joint_setting(text: &str) -> Result<(String, [f64; 3]), String> {
    parse_three(text, "joint=bend,twist,side")
}

/// Reads `end=x,y,z`, the command line's way of pulling a hand or a foot to a point.
pub fn parse_reach_setting(text: &str) -> Result<(String, [f64; 3]), String> {
    parse_three(text, "end=x,y,z")
}

fn parse_three(text: &str, form: &str) -> Result<(String, [f64; 3]), String> {
    let (name, values) = text
        .split_once('=')
        .ok_or_else(|| format!("{text:?} is not {form}"))?;
    let numbers: Vec<f64> = values
        .split(',')
        .map(|v| v.trim().parse::<f64>())
        .collect::<Result<_, _>>()
        .map_err(|_| format!("{text:?} needs three numbers after ="))?;
    match numbers.as_slice() {
        [a, b, c] => Ok((name.trim().to_owned(), [*a, *b, *c])),
        _ => Err(format!("{text:?} needs three numbers after =")),
    }
}

/// Everything a figure can be asked to be: the bodies, the built-in poses, the proportions with
/// their ranges, the joints with how far each turns, and the hands and feet that reach.
pub fn catalog() -> Value {
    let pair = |(a, b): (f64, f64)| json!([a, b]);
    json!({
        "bodies": Body::ALL.iter().map(|b| b.id()).collect::<Vec<_>>(),
        "poses": presets().iter().map(|p| p.id.as_str()).collect::<Vec<_>>(),
        "proportions": Proportion::ALL.iter().map(|p| json!({
            "id": p.id(),
            "range": pair(p.range()),
        })).collect::<Vec<_>>(),
        "joints": Joint::ALL.iter().map(|j| {
            let limits = j.limits();
            json!({
                "id": j.id(),
                "parent": j.parent().map(Joint::id),
                "bend": pair(limits.bend),
                "twist": pair(limits.twist),
                "side": pair(limits.side),
            })
        }).collect::<Vec<_>>(),
        "reach": ENDS.iter().map(|j| j.id()).collect::<Vec<_>>(),
        "lift": pair((-MAX_LIFT, MAX_LIFT)),
        "pitch": pair((-MAX_PITCH, MAX_PITCH)),
        "fov": pair((0.0, MAX_FOV)),
    })
}
