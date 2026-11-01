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
fn deleting_removes_the_span() {
    let mut f = fixture();
    add(&mut f, rect_span(0, 5, 0.0, 0.0, 5.0, 5.0));
    f.session.apply(delete(f.scene, at(&f, 0))).expect("delete");
    assert!(find_span(f.session.document(), f.scene, at(&f, 0)).is_none());
}
