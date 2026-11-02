#![allow(clippy::expect_used, unused_imports)]

use crate::ops::*;
use crate::test_support::*;
use emath::{Pos2, Rect};
use flare_commands::Command;
use flare_core::{
    DocumentId, FrameRate, InstanceId, LayerId, Rgba8, SceneId, ShapeId, SymbolId, Transform2D,
};
use flare_document::{
    Content, Document, FrameSpan, Instance, Layer, PathSegment, Scene, Shape, Symbol, Timeline,
    Tween,
};
use flare_history::EditorSession;
use glamx::{DAffine2, DVec2};
use std::num::NonZeroU32;

#[test]
fn hits_resolve_to_the_owning_scene_level_span() {
    let mut f = fixture();
    add(&mut f, rect_span(0, 24, 0.0, 0.0, 10.0, 10.0));
    let (commands, symbol) =
        convert_to_symbol(f.session.document(), f.scene, at(&f, 0)).expect("convert");
    f.session.apply(commands).expect("convert");
    let shape_layer = f.layer;
    let commands = place_instance(
        f.session.document(),
        f.scene,
        symbol,
        DVec2::new(100.0, 100.0),
    )
    .expect("place");
    f.session.apply(commands).expect("place");

    let doc = f.session.document();
    let frame = flare_eval::evaluate_frame(doc, f.scene, 3).expect("eval");
    // Paint order: the new layer (last) is drawn first, so the original instance is on top.
    for drawn in &frame.shapes {
        let found = locate_hit(doc, f.scene, 3, drawn).expect("located");
        assert_eq!(found.start, 0);
        assert!(
            doc.scenes()[0]
                .timeline()
                .layers()
                .iter()
                .any(|l| l.id() == found.layer)
        );
    }
    assert!(
        frame
            .shapes
            .iter()
            .any(|d| locate_hit(doc, f.scene, 3, d).is_some_and(|r| r.layer == shape_layer))
    );
}
