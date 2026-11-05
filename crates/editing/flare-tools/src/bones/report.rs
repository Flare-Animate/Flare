//! What an armature is at a frame, in plain numbers: what the command line and the MCP server
//! print, and what a test reads.

use flare_core::SceneId;
use flare_document::Document;
use serde::Serialize;

use crate::common::scene_timeline;

/// One joint as posed at the frame. Angles are in degrees, clockwise.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct JointReport {
    pub joint: usize,
    pub instance: String,
    pub symbol: String,
    pub parent: Option<usize>,
    pub x: f64,
    pub y: f64,
    /// How far the joint is turned from rest.
    pub turn: f64,
    pub limit: Option<[f64; 2]>,
}

/// One armature layer at the frame.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ArmatureReport {
    pub layer: String,
    pub name: String,
    /// The first frame of the pose the frame is in.
    pub pose_start: u32,
    pub joints: Vec<JointReport>,
}

/// Every armature of `scene` with something at `frame`, top layer first.
pub fn report(document: &Document, scene: SceneId, frame: u32) -> Vec<ArmatureReport> {
    let Some(timeline) = scene_timeline(document, scene) else {
        return Vec::new();
    };
    timeline
        .layers()
        .iter()
        .filter_map(|layer| {
            let (posed, span) = super::armature_at(document, scene, layer.id(), frame).ok()?;
            let joints = posed
                .joints()
                .iter()
                .zip(posed.posed())
                .zip(posed.pose())
                .enumerate()
                .map(|(index, ((joint, (at, _)), pose))| JointReport {
                    joint: index,
                    instance: joint.instance().id().as_uuid().to_string(),
                    symbol: joint.instance().symbol().as_uuid().to_string(),
                    parent: joint.parent(),
                    x: at.x,
                    y: at.y,
                    turn: pose.turn.to_degrees(),
                    limit: joint.limit().map(|(a, b)| [a.to_degrees(), b.to_degrees()]),
                })
                .collect();
            Some(ArmatureReport {
                layer: layer.id().as_uuid().to_string(),
                name: layer.name().to_owned(),
                pose_start: span.start(),
                joints,
            })
        })
        .collect()
}

/// Whether `layer` holds an armature anywhere.
pub fn is_armature_layer(layer: &flare_document::Layer) -> bool {
    layer
        .spans()
        .iter()
        .any(|s| matches!(s.content(), flare_document::Content::Armature(_)))
}
