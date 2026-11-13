#![allow(clippy::expect_used)]

use flare_core::SceneId;
use flare_document::Content;
use flare_mannequin::{Body, Mannequin, Placement, View, preset};

use super::*;
use crate::test_support::{add, fixture, rect_span};

fn figure() -> Mannequin {
    let mut m = Mannequin::new(Body::Female);
    m.pose = preset("walk").expect("walk");
    m.view = View::new(30.0, 5.0, 25.0);
    m
}

#[test]
fn the_figure_goes_on_an_unlocked_see_through_layer_on_top_in_one_undo_step() {
    let mut f = fixture();
    add(&mut f, rect_span(0, 12, 0.0, 0.0, 10.0, 10.0));
    let at = Placement::new([275.0, 200.0], 300.0).expect("placement");
    let placed =
        place(f.session.document(), f.scene, 3, &figure(), at, "Mannequin").expect("placed");
    f.session.apply(placed.commands.clone()).expect("applies");

    let document = f.session.document();
    let layers = document.scenes()[0].timeline().layers();
    assert_eq!(layers.len(), 2);
    let layer = &layers[0];
    assert_eq!((layer.id(), layer.name()), (placed.layer, "Mannequin"));
    // Unlocked, so the picture can be moved and scaled like any other.
    assert!(!layer.is_locked() && layer.is_visible());
    assert_eq!(layer.view_opacity(), REFERENCE_OPACITY);
    let span = &layer.spans()[0];
    assert_eq!(
        (span.start(), span.duration().get()),
        (3, 9),
        "to the end of the scene"
    );
    let Content::Bitmap(shown) = span.content() else {
        panic!("a bitmap");
    };
    assert_eq!(shown.bitmap(), placed.bitmap);
    let bitmap = document.bitmap(placed.bitmap).expect("in the library");
    assert_eq!((bitmap.width(), bitmap.height()), placed.size);
    assert!(bitmap.pixels().chunks(4).any(|p| p[3] == 255), "the body");
    assert!(bitmap.pixels().chunks(4).any(|p| p[3] == 0), "round it");

    assert!(f.session.undo());
    let document = f.session.document();
    assert_eq!(document.scenes()[0].timeline().layers().len(), 1);
    assert!(document.bitmap(placed.bitmap).is_none());
}

#[test]
fn the_picture_is_where_the_figure_was_placed_and_the_size_it_was_given() {
    let mut m = figure();
    m.view = View::new(0.0, 0.0, 0.0);
    m.pose = flare_mannequin::Pose::default();
    let at = Placement::new([100.0, 300.0], 200.0).expect("placement");
    let (picture, corner, scale) = picture(&m, at).expect("picture");
    assert_eq!(scale, RESOLUTION);
    let rows: Vec<usize> = (0..picture.height() as usize)
        .filter(|y| {
            let row =
                &picture.rgba()[y * picture.width() as usize * 4..][..picture.width() as usize * 4];
            row.chunks(4).any(|p| p[3] > 128)
        })
        .collect();
    let stage = |row: usize| corner[1] + row as f64 / scale;
    let (top, bottom) = (stage(rows[0]), stage(rows[rows.len() - 1]));
    // The figure is a unit tall about its middle: 200 tall, from 200 down to 400.
    assert!(
        (top - 200.0).abs() < 2.0 && (bottom - 400.0).abs() < 2.0,
        "{top} to {bottom}"
    );
}

#[test]
fn a_huge_figure_is_pictured_at_the_most_a_picture_may_be() {
    let at = Placement::new([0.0, 0.0], 50_000.0).expect("placement");
    let (picture, _, scale) = picture(&figure(), at).expect("picture");
    assert!(scale < RESOLUTION);
    assert!(picture.width().max(picture.height()) <= MAX_PICTURE_SIDE);
}

#[test]
fn an_empty_scene_gets_one_frame_and_a_missing_scene_is_refused() {
    let f = fixture();
    let at = Placement::new([0.0, 0.0], 50.0).expect("placement");
    let placed = place(f.session.document(), f.scene, 0, &figure(), at, "M").expect("placed");
    let Command::AddLayer { layer, .. } = &placed.commands[1] else {
        panic!("adds a layer");
    };
    assert_eq!(layer.spans()[0].duration().get(), 1);
    assert_eq!(
        place(f.session.document(), SceneId::new(), 0, &figure(), at, "M"),
        Err(PlaceRefusal::NoSuchScene)
    );
}

/// The spans of the fixture's layer, as (start, length, elements), and the layer itself.
fn spans(f: &crate::test_support::Fixture) -> Vec<(u32, u32, usize)> {
    let layers = f.session.document().scenes()[0].timeline().layers();
    let layer = layers.iter().find(|l| l.id() == f.layer).expect("layer");
    layer
        .spans()
        .iter()
        .map(|s| (s.start(), s.duration().get(), s.element_count()))
        .collect()
}

