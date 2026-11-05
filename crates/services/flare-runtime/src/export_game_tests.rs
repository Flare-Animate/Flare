#![allow(clippy::expect_used)]

use super::*;
use flare_commands::Command;
use flare_core::{DocumentId, FrameRate, LayerId, SceneId};
use flare_document::{Layer, Scene, Timeline};
use flare_game::{Component, EntitySpec, GameSpec, Sprite, Transform};
use flare_history::EditorSession;

fn document() -> Document {
    let layer = Layer::new(LayerId::new(), "Layer 1", vec![]).expect("layer");
    let scene = Scene::new(SceneId::new(), "Scene 1", Timeline::new(vec![layer]));
    let rate = FrameRate::new(30, 1).expect("rate");
    Document::new(DocumentId::new(), rate, vec![scene], vec![]).expect("document")
}

fn with_hero(document: Document) -> Document {
    let mut session = EditorSession::new(document);
    let id = GameSpec::new().next_id().expect("id");
    let hero = EntitySpec::new(id, "Hero")
        .expect("entity")
        .with_component(Component::Transform(Transform {
            x: 4.0,
            ..Transform::default()
        }))
        .expect("transform")
        .with_component(Component::Sprite(Sprite {
            frames: 4,
            fps: 8.0,
            ..Sprite::default()
        }))
        .expect("sprite");
    session
        .apply([Command::AddGameEntity(hero)])
        .expect("add the entity");
    session.document().clone()
}

#[test]
fn a_project_without_a_game_is_refused() {
    assert!(export_game(&document(), 0, None, None, "game").is_err());
}

#[test]
fn the_description_names_its_sheet_and_round_trips() {
    let document = with_hero(document());
    let export = export_game(&document, 0, None, None, "hero").expect("export");
    let value: serde_json::Value = serde_json::from_str(&export.json).expect("json");
    assert_eq!(value["format"], GAME_FORMAT);
    assert_eq!(value["version"], GAME_FORMAT_VERSION);
    assert_eq!(value["frames_per_second"], 30.0);
    assert_eq!(value["sheet"]["image"], "hero.png");
    assert_eq!(value["sheet"]["atlas"], "hero.sheet.json");
    assert_eq!(value["sheet"]["first_frame"], 0);
    assert_eq!(value["sheet"]["frames"], export.sheet.frames);

    let back: GameSpec = serde_json::from_value(value["game"].clone()).expect("game");
    assert_eq!(Some(&back), document.game());
    assert!(export.sheet.png.starts_with(&[0x89, b'P', b'N', b'G']));
}

#[test]
fn a_range_outside_the_scene_is_refused() {
    let document = with_hero(document());
    assert!(export_game(&document, 5, Some(2), None, "hero").is_err());
    assert!(export_game(&document, 0, Some(999), None, "hero").is_err());
}
