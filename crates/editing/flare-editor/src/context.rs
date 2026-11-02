//! In-place symbol editing support.
//!
//! Editing a symbol's timeline reuses every scene tool by presenting the symbol as a
//! one-scene scratch document. When the edit is done the caller applies
//! [`Command::ReplaceSymbolTimeline`] to the parent document, which validates the result
//! (including symbol cycles) like any other edit.

use flare_core::{SceneId, SymbolId};
use flare_document::{Document, Scene, Timeline};

/// A scratch document whose only scene is `symbol`'s timeline, plus that scene's ID.
///
/// The edited symbol stays in the library with an empty timeline so other symbols that
/// contain it remain valid, and so its own content is not duplicated (which would
/// repeat layer and shape IDs). Returns `None` when the symbol does not exist.
pub fn symbol_workspace(document: &Document, symbol: SymbolId) -> Option<(Document, SceneId)> {
    let edited = document.symbol(symbol)?;
    let scene_id = SceneId::new();
    let scene = Scene::new(scene_id, edited.name(), edited.timeline().clone());
    let symbols = document
        .symbols()
        .iter()
        .map(|s| {
            if s.id() == symbol {
                s.clone().with_timeline(Timeline::default())
            } else {
                s.clone()
            }
        })
        .collect();
    let scratch = Document::new(document.id(), document.frame_rate(), vec![scene], symbols)
        .ok()?
        .with_stage(document.stage());
    Some((scratch, scene_id))
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::num::NonZeroU32;

    use flare_core::{DocumentId, FrameRate, InstanceId, LayerId, Rgba8, ShapeId, Transform2D};
    use flare_document::{Content, FrameSpan, Instance, Layer, Shape, Symbol};

    use super::*;
    use flare_commands::{Command, EditError};
    use flare_history::EditorSession;

    fn one_frame(content: Content) -> FrameSpan {
        FrameSpan::new(0, NonZeroU32::MIN, content)
    }

    fn rect() -> Content {
        Content::Shape(Shape::rect(ShapeId::new(), 0.0, 0.0, 4.0, 4.0, Rgba8::WHITE).expect("rect"))
    }

    fn timeline(content: Content) -> Timeline {
        Timeline::new(vec![
            Layer::new(LayerId::new(), "L", vec![one_frame(content)]).expect("layer"),
        ])
    }

    fn instance(symbol: SymbolId) -> Content {
        Content::Instance(Instance::new(
            InstanceId::new(),
            symbol,
            Transform2D::IDENTITY,
        ))
    }

    /// outer contains inner; the scene contains outer.
    fn nested() -> (Document, SymbolId, SymbolId) {
        let (inner, outer) = (SymbolId::new(), SymbolId::new());
        let symbols = vec![
            Symbol::new(inner, "Inner", timeline(rect())),
            Symbol::new(outer, "Outer", timeline(instance(inner))),
        ];
        let scene = Scene::new(SceneId::new(), "S", timeline(instance(outer)));
        let rate = FrameRate::new(24, 1).expect("rate");
        (
            Document::new(DocumentId::new(), rate, vec![scene], symbols).expect("doc"),
            inner,
            outer,
        )
    }

    #[test]
    fn the_workspace_shows_the_symbol_as_a_scene_and_keeps_ancestors_valid() {
        let (doc, inner, outer) = nested();
        let (scratch, scene) = symbol_workspace(&doc, inner).expect("workspace");
        assert_eq!(scratch.scenes().len(), 1);
        assert_eq!(scratch.scenes()[0].id(), scene);
        assert_eq!(
            scratch.scenes()[0].timeline(),
            doc.symbol(inner).expect("inner").timeline()
        );
        assert_eq!(
            scratch.symbols().len(),
            2,
            "outer still refers to inner, so inner stays listed"
        );
        assert!(
            scratch
                .symbol(inner)
                .expect("inner")
                .timeline()
                .layers()
                .is_empty()
        );
        assert!(scratch.symbol(outer).is_some());
        assert_eq!(
            (scratch.stage(), scratch.frame_rate()),
            (doc.stage(), doc.frame_rate())
        );
        assert!(symbol_workspace(&doc, SymbolId::new()).is_none());
    }

    #[test]
    fn edits_made_in_the_workspace_replace_the_symbol_in_one_undo_step() {
        let (doc, inner, _) = nested();
        let mut main = EditorSession::new(doc);
        let (scratch, scene) = symbol_workspace(main.document(), inner).expect("workspace");
        let mut inner_session = EditorSession::new(scratch);
        let layer = inner_session.document().scenes()[0].timeline().layers()[0].id();
        let extra = Shape::rect(ShapeId::new(), 10.0, 10.0, 2.0, 2.0, Rgba8::WHITE).expect("rect");
        let span = FrameSpan::new(1, NonZeroU32::MIN, Content::Shape(extra));
        inner_session
            .apply([Command::InsertSpan { scene, layer, span }])
            .expect("edit");

        let timeline = inner_session.document().scenes()[0].timeline().clone();
        main.apply([Command::ReplaceSymbolTimeline {
            symbol: inner,
            timeline,
        }])
        .expect("replace");
        assert_eq!(
            main.document()
                .symbol(inner)
                .expect("inner")
                .timeline()
                .frame_count(),
            2
        );
        main.undo();
        assert_eq!(
            main.document()
                .symbol(inner)
                .expect("inner")
                .timeline()
                .frame_count(),
            1
        );
    }

    #[test]
    fn replacing_a_timeline_is_validated_including_cycles_and_unknown_symbols() {
        let (doc, inner, outer) = nested();
        let mut main = EditorSession::new(doc);
        // Putting `outer` inside `inner` closes a loop: outer -> inner -> outer.
        let cyclic = Command::ReplaceSymbolTimeline {
            symbol: inner,
            timeline: timeline(instance(outer)),
        };
        assert!(matches!(main.apply([cyclic]), Err(EditError::Invalid(_))));
        let ghost = SymbolId::new();
        let missing = Command::ReplaceSymbolTimeline {
            symbol: ghost,
            timeline: Timeline::default(),
        };
        assert_eq!(main.apply([missing]), Err(EditError::NoSuchSymbol(ghost)));
        assert!(!main.can_undo(), "failed replacements change nothing");
    }

    #[test]
    fn merging_moves_artwork_into_a_new_symbol_as_a_single_valid_step() {
        let (doc, inner, _) = nested();
        let mut main = EditorSession::new(doc);
        // The shape inside `inner` moves into a brand-new symbol and `inner` holds an instance of it.
        let moved = main
            .document()
            .symbol(inner)
            .expect("inner")
            .timeline()
            .clone();
        let fresh = SymbolId::new();
        let new_symbol = Symbol::new(fresh, "Fresh", moved);
        let merge = Command::MergeSymbolEdit {
            symbol: inner,
            timeline: timeline(instance(fresh)),
            new_symbols: vec![new_symbol.clone()],
        };
        main.apply([merge]).expect("valid as a whole");
        assert!(main.document().symbol(fresh).is_some());
        assert!(
            main.document()
                .symbol(inner)
                .expect("inner")
                .timeline()
                .referenced_symbols()
                .any(|s| s == fresh)
        );
        // Applied separately the same two edits would each be invalid (duplicate shape, dangling reference).
        main.undo();
        let add_first = Command::AddSymbol(new_symbol);
        assert!(matches!(
            main.apply([add_first]),
            Err(EditError::Invalid(_))
        ));
        let ghost = SymbolId::new();
        let bad = Command::MergeSymbolEdit {
            symbol: ghost,
            timeline: Timeline::default(),
            new_symbols: vec![],
        };
        assert_eq!(main.apply([bad]), Err(EditError::NoSuchSymbol(ghost)));
    }
}
