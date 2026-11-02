//! P1 gaps (Oct 15 2026): Alt-drag copy, registration point on Convert to
//! Symbol, Document Settings, library folders and linkage, text paragraphs
//! and text fields.

use ac_core::commands::execute;
use ac_core::editor::LibItem;
use ac_core::text::{Align, Baseline, TextKind};
use ac_core::{pt, Editor, ElementKind, SymbolKind};
use serde_json::json;

fn rect(ed: &mut Editor, x: f32, y: f32, w: f32, h: f32) {
    execute(ed, "draw.rect", &json!({"x": x, "y": y, "w": w, "h": h})).unwrap();
}
fn els(ed: &Editor) -> &Vec<ac_core::Element> {
    &ed.timeline().layers[0].keyframes[&0].elements
}

#[test]
fn alt_drag_leaves_a_copy_in_one_step() {
    let mut ed = Editor::new(Default::default());
    rect(&mut ed, 10.0, 10.0, 20.0, 20.0);
    execute(&mut ed, "select.all", &json!({})).unwrap();
    let steps = ed.history().0.len();
    assert!(ed.alt_drag_copy());
    assert_eq!(els(&ed).len(), 2);
    assert_eq!(ed.selection[0].element, 1, "the copy is selected");
    ed.modify_selected_continuous(|e| e.matrix.tx += 50.0);
    assert_eq!(ed.history().0.len(), steps + 1);
    assert!((els(&ed)[0].matrix.tx - els(&ed)[1].matrix.tx).abs() > 49.0);
    ed.undo();
    assert_eq!(els(&ed).len(), 1);
}

#[test]
fn registration_point_puts_the_origin_on_the_grid() {
    let mut ed = Editor::new(Default::default());
    rect(&mut ed, 100.0, 50.0, 40.0, 20.0);
    execute(&mut ed, "select.all", &json!({})).unwrap();
    execute(&mut ed, "selection.to_symbol", &json!({"name": "Box", "registration": "top_left"})).unwrap();
    let m = els(&ed)[0].matrix;
    assert_eq!((m.tx, m.ty), (100.0, 50.0));
    let mut ed = Editor::new(Default::default());
    rect(&mut ed, 100.0, 50.0, 40.0, 20.0);
    execute(&mut ed, "select.all", &json!({})).unwrap();
    ed.convert_to_symbol_at("B", SymbolKind::Graphic, [2, 2]).unwrap();
    let m = els(&ed)[0].matrix;
    assert_eq!((m.tx, m.ty), (140.0, 70.0));
}

#[test]
fn document_settings_anchor_scale_content_and_spans() {
    let mut ed = Editor::new(Default::default()); // 550 × 400, 24 fps
    rect(&mut ed, 0.0, 0.0, 10.0, 10.0);
    let bg = ed.doc.background;
    // centre anchor: content moves by half the growth
    ed.document_settings(650.0, 500.0, [1, 1], false, 24.0, false, bg);
    let m = els(&ed)[0].matrix;
    assert_eq!((m.tx, m.ty), (50.0, 50.0));
    // scale content
    ed.document_settings(1300.0, 1000.0, [0, 0], true, 24.0, false, bg);
    let m = els(&ed)[0].matrix;
    assert!((m.a - 2.0).abs() < 1e-4 && (m.tx - 100.0).abs() < 1e-3, "{m:?}");
    // 24 → 12 fps with scale spans: a keyframe at 10 moves to 5, the layer halves
    execute(&mut ed, "frame.goto", &json!({"frame": 11})).ok();
    ed.frame = 10;
    ed.insert_keyframe();
    let end = ed.timeline().layers[0].end;
    ed.document_settings(1300.0, 1000.0, [0, 0], false, 12.0, true, bg);
    let keys: Vec<u32> = ed.timeline().layers[0].keyframes.keys().copied().collect();
    assert_eq!(keys, vec![0, 5]);
    assert_eq!(ed.timeline().layers[0].end, (end as f32 / 2.0).round() as u32);
    assert_eq!(ed.doc.fps, 12.0);
    // Match Contents
    assert!(ed.match_contents());
    assert_eq!((ed.doc.width, ed.doc.height), (20.0, 20.0));
}

