//! The tools and what they set are spoken in the interface's language: their names, their
//! tooltips and help, the Properties of a tool or of an object, and the words of the stage bar.

use super::*;
use crate::workspace::panels::tool_slots::{SLOT_GROUPS, name_key};
use flare_i18n::Language;

const TAGS: [&str; 8] = [
    "en-US", "de-DE", "es-ES", "fr-FR", "pt-BR", "ja-JP", "ko-KR", "zh-CN",
];

/// Every piece of text drawn in a few passes, so panels have settled.
fn drawn_text(ws: &mut Workspace) -> Vec<String> {
    let ctx = egui::Context::default();
    flare_ui::icons::install(&ctx);
    let mut all = Vec::new();
    for step in 0..5 {
        let mut out = ctx.run_ui(input(f64::from(step) * 0.1, vec![]), |ui| ws.show(ui));
        out.textures_delta.clear();
        all = out
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::epaint::Shape::Text(t) => Some(t.galley.text().to_owned()),
                _ => None,
            })
            .collect();
    }
    all
}

/// The text drawn before and after the wheel scrolls Properties down from over `over`: a tool
/// with many amounts runs past the panel's bottom, as Animate's does, and is scrolled to.
fn drawn_text_scrolled(ws: &mut Workspace, over: &str) -> Vec<String> {
    let ctx = egui::Context::default();
    flare_ui::icons::install(&ctx);
    let mut all = Vec::new();
    let mut at = None;
    for step in 0..12 {
        let events = match (step, at) {
            (5, Some(p)) => vec![Event::PointerMoved(p)],
            (6..=9, Some(_)) => vec![Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: Vec2::new(0.0, -300.0),
                phase: egui::TouchPhase::Move,
                source: egui::MouseWheelSource::Wheel,
                modifiers: Modifiers::NONE,
            }],
            _ => vec![],
        };
        let mut out = ctx.run_ui(input(f64::from(step) * 0.1, events), |ui| ws.show(ui));
        out.textures_delta.clear();
        for s in &out.shapes {
            if let egui::epaint::Shape::Text(t) = &s.shape {
                let text = t.galley.text().to_owned();
                if text == over {
                    at = Some(t.visual_bounding_rect().center() + Vec2::new(0.0, 40.0));
                }
                all.push(text);
            }
        }
    }
    all
}

fn has(texts: &[String], wanted: &str) -> bool {
    texts.iter().any(|t| t == wanted)
}

fn speaking(tag: &str) -> Workspace {
    let mut ws = blank_workspace();
    ws.set_system_locale(tag);
    ws
}

const ALL_TOOLS: [Tool; 26] = [
    Tool::Select,
    Tool::Subselection,
    Tool::AddAnchor,
    Tool::DeleteAnchor,
    Tool::ConvertAnchor,
    Tool::FreeTransform,
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
    Tool::Pencil,
    Tool::Brush,
    Tool::Ruler,
    Tool::Mirror,
];

#[test]
fn the_rectangle_tool_and_its_options_are_in_the_language() {
    let mut ws = speaking("es-ES");
    ws.set_tool(Tool::Rectangle);
    let text = drawn_text(&mut ws);
    for wanted in [
        "Herramienta Rectángulo",
        "Esquinas",
        "Arrastra en el escenario.",
        "Rectángulo: arrastra en el escenario",
        "Fotograma 1 / 1",
        "Relleno y trazo",
        "Anchura",
        "Extremo",
        "Unión",
    ] {
        assert!(has(&text, wanted), "{wanted}: {text:?}");
    }
    for english in [
        "Fill and stroke",
        "Width",
        "Cap",
        "Join",
        "Round",
        "Outline",
        "Rectangle tool",
        "Corners",
        "Drag on the stage.",
        "Rectangle: drag on the stage",
        "Frame 1 / 1",
    ] {
        assert!(!has(&text, english), "{english} is still English: {text:?}");
    }
}

#[test]
fn every_tool_has_a_status_hint_in_every_language_that_is_not_the_english_one() {
    let english: Vec<String> = ALL_TOOLS
        .iter()
        .map(|tool| {
            let mut ws = speaking("en-US");
            ws.set_tool(*tool);
            ws.tool_hint()
        })
        .collect();
    for tag in &TAGS[1..] {
        for (tool, english) in ALL_TOOLS.iter().zip(&english) {
            let mut ws = speaking(tag);
            ws.set_tool(*tool);
            let hint = ws.tool_hint();
            assert!(!hint.starts_with("tools-"), "{tag} {tool:?} has no hint");
            assert_ne!(&hint, english, "{tag} {tool:?} is still English");
        }
    }
}

