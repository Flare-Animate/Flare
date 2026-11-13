use super::*;
use crate::workspace::panels::tool_slots::{SLOT_GROUPS, Slot, front};

#[test]
fn every_tool_is_in_exactly_one_slot_and_the_strip_has_categories() {
    let all = [
        Tool::Select,
        Tool::Rectangle,
        Tool::Ellipse,
        Tool::Line,
        Tool::Pen,
        Tool::Text,
        Tool::Hand,
        Tool::RotateView,
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
        Tool::Blur,
        Tool::Smudge,
    ];
    let listed: Vec<Tool> = SLOT_GROUPS
        .iter()
        .flat_map(|group| group.iter())
        .flat_map(|slot| slot.iter().filter_map(|v| v.tool))
        .collect();
    for tool in all {
        assert_eq!(listed.iter().filter(|t| **t == tool).count(), 1, "{tool:?}");
    }
    assert_eq!(listed.len(), all.len());
    assert!(
        SLOT_GROUPS.len() >= 4,
        "the strip is divided into categories"
    );
    let slots: Vec<Slot> = SLOT_GROUPS.iter().flat_map(|g| g.iter().copied()).collect();
    assert!(slots.iter().all(|slot| !slot.is_empty()));
    let names: Vec<&str> = slots
        .iter()
        .flat_map(|s| s.iter().map(|v| v.name))
        .collect();
    for (i, name) in names.iter().enumerate() {
        assert!(!names[..i].contains(name), "{name} is listed twice");
    }
    for slot in &slots {
        for v in slot.iter() {
            // Some built tools have no key, as in Animate (the polygon lasso).
            assert!(
                v.key.is_empty() || v.tool.is_some(),
                "{}: a key only for a built tool",
                v.name
            );
        }
    }
}

#[test]
fn a_slot_shows_the_tool_in_hand_then_the_one_picked_then_the_first_built() {
    let slot = SLOT_GROUPS[1][3];
    assert_eq!(slot[0].name, "Rectangle");
    assert_eq!(front(slot, Tool::Pen, None).name, "Rectangle");
    assert_eq!(front(slot, Tool::Pen, Some(1)).name, "Oval");
    assert_eq!(front(slot, Tool::Ellipse, None).name, "Oval");
    assert_eq!(front(slot, Tool::PolyStar, Some(1)).name, "PolyStar");
    // A choice of a tool that is not built is ignored.
    assert_eq!(front(slot, Tool::Pen, Some(2)).name, "Rectangle");
    // A slot with nothing built shows its first and cannot be picked.
    let warp = SLOT_GROUPS[2][4];
    assert_eq!(warp[0].name, "Asset Warp");
    assert!(front(warp, Tool::Pen, None).tool.is_none());
}

#[test]
fn the_tool_strip_draws_at_narrow_and_wide_widths() {
    for width in [30.0, 36.0, 60.0, 120.0, 600.0] {
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

/// The lowest edge of anything the strip drew, and what was drawn there.
fn lowest_edge(ws: &mut Workspace, ctx: &egui::Context, raw: RawInput) -> f32 {
    let mut output = ctx.run_ui(raw, |ui| ws.tools_panel(ui));
    output.textures_delta.clear();
    output
        .shapes
        .iter()
        .map(|s| s.shape.visual_bounding_rect().max.y)
        .filter(|y| y.is_finite())
        .fold(0.0, f32::max)
}

#[test]
fn a_tool_strip_taller_than_its_panel_scrolls_so_the_color_box_can_be_reached() {
    // One narrow column and a short window: the tools and the color box do not fit.
    let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(60.0, 300.0));
    let raw = |time, events| RawInput {
        time: Some(time),
        screen_rect: Some(screen),
        events,
        ..RawInput::default()
    };
    let mut ws = workspace();
    let ctx = egui::Context::default();
    let inside = Pos2::new(30.0, 150.0);
    // Two passes to settle the layout, with the pointer over the strip.
    lowest_edge(&mut ws, &ctx, raw(0.0, vec![Event::PointerMoved(inside)]));
    let before = lowest_edge(&mut ws, &ctx, raw(0.1, vec![]));
    assert!(
        before > screen.height(),
        "the strip is taller than its panel: {before}"
    );
    let wheel = Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: Vec2::new(0.0, -2000.0),
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Wheel,
        modifiers: Modifiers::NONE,
    };
    lowest_edge(&mut ws, &ctx, raw(0.2, vec![wheel]));
    // The scroll eases in over a few passes.
    let mut after = 0.0;
    for pass in 1..=8 {
        after = lowest_edge(&mut ws, &ctx, raw(0.2 + f64::from(pass) * 0.1, vec![]));
    }
    assert!(
        after <= screen.height() + 8.0,
        "scrolled to the end, the color box is on screen: {after} against {}",
        screen.height()
    );
}

