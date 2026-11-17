//! A tool's options fit the panel they are drawn in. A row wider than the panel pushes what
//! comes after it out of sight too: each slider puts its number at the panel's right edge, so one
//! wide row left every number below it drawn past the edge, where nobody could read it.

use super::*;
use crate::workspace::panels::tool_slots::SLOT_GROUPS;

/// A narrow Properties panel, in points.
const PANEL: f32 = 240.0;

/// How wide the options of the tool in hand are when drawn in a panel `PANEL` wide.
fn options_width(ws: &mut Workspace) -> f32 {
    let ctx = egui::Context::default();
    flare_ui::icons::install(&ctx);
    let mut width = 0.0;
    for step in 0..3 {
        let mut out = ctx.run_ui(input(f64::from(step) * 0.1, vec![]), |ui| {
            let rect = egui::Rect::from_min_size(ui.min_rect().min, egui::vec2(PANEL, 4000.0));
            width = ui
                .scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
                    ws.tool_options(ui);
                    ui.min_rect().width()
                })
                .inner;
        });
        out.textures_delta.clear();
    }
    width
}

#[test]
fn every_tools_options_fit_a_narrow_panel_in_any_language() {
    let tools: Vec<Tool> = SLOT_GROUPS
        .iter()
        .flat_map(|group| group.iter())
        .flat_map(|slot| slot.iter().filter_map(|v| v.tool))
        .collect();
    assert!(tools.contains(&Tool::Brush));
    let mut too_wide = Vec::new();
    for tag in ["en-US", "de-DE", "fr-FR"] {
        for tool in &tools {
            let mut ws = blank_workspace();
            ws.set_system_locale(tag);
            ws.set_tool(*tool);
            let width = options_width(&mut ws);
            if width > PANEL + 0.5 {
                too_wide.push(format!("{tag} {tool:?}: {width}"));
            }
        }
    }
    assert!(too_wide.is_empty(), "wider than {PANEL}: {too_wide:#?}");
}