fn onto(
    f: &mut crate::test_support::Fixture,
    frame: u32,
    keyframe: Keyframe,
) -> Result<Placed, PlaceRefusal> {
    let at = Placement::new([100.0, 100.0], 120.0).expect("placement");
    let placed = place_onto(
        f.session.document(),
        f.scene,
        frame,
        &figure(),
        at,
        f.layer,
        keyframe,
        "Mannequin",
    )?;
    f.session.apply(placed.commands.clone()).expect("applies");
    Ok(placed)
}

#[test]
fn figures_join_the_keyframe_at_the_frame_so_several_share_its_frames() {
    let mut f = fixture();
    add(&mut f, rect_span(0, 12, 0.0, 0.0, 10.0, 10.0));
    let first = onto(&mut f, 0, Keyframe::Join).expect("placed");
    let second = onto(&mut f, 4, Keyframe::Join).expect("placed");
    assert_eq!(spans(&f), vec![(0, 12, 3)], "the rectangle and two figures");
    assert_eq!((first.at.start, first.at.element), (0, 1));
    assert_eq!((second.at.start, second.at.element), (0, 2));
    // The same pose placed again shows the picture the library has, not a copy.
    assert_eq!(second.bitmap, first.bitmap);
    assert_eq!(f.session.document().bitmaps().len(), 1);
    let document = f.session.document();
    for placed in [&first, &second] {
        let found = crate::ops::find_element(document, f.scene, placed.at).expect("element");
        assert!(matches!(found, Content::Bitmap(b) if b.bitmap() == placed.bitmap));
    }
    // Each placing is one undo step.
    assert!(f.session.undo());
    assert_eq!(spans(&f), vec![(0, 12, 2)]);
}

#[test]
fn a_new_keyframe_cuts_the_one_held_there_so_each_frame_range_has_its_own_figure() {
    let mut f = fixture();
    add(&mut f, rect_span(0, 12, 0.0, 0.0, 10.0, 10.0));
    let placed = onto(&mut f, 5, Keyframe::New).expect("placed");
    assert_eq!(spans(&f), vec![(0, 5, 1), (5, 7, 1)]);
    assert_eq!((placed.at.start, placed.at.element), (5, 0));
    // Another one at the same frame joins the keyframe that now starts there.
    onto(&mut f, 5, Keyframe::New).expect("placed");
    assert_eq!(spans(&f), vec![(0, 5, 1), (5, 7, 2)]);
    // Where the layer shows nothing, it starts a keyframe up to the layer's next one.
    let mut h = fixture();
    add(&mut h, rect_span(0, 3, 0.0, 0.0, 10.0, 10.0));
    add(&mut h, rect_span(10, 2, 0.0, 0.0, 10.0, 10.0));
    onto(&mut h, 5, Keyframe::Join).expect("placed");
    assert_eq!(
        spans(&h),
        vec![(0, 3, 1), (5, 5, 1), (10, 2, 1)],
        "up to the next keyframe"
    );
    assert!(f.session.undo() && f.session.undo());
    assert_eq!(spans(&f), vec![(0, 12, 1)]);
}

#[test]
fn a_locked_hidden_or_missing_layer_is_refused() {
    let mut f = fixture();
    add(&mut f, rect_span(0, 12, 0.0, 0.0, 10.0, 10.0));
    let layer = spans(&f);
    let lock = Command::UpdateLayer {
        scene: f.scene,
        layer: f.layer,
        change: flare_commands::LayerChange::Locked(true),
    };
    f.session.apply([lock]).expect("applies");
    for keyframe in [Keyframe::Join, Keyframe::New] {
        assert_eq!(
            onto(&mut f, 5, keyframe).map(|p| p.at),
            Err(PlaceRefusal::Layer(crate::drawing::DrawRefusal::Locked))
        );
    }
    assert_eq!(spans(&f), layer, "nothing changed");
    let at = Placement::new([0.0, 0.0], 50.0).expect("placement");
    let missing = place_onto(
        f.session.document(),
        f.scene,
        0,
        &figure(),
        at,
        flare_core::LayerId::new(),
        Keyframe::Join,
        "M",
    );
    assert_eq!(
        missing.map(|p| p.at),
        Err(PlaceRefusal::Layer(
            crate::drawing::DrawRefusal::NoSuchLayer
        ))
    );
    let said = PlaceRefusal::Layer(crate::drawing::DrawRefusal::Locked).to_string();
    assert!(said.contains("locked"), "{said}");
}
