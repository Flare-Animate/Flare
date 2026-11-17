#![allow(clippy::unwrap_used, clippy::expect_used)]
//! The round trip: write the model back as a Moho project document and read it again —
//! `parse(write(parse(x))) == parse(x)`. What the model does not keep cannot come back; the
//! writer's doc header lists those, and the tests here check the rest.

use flare_moho::{
    Binding, Channel, ChannelKind, KeyValue, Layer, LayerKind, Mesh, Project, Track, parse_bytes,
    write_document,
};

const TINY: &str = include_str!("fixtures/tiny.mohoproj.json");

fn round_trip(source: &str) -> Project {
    let first = parse_bytes(source.as_bytes(), "x.mohoproj").expect("first parse");
    let written = write_document(&first).expect("write");
    let text = String::from_utf8(written).expect("utf8");
    parse_bytes(text.as_bytes(), "x.mohoproj").expect("second parse")
}

#[test]
fn the_fixture_round_trips_model_equal() {
    // Both parses use the same entry name, which is carried into the model.
    let first = parse_bytes(TINY.as_bytes(), "x.mohoproj").expect("parse");
    let written = write_document(&first).expect("write");
    let text = String::from_utf8(written).expect("utf8");
    let again = parse_bytes(text.as_bytes(), "x.mohoproj").expect("second parse");
    assert_eq!(again, first);
}

#[test]
fn a_bone_layer_skeleton_keeps_its_bones() {
    let source = r#"
    {
      "version": 1045, "major_version": 13,
      "project_data": {"width": 640, "height": 480, "fps": 24.0, "start_frame": 0, "end_frame": 10},
      "layers": [
        {"name": "Rig", "type": "BoneLayer", "uuid": "r1", "visible": true,
         "skeleton": {"bones": [
           {"name": "hip", "parent": -1, "length": 1.0, "strength": 0.0, "bone_dynamics": false},
           {"name": "spine", "parent": 0, "length": 2.0, "strength": 0.5, "bone_dynamics": true}
         ]}}
      ]
    }"#;
    let again = round_trip(source);
    assert_eq!(again.bones.len(), 2);
    assert_eq!(again.bones[0].name, "hip");
    assert_eq!(again.bones[0].parent, None);
    assert_eq!(again.bones[1].name, "spine");
    assert_eq!(again.bones[1].parent, Some(0));
    assert!(again.bones[1].dynamic);
    // The written document carries the skeleton in the bone layer, as Moho 13 does.
    assert!(round_trip(source).layers[0].kind == LayerKind::Bone);
}

#[test]
fn both_skeletons_and_every_track_come_back() {
    let source = r#"
    {
      "version": 1045, "major_version": 13,
      "project_data": {"width": 640, "height": 480, "fps": 12.0, "start_frame": 0, "end_frame": 24},
      "skeleton": {"bones": [
        {"name": "root", "parent": -1, "length": 1.0, "strength": 0.0},
        {"name": "leg", "parent": 0, "length": 3.0, "strength": 1.0}
      ]},
      "layers": [
        {"name": "A", "type": "MeshLayer", "uuid": "a", "visible": true,
         "parent_bone": 1, "flexi_bone_subset": "0|1",
         "mesh": {"points": [{"position": {"x": 0.0, "y": 0.0}}, {"position": {"x": 4.0, "y": 2.0}}],
                  "curves": [{"closed": true, "points": [{"point": 0}, {"point": 1}]}],
                  "shapes": [{"has_fill": true, "style": {"fill_color": {"r": 1.0, "g": 0.5, "b": 0.25, "a": 1.0}}}]}
        },
        {"name": "B", "type": "BoneLayer", "uuid": "b",
         "skeleton": {"bones": [{"name": "neck", "parent": -1, "length": 0.5}]}
        }
      ]
    }"#;
    let again = round_trip(source);
    // Three bones: two from the root skeleton, one from the bone layer.
    assert_eq!(again.bones.len(), 3);
    assert_eq!(again.bones[2].name, "neck");
    let mesh = again.layers[0].mesh.clone().expect("mesh");
    assert_eq!(mesh.points, [[0.0, 0.0], [4.0, 2.0]]);
    assert_eq!(mesh.curves.len(), 1);
    assert!(mesh.curves[0].closed);
    assert_eq!(mesh.curves[0].points, [0, 1]);
    assert_eq!(mesh.fill, Some([1.0, 0.5, 0.25, 1.0]));
    assert_eq!(mesh.fills, 1);
    assert_eq!(again.layers[0].binding, Binding::Bone(1));
    assert_eq!(again.layers[0].flexible_bone_subset, "0|1");
}

