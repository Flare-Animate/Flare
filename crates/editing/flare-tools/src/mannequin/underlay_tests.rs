//! The mannequin as a guide under the drawing: at the bottom of the layers, on the stage and in
//! no export, with an opacity of its own that can be changed, and hidden to switch it off.
#![allow(clippy::expect_used)]

use flare_document::Content;
use flare_mannequin::{Body, Mannequin, Placement, View, preset};

use super::*;
use crate::test_support::{add, fixture, rect_span};

fn figure() -> Mannequin {
    let mut m = Mannequin::new(Body::Male);
    m.pose = preset("stand").expect("stand");
    m.view = View::new(0.0, 0.0, 0.0);
    m
}

fn placed_under(f: &mut crate::test_support::Fixture) -> Placed {
    let at = Placement::new([100.0, 100.0], 120.0).expect("placement");
    let placed =
        place_under(f.session.document(), f.scene, 0, &figure(), at, "Guide").expect("placed");
    f.session.apply(placed.commands.clone()).expect("applies");
    placed
}

#[test]
fn the_guide_goes_below_the_drawing_layers_locked_see_through_and_holding() {
    let mut f = fixture();
    add(&mut f, rect_span(0, 12, 0.0, 0.0, 10.0, 10.0));
    let placed = placed_under(&mut f);
    let layers = f.session.document().scenes()[0]
        .timeline()
        .layers()
        .to_vec();
    assert_eq!(layers.len(), 2);
    assert_eq!(layers[0].id(), f.layer, "the drawing layer stays on top");
    let guide = &layers[1];
    assert_eq!(guide.id(), placed.layer, "the guide is the bottom layer");
    assert!(guide.is_reference() && guide.is_locked() && guide.is_hold());
    assert_eq!(guide.view_opacity(), REFERENCE_OPACITY);
    assert!(matches!(guide.spans()[0].content(), Content::Bitmap(_)));
    // One undo step takes it, and its picture, away.
    assert!(f.session.undo());
    assert_eq!(
        f.session.document().scenes()[0].timeline().layers().len(),
        1
    );
    assert!(f.session.document().bitmap(placed.bitmap).is_none());
}

#[test]
fn the_guide_is_on_the_stage_but_in_no_export() {
    let mut f = fixture();
    add(&mut f, rect_span(0, 12, 0.0, 0.0, 10.0, 10.0));
    placed_under(&mut f);
    let document = f.session.document();
    let stage = flare_eval::evaluate_frame_without_camera(document, f.scene, 0).expect("stage");
    assert_eq!(stage.bitmaps.len(), 1, "the editing view shows the guide");
    assert_eq!(stage.shapes.len(), 1, "and the drawing over it");
    let export = flare_eval::evaluate_frame(document, f.scene, 0).expect("export");
    assert!(export.bitmaps.is_empty(), "an export has only the drawing");
    assert_eq!(export.shapes.len(), 1);
    let level =
        flare_eval::evaluate_level(document, document.scenes()[0].timeline(), 0).expect("level");
    assert!(
        level
            .items
            .iter()
            .all(|i| !matches!(i.content, flare_eval::LevelContent::Bitmap { .. })),
        "nor is it published"
    );
}

#[test]
fn the_opacity_changes_and_hiding_the_guide_switches_it_off() {
    let mut f = fixture();
    add(&mut f, rect_span(0, 12, 0.0, 0.0, 10.0, 10.0));
    let placed = placed_under(&mut f);
    f.session
        .apply([
            flare_commands::Command::UpdateLayer {
                scene: f.scene,
                layer: placed.layer,
                change: flare_commands::LayerChange::ViewOpacity(20),
            },
            flare_commands::Command::UpdateLayer {
                scene: f.scene,
                layer: placed.layer,
                change: flare_commands::LayerChange::Visible(false),
            },
        ])
        .expect("changes");
    let document = f.session.document();
    let guide = &document.scenes()[0].timeline().layers()[1];
    assert_eq!(guide.view_opacity(), 20);
    let stage = flare_eval::evaluate_frame_without_camera(document, f.scene, 0).expect("stage");
    assert!(stage.bitmaps.is_empty(), "hidden, it is not shown");
}
