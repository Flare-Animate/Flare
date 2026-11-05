use super::*;
use crate::codec::Packer;
use crate::records::Parts;
use flare_document::{Diagnostic, SymbolKind};
use flare_game::{Body, Component, EntitySpec, GameSpec, Transform};

fn with_game(symbol: Option<SymbolId>) -> Document {
    let layer = Layer::new(LayerId::new(), "L", vec![]).expect("layer");
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(vec![layer]));
    let base = Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![scene],
        vec![
            Symbol::new(SymbolId::new(), "hero", Timeline::default())
                .with_kind(SymbolKind::Graphic),
        ],
    )
    .expect("document");
    let hero = base.symbols()[0].id();
    let entity = EntitySpec::new(flare_game::EntityId::new(1).expect("id"), "hero")
        .expect("name")
        .with_symbol(symbol.or(Some(hero)))
        .with_component(Component::Transform(Transform::default()))
        .expect("transform")
        .with_component(Component::Body(Body::default()))
        .expect("body");
    let mut game = GameSpec::new().with_gravity(0.0, 9.8).expect("gravity");
    game.add_entity(entity).expect("entity");
    base.with_game(Some(game)).expect("game is valid")
}

#[test]
fn a_game_survives_saving_and_opening() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("game.flare");
    let document = with_game(None);
    save(&document, &path).expect("save");
    let back = load(&path).expect("load");
    assert_eq!(back, document);
    assert_eq!(back.game().map(GameSpec::len), Some(1));
}

#[test]
fn a_project_with_a_game_is_schema_three_and_one_without_is_still_one() {
    let dir = tempfile::tempdir().expect("dir");
    let with = dir.path().join("with.flare");
    save(&with_game(None), &with).expect("save");
    let conn = Connection::open(&with).expect("open");
    let version: i64 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("version");
    assert_eq!(version, crate::records::GAME_SCHEMA_VERSION);
    drop(conn);

    let plain_document = Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![Scene::new(
            SceneId::new(),
            "S",
            Timeline::new(vec![
                Layer::new(LayerId::new(), "L", vec![]).expect("layer"),
            ]),
        )],
        vec![],
    )
    .expect("document");
    let plain = dir.path().join("plain.flare");
    save(&plain_document, &plain).expect("save");
    let conn = Connection::open(&plain).expect("open");
    let version: i64 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("version");
    assert_eq!(version, SCHEMA_VERSION);
    assert!(load(&plain).expect("load").game().is_none());
}

/// Replaces the game record of `parts` with `game`, packed the way `to_parts` packs it.
fn replace_game(parts: &mut Parts, game: &GameSpec) {
    let raw = wire::game::frame(game).expect("frame");
    let mut codec = NativeZstd::new().expect("zstd");
    let stored = Packer::new(&mut codec).pack(&raw).expect("pack");
    let record = parts
        .records
        .iter_mut()
        .find(|r| r.kind == "game")
        .expect("game record");
    record.codec = stored.codec.name().to_owned();
    record.size = i64::try_from(stored.size).expect("size");
    record.digest = stored.digest.to_vec();
    record.payload = stored.bytes.into_owned();
}

#[test]
fn a_stored_game_that_draws_a_symbol_the_library_lacks_is_refused() {
    let document = with_game(None);
    let mut parts = to_parts(&document, &mut NativeZstd::new().expect("zstd")).expect("parts");
    let missing = SymbolId::new();
    let mut game = GameSpec::new();
    game.add_entity(
        EntitySpec::new(flare_game::EntityId::new(1).expect("id"), "ghost")
            .expect("name")
            .with_symbol(Some(missing))
            .with_component(Component::Transform(Transform::default()))
            .expect("transform"),
    )
    .expect("entity");
    replace_game(&mut parts, &game);
    let err = from_parts(parts, &mut NativeZstd::new().expect("zstd")).expect_err("refused");
    match err {
        StorageError::Invalid(diagnostics) => assert!(diagnostics.contains(&Diagnostic::Game(
            flare_game::GameError::NoSuchSymbol(missing)
        ))),
        other => panic!("expected an invalid document, got {other:?}"),
    }
}

#[test]
fn a_damaged_game_record_is_corrupt_not_guessed_at() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("damaged.flare");
    save(&with_game(None), &path).expect("save");
    set_raw(&path, "game", b"not cbor at all");
    assert!(matches!(load(&path), Err(StorageError::Corrupt(_))));
}
