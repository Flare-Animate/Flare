//! Raster and vector layers: the Brush paints pixels and the Eraser rubs them out on a raster
//! layer, and draws shapes on a vector one (what a new layer is); a layer's blend mode; and the
//! Brush's presets there.

use super::blend::{at, at_stage, drag, empty_workspace};
use super::*;
use flare_document::LayerBlend;
use flare_tools::blend::RasterPreset;

fn first_layer(ws: &Workspace) -> &Layer {
    &ws.document().scenes()[0].timeline().layers()[0]
}

fn active(ws: &Workspace) -> &Layer {
    ws.active_layer().expect("a layer")
}

/// The pictures on the active layer at frame 0, bottom first.
fn pictures(ws: &Workspace) -> Vec<flare_document::Bitmap> {
    let document = ws.document();
    let Some(span) = active(ws).span_at(0) else {
        return Vec::new();
    };
    span.elements()
        .filter_map(|content| match content {
            Content::Bitmap(placed) => document.bitmap(placed.bitmap()).cloned(),
            _ => None,
        })
        .collect()
}

#[test]
fn a_new_layer_is_vector_and_new_raster_layer_makes_a_raster_one_in_front() {
    let mut ws = empty_workspace();
    ws.perform(MenuAction::NewLayer);
    assert!(!active(&ws).is_raster(), "layers are vector unless asked");
    let steps = ws.active().undo_labels().count();
    assert!(ws.is_enabled(MenuAction::NewRasterLayer));
    ws.perform(MenuAction::NewRasterLayer);
    assert_eq!(ws.active().undo_labels().count(), steps + 1, "one step");
    let layers = ws.document().scenes()[0].timeline().layers();
    assert_eq!(layers.iter().filter(|l| l.is_raster()).count(), 1);
    assert!(active(&ws).is_raster(), "the new layer is chosen");
    assert!(ws.on_raster_layer());
}

#[test]
fn the_brush_paints_pixels_on_a_raster_layer_and_shapes_on_a_vector_one() {
    let mut ws = empty_workspace();
    ws.set_tool(Tool::Brush);
    assert_eq!(ws.blend_kind(), None, "a vector layer");
    ws.perform(MenuAction::NewRasterLayer);
    assert_eq!(
        ws.blend_kind(),
        Some(flare_tools::blend::BlendKind::RasterBrush)
    );
    ws.paint.fill = Color32::from_rgb(0, 0, 255);
    drag(&mut ws, &[(20.0, 50.0), (180.0, 50.0)]);
    let painted = pictures(&ws);
    assert_eq!(painted.len(), 1, "a painting on the raster layer");
    assert_eq!(at_stage(&painted[0], 100, 50), [0, 0, 255, 255]);
}

#[test]
fn the_eraser_rubs_pixels_out_on_a_raster_layer() {
    let mut ws = empty_workspace();
    ws.perform(MenuAction::NewRasterLayer);
    ws.set_tool(Tool::Brush);
    ws.paint.blend.raster.size = 30.0;
    drag(&mut ws, &[(10.0, 50.0), (190.0, 50.0)]);
    ws.set_tool(Tool::Eraser);
    assert_eq!(ws.blend_kind(), Some(flare_tools::blend::BlendKind::Erase));
    assert!(!ws.eraser_rubs(), "not the shape eraser");
    let steps = ws.active().undo_labels().count();
    drag(&mut ws, &[(100.0, 20.0), (100.0, 80.0)]);
    assert_eq!(ws.active().undo_labels().count(), steps + 1, "one step");
    let painting = &pictures(&ws)[0];
    assert_eq!(at_stage(painting, 100, 50)[3], 0, "rubbed out");
    assert_eq!(at_stage(painting, 40, 50)[3], 255, "the rest stays");

    // A click on nothing to rub out says so and changes nothing.
    let before = ws.document().clone();
    ws.freehand = vec![at(100.0, 5.0)];
    ws.finish_freehand(1.0);
    assert_eq!(ws.document(), &before);
}

#[test]
fn the_layer_tab_sets_the_blend_mode_as_one_step_and_draws() {
    let mut ws = empty_workspace();
    let layer = first_layer(&ws).id();
    let steps = ws.active().undo_labels().count();
    ws.set_layer_blend(layer, LayerBlend::Multiply);
    assert_eq!(first_layer(&ws).blend(), LayerBlend::Multiply);
    ws.set_layer_blend(layer, LayerBlend::Multiply);
    assert_eq!(
        ws.active().undo_labels().count(),
        steps + 1,
        "the same mode is no step"
    );
    ws.undo();
    assert_eq!(first_layer(&ws).blend(), LayerBlend::Normal);
    ws.show_props_context(PropsContext::Layer);
    assert_eq!(ws.props_context(), PropsContext::Layer);
    let ctx = egui::Context::default();
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| ws.properties_panel(ui));
    output.textures_delta.clear();
}

