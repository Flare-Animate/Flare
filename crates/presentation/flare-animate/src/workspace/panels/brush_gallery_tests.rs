#![allow(clippy::expect_used)]

use super::*;
use egui::RawInput;

fn meshes(output: &egui::FullOutput) -> Vec<&Mesh> {
    output
        .shapes
        .iter()
        .filter_map(|clipped| match &clipped.shape {
            egui::Shape::Mesh(mesh) if !mesh.vertices.is_empty() => Some(mesh.as_ref()),
            _ => None,
        })
        .collect()
}

#[test]
fn every_brush_draws_a_swatch_inside_its_box_and_a_textured_one_draws_strands() {
    let rect = Rect::from_min_size(Pos2::new(10.0, 20.0), ENTRY);
    let mut counts = Vec::new();
    for profile in BuiltinBrushProfile::ALL {
        let stroke = profile.preset().stroke(swatch_width(ENTRY));
        let m = swatch_mesh(&stroke, rect, Color32::WHITE, 1.0);
        assert!(!m.vertices.is_empty(), "{profile:?}");
        assert!(m.indices.iter().all(|i| (*i as usize) < m.vertices.len()));
        // Feathering reaches a pixel past the outline, and no further.
        let bounds = rect.expand(1.0);
        assert!(
            m.vertices.iter().all(|v| bounds.contains(v.pos)),
            "{profile:?} stays in its box"
        );
        counts.push((profile, m.vertices.len()));
    }
    let plain = counts[0].1;
    for (profile, count) in counts {
        if profile.is_textured() {
            assert!(count > plain, "{profile:?} has strands");
        }
    }
    let none = Rect::from_min_size(Pos2::ZERO, Vec2::splat(4.0));
    let stroke = BuiltinBrushProfile::Plain.preset().stroke(8.0);
    assert!(
        swatch_mesh(&stroke, none, Color32::WHITE, 1.0)
            .vertices
            .is_empty()
    );
}

#[test]
fn the_gallery_shows_every_built_in_and_custom_brush_and_the_picker_the_one_in_hand() {
    let mut ws = Workspace::untitled().expect("workspace");
    ws.brush_shelf.keep_nib().expect("room");
    ws.brush_shelf.keep_nib().expect("room");
    let ctx = egui::Context::default();
    let mut output = ctx.run_ui(RawInput::default(), |ui| {
        assert_eq!(ws.brush_gallery(ui), None, "nothing clicked");
    });
    let swatches = meshes(&output).len();
    output.textures_delta.clear();
    assert_eq!(
        swatches,
        BuiltinBrushProfile::ALL.len() + 2,
        "one swatch each"
    );
    let mut output = ctx.run_ui(RawInput::default(), |ui| ws.brush_picker(ui));
    let swatches = meshes(&output).len();
    output.textures_delta.clear();
    assert_eq!(swatches, 1, "the brush in hand");
}

#[test]
fn picking_a_brush_takes_its_pressure_and_texture_in_hand() {
    let mut ws = Workspace::untitled().expect("workspace");
    ws.pick_brush(BrushPick::Builtin(BuiltinBrushProfile::Chalk));
    let chalk = BuiltinBrushProfile::Chalk.preset();
    assert_eq!(ws.brush_shelf.builtin(), Some(BuiltinBrushProfile::Chalk));
    assert_eq!(ws.brush_shelf.texture(), chalk.texture());
    assert_eq!(ws.brush_light, chalk.pressure_min());
    assert_eq!(ws.brush_in_hand_name(), ws.tr("opt-profile-chalk"));
    ws.brush_shelf.keep_nib().expect("room");
    ws.pick_brush(BrushPick::Builtin(BuiltinBrushProfile::Plain));
    ws.pick_brush(BrushPick::Custom(0));
    assert_eq!(ws.brush_shelf.selected(), Some(0));
    assert_eq!(
        ws.brush_shelf.texture(),
        chalk.texture(),
        "kept from the chalk"
    );
}
