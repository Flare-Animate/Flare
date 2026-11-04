#![allow(clippy::expect_used)]

use flare_core::SymbolId;
use flare_document::{PlayMode, Playback, SymbolKind};
use flare_rig::*;
use proptest::prelude::*;

fn bone(name: &str, parent: Option<usize>, length: f64) -> Bone {
    Bone {
        name: name.into(),
        parent,
        length,
        rest_angle: 0.0,
        limit: None,
        target: None,
    }
}

fn arm() -> Rig {
    Rig::new(SymbolId::new(), SymbolKind::Graphic)
        .apply(&[
            RigCommand::AddBone(bone("upper", None, 10.0)),
            RigCommand::AddBone(bone("lower", Some(0), 10.0)),
        ])
        .expect("arm")
}

fn goal(target: [f64; 2]) -> PoseKey {
    PoseKey {
        pose: Pose {
            ik: vec![IkGoal {
                tip: 1,
                chain: 2,
                target,
            }],
            ..Pose::default()
        },
        interp: Interp::Linear,
    }
}

fn tip_of(t: &[BoneTransform]) -> [f64; 2] {
    let b = t[1];
    [
        b.origin[0] + b.length * b.angle.cos(),
        b.origin[1] + b.length * b.angle.sin(),
    ]
}

#[test]
fn fk_pose_rotates_children_with_parents() {
    let mut key = PoseKey::default();
    key.pose.rotations.insert(0, std::f64::consts::FRAC_PI_2);
    let rig = arm()
        .apply(&[RigCommand::SetRigPose { frame: 0, key }])
        .expect("pose");
    let t = evaluate_rig(&rig, 5).expect("eval");
    assert!((t[1].origin[1] - 10.0).abs() < 1e-9 && t[1].origin[0].abs() < 1e-9);
}

#[test]
fn keys_ease_between_frames() {
    let mut a = PoseKey::default();
    a.pose.rotations.insert(0, 0.0);
    let mut b = PoseKey::default();
    b.pose.rotations.insert(0, 1.0);
    let rig = arm()
        .apply(&[
            RigCommand::SetRigPose { frame: 0, key: a },
            RigCommand::SetRigPose { frame: 10, key: b },
        ])
        .expect("keys");
    assert!((evaluate_rig(&rig, 5).expect("mid")[0].angle - 0.5).abs() < 1e-9);
}

#[test]
fn angle_limit_clamps_pose() {
    let mut key = PoseKey::default();
    key.pose.rotations.insert(0, 3.0);
    let rig = arm()
        .apply(&[
            RigCommand::SetBoneAngleLimit {
                bone: 0,
                limit: Some(AngleLimit::new(-0.5, 0.5).expect("l")),
            },
            RigCommand::SetRigPose { frame: 0, key },
        ])
        .expect("rig");
    assert_eq!(evaluate_rig(&rig, 0).expect("eval")[0].angle, 0.5);
}

#[test]
fn ik_reaches_reachable_and_clamps_unreachable() {
    let rig = arm()
        .apply(&[RigCommand::SetRigPose {
            frame: 0,
            key: goal([8.0, 8.0]),
        }])
        .expect("rig");
    let p = tip_of(&evaluate_rig(&rig, 0).expect("eval"));
    assert!((p[0] - 8.0).abs() < 1e-3 && (p[1] - 8.0).abs() < 1e-3);
    let far = arm()
        .apply(&[RigCommand::SetRigPose {
            frame: 0,
            key: goal([100.0, 0.0]),
        }])
        .expect("rig");
    let p = tip_of(&evaluate_rig(&far, 0).expect("eval"));
    assert!((p[0] - 20.0).abs() < 1e-6 && p[1].abs() < 1e-6);
}

#[test]
fn bad_edits_fail_and_change_nothing() {
    let rig = arm();
    assert_eq!(
        rig.apply(&[RigCommand::AddBone(bone("x", Some(5), 1.0))]),
        Err(RigError::BadParent(5))
    );
    assert!(
        rig.apply(&[RigCommand::AddBone(bone("x", None, f64::NAN))])
            .is_err()
    );
    assert!(
        rig.apply(&[RigCommand::AddBone(bone(" ", None, 1.0))])
            .is_err()
    );
    assert!(AngleLimit::new(1.0, -1.0).is_err());
    let mut key = goal([1.0, 1.0]);
    key.pose.ik[0].chain = 3;
    assert_eq!(
        rig.apply(&[RigCommand::SetRigPose { frame: 0, key }]),
        Err(RigError::BadChain)
    );
    assert_eq!(rig.bones.len(), 2);
}

#[test]
fn bone_count_is_bounded() {
    let mut rig = Rig::new(SymbolId::new(), SymbolKind::Graphic);
    for _ in 0..MAX_BONES {
        rig = rig
            .apply(&[RigCommand::AddBone(bone("b", None, 1.0))])
            .expect("bone");
    }
    assert_eq!(
        rig.apply(&[RigCommand::AddBone(bone("b", None, 1.0))]),
        Err(RigError::TooManyBones)
    );
}

#[test]
fn old_files_load_with_defaults_and_newer_ones_are_refused() {
    let id = serde_json::to_value(SymbolId::new()).expect("id");
    let old: Rig = serde_json::from_value(serde_json::json!({ "symbol": id })).expect("old");
    assert_eq!(
        (old.version, old.clock, old.bones.len()),
        (1, Clock::Locked, 0)
    );
    let mut newer = old;
    newer.version = 99;
    assert_eq!(newer.validate(), Err(RigError::NewerVersion(99)));
}

#[test]
fn movie_clip_rig_keeps_its_own_clock_and_graphic_follows_playback() {
    let pb = Playback {
        mode: PlayMode::SingleFrame,
        first_frame: 1,
    };
    let free = Rig::new(SymbolId::new(), SymbolKind::MovieClip);
    let locked = Rig::new(SymbolId::new(), SymbolKind::Graphic);
    assert_eq!(rig_symbol_frame(&free, pb, 7, 5), 2);
    assert_eq!(rig_symbol_frame(&locked, pb, 7, 5), 1);
}

proptest! {
    #[test]
    fn ik_is_finite_bounded_and_respects_limits(
        tx in -1000.0f64..1000.0, ty in -1000.0f64..1000.0,
        lo in -3.0f64..0.0, hi in 0.0f64..3.0, l1 in 0.0f64..50.0, l2 in 0.0f64..50.0,
    ) {
        let rig = Rig::new(SymbolId::new(), SymbolKind::Graphic)
            .apply(&[
                RigCommand::AddBone(bone("a", None, l1)),
                RigCommand::AddBone(bone("b", Some(0), l2)),
                RigCommand::SetBoneAngleLimit { bone: 1, limit: Some(AngleLimit::new(lo, hi).expect("l")) },
                RigCommand::SetRigPose { frame: 0, key: goal([tx, ty]) },
            ])
            .expect("rig");
        let t = evaluate_rig(&rig, 0).expect("eval");
        for b in &t {
            prop_assert!(b.origin.iter().all(|v| v.is_finite()) && b.angle.is_finite());
        }
        let rel = t[1].angle - t[0].angle;
        let rel = (rel + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI;
        prop_assert!(rel >= lo - 1e-9 && rel <= hi + 1e-9);
    }
}
