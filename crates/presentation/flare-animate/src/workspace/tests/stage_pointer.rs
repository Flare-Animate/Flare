//! Over the stage the pointer is the tool in hand: its icon, or for a tool that lays down a mark,
//! only the tip it marks with.

use super::*;
use crate::workspace::tools::stage_pointer::{StagePointer, stage_pointer};

const EVERY_TOOL: [Tool; 27] = [
    Tool::Select,
    Tool::Rectangle,
    Tool::Ellipse,
    Tool::Line,
    Tool::Pen,
    Tool::Text,
    Tool::Hand,
    Tool::Zoom,
    Tool::Eyedropper,
    Tool::Bucket,
    Tool::InkBottle,
    Tool::Eraser,
    Tool::PolyStar,
    Tool::Lasso,
    Tool::PolygonLasso,
    Tool::MagicWand,
    Tool::Camera,
    Tool::Bone,
    Tool::Subselection,
    Tool::AddAnchor,
    Tool::DeleteAnchor,
    Tool::ConvertAnchor,
    Tool::FreeTransform,
    Tool::Pencil,
    Tool::Brush,
    Tool::Ruler,
    Tool::Mirror,
];

/// Moves the pointer over the middle of the stage and returns the system pointer the pass asked
/// for.
fn pointer_over_stage(ws: &mut Workspace, ctx: &egui::Context, time: f64) -> egui::CursorIcon {
    let over_stage = Pos2::new(600.0, 360.0);
    let mut output = ctx.run_ui(input(time, vec![Event::PointerMoved(over_stage)]), |ui| {
        ws.show(ui)
    });
    output.textures_delta.clear();
    output.platform_output.cursor_icon
}

#[test]
fn every_tool_but_the_selection_tool_hides_the_system_arrow_over_the_stage() {
    let mut ws = workspace();
    let ctx = egui::Context::default();
    run(&mut ws, &ctx, 0.0);
    run(&mut ws, &ctx, 0.1);
    for (n, tool) in EVERY_TOOL.into_iter().enumerate() {
        ws.set_tool(tool);
        let shown = pointer_over_stage(&mut ws, &ctx, 0.2 + n as f64 * 0.1);
        if tool == Tool::Select {
            assert_eq!(shown, egui::CursorIcon::Default, "the arrow is its icon");
        } else {
            assert_eq!(
                shown,
                egui::CursorIcon::None,
                "{tool:?} draws its own pointer"
            );
        }
    }
}

#[test]
fn the_marking_tools_show_only_their_tip_and_the_others_their_icon() {
    for tool in [Tool::Brush, Tool::Pencil] {
        assert_eq!(stage_pointer(tool, false), StagePointer::Tip, "{tool:?}");
    }
    assert_eq!(stage_pointer(Tool::Eraser, true), StagePointer::Tip);
    assert_eq!(
        stage_pointer(Tool::Eraser, false),
        StagePointer::Marked {
            icon: "tool-eraser"
        },
        "Faucet deletes what is clicked, so it has no tip to show"
    );
    assert_eq!(stage_pointer(Tool::Select, false), StagePointer::Arrow);
    for tool in EVERY_TOOL {
        match stage_pointer(tool, false) {
            StagePointer::Picture { icon, tip } => {
                assert!(flare_ui::icons::exists(icon), "{icon}");
                assert!(
                    (0.0..=24.0).contains(&tip.x) && (0.0..=24.0).contains(&tip.y),
                    "{tool:?} touches inside its icon"
                );
            }
            StagePointer::Marked { icon } => assert!(flare_ui::icons::exists(icon), "{icon}"),
            StagePointer::Arrow | StagePointer::Tip => {}
        }
    }
}

#[test]
fn the_pencil_tip_is_as_wide_as_the_line_it_draws() {
    let mut ws = workspace();
    ws.set_tool(Tool::Pencil);
    ws.paint.stroke_width = 6.0;
    ws.pointer_stage = Some(Pos2::new(40.0, 30.0));
    let tip = ws.brush_cursor().expect("the tip");
    assert!(
        tip.iter()
            .all(|p| ((*p - Pos2::new(40.0, 30.0)).length() - 3.0).abs() < 1e-3)
    );
    ws.pointer_stage = None;
    assert!(ws.brush_cursor().is_none(), "no pointer over the stage");
}
