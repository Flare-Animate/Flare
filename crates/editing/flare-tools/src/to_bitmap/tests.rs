#![allow(clippy::expect_used)]

use std::num::NonZeroU32;

use flare_core::{LayerId, Rgba8, ShapeId};
use flare_document::{FrameSpan, Layer, Shape};

use super::*;
use crate::common::find_element;
use crate::test_support::{Fixture, add, add_on_new_layer, at, fixture, rect_span};

/// A keyframe at 0 holding a rectangle at `x` for each of `xs`, each 10 by 10 at `y` 10.
fn rects(f: &mut Fixture, xs: &[f64]) {
    let mut span = rect_span(0, 5, xs[0], 10.0, 10.0, 10.0);
    for &x in &xs[1..] {
        let shape = Shape::rect(
            ShapeId::new(),
            x,
            10.0,
            10.0,
            10.0,
            Rgba8::opaque(0, 0, 255),
        )
        .expect("rect");
        span = span.with_appended(Content::Shape(shape)).expect("room");
    }
    add(f, span);
}

/// Pixels for `plan` that are clear but for an opaque block from `(x0, y0)` to `(x1, y1)`.
fn drawn(plan: &ToBitmapPlan, [x0, y0, x1, y1]: [u32; 4]) -> Vec<u8> {
    let mut rgba = vec![0u8; (plan.width * plan.height * 4) as usize];
    for y in y0..y1 {
        for x in x0..x1 {
            let i = ((y * plan.width + x) * 4) as usize;
            rgba[i..i + 4].copy_from_slice(&[10, 20, 30, 255]);
        }
    }
    rgba
}

fn bitmap_at(f: &Fixture, at: SpanRef) -> (BitmapInstance, Bitmap) {
    let doc = f.session.document();
    let Some(Content::Bitmap(placed)) = find_element(doc, f.scene, at) else {
        panic!("not a bitmap");
    };
    let bitmap = doc.bitmap(placed.bitmap()).expect("in the library").clone();
    (placed.clone(), bitmap)
}

#[test]
fn the_plan_covers_the_selection_with_a_margin_at_the_scale() {
    let mut f = fixture();
    rects(&mut f, &[10.0]);
    let one = plan(f.session.document(), f.scene, &[at(&f, 0)], 1).expect("plan");
    assert_eq!((one.x, one.y), (8.0, 8.0));
    assert_eq!((one.width, one.height), (14, 14));
    assert_eq!(one.contents.len(), 1);
    let two = plan(f.session.document(), f.scene, &[at(&f, 0)], 2).expect("plan");
    assert_eq!(
        (two.width, two.height),
        (28, 28),
        "twice the pixels each way"
    );
}

#[test]
fn the_pixels_take_the_shapes_place_trimmed_and_where_they_were_drawn_in_one_step() {
    let mut f = fixture();
    rects(&mut f, &[10.0]);
    let doc = f.session.document();
    let plan = plan(doc, f.scene, &[at(&f, 0)], 2).expect("plan");
    // The rectangle drawn 2 pixels to a unit from (8, 8): pixels 4 to 24 each way.
    let edit = place(
        doc,
        f.scene,
        &plan,
        &drawn(&plan, [4, 4, 24, 24]),
        "Bitmap 1",
    )
    .expect("place");
    let steps = f.session.undo_labels().len();
    f.session.apply(edit.commands.clone()).expect("apply");
    assert_eq!(f.session.undo_labels().len(), steps + 1);
    let (placed, bitmap) = bitmap_at(&f, edit.at);
    assert_eq!(
        (bitmap.width(), bitmap.height()),
        (20, 20),
        "the clear margin is trimmed"
    );
    assert_eq!(bitmap.name(), "Bitmap 1");
    let matrix = placed.transform().as_affine();
    assert_eq!(matrix.transform_point2(DVec2::ZERO), DVec2::new(10.0, 10.0));
    assert_eq!(
        matrix.transform_point2(DVec2::new(20.0, 20.0)),
        DVec2::new(20.0, 20.0)
    );
    let span = find_span(f.session.document(), f.scene, at(&f, 0)).expect("span");
    assert_eq!(span.element_count(), 1, "the shape is gone");
    assert!(f.session.undo());
    assert!(matches!(
        find_element(f.session.document(), f.scene, at(&f, 0)),
        Some(Content::Shape(_))
    ));
    assert!(f.session.document().bitmaps().is_empty());
}

#[test]
fn the_bitmap_takes_the_topmost_selected_place_and_the_rest_stay() {
    let mut f = fixture();
    rects(&mut f, &[10.0, 30.0, 50.0]);
    let doc = f.session.document();
    let selection = [at(&f, 0), at(&f, 0).with_element(2)];
    let plan = plan(doc, f.scene, &selection, 1).expect("plan");
    assert_eq!(plan.elements, vec![0, 2]);
    let edit = place(doc, f.scene, &plan, &drawn(&plan, [2, 2, 52, 12]), "B").expect("place");
    f.session.apply(edit.commands).expect("apply");
    let span = find_span(f.session.document(), f.scene, at(&f, 0)).expect("span");
    let kinds: Vec<bool> = span
        .elements()
        .map(|c| matches!(c, Content::Bitmap(_)))
        .collect();
    assert_eq!(
        kinds,
        vec![false, true],
        "the middle rectangle under the bitmap"
    );
    assert_eq!(edit.at.element, 1);
}

