//! What the window says in each language: panel names, the Document panel, the tab and window
//! title, and the names a new document and a new layer are given.

use super::*;
use flare_i18n::Language;

/// Every piece of text drawn in a few passes, so panels have settled.
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

fn has(texts: &[String], wanted: &str) -> bool {
    texts.iter().any(|t| t == wanted)
}

fn speaking(tag: &str) -> Workspace {
    let mut ws = workspace();
    ws.set_system_locale(tag);
    ws
}

#[test]
fn the_panels_the_document_and_the_timeline_are_named_in_the_language() {
    let mut ws = speaking("es-ES");
    // With the Selection tool and nothing selected, Properties shows the document.
    ws.set_tool(Tool::Select);
    let mut text = drawn(&mut ws);
    // Panels kept in the strip are named when they are open beside it.
    for pane in [Pane::Transform, Pane::Layers, Pane::Align] {
        ws.show_pane(pane);
        text.extend(drawn(&mut ws));
    }
    for wanted in [
        "Propiedades",
        "Biblioteca",
        "Capas",
        "Alinear",
        "Transformar",
        "Línea de tiempo",
        "Salida",
        "Documento",
        "Configuración de publicación",
        "Velocidad de fotogramas",
        "Escenas",
        "Sonidos",
    ] {
        assert!(has(&text, wanted), "{wanted}: {text:?}");
    }
    for english in [
        "Properties",
        "Library",
        "Layers",
        "Align",
        "Transform",
        "Timeline",
        "Output",
        "Document",
        "Publish Settings",
        "Frame rate",
        "Scenes",
        "Sounds",
        "Untitled",
    ] {
        assert!(!has(&text, english), "{english} is still English: {text:?}");
    }
}

#[test]
fn an_unsaved_document_is_untitled_in_the_language_in_the_tab_and_the_window_title() {
    let mut ws = speaking("es-ES");
    assert!(ws.title().starts_with("Sin título"), "{}", ws.title());
    assert_eq!(ws.tabs()[0].title, "Sin título");
    assert!(drawn(&mut ws).iter().any(|t| t == "Sin título"));
    ws.set_system_locale("ja-JP");
    assert_eq!(ws.tabs()[0].title, "名称未設定");
    ws.set_system_locale("en-US");
    assert_eq!(ws.tabs()[0].title, "Untitled");
}

#[test]
fn a_new_document_names_its_scene_and_layer_in_the_language() {
    for (tag, scene, layer) in [
        ("es-ES", "Escena 1", "Capa 1"),
        ("de-DE", "Szene 1", "Ebene 1"),
        ("fr-FR", "Séquence 1", "Calque 1"),
        ("pt-BR", "Cena 1", "Camada 1"),
        ("ja-JP", "シーン 1", "レイヤー 1"),
        ("ko-KR", "장면 1", "레이어 1"),
        ("zh-CN", "场景 1", "图层 1"),
        ("en-US", "Scene 1", "Layer 1"),
    ] {
        let mut ws = speaking(tag);
        ws.new_tab();
        let doc = ws.document();
        assert_eq!(doc.scenes()[0].name(), scene, "{tag}");
        assert_eq!(
            doc.scenes()[0].timeline().layers()[0].name(),
            layer,
            "{tag}"
        );
    }
}

#[test]
fn the_document_made_at_start_up_is_renamed_once_the_language_is_known_but_work_is_left_alone() {
    let mut ws = Workspace::untitled().expect("workspace");
    assert_eq!(ws.document().scenes()[0].name(), "Scene 1");
    ws.set_system_locale("es-ES");
    ws.localize_new_document();
    assert_eq!(ws.document().scenes()[0].name(), "Escena 1");
    assert_eq!(
        ws.document().scenes()[0].timeline().layers()[0].name(),
        "Capa 1"
    );

    let mut worked = Workspace::untitled().expect("workspace");
    worked.add_layer();
    worked.set_system_locale("es-ES");
    worked.localize_new_document();
    assert_eq!(
        worked.document().scenes()[0].name(),
        "Scene 1",
        "a document that was edited keeps its names"
    );
}

#[test]
fn a_layer_added_later_is_numbered_in_the_language() {
    let mut ws = speaking("es-ES");
    ws.add_layer();
    let layers = ws.document().scenes()[0].timeline().layers().to_vec();
    assert!(
        layers.iter().any(|l| l.name() == "Capa 2"),
        "{:?}",
        layers.iter().map(|l| l.name()).collect::<Vec<_>>()
    );
}

#[test]
fn every_language_names_every_panel_of_the_dock() {
    for language in Language::ALL {
        let ws = {
            let mut ws = workspace();
            ws.set_system_locale(language.code());
            ws
        };
        for pane in flare_dock::Pane::ALL {
            let said = ws.tr(pane.key());
            assert_ne!(
                said,
                pane.key(),
                "{} has no name for {pane:?}",
                language.code()
            );
            if language != Language::English {
                // Names that really are the same word in a language (Color, Info) are allowed.
                let same = matches!(pane.title(), "Color" | "Info");
                assert!(
                    same || said != pane.title(),
                    "{} left {pane:?} in English",
                    language.code()
                );
            }
        }
    }
}
