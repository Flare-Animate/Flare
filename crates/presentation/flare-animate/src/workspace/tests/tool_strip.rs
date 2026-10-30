use super::*;
use crate::workspace::panels::color_wells::well_rects;
use crate::workspace::panels::tools::TOOL_GROUPS;

#[test]
fn every_tool_is_in_exactly_one_category() {
    let all = [
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
        Tool::Subselection,
        Tool::FreeTransform,
        Tool::Pencil,
        Tool::Brush,
        Tool::Ruler,
        Tool::Mirror,
    ];
    let listed: Vec<Tool> = TOOL_GROUPS
        .iter()
        .flat_map(|group| group.iter().map(|cell| cell.3))
        .collect();
    for tool in all {
        assert_eq!(listed.iter().filter(|t| **t == tool).count(), 1, "{tool:?}");
    }
    assert_eq!(listed.len(), all.len());
    assert!(
        TOOL_GROUPS.len() >= 4,
        "the strip is divided into categories"
    );
    for group in TOOL_GROUPS {
        assert_eq!(group.len() % 2, 0, "a group fills whole rows of two");
    }
}

#[test]
fn the_color_box_overlaps_the_squares_but_not_the_buttons() {
    let r = well_rects(Pos2::new(10.0, 20.0));
    assert!(r.fill.intersects(r.stroke), "the stroke sits over the fill");
    assert!(r.stroke.min.x > r.fill.min.x && r.stroke.min.y > r.fill.min.y);
    for button in [r.swap, r.defaults] {
        assert!(!button.intersects(r.fill) && !button.intersects(r.stroke));
    }
    assert!(!r.swap.intersects(r.defaults));
}

#[test]
fn the_tool_strip_draws_at_narrow_and_wide_widths() {
    for width in [60.0, 120.0, 600.0] {
        let mut ws = workspace();
        ws.paint.stroke_color = Color32::from_rgb(1, 2, 3);
        let ctx = egui::Context::default();
        let raw = RawInput {
            time: Some(0.0),
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(width, 700.0))),
            ..RawInput::default()
        };
        let mut output = ctx.run_ui(raw, |ui| ws.tools_panel(ui));
        output.textures_delta.clear();
        assert_eq!(
            ws.tool(),
            Tool::Rectangle,
            "drawing the strip changes nothing"
        );
    }
}

#[test]
fn the_color_box_fits_inside_the_narrowest_tool_strip_and_its_own_height() {
    use crate::workspace::panels::TOOL_ICON;
    use crate::workspace::panels::color_wells::{BOX, OFFSET, SQUARE};
    let narrowest = (TOOL_ICON + 6.0) * 2.0 + 3.0;
    assert!(
        OFFSET + SQUARE <= narrowest,
        "{} against {narrowest}",
        OFFSET + SQUARE
    );
    let r = well_rects(Pos2::ZERO);
    for rect in [r.fill, r.stroke, r.swap, r.defaults] {
        assert!(
            rect.max.y <= BOX && rect.max.x <= OFFSET + SQUARE,
            "{rect:?}"
        );
    }
}