#[test]
fn a_converted_bitmap_leaves_the_library_unless_it_is_shown_elsewhere() {
    let mut f = fixture();
    rects(&mut f, &[10.0]);
    let doc = f.session.document();
    let first = plan(doc, f.scene, &[at(&f, 0)], 1).expect("plan");
    let edit = place(doc, f.scene, &first, &drawn(&first, [2, 2, 12, 12]), "A").expect("place");
    f.session.apply(edit.commands).expect("apply");
    let old = bitmap_at(&f, edit.at).1.id();
    // Converting the bitmap again at twice the scale makes a new one and drops the old.
    let doc = f.session.document();
    let again = plan(doc, f.scene, &[edit.at], 2).expect("plan");
    let edit = place(doc, f.scene, &again, &drawn(&again, [4, 4, 24, 24]), "A").expect("place");
    f.session.apply(edit.commands).expect("apply");
    let library = f.session.document().bitmaps();
    assert_eq!(library.len(), 1);
    assert_ne!(library[0].id(), old);
    // Shown on another layer too, the picture stays when one of its places is converted.
    let mut g = fixture();
    rects(&mut g, &[10.0]);
    let doc = g.session.document();
    let p = plan(doc, g.scene, &[at(&g, 0)], 1).expect("plan");
    let edit = place(doc, g.scene, &p, &drawn(&p, [2, 2, 12, 12]), "A").expect("place");
    g.session.apply(edit.commands).expect("apply");
    let (shown, picture) = bitmap_at(&g, edit.at);
    let five = NonZeroU32::new(5).expect("five");
    let other = add_on_new_layer(
        &mut g,
        FrameSpan::new(0, five, Content::Bitmap(shown.clone())),
    );
    let doc = g.session.document();
    let p = plan(doc, g.scene, &[other], 1).expect("plan");
    let edit = place(doc, g.scene, &p, &drawn(&p, [3, 3, 9, 9]), "A").expect("place");
    g.session.apply(edit.commands).expect("apply");
    assert!(
        g.session.document().bitmap(picture.id()).is_some(),
        "still shown"
    );
}

#[test]
fn what_cannot_be_converted_is_refused() {
    let mut f = fixture();
    rects(&mut f, &[10.0]);
    let other = add_on_new_layer(&mut f, rect_span(0, 5, 40.0, 40.0, 5.0, 5.0));
    let doc = f.session.document();
    let here = at(&f, 0);
    assert_eq!(
        plan(doc, f.scene, &[], 1),
        Err(ToBitmapError::NothingSelected)
    );
    assert_eq!(
        plan(doc, f.scene, &[here, other], 1),
        Err(ToBitmapError::NotOneKeyframe)
    );
    assert_eq!(plan(doc, f.scene, &[here], 0), Err(ToBitmapError::Scale));
    assert_eq!(
        plan(doc, f.scene, &[here], MAX_SCALE + 1),
        Err(ToBitmapError::Scale)
    );
    assert_eq!(
        plan(doc, f.scene, &[here.with_element(7)], 1),
        Err(ToBitmapError::Missing)
    );
    let ok = plan(doc, f.scene, &[here], 1).expect("plan");
    assert_eq!(
        place(doc, f.scene, &ok, &[0; 4], "A"),
        Err(ToBitmapError::Pixels)
    );
    let clear = vec![0u8; (ok.width * ok.height * 4) as usize];
    assert_eq!(
        place(doc, f.scene, &ok, &clear, "A"),
        Err(ToBitmapError::Empty)
    );
}

#[test]
fn a_tweened_keyframe_a_locked_layer_and_a_huge_selection_are_refused() {
    let mut f = fixture();
    add(
        &mut f,
        rect_span(0, 5, 0.0, 0.0, 10.0, 10.0).with_tween(Tween::Linear),
    );
    let doc = f.session.document();
    assert_eq!(
        plan(doc, f.scene, &[at(&f, 0)], 1),
        Err(ToBitmapError::Tweened)
    );

    let mut g = fixture();
    let locked = LayerId::new();
    g.session
        .apply([Command::AddLayer {
            scene: g.scene,
            layer: Layer::new(
                locked,
                "Locked",
                vec![rect_span(0, 5, 0.0, 0.0, 10.0, 10.0)],
            )
            .expect("layer")
            .with_locked(true),
        }])
        .expect("layer");
    let target = SpanRef {
        layer: locked,
        start: 0,
        element: 0,
    };
    assert_eq!(
        plan(g.session.document(), g.scene, &[target], 1),
        Err(ToBitmapError::Locked)
    );

    let mut h = fixture();
    add(&mut h, rect_span(0, 5, 0.0, 0.0, 9000.0, 10.0));
    assert!(matches!(
        plan(h.session.document(), h.scene, &[at(&h, 0)], 1),
        Err(ToBitmapError::TooLarge(_, _))
    ));
}

#[test]
fn a_keyframe_is_named_by_its_layer_and_frame_with_all_or_some_of_its_elements() {
    let mut f = fixture();
    rects(&mut f, &[10.0, 30.0]);
    let doc = f.session.document();
    let all = keyframe_selection(doc, f.scene, None, 3, None).expect("all");
    assert_eq!(all, vec![at(&f, 0), at(&f, 0).with_element(1)]);
    let one = keyframe_selection(doc, f.scene, Some(f.layer), 0, Some(&[1])).expect("one");
    assert_eq!(one, vec![at(&f, 0).with_element(1)]);
    assert_eq!(
        keyframe_selection(doc, f.scene, Some(LayerId::new()), 0, None),
        Err(ToBitmapError::NoSuchLayer)
    );
    assert_eq!(
        keyframe_selection(doc, f.scene, None, 50, None),
        Err(ToBitmapError::NoKeyframe)
    );
    assert_eq!(
        keyframe_selection(doc, f.scene, None, 0, Some(&[2])),
        Err(ToBitmapError::Missing)
    );
}