#[test]
fn subselection_is_in_the_selection_slot_and_the_strip_has_animates_other_tools() {
    let slots: Vec<Slot> = SLOT_GROUPS.iter().flat_map(|g| g.iter().copied()).collect();
    let selection = slots
        .iter()
        .find(|s| s[0].name == "Selection")
        .expect("slot");
    assert_eq!(selection[1].name, "Subselection");
    assert_eq!(selection[1].tool, Some(Tool::Subselection));
    // With Subselection in hand, its own slot shows it, and picking it elsewhere does not move it.
    assert_eq!(
        front(selection, Tool::Subselection, None).name,
        "Subselection"
    );
    assert_eq!(front(selection, Tool::Select, None).name, "Selection");
    let names: Vec<&str> = slots
        .iter()
        .flat_map(|s| s.iter().map(|v| v.name))
        .collect();
    for name in [
        "Asset Warp",
        "Rotation",
        "Time scrub",
        "Paint Brush",
        "Width",
        "Gradient Transform",
        "Ink Bottle",
        "Oval",
    ] {
        assert!(names.contains(&name), "{name} is in the strip's lists");
    }
    // The Rotation tool sits under the Hand, as in Animate, and is built.
    let hand = slots
        .iter()
        .find(|slot| slot[0].tool == Some(Tool::Hand))
        .expect("the Hand's slot");
    assert_eq!(hand[1].tool, Some(Tool::RotateView));
    // Those not built are marked so, never selectable.
    for slot in &slots {
        for v in slot.iter() {
            if ["Asset Warp", "Time scrub", "Paint Brush", "Width"].contains(&v.name) {
                assert!(v.tool.is_none(), "{} is not built", v.name);
            }
        }
    }
}

#[test]
fn the_strip_is_in_animates_order() {
    let order: Vec<Vec<&str>> = SLOT_GROUPS
        .iter()
        .map(|group| group.iter().map(|slot| slot[0].name).collect())
        .collect();
    assert_eq!(
        order,
        [
            vec!["Selection", "Free Transform", "Lasso"],
            vec!["Pencil", "Brush", "Eraser", "Rectangle", "Line"],
            vec!["Pen", "Text", "Paint Bucket", "Eyedropper", "Asset Warp"],
            vec!["Hand", "Zoom", "Mirror"],
        ],
        "the last cell is the dots; its list starts with Mirror"
    );
}

#[test]
fn the_strip_draws_one_picture_for_each_slot_and_nothing_else_but_the_color_section() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    flare_ui::icons::install(&ctx);
    let slots: usize = SLOT_GROUPS.iter().map(|group| group.len()).sum();
    let mut pictures = 0;
    for pass in 0..3 {
        let raw = RawInput {
            time: Some(f64::from(pass) * 0.1),
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(120.0, 900.0))),
            ..RawInput::default()
        };
        let mut out = ctx.run_ui(raw, |ui| ws.tools_panel(ui));
        out.textures_delta.clear();
        pictures = out
            .shapes
            .iter()
            .filter(|s| {
                matches!(&s.shape, egui::epaint::Shape::Rect(r)
                    if r.fill_texture_id() != egui::TextureId::default())
            })
            .count();
    }
    // The color section has two small buttons. A
    // second copy of any cell, the dots included, would make one more.
    assert_eq!(pictures, slots + 2, "{slots} slots, two small buttons");
}

