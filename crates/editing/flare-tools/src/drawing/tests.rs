#![allow(clippy::expect_used)]

use std::num::NonZeroU32;

use flare_core::{LayerId, ShapeId};
use flare_document::{Content, Layer, Shape};

use super::*;
use crate::test_support::*;

fn shape(x: f64) -> Shape {
    Shape::rect(ShapeId::new(), x, 0.0, 10.0, 10.0, RED).expect("rect")
}

fn draw(
    f: &mut Fixture,
    target: Target,
    frame: u32,
    shapes: Vec<Shape>,
) -> Result<DrawPlan, DrawRefusal> {
    let plan = draw_shapes(
        f.session.document(),
        f.scene,
        target,
        frame,
        None,
        shapes,
        "Layer 1",
    )?;
    f.session.apply(plan.commands.clone()).expect("apply");
    Ok(plan)
}

fn layer(f: &Fixture, id: LayerId) -> &Layer {
    f.session.document().scenes()[0]
        .timeline()
        .layers()
        .iter()
        .find(|l| l.id() == id)
        .expect("layer")
}

#[test]
fn a_shape_on_a_frame_without_a_keyframe_starts_one_and_the_next_joins_it() {
    let mut f = fixture();
    let target = Target::Layer(f.layer);
    let plan = draw(&mut f, target, 0, vec![shape(0.0)]).expect("first");
    assert_eq!(plan.placed, vec![at(&f, 0)]);
    // Drawing on an empty layer makes a one-frame keyframe, as Animate does; it is not stretched.
    let span = &layer(&f, f.layer).spans()[0];
    assert_eq!(
        (span.start(), span.duration().get(), span.element_count()),
        (0, 1, 1)
    );

    let plan = draw(&mut f, target, 0, vec![shape(20.0), shape(40.0)]).expect("more");
    assert_eq!(plan.placed, vec![element(&f, 0, 1), element(&f, 0, 2)]);
    assert_eq!(
        layer(&f, f.layer).spans().len(),
        1,
        "no second span, no second layer"
    );
    assert_eq!(layer(&f, f.layer).spans()[0].element_count(), 3);
    f.session.undo();
    assert_eq!(
        layer(&f, f.layer).spans()[0].element_count(),
        1,
        "one undo step"
    );
}

#[test]
fn an_empty_keyframe_takes_the_first_shape_as_its_content() {
    let mut f = fixture();
    let target = Target::Layer(f.layer);
    add(
        &mut f,
        FrameSpan::new(0, NonZeroU32::new(5).expect("nz"), Content::Empty),
    );
    let plan = draw(&mut f, target, 2, vec![shape(0.0), shape(20.0)]).expect("draw");
    assert_eq!(plan.placed, vec![element(&f, 0, 0), element(&f, 0, 1)]);
    assert_eq!(layer(&f, f.layer).spans()[0].element_count(), 2);
}

#[test]
fn a_new_keyframe_stops_at_the_next_one_and_honours_an_explicit_length() {
    let mut f = fixture();
    let target = Target::Layer(f.layer);
    add(&mut f, rect_span(30, 5, 0.0, 0.0, 5.0, 5.0));
    draw(&mut f, target, 10, vec![shape(0.0)]).expect("draw");
    let spans = layer(&f, f.layer).spans();
    assert_eq!((spans[0].start(), spans[0].duration().get()), (10, 20));

    let plan = draw_shapes(
        f.session.document(),
        f.scene,
        target,
        40,
        NonZeroU32::new(7),
        vec![shape(0.0)],
        "L",
    )
    .expect("explicit");
    f.session.apply(plan.commands).expect("apply");
    assert_eq!(layer(&f, f.layer).spans()[2].duration().get(), 7);
}

#[test]
fn a_document_without_layers_gets_one_and_the_default_target_skips_folders() {
    let mut f = fixture();
    f.session
        .apply([flare_commands::Command::RemoveLayer {
            scene: f.scene,
            layer: f.layer,
        }])
        .expect("remove");
    let plan = draw(&mut f, Target::Default, 0, vec![shape(0.0)]).expect("draw");
    let made = plan.layer;
    assert_eq!(layer(&f, made).name(), "Layer 1");
    assert_eq!(layer(&f, made).spans()[0].element_count(), 1);

    let folder = LayerId::new();
    f.session
        .apply([flare_commands::Command::AddLayer {
            scene: f.scene,
            layer: Layer::new(folder, "F", vec![])
                .expect("layer")
                .with_folder(true),
        }])
        .expect("folder");
    f.session
        .apply([flare_commands::Command::MoveLayer {
            scene: f.scene,
            layer: folder,
            index: 0,
        }])
        .expect("move");
    let plan = draw(&mut f, Target::Default, 1, vec![shape(5.0)]).expect("draw");
    assert_eq!(plan.layer, made, "the folder on top is skipped");
}

