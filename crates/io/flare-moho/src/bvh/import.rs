//! A BVH skeleton and its motion as a Flare document: every joint is a folder with an art layer
//! (a line to its end site or first child) whose spans hold one pose per frame. Flare's document
//! is two dimensional, so only the X and Y positions and the Z rotation are used; any other
//! channel is reported. Joint offsets are added up along the hierarchy, because folders do not
//! compose transforms.

use flare_core::{DocumentPoint, Rgba8, ShapeId};
use flare_document::{PathSegment, Shape, Stage, Stroke};

use super::{Axis, Bvh, Motion};
use crate::convert::{Builder, Converted, Severity, finish, frame_rate};
use crate::convert_anim::{Rig, place};
use crate::error::MohoError;
use crate::model::{Channel, ChannelKind, Key, KeyValue};
use flare_core::{LayerId, SceneId};

/// Most keyframes (frames times joints) one import makes.
pub const MAX_POSES: usize = 2_000_000;

fn channel(kind: ChannelKind, rows: impl Iterator<Item = (i64, KeyValue)>) -> Channel {
    let keys: Vec<Key> = rows.map(|(frame, value)| Key { frame, value }).collect();
    Channel {
        kind,
        interp: vec!["Linear".to_owned(); keys.len()],
        keys,
        muted: false,
    }
}

/// Imports a validated [`Bvh`]. Fails with [`MohoError::Limit`] when it holds too many poses.
pub fn to_document(bvh: &Bvh) -> Result<Converted, MohoError> {
    if bvh.frame_count.saturating_mul(bvh.joints.len()) > MAX_POSES {
        return Err(MohoError::Limit {
            what: "BVH poses (frames times joints)",
            limit: MAX_POSES,
        });
    }
    bvh.validate()
        .map_err(|e| MohoError::Document(e.to_string()))?;
    let mut b = Builder {
        scene: SceneId::new(),
        commands: Vec::new(),
        notes: Vec::new(),
        origin: [960.0, 540.0],
        units_to_px: 1.0,
        end_frame: i64::try_from(bvh.frame_count)
            .unwrap_or(0)
            .saturating_sub(1),
    };
    let rate = frame_rate(bvh.frame_time.recip())
        .ok_or_else(|| MohoError::Document("the BVH frame time is not usable".into()))?;
    let stage =
        Stage::new(1920, 1080, Rgba8::WHITE).map_err(|e| MohoError::Document(e.to_string()))?;
    let mut first_channel = Vec::with_capacity(bvh.joints.len());
    let mut at = 0;
    for joint in &bvh.joints {
        first_channel.push(at);
        at += joint.channels.len();
    }
    let root = b.folder("Skeleton", None, true);
    let mut folders: Vec<LayerId> = Vec::new();
    let mut rest: Vec<[f64; 2]> = Vec::new();
    for (j, joint) in bvh.joints.iter().enumerate() {
        let base = joint
            .parent
            .and_then(|p| rest.get(p).copied())
            .unwrap_or([0.0, 0.0]);
        let here = [base[0] + joint.offset[0], base[1] + joint.offset[1]];
        rest.push(here);
        let parent = joint
            .parent
            .and_then(|p| folders.get(p).copied())
            .unwrap_or(root);
        let folder = b.folder(&joint.name, Some(parent), true);
        folders.push(folder);
        let column = |motion: Motion| {
            joint
                .channels
                .iter()
                .position(|m| *m == motion)
                .map(|c| first_channel[j] + c)
        };
        let series = |c: Option<usize>| -> Vec<f64> {
            c.map_or_else(Vec::new, |c| {
                (0..bvh.frame_count)
                    .filter_map(|f| bvh.value(f, c))
                    .collect()
            })
        };
        let (px, py) = (
            series(column(Motion::Position(Axis::X))),
            series(column(Motion::Position(Axis::Y))),
        );
        let rz = series(column(Motion::Rotation(Axis::Z)));
        for m in &joint.channels {
            if matches!(
                m,
                Motion::Position(Axis::Z) | Motion::Rotation(Axis::X | Axis::Y)
            ) {
                b.note(
                    Severity::Unsupported,
                    &joint.name,
                    format!("{} is not 2D and is dropped", m.name()),
                );
            }
        }
        let frame = |f: usize| i64::try_from(f).unwrap_or(0);
        let at = |v: &Vec<f64>, f: usize| v.get(f).copied().unwrap_or(0.0);
        let translation = channel(
            ChannelKind::Vec2,
            (0..bvh.frame_count).map(|f| {
                (
                    frame(f),
                    KeyValue::Vector(vec![here[0] + at(&px, f), here[1] + at(&py, f)]),
                )
            }),
        );
        let rotation = channel(
            ChannelKind::Val,
            (0..bvh.frame_count).map(|f| (frame(f), KeyValue::Number(at(&rz, f).to_radians()))),
        );
        let rig = Rig {
            translation: Some(&translation),
            rotation: (!rz.is_empty()).then_some(&rotation),
            scale: None,
        };
        let placement = place(rig, b.end_frame, b.origin, b.units_to_px, false);
        let tip = joint
            .end_site
            .or_else(|| {
                bvh.joints
                    .iter()
                    .find(|c| c.parent == Some(j))
                    .map(|c| c.offset)
            })
            .unwrap_or([10.0, 0.0, 0.0]);
        let symbol = b.symbol(&joint.name, bone_line(tip));
        b.posed_layer(
            &format!("{} pose", joint.name),
            symbol,
            placement,
            Some(folder),
            true,
        );
    }
    b.note(
        Severity::Info,
        "skeleton",
        "rotations are not composed down the hierarchy",
    );
    finish(b, rate, stage)
}

/// A line from the joint to its tip, which Flare draws in the joint's own frame.
fn bone_line(tip: [f64; 3]) -> Option<Shape> {
    let end = DocumentPoint::new(tip[0], -tip[1]).ok()?;
    let stroke = Stroke::new(3.0, Rgba8::new(200, 60, 20, 255)).ok()?;
    Shape::new(
        ShapeId::new(),
        vec![
            PathSegment::MoveTo(DocumentPoint::ORIGIN),
            PathSegment::LineTo(end),
        ],
        Rgba8::TRANSPARENT,
    )
    .ok()
    .map(|s| s.with_stroke(stroke))
}
