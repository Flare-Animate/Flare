//! Game edits go through validated batches like every other edit: a refused one changes
//! nothing, references into the library are checked, and a batch is one step.

#![allow(clippy::expect_used)]

use flare_commands::{Command, EditError, apply_all};
use flare_core::{DocumentId, FrameRate, LayerId, SceneId, SymbolId};
use flare_document::{Diagnostic, Document, Layer, Scene, Symbol, Timeline};
use flare_game::{
    Body, Camera, Component, ComponentKind, EntityId, EntitySpec, GameError, Transform,
};

fn id(raw: u32) -> EntityId {
    EntityId::new(raw).expect("id")
}

fn document(symbols: Vec<Symbol>) -> Document {
    let layer = Layer::new(LayerId::new(), "L", vec![]).expect("layer");
    Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![Scene::new(SceneId::new(), "S", Timeline::new(vec![layer]))],
        symbols,
    )
    .expect("document")
}

fn entity(raw: u32, symbol: Option<SymbolId>) -> EntitySpec {
    EntitySpec::new(id(raw), format!("e{raw}"))
        .expect("name")
        .with_symbol(symbol)
        .with_component(Component::Transform(Transform::default()))
        .expect("transform")
}

#[test]
fn a_batch_adds_entities_and_components_as_one_document() {
    let doc = document(vec![]);
    let batch = vec![
        Command::AddGameEntity(entity(1, None)),
        Command::SetGameComponent {
            entity: id(1),
            component: Component::Body(Body::default()),
        },
        Command::SetGameGravity { x: 0.0, y: 9.8 },
    ];
    let next = apply_all(&doc, batch).expect("batch").expect("changed");
    let game = next.game().expect("the document has a game now");
    assert_eq!(game.len(), 1);
    assert!(
        game.entity(id(1))
            .expect("entity")
            .component(ComponentKind::Body)
            .is_some()
    );
    assert_eq!(game.gravity(), (0.0, 9.8));
    assert!(doc.game().is_none(), "the input document is untouched");
}

#[test]
fn a_refused_command_fails_the_whole_batch() {
    let doc = document(vec![]);
    let batch = vec![
        Command::AddGameEntity(entity(1, None)),
        Command::SetGameComponent {
            entity: id(2),
            component: Component::Body(Body::default()),
        },
    ];
    let err = apply_all(&doc, batch).expect_err("entity 2 does not exist");
    assert_eq!(err, EditError::Game(GameError::NoSuchEntity(id(2))));
    assert!(doc.game().is_none());
}

#[test]
fn an_entity_must_draw_a_symbol_of_the_library() {
    let symbol = Symbol::new(SymbolId::new(), "hero", Timeline::new(vec![]));
    let known = symbol.id();
    let doc = document(vec![symbol]);
    let missing = SymbolId::new();
    let err = apply_all(&doc, vec![Command::AddGameEntity(entity(1, Some(missing)))])
        .expect_err("symbol is not in the library");
    assert!(matches!(err, EditError::Invalid(ref ds)
        if ds.contains(&Diagnostic::Game(GameError::NoSuchSymbol(missing)))));

    let next = apply_all(&doc, vec![Command::AddGameEntity(entity(1, Some(known)))])
        .expect("batch")
        .expect("changed");
    assert_eq!(
        next.game()
            .and_then(|g| g.entity(id(1)))
            .and_then(|e| e.symbol()),
        Some(known)
    );
}

#[test]
fn removing_a_symbol_that_a_game_entity_draws_is_refused() {
    let symbol = Symbol::new(SymbolId::new(), "hero", Timeline::new(vec![]));
    let sid = symbol.id();
    let doc = apply_all(
        &document(vec![symbol]),
        vec![Command::AddGameEntity(entity(1, Some(sid)))],
    )
    .expect("batch")
    .expect("changed");
    let err = apply_all(&doc, vec![Command::RemoveSymbol(sid)]).expect_err("still drawn");
    assert!(matches!(err, EditError::Invalid(_)));
}

#[test]
fn a_followed_entity_cannot_be_removed() {
    let doc = apply_all(
        &document(vec![]),
        vec![
            Command::AddGameEntity(entity(1, None)),
            Command::AddGameEntity(
                entity(2, None)
                    .with_component(Component::Camera(Camera {
                        follow: Some(id(1)),
                        ..Camera::default()
                    }))
                    .expect("camera"),
            ),
        ],
    )
    .expect("batch")
    .expect("changed");
    assert_eq!(
        apply_all(&doc, vec![Command::RemoveGameEntity(id(1))]).map(|_| ()),
        Err(EditError::Game(GameError::Followed(id(1))))
    );
    apply_all(
        &doc,
        vec![
            Command::RemoveGameEntity(id(2)),
            Command::RemoveGameEntity(id(1)),
        ],
    )
    .expect("both can go in one batch");
}

#[test]
fn a_game_edit_keeps_the_rest_of_the_document() {
    let symbol = Symbol::new(SymbolId::new(), "hero", Timeline::new(vec![]));
    let doc = document(vec![symbol]);
    let scene = doc.scenes()[0].id();
    let next = apply_all(&doc, vec![Command::AddGameEntity(entity(1, None))])
        .expect("batch")
        .expect("changed");
    assert_eq!(next.symbols().len(), 1);
    assert_eq!(next.scenes()[0].id(), scene);
    assert_eq!(next.frame_rate(), doc.frame_rate());
}

#[test]
fn a_new_entity_takes_the_next_free_id() {
    let doc = apply_all(
        &document(vec![]),
        vec![Command::AddGameEntity(entity(3, None))],
    )
    .expect("batch")
    .expect("changed");
    let game = doc.game().expect("game");
    assert_eq!(game.next_id().expect("free").get(), 4);
}