#[test]
fn every_tool_in_the_strip_is_named_in_every_language() {
    let english = speaking("en-US");
    let mut total = 0;
    for tag in &TAGS[1..] {
        let ws = speaking(tag);
        let mut same = 0;
        for group in SLOT_GROUPS {
            for slot in *group {
                for variant in *slot {
                    let key = name_key(variant.icon);
                    let name = ws.tr(&key);
                    assert!(name != key && !name.is_empty(), "{tag} lacks {key}");
                    // A few names are spelled the same in a language ("Zoom", "Text").
                    same += usize::from(name == english.tr(&key));
                    total += 1;
                }
            }
        }
        assert!(same < 12, "{tag} leaves {same} tool names in English");
    }
    assert!(total > 100, "the whole strip was walked: {total}");
    assert_eq!(speaking("ja-JP").tr(&name_key("tool-selection")), "選択");
    assert_eq!(
        speaking("de-DE").tr(&name_key("tool-eraser")),
        "Radiergummi"
    );
    assert_eq!(english.tr(&name_key("tool-eraser")), "Eraser");
}

#[test]
fn the_properties_of_a_selected_shape_are_in_the_language() {
    let mut ws = speaking("ja-JP");
    draw_and_select(&mut ws);
    let text = drawn_text(&mut ws);
    for wanted in [
        "位置とサイズ",
        "塗りと線",
        "線端",
        "結合",
        "アウトライン",
        "幅",
    ] {
        assert!(has(&text, wanted), "{wanted}: {text:?}");
    }
    for english in [
        "Position and size",
        "Fill and stroke",
        "Cap",
        "Join",
        "Outline",
    ] {
        assert!(!has(&text, english), "{english} is still English: {text:?}");
    }
}

#[test]
fn the_brush_options_and_the_symmetry_choices_are_in_the_language() {
    let mut ws = speaking("fr-FR");
    ws.set_tool(Tool::Brush);
    let text = drawn_text_scrolled(&mut ws, "Outil Pinceau");
    for wanted in [
        "Outil Pinceau",
        "Lissage",
        "Toucher léger",
        "Pointe",
        "Symétrie",
    ] {
        assert!(has(&text, wanted), "{wanted}: {text:?}");
    }
    for english in ["Brush tool", "Smoothing", "Light touch", "Nib", "Symmetry"] {
        assert!(!has(&text, english), "{english} is still English: {text:?}");
    }
}

#[test]
fn a_drawn_shape_names_its_undo_step_in_the_language_and_leaves_the_layer_alone() {
    for (tag, step) in [
        ("de-DE", "Rechteck zeichnen"),
        ("es-ES", "Dibujar rectángulo"),
        ("zh-CN", "绘制矩形"),
        ("en-US", "Draw rectangle"),
    ] {
        let mut ws = speaking(tag);
        let first_layer = layer_names(&ws);
        rect(&mut ws);
        assert_eq!(
            layer_names(&ws),
            first_layer,
            "{tag}: the shape joins the layer, whose name stays"
        );
        assert_eq!(ws.history_entries().0[1], step, "{tag}");
    }
}

#[test]
fn the_camera_tool_speaks_the_language_in_its_options_its_history_and_its_reports() {
    let mut ws = speaking("fr-FR");
    ws.set_tool(Tool::Camera);
    assert_eq!(ws.history_entries().0[1], "Ajouter une caméra");
    let text = drawn_text(&mut ws);
    for wanted in ["Supprimer la caméra", "Aperçu à travers la caméra"] {
        assert!(has(&text, wanted), "{wanted}: {text:?}");
    }
    assert!(!has(&text, "Remove camera"), "{text:?}");

    let mut pen = speaking("es-ES");
    pen.set_tool(Tool::Pen);
    pen.pen_press(Pos2::new(5.0, 5.0));
    pen.pen_release();
    pen.pen_finish();
    assert_eq!(
        pen.status(),
        Some("Un trazado necesita al menos dos puntos")
    );
}

#[test]
fn the_stage_bar_and_the_status_bar_use_the_language() {
    let mut ws = speaking("ko-KR");
    let text = drawn_text(&mut ws);
    assert!(has(&text, "프레임 1 / 1"), "{text:?}");
    assert!(!has(&text, "Frame 1 / 1"), "{text:?}");
    assert_eq!(ws.language(), Language::Korean);
}