#[test]
fn animated_tracks_land_back_where_they_were_read() {
    let source = r#"
    {
      "version": 1045, "major_version": 13,
      "project_data": {"width": 640, "height": 480, "fps": 12.0, "start_frame": 0, "end_frame": 24},
      "layers": [
        {"name": "A", "type": "MeshLayer", "uuid": "a", "visible": true,
         "transforms": {
           "translation": {"type": "Vec2", "when": [0, 12], "val": [{"x": 1.0, "y": 2.0}, {"x": 3.0, "y": 4.0}], "interp": ["Smooth", "Linear"]},
           "rotation": {"type": "Val", "when": [0], "val": [45.0], "mute": true}
         }
        }
      ]
    }"#;
    let again = round_trip(source);
    // The reader walks JSON keys alphabetically, so find tracks by their path.
    let track = |suffix: &str| {
        again
            .tracks
            .iter()
            .find(|t| t.path.ends_with(suffix))
            .expect("track")
    };
    let paths: Vec<&str> = again.tracks.iter().map(|t| t.path.as_str()).collect();
    assert!(
        paths.contains(&"layers[0].transforms.translation"),
        "{paths:?}"
    );
    assert!(
        paths.contains(&"layers[0].transforms.rotation"),
        "{paths:?}"
    );
    let translation = track("transforms.translation");
    assert_eq!(translation.channel.keys.len(), 2);
    assert_eq!(
        translation.channel.keys[1].value,
        KeyValue::Vector(vec![3.0, 4.0])
    );
    assert_eq!(translation.channel.interp, ["Smooth", "Linear"]);
    // A muted channel stays muted.
    let rotation = track("transforms.rotation");
    assert!(rotation.channel.muted);
}

#[test]
fn a_switch_and_its_keys_come_back() {
    let source = r#"
    {
      "version": 1045, "major_version": 13,
      "project_data": {"width": 640, "height": 480, "fps": 12.0, "start_frame": 0, "end_frame": 24},
      "layers": [
        {"name": "Eyes", "type": "SwitchLayer", "uuid": "s1", "visible": true,
         "switch_keys": {"type": "String", "when": [0, 10], "val": ["open", "shut"]},
         "layers": [
           {"name": "open", "type": "ImageLayer", "uuid": "o1", "image_path": "a.png"},
           {"name": "shut", "type": "ImageLayer", "uuid": "c1", "image_path": "b.png"}
         ]}
      ]
    }"#;
    let again = round_trip(source);
    let switch = again.layers[0].switch.as_ref().expect("switch");
    assert_eq!(switch.alternatives, ["open", "shut"]);
    assert_eq!(switch.keys.keys.len(), 2);
    assert_eq!(switch.at_rest.as_deref(), Some("open"));
    assert_eq!(switch.at_end.as_deref(), Some("shut"));
    assert_eq!(again.images, ["a.png", "b.png"]);
}

#[test]
fn an_unknown_layer_type_is_written_back_as_it_was_read() {
    let source = r#"
    {
      "version": 1045, "major_version": 13,
      "project_data": {"width": 640, "height": 480, "fps": 12.0, "start_frame": 0, "end_frame": 24},
      "layers": [
        {"name": "Notes", "type": "NoteLayer", "uuid": "n1", "visible": true}
      ]
    }"#;
    let again = round_trip(source);
    assert_eq!(
        again.layers[0].kind,
        LayerKind::Other("NoteLayer".to_owned())
    );
}

#[test]
fn unknown_key_values_are_dropped_and_the_channel_stays_whole() {
    // A model assembled by hand, with one unrepresentable key in the middle.
    let project = Project {
        layers: vec![Layer {
            name: "A".into(),
            kind: LayerKind::Vector,
            uuid: String::new(),
            visible: true,
            depth: 0,
            binding: Binding::None,
            flexible_bone_subset: String::new(),
            image_path: None,
            texture_path: None,
            audio_path: None,
            switch: None,
            mesh: Some(Mesh::default()),
            children: Vec::new(),
        }],
        tracks: vec![Track {
            path: "layers[0].mesh.points[0].position".into(),
            channel: Channel {
                kind: ChannelKind::Vec2,
                keys: vec![
                    flare_moho::Key {
                        frame: 0,
                        value: KeyValue::Vector(vec![1.0, 2.0]),
                    },
                    flare_moho::Key {
                        frame: 6,
                        value: KeyValue::Unknown,
                    },
                    flare_moho::Key {
                        frame: 12,
                        value: KeyValue::Vector(vec![3.0, 4.0]),
                    },
                ],
                interp: vec![],
                muted: false,
            },
        }],
        ..Project::default()
    };
    let written = write_document(&project).expect("write");
    let text = String::from_utf8(written).expect("utf8");
    let again = parse_bytes(text.as_bytes(), "x.mohoproj").expect("parse");
    let kept = &again.tracks[0].channel.keys;
    assert_eq!(kept.len(), 2);
    assert_eq!(kept[0].frame, 0);
    assert_eq!(kept[1].frame, 12);
    assert_eq!(kept[1].value, KeyValue::Vector(vec![3.0, 4.0]));
}