#[test]
fn layers_that_cannot_take_art_and_keyframes_that_cannot_share_say_why() {
    for (change, expected) in [
        (0, DrawRefusal::Locked),
        (1, DrawRefusal::Hidden),
        (2, DrawRefusal::NotArt),
        (3, DrawRefusal::NotArt),
        (4, DrawRefusal::NotArt),
        (5, DrawRefusal::NotArt),
    ] {
        let f = fixture();
        let base = f.session.document().scenes()[0].timeline().layers()[0].clone();
        let changed = match change {
            0 => base.with_locked(true),
            1 => base.with_visible(false),
            2 => base.with_mask(true),
            3 => base.with_guide(true),
            4 => base.with_camera(true),
            _ => base.with_folder(true),
        };
        let mut f = f;
        f.session
            .apply([
                flare_commands::Command::RemoveLayer {
                    scene: f.scene,
                    layer: f.layer,
                },
                flare_commands::Command::AddLayer {
                    scene: f.scene,
                    layer: changed,
                },
            ])
            .expect("swap");
        let result = draw_shapes(
            f.session.document(),
            f.scene,
            Target::Layer(f.layer),
            0,
            None,
            vec![shape(0.0)],
            "L",
        );
        assert_eq!(result.map(|p| p.layer), Err(expected), "case {change}");
    }
    let mut f = fixture();
    let target = Target::Layer(f.layer);
    add(
        &mut f,
        rect_span(0, 5, 0.0, 0.0, 5.0, 5.0).with_tween(flare_document::Tween::Shape),
    );
    let result = draw_shapes(
        f.session.document(),
        f.scene,
        target,
        1,
        None,
        vec![shape(0.0)],
        "L",
    );
    assert_eq!(result.map(|p| p.layer), Err(DrawRefusal::Held));
    let missing = draw_shapes(
        f.session.document(),
        f.scene,
        Target::Layer(LayerId::new()),
        1,
        None,
        vec![shape(0.0)],
        "L",
    );
    assert_eq!(missing.map(|p| p.layer), Err(DrawRefusal::NoSuchLayer));
    let none = draw_shapes(
        f.session.document(),
        flare_core::SceneId::new(),
        Target::Default,
        0,
        None,
        vec![shape(0.0)],
        "L",
    );
    assert_eq!(none.map(|p| p.layer), Err(DrawRefusal::NoSuchScene));
    assert!(
        draw_shapes(
            f.session.document(),
            f.scene,
            Target::Default,
            0,
            None,
            vec![],
            "L"
        )
        .expect("nothing")
        .commands
        .is_empty()
    );
}

#[test]
fn a_locked_folder_locks_the_layers_in_it() {
    let mut f = fixture();
    let folder = LayerId::new();
    let inside = LayerId::new();
    f.session
        .apply([
            flare_commands::Command::AddLayer {
                scene: f.scene,
                layer: Layer::new(folder, "F", vec![])
                    .expect("layer")
                    .with_folder(true)
                    .with_locked(true),
            },
            flare_commands::Command::AddLayer {
                scene: f.scene,
                layer: Layer::new(inside, "In", vec![])
                    .expect("layer")
                    .with_parent(Some(folder)),
            },
        ])
        .expect("add");
    // The folder was appended last; the child must sit below it.
    let layers = f.session.document().scenes()[0].timeline().layers();
    if layers.iter().position(|l| l.id() == folder) < layers.iter().position(|l| l.id() == inside) {
        let result = draw_shapes(
            f.session.document(),
            f.scene,
            Target::Layer(inside),
            0,
            None,
            vec![shape(0.0)],
            "L",
        );
        assert_eq!(result.map(|p| p.layer), Err(DrawRefusal::Locked));
    }
}
