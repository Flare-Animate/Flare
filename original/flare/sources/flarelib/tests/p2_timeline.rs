//! P2 group 1 (Oct 15 2026): Synchronize Symbols and Duplicate Symbol and swap.

use ac_core::commands::execute;
use ac_core::{ElementKind, Editor, FrameSel, SymbolKind, Tween};
use serde_json::json;

fn graphic_on_stage(ed: &mut Editor) -> ac_core::SymbolId {
    execute(ed, "draw.rect", &json!({"x": 0, "y": 0, "w": 20, "h": 20})).unwrap();
    execute(ed, "select.all", &json!({})).unwrap();
    ed.convert_to_symbol("Walk", SymbolKind::Graphic).unwrap()
}

fn first_frame(ed: &Editor, key: u32) -> u32 {
    match &ed.timeline().layers[0].keyframes[&key].elements[0].kind {
        ElementKind::Instance(i) => i.first_frame,
        k => panic!("{k:?}"),
    }
}

#[test]
fn synchronize_symbols_continues_instances_across_keyframes() {
    let mut ed = Editor::new(Default::default());
    let id = graphic_on_stage(&mut ed);
    for f in [6, 10] {
        ed.frame = f;
        ed.frame_sel = None;
        ed.insert_keyframe();
    }
    assert_eq!((first_frame(&ed, 6), first_frame(&ed, 10)), (0, 0), "keyframes restart the symbol");
    ed.frame = 0;
    ed.frame_sel = None;
    assert_eq!(ed.synchronize_symbols(), 2);
    assert_eq!((first_frame(&ed, 6), first_frame(&ed, 10)), (6, 10), "the symbol keeps counting");
    assert_eq!(ed.synchronize_symbols(), 0, "already in sync");
    ed.undo();
    ed.undo();
    assert_eq!(first_frame(&ed, 10), 0);

    // a classic tween gets its Sync option instead
    ed.frame = 0;
    ed.select_frames(FrameSel::cell(0, 0));
    ed.set_tween(Tween::Classic { ease: 0.0, rotate_turns: 0 });
    ed.select_frames(FrameSel { layer0: 0, layer1: 0, f0: 0, f1: 12 });
    assert_eq!(ed.synchronize_symbols(), 2, "sync on the tween + the untweened keyframe after it");
    assert!(ed.timeline().layers[0].keyframes[&0].sync);
    assert_eq!(first_frame(&ed, 10), 10, "continues from where the synced tween left off");
    // single-frame instances are left alone
    let _ = id;
}

#[test]
fn duplicate_and_swap_is_one_step_and_keeps_the_folder() {
    let mut ed = Editor::new(Default::default());
    let id = graphic_on_stage(&mut ed);
    let f = ed.new_folder("");
    ed.move_to_folder(ac_core::editor::LibItem::Symbol(id), &f);
    execute(&mut ed, "select.all", &json!({})).unwrap();
    let steps = ed.history().0.len();
    let copy = ed.duplicate_and_swap("").unwrap();
    assert_eq!(ed.history().0.len(), steps + 1);
    assert_eq!(ed.doc.symbol(copy).unwrap().name, format!("{f}/Walk copy"));
    match &ed.selected_element().unwrap().kind {
        ElementKind::Instance(i) => assert_eq!(i.symbol, copy),
        k => panic!("{k:?}"),
    }
    let copy2 = ed.duplicate_and_swap("").unwrap();
    assert_eq!(ed.doc.symbol(copy2).unwrap().name, format!("{f}/Walk copy 2"));
    assert!(ed.duplicate_and_swap("Walk").is_none(), "names must be free");
    assert!(ed.duplicate_and_swap("a/b").is_none());
    assert_eq!(ed.duplicate_and_swap("Run").map(|i| ed.doc.symbol(i).unwrap().name.clone()), Some(format!("{f}/Run")));
    ed.undo();
    ed.undo();
    execute(&mut ed, "select.all", &json!({})).unwrap();
    match &ed.selected_element().unwrap().kind {
        ElementKind::Instance(i) => assert_eq!(i.symbol, copy),
        k => panic!("{k:?}"),
    }
    assert!(ed.doc.symbol(copy2).is_none());
}
