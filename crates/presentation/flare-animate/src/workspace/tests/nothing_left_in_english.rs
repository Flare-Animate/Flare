//! No word on screen is typed into the program: draw the interface in many states with the
//! pseudo language, which puts every message between ⟦ and ⟧, and every word seen outside the
//! marks is English that was never moved to a message file.

use super::*;
use flare_i18n::Translator;
use std::collections::BTreeSet;

/// Words and symbols that are not translated: the program's name, units, and what the person
/// typed (names of things in the test document are made by the program in the pseudo language).
const KEPT: &[&str] = &[
    "Flare",
    "px",
    "fps",
    "KB",
    "MB",
    "GB",
    "XFL",
    "FLA",
    "SWF",
    "GIF",
    "MP4",
    // Product and service names, and a font family name that is a CSS keyword.
    "YouTube",
    "Twitter",
    "Newgrounds",
    "Facebook",
    "GitHub",
    "Discord",
    "sans",
    "serif",
    // Keyboard modifiers and shortcut glyphs are not translatable.
    "Ctrl",
    "Shift",
    "Alt",
];

fn is_english(shown: &str) -> bool {
    if shown.contains('⟦') {
        return false;
    }
    // A run of two letters or more that is not a kept word.
    let mut word = String::new();
    let mut words = Vec::new();
    for c in shown.chars().chain(std::iter::once(' ')) {
        if c.is_ascii_alphabetic() {
            word.push(c);
        } else {
            if word.len() >= 2 {
                words.push(std::mem::take(&mut word));
            } else {
                word.clear();
            }
        }
    }
    words.iter().any(|w| !KEPT.contains(&w.as_str()))
}

fn pseudo() -> Workspace {
    let mut ws = workspace();
    ws.translator = Translator::pseudo();
    // The names a new document gets come from messages too. Installing it makes a new workspace.
    let document = ws.new_blank_document().expect("document");
    ws.install_document(document, None);
    ws.translator = Translator::pseudo();
    ws
}

/// Everything drawn in a few passes.
fn drawn(ws: &mut Workspace) -> Vec<String> {
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

fn leftovers(state: &str, ws: &mut Workspace, found: &mut BTreeSet<String>) {
    for shown in drawn(ws) {
        if is_english(&shown) {
            found.insert(format!("{state}: {shown:?}"));
        }
    }
}

#[test]
fn every_tool_and_every_window_draws_only_messages() {
    let mut found = BTreeSet::new();
    for tool in [
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
        Tool::Subselection,
        Tool::AddAnchor,
        Tool::DeleteAnchor,
        Tool::ConvertAnchor,
        Tool::FreeTransform,
        Tool::Pencil,
        Tool::Brush,
        Tool::Ruler,
        Tool::Mirror,
    ] {
        let mut ws = pseudo();
        ws.set_tool(tool);
        leftovers(&format!("{tool:?}"), &mut ws, &mut found);
    }
    for pane in flare_dock::Pane::ALL {
        let mut ws = pseudo();
        ws.set_tool(Tool::Select);
        ws.show_pane(pane);
        leftovers(&format!("{pane:?} panel"), &mut ws, &mut found);
    }
    type Open = fn(&mut Workspace);
    let windows: [(&str, Open); 6] = [
        ("Home", |ws| ws.set_home(true)),
        ("Preferences", |ws| ws.windows.preferences = true),
        ("Publish Settings", |ws| ws.windows.publish_settings = true),
        ("Publish", |ws| ws.windows.publish = true),
        ("About", |ws| ws.windows.about = true),
        ("Shortcuts", |ws| ws.windows.shortcuts = true),
    ];
    for (name, open) in windows {
        let mut ws = pseudo();
        open(&mut ws);
        leftovers(name, &mut ws, &mut found);
    }
    assert!(
        found.is_empty(),
        "{} drawn words are still English:\n{}",
        found.len(),
        found.into_iter().collect::<Vec<_>>().join("\n")
    );
}
