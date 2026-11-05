#![allow(clippy::expect_used)]

use super::*;
use flare_core::{DocumentId, FrameRate, LayerId, SceneId};
use flare_document::{Layer, Scene, Timeline};
use flare_history::EditorSession;

fn session() -> EditorSession {
    let layer = Layer::new(LayerId::new(), "Layer 1", vec![]).expect("layer");
    let scene = Scene::new(SceneId::new(), "Scene 1", Timeline::new(vec![layer]));
    let rate = FrameRate::new(24, 1).expect("rate");
    let document = Document::new(DocumentId::new(), rate, vec![scene], vec![]).expect("document");
    EditorSession::new(document)
}

fn fields(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect()
}

fn apply(session: &mut EditorSession, commands: Vec<Command>) {
    session.apply(commands).expect("batch applies");
}

#[test]
fn an_entity_is_added_with_its_id_and_undone_in_one_step() {
    let mut session = session();
    assert_eq!(describe(session.document()), Value::Null);
    let (id, commands) = add_entity(session.document(), "Hero", None).expect("add");
    assert_eq!(u32::from(id), 1);
    apply(&mut session, commands);
    let described = describe(session.document());
    assert_eq!(described["entities"]["1"]["name"], "Hero");
    assert!(session.undo());
    assert_eq!(describe(session.document()), Value::Null);
}

#[test]
fn a_component_is_set_checked_and_removed() {
    let mut session = session();
    let (id, commands) = add_entity(session.document(), "Hero", None).expect("add");
    apply(&mut session, commands);
    apply(
        &mut session,
        set_component(id, "transform", &fields(&[("x", "3")])).expect("transform"),
    );
    let sprite = fields(&[("frames", "4"), ("fps", "8")]);
    apply(
        &mut session,
        set_component(id, "sprite", &sprite).expect("sprite"),
    );
    let described = describe(session.document());
    assert_eq!(described["entities"]["1"]["name"], "Hero");
    assert_eq!(
        described["entities"]["1"]["components"]
            .as_object()
            .map(|m| m.len()),
        Some(2)
    );

    let misspelt = fields(&[("fsp", "8")]);
    assert!(set_component(id, "sprite", &misspelt).is_err());
    assert!(set_component(id, "nonsense", &[]).is_err());

    apply(
        &mut session,
        remove_component(id, "sprite").expect("remove"),
    );
    assert_eq!(
        describe(session.document())["entities"]["1"]["components"]
            .as_object()
            .map(|m| m.len()),
        Some(1)
    );
    assert!(remove_component(id, "nonsense").is_err());
}

#[test]
fn a_failed_batch_changes_nothing() {
    let mut session = session();
    let (_id, commands) = add_entity(session.document(), "Hero", None).expect("add");
    apply(&mut session, commands);
    let before = describe(session.document());
    let missing = EntityId::new(9).expect("id");
    assert!(session.apply(remove_entity(missing)).is_err());
    assert!(session.apply(set_gravity(f64::NAN, 0.0)).is_err());
    assert_eq!(describe(session.document()), before);
}

#[test]
fn gravity_is_set_and_described() {
    let mut session = session();
    apply(&mut session, set_gravity(0.0, 980.0));
    assert_eq!(describe(session.document())["gravity"], json!([0.0, 980.0]));
}

#[test]
fn a_run_without_a_game_is_refused_and_a_run_repeats_exactly() {
    let mut session = session();
    assert!(run(session.document(), 60.0, 10, None).is_err());
    let (id, commands) = add_entity(session.document(), "Hero", None).expect("add");
    apply(&mut session, commands);
    apply(
        &mut session,
        set_component(id, "transform", &fields(&[("x", "3")])).expect("transform"),
    );
    let first = run(session.document(), 60.0, 30, Some("right;")).expect("run");
    let second = run(session.document(), 60.0, 30, Some("right;")).expect("run");
    assert_eq!(first, second);
    assert_eq!(first.frames, 30);
    let json = playback_json(&first);
    assert_eq!(json["frames"], 30);
    assert_eq!(json["digest"].as_str().map(str::len), Some(16));
}

#[test]
fn a_script_longer_than_the_run_is_refused() {
    let mut session = session();
    let (_, commands) = add_entity(session.document(), "Hero", None).expect("add");
    apply(&mut session, commands);
    let script = ";".repeat(5);
    assert!(run(session.document(), 60.0, 2, Some(&script)).is_err());
}