#[test]
fn library_folders_move_rename_delete_and_linkage() {
    let mut ed = Editor::new(Default::default());
    rect(&mut ed, 0.0, 0.0, 10.0, 10.0);
    execute(&mut ed, "select.all", &json!({})).unwrap();
    let id = ed.convert_to_symbol("Hero", SymbolKind::MovieClip).unwrap();
    let f = ed.new_folder("");
    assert_eq!(f, "untitled folder 1");
    assert!(ed.rename_folder(&f, "Characters"));
    assert!(ed.move_to_folder(LibItem::Symbol(id), "Characters"));
    assert_eq!(ed.doc.symbol(id).unwrap().name, "Characters/Hero");
    let sub = ed.new_folder("Characters");
    assert!(ed.rename_folder(&sub, "Extras"));
    assert_eq!(ed.library_folders(), vec!["Characters".to_string(), "Characters/Extras".to_string()]);
    assert!(ed.rename_folder("Characters", "Cast"));
    assert_eq!(ed.doc.symbol(id).unwrap().name, "Cast/Hero");
    assert!(ed.library_folders().contains(&"Cast/Extras".to_string()));
    ed.delete_folder("Cast");
    assert_eq!(ed.doc.symbol(id).unwrap().name, "Hero", "items move up");
    assert!(ed.library_folders().contains(&"Extras".to_string()));
    assert!(ed.set_item_props(LibItem::Symbol(id), "Player", Some(SymbolKind::Graphic), Some("PlayerClip".into())));
    let s = ed.doc.symbol(id).unwrap();
    assert_eq!((s.name.as_str(), s.kind), ("Player", SymbolKind::Graphic));
    assert_eq!(ed.doc.linkage.get(&id).map(|s| s.as_str()), Some("PlayerClip"));
    assert!(!ed.set_item_props(LibItem::Symbol(id), "a/b", None, None), "no slashes in names");
}

fn text_el(ed: &Editor) -> ac_core::TextEl {
    match &els(ed)[0].kind {
        ElementKind::Text(t) => t.clone(),
        k => panic!("{k:?}"),
    }
}

#[test]
fn text_wraps_justifies_indents_and_shifts() {
    let mut ed = Editor::new(Default::default());
    let words = "one two three four five six seven eight nine ten";
    execute(&mut ed, "draw.text", &json!({"x": 0, "y": 0, "text": words, "size": 20})).unwrap();
    let t = text_el(&ed);
    let (w1, h1) = t.box_size();
    let narrow = ac_core::TextEl { para: ac_core::text::Paragraph { width: Some(120.0), ..Default::default() }, ..t.clone() };
    let (w2, h2) = narrow.box_size();
    assert_eq!(w2, 120.0);
    assert!(h2 > h1 * 2.5, "wrapped onto several lines: {h1} → {h2}");
    let b = narrow.outline().bounds().unwrap();
    assert!(b.1.x <= 121.0, "no line past the box: {b:?}");
    // justify: the first line reaches the right edge
    let just = ac_core::TextEl { align: Align::Justify, ..narrow.clone() };
    let first_line_right = |t: &ac_core::TextEl| {
        let lp = ac_core::text::letter_positions_para(&t.text, t.size, None, t.font, t.align, 0.0, &t.para);
        lp.iter().filter(|(_, p)| p.y < 5.0).map(|(_, p)| p.x).fold(0.0, f32::max)
    };
    assert!(first_line_right(&just) > first_line_right(&narrow) + 5.0);
    // line spacing and indent
    let spaced = ac_core::TextEl { para: ac_core::text::Paragraph { width: Some(120.0), line_spacing: 10.0, indent: 15.0, ..Default::default() }, ..t.clone() };
    assert!(spaced.box_size().1 > h2 + 15.0);
    let lp = ac_core::text::letter_positions_para(&spaced.text, 20.0, None, spaced.font, Align::Left, 0.0, &spaced.para);
    assert!((lp[0].1.x - 15.0).abs() < 2.0, "first line indented: {:?}", lp[0]);
    // superscript: smaller and higher
    let sup = ac_core::TextEl { para: ac_core::text::Paragraph { baseline: Baseline::Superscript, ..Default::default() }, ..t.clone() };
    let (a, b2) = (t.outline().bounds().unwrap(), sup.outline().bounds().unwrap());
    assert!(b2.1.x - b2.0.x < (a.1.x - a.0.x) * 0.75 && b2.0.y < a.0.y);
    let _ = w1;
}

#[test]
fn text_fields_password_border_and_kinds() {
    let mut ed = Editor::new(Default::default());
    execute(&mut ed, "draw.text", &json!({"x": 0, "y": 0, "text": "secret", "kind": "input", "name": "pw"})).unwrap();
    assert!(execute(&mut ed, "draw.text", &json!({"x": 0, "y": 0, "text": "x", "kind": "weird"})).is_err());
    let mut t = text_el(&ed);
    assert_eq!(t.field.kind, TextKind::Input);
    assert!(t.field.selectable);
    assert_eq!(els(&ed)[0].name, "pw");
    t.field.password = true;
    assert_eq!(t.shown(), "••••••");
    t.field.border = true;
    let el = ac_core::Element { kind: ElementKind::Text(t), ..els(&ed)[0].clone() };
    let mut doc = ed.doc.clone();
    doc.scene.layers[0].keyframes.get_mut(&0).unwrap().elements = vec![el];
    let ctx = ac_core::EvalCtx::new(&doc, 0);
    let mut items = vec![];
    ac_core::evaluate(&ctx, &doc.scene, 0, &ac_core::Matrix::IDENTITY, 1.0, 0, &mut items);
    assert_eq!(items.len(), 2, "border box + text");
    assert!(items[0].stroke.is_some());
    let _ = pt(0.0, 0.0);
}