/// The pictures the strip drew about as large as a tool, as rectangles on screen.
fn tool_pictures(ws: &mut Workspace, ctx: &egui::Context, width: f32) -> Vec<Rect> {
    let mut last = Vec::new();
    for pass in 0..3 {
        let raw = RawInput {
            time: Some(f64::from(pass) * 0.1),
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(width, 900.0))),
            ..RawInput::default()
        };
        let mut out = ctx.run_ui(raw, |ui| ws.tools_panel(ui));
        out.textures_delta.clear();
        last = out
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::epaint::Shape::Rect(r)
                    if r.fill_texture_id() != egui::TextureId::default()
                        && (r.rect.width() - crate::workspace::panels::TOOL_ICON).abs() < 0.5 =>
                {
                    Some(r.rect)
                }
                _ => None,
            })
            .collect();
    }
    last
}

#[test]
fn the_default_strip_is_one_tool_wide_and_a_wide_one_has_two_columns() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    flare_ui::icons::install(&ctx);
    let columns = |pictures: &[Rect]| {
        let mut xs: Vec<i32> = pictures
            .iter()
            .map(|r| r.center().x.round() as i32)
            .collect();
        xs.sort_unstable();
        xs.dedup();
        xs.len()
    };
    // The default share of a 1280 or 1920 wide window, and a strip dragged wider.
    for width in [38.0, 58.0] {
        let pictures = tool_pictures(&mut ws, &ctx, width);
        assert!(
            pictures.len() >= 16,
            "every tool is drawn: {}",
            pictures.len()
        );
        assert_eq!(columns(&pictures), 1, "one column at {width}");
    }
    let wide = tool_pictures(&mut ws, &ctx, 120.0);
    // Two columns, and the odd last cell (the dots) centered under them: three places across.
    assert!(columns(&wide) > 1, "more than one column at 120");
}

#[test]
fn the_strip_is_a_bar_of_its_own_with_a_thin_dark_line_down_its_right_edge() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    flare_ui::icons::install(&ctx);
    let width = 48.0;
    let mut shapes = Vec::new();
    for pass in 0..2 {
        let raw = RawInput {
            time: Some(f64::from(pass) * 0.1),
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(width, 300.0))),
            ..RawInput::default()
        };
        let mut out = ctx.run_ui(raw, |ui| ws.tools_panel(ui));
        out.textures_delta.clear();
        shapes = out.shapes.into_iter().map(|s| s.shape).collect();
    }
    let fill = shapes.iter().find_map(|s| match s {
        egui::epaint::Shape::Rect(r)
            if r.rect.width() >= width - 1.0 && r.rect.height() >= 299.0 =>
        {
            Some(r.fill)
        }
        _ => None,
    });
    assert_eq!(
        fill,
        Some(flare_ui::theme::panel_raised()),
        "a lighter bar behind the tools"
    );
    let line = shapes.iter().find_map(|s| match s {
        egui::epaint::Shape::LineSegment { points, stroke }
            if (points[0].x - points[1].x).abs() < 0.01 && points[0].x > width - 2.0 =>
        {
            Some((points[0].x, *stroke))
        }
        _ => None,
    });
    let (x, stroke) = line.expect("a vertical line at the right edge");
    assert!((x - (width - 0.5)).abs() < 0.01, "on the edge: {x}");
    assert!((stroke.width - 1.0).abs() < 0.01, "thin: {}", stroke.width);
    assert_eq!(stroke.color, flare_ui::theme::stage_surround());
    let luma = |c: egui::Color32| u32::from(c.r()) + u32::from(c.g()) + u32::from(c.b());
    assert!(
        luma(stroke.color) < luma(flare_ui::theme::panel_raised()),
        "the line is darker than the bar"
    );
}