#[test]
fn converting_a_layer_keeps_what_it_shows_and_undoes_in_one_step() {
    let mut ws = empty_workspace();
    let layer = first_layer(&ws).id();
    let scene = ws.scene_id().expect("scene");
    let band = flare_document::Shape::rect(
        flare_core::ShapeId::new(),
        20.0,
        30.0,
        100.0,
        40.0,
        Rgba8::opaque(255, 0, 0),
    )
    .expect("rect");
    ws.run_batch(
        vec![Command::InsertSpan {
            scene,
            layer,
            span: FrameSpan::new(0, NonZeroU32::MIN, Content::Shape(band)),
        }],
        "draw",
    );
    let before = ws.document().clone();
    let steps = ws.active().undo_labels().count();
    ws.convert_layer(layer, true);
    assert!(!ws.converting_layer(), "the worker answered");
    assert!(first_layer(&ws).is_raster());
    assert_eq!(pictures(&ws).len(), 1, "the band is a painting");
    assert_eq!(at_stage(&pictures(&ws)[0], 40, 20), [255, 0, 0, 255]);
    assert_eq!(ws.active().undo_labels().count(), steps + 1, "one step");
    assert_eq!(ws.active().undo_labels().last(), Some("Convert the layer"));

    ws.convert_layer(layer, false);
    assert!(!first_layer(&ws).is_raster());
    assert!(pictures(&ws).is_empty(), "traced back into shapes");
    assert!(ws.document().bitmaps().is_empty());
    let span = first_layer(&ws).span_at(0).expect("keyframe");
    assert!(span.elements().all(|c| matches!(c, Content::Shape(_))));

    ws.undo();
    ws.undo();
    assert_eq!(ws.document(), &before, "two undos put it all back");
}

#[test]
fn a_preset_sets_the_brush_and_the_round_one_is_where_it_starts() {
    let mut ws = empty_workspace();
    assert_eq!(
        RasterPreset::of(&ws.paint.blend.raster),
        Some(RasterPreset::Round)
    );
    ws.paint.blend.raster = RasterPreset::Airbrush.settings();
    ws.perform(MenuAction::NewRasterLayer);
    ws.set_tool(Tool::Brush);
    assert_eq!(ws.blend_size(), Some(80.0), "the airbrush's size");
}

#[test]
fn the_tool_strip_has_no_raster_brush_of_its_own() {
    let names: Vec<&str> = crate::workspace::panels::tool_slots::SLOT_GROUPS
        .iter()
        .flat_map(|g| g.iter())
        .flat_map(|slot| slot.iter().map(|v| v.name))
        .collect();
    assert!(!names.contains(&"Raster Brush"), "{names:?}");
}

#[test]
fn the_brush_options_show_each_presets_swatch_and_make_it_once() {
    let mut ws = empty_workspace();
    ws.perform(MenuAction::NewRasterLayer);
    ws.set_tool(Tool::Brush);
    let ctx = egui::Context::default();
    let draw = |ws: &mut Workspace| {
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| ws.raster_presets(ui));
        output.textures_delta.clear();
    };
    draw(&mut ws);
    let made = ws.paint.blend.swatches.len();
    // The list's swatches are made with the row, and the brush in hand is drawn under it.
    assert_eq!(
        made,
        RasterPreset::ALL.len() + 1,
        "each preset and the brush in hand"
    );
    draw(&mut ws);
    assert_eq!(
        ws.paint.blend.swatches.len(),
        made,
        "drawn again, made again none"
    );
    // Swatches are tinted where they are shown, as the vector gallery's are, not painted in the
    // fill color, so a new color makes none again.
    ws.paint.fill = egui::Color32::from_rgb(0, 160, 80);
    draw(&mut ws);
    assert_eq!(ws.paint.blend.swatches.len(), made, "a new fill, made none");
    ws.paint.blend.raster.size = 55.0;
    draw(&mut ws);
    assert_eq!(
        ws.paint.blend.swatches.len(),
        made + 1,
        "an edited brush is drawn anew"
    );
}
