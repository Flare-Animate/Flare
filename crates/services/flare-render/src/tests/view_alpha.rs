//! Transparent layers on the stage: faded as one sheet, folders and symbols included, and never
//! in an export.

use super::*;

const BLUE: Rgba8 = Rgba8::opaque(0, 0, 255);

fn stage(doc: &Document, scene: SceneId) -> Image {
    let frame = flare_eval::evaluate_frame(doc, scene, 0).expect("eval");
    render_stage_viewport(&frame, &Viewport::full(20, 20), 20, 20, Rgba8::WHITE).expect("stage")
}

fn near(got: Rgba8, want: (u8, u8, u8)) -> bool {
    let close = |a: u8, b: u8| a.abs_diff(b) <= 2;
    close(got.r, want.0) && close(got.g, want.1) && close(got.b, want.2)
}

#[test]
fn a_transparent_layer_is_faded_on_the_stage_as_one_sheet_and_not_in_an_export() {
    // Red under blue, both on one layer at 50%, over an opaque blue layer below on the left.
    let top = Layer::new(
        LayerId::new(),
        "Rough",
        vec![
            FrameSpan::new(
                0,
                NonZeroU32::new(1).expect("one"),
                rect(0.0, 0.0, 20.0, 20.0, RED),
            )
            .with_extras(vec![rect(10.0, 0.0, 10.0, 20.0, BLUE)])
            .expect("extras"),
        ],
    )
    .expect("layer")
    .with_view_opacity(50);
    let (doc, id) = scene_with_layers(vec![top]);
    let image = stage(&doc, id);
    let left = image.pixel(5, 10).expect("pixel");
    assert!(near(left, (255, 128, 128)), "half red over white: {left:?}");
    // Where blue lies over red in the same layer, only blue shows through: the layer is faded
    // whole, not shape by shape.
    let right = image.pixel(15, 10).expect("pixel");
    assert!(
        near(right, (128, 128, 255)),
        "half blue over white: {right:?}"
    );
    // An export draws it whole.
    let export = draw(&doc, id, Rgba8::WHITE);
    assert_eq!(export.pixel(5, 10), Some(RED));
}

#[test]
fn a_transparent_folder_fades_what_it_holds_and_symbols_inside() {
    let folder = LayerId::new();
    let symbol = SymbolId::new();
    let art = Symbol::new(
        symbol,
        "Art",
        Timeline::new(vec![
            Layer::new(
                LayerId::new(),
                "In",
                vec![span(rect(0.0, 0.0, 20.0, 20.0, RED))],
            )
            .expect("inner")
            .with_view_opacity(50),
        ]),
    );
    let layers = vec![
        Layer::new(folder, "F", vec![])
            .expect("folder")
            .with_folder(true)
            .with_view_opacity(50),
        Layer::new(
            LayerId::new(),
            "L",
            vec![span(Content::Instance(Instance::new(
                InstanceId::new(),
                symbol,
                Transform2D::IDENTITY,
            )))],
        )
        .expect("layer")
        .with_parent(Some(folder)),
    ];
    let id = SceneId::new();
    let doc = Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![Scene::new(id, "S", Timeline::new(layers))],
        vec![art],
    )
    .expect("doc");
    let frame = flare_eval::evaluate_frame(&doc, id, 0).expect("eval");
    assert_eq!(frame.shapes.len(), 1);
    assert!(
        (frame.shapes[0].view_alpha - 0.25).abs() < 1e-6,
        "50% of 50%"
    );
    let pixel = stage(&doc, id).pixel(10, 10).expect("pixel");
    assert!(near(pixel, (255, 191, 191)), "a quarter red: {pixel:?}");
}

#[test]
fn without_transparent_layers_the_stage_is_the_plain_picture() {
    let (doc, id) = scene_with(vec![span(rect(2.0, 2.0, 10.0, 10.0, RED))], vec![]);
    let frame = flare_eval::evaluate_frame(&doc, id, 0).expect("eval");
    assert!(!has_view_alpha(&frame));
    assert_eq!(stage(&doc, id).rgba(), draw(&doc, id, Rgba8::WHITE).rgba());
}
