//! Features folded in from Flare's Studio: the two paste modes, layer
//! clipboard, graphic playback clock, bitmap-fill library usage.

use ac_core::commands::execute;
use ac_core::*;
use serde_json::json;

fn keys(ed: &Editor, l: usize) -> Vec<u32> {
    ed.timeline().layers[l].keyframes.keys().copied().collect()
}

/// One layer: keyframes at 0, 5, 10, layer 15 frames long.
fn three_keys() -> Editor {
    let mut ed = Editor::new(Document::default());
    execute(&mut ed, "draw.rect", &json!({"x": 0, "y": 0, "w": 10, "h": 10})).unwrap();
    for f in [6, 11] {
        execute(&mut ed, "timeline.insert_keyframe", &json!({"frame": f})).unwrap();
    }
    execute(&mut ed, "timeline.insert_frames", &json!({"frame": 15})).unwrap();
    ed
}

#[test]
fn paste_frames_shifts_later_frames_and_overwrite_keeps_them() {
    let mut ed = three_keys();
    assert_eq!(keys(&ed, 0), vec![0, 5, 10]);
    let end = ed.timeline().layers[0].end;
    // copy frames 1-3 (three frames), paste over the single frame 6 (index 5)
    ed.select_frames(FrameSel { layer0: 0, layer1: 0, f0: 0, f1: 2 });
    ed.copy_frames();
    ed.select_frames(FrameSel::cell(0, 5));
    ed.paste_frames();
    // frames 6-8 are the pasted ones; the original span resumes at 9 and the
    // key that was at 11 is now at 13 (3 pasted − 1 replaced)
    assert_eq!(keys(&ed, 0), vec![0, 5, 8, 12]);
    assert_eq!(ed.timeline().layers[0].end, end + 2);
    ed.undo();
    ed.select_frames(FrameSel::cell(0, 5));
    ed.paste_overwrite_frames();
    assert_eq!(keys(&ed, 0), vec![0, 5, 8, 10], "overwrite: later keys stay, frame 8 pinned");
    assert_eq!(ed.timeline().layers[0].end, end);
    // the command takes both modes
    ed.undo();
    execute(&mut ed, "timeline.paste_frames", &json!({"frame": 6, "overwrite": true})).unwrap();
    assert_eq!(keys(&ed, 0), vec![0, 5, 8, 10]);
}

#[test]
fn copy_cut_paste_layers_keep_folders_and_parenting() {
    let mut ed = Editor::new(Document::default());
    execute(&mut ed, "draw.rect", &json!({"x": 0, "y": 0, "w": 10, "h": 10})).unwrap();
    execute(&mut ed, "layer.add", &json!({"name": "Arm"})).unwrap();
    execute(&mut ed, "draw.rect", &json!({"x": 20, "y": 0, "w": 10, "h": 10})).unwrap();
    // put "Arm" in a folder; parent "Arm" to the bottom layer
    execute(&mut ed, "layer.wrap", &json!({"kind": "folder", "name": "Rig", "layer": "Arm"})).unwrap();
    let names = |ed: &Editor| ed.timeline().layers.iter().map(|l| l.name.clone()).collect::<Vec<_>>();
    assert_eq!(names(&ed), ["Rig", "Arm", "Layer 1"]);
    execute(&mut ed, "layer.copy", &json!({"layer": "Rig"})).unwrap();
    execute(&mut ed, "layer.paste", &json!({"layer": "Layer 1"})).unwrap();
    assert_eq!(names(&ed), ["Rig", "Arm", "Rig", "Arm", "Layer 1"], "folder copied with its contents");
    let tl = ed.timeline();
    assert_eq!(tl.layers[3].parent, Some(2), "the pasted Arm sits in the pasted Rig");
    assert_eq!(tl.layers[1].parent, Some(0), "the original stays put");
    assert_eq!(tl.layers[3].keyframes[&0].elements.len(), 1);
    // cut removes them (one undo step), paste brings them back
    execute(&mut ed, "layer.cut", &json!({"layer": 3, "to_layer": 4})).unwrap();
    assert_eq!(names(&ed), ["Rig", "Arm", "Layer 1"]);
    ed.undo();
    assert_eq!(names(&ed).len(), 5);
    assert!(execute(&mut Editor::new(Document::default()), "layer.paste", &json!({})).is_err());
}

#[test]
fn f6_inside_a_graphic_span_keeps_it_playing() {
    // a 4-frame graphic symbol whose frames differ, placed on a 12-frame span
    let mut ed = Editor::new(Document::default());
    execute(&mut ed, "draw.rect", &json!({"x": 0, "y": 0, "w": 10, "h": 10})).unwrap();
    execute(&mut ed, "select.all", &json!({})).unwrap();
    execute(&mut ed, "selection.to_symbol", &json!({"name": "Blink", "kind": "graphic"})).unwrap();
    let sym = ed.doc.library[0].id;
    {
        let l = &mut ed.doc.library[0].timeline.layers[0];
        for f in 1..4 {
            let mut k = l.keyframes[&0].clone();
            k.elements[0].matrix = Matrix::translate(f as f32 * 10.0, 0.0);
            l.keyframes.insert(f, k);
        }
        l.end = 4;
    }
    execute(&mut ed, "timeline.insert_frames", &json!({"frame": 12})).unwrap();
    let x = |ed: &Editor, f: u32| ed.evaluate_current(f)[0].matrix.tx;
    let before: Vec<f32> = (0..12).map(|f| x(&ed, f)).collect();
    execute(&mut ed, "timeline.insert_keyframe", &json!({"frame": 7})).unwrap();
    let after: Vec<f32> = (0..12).map(|f| x(&ed, f)).collect();
    assert_eq!(before, after, "the symbol keeps counting through the new keyframe");
    assert_eq!(ed.timeline().layers[0].keyframes[&6].playback_offset, 6);
    let _ = sym;
}

#[test]
fn library_counts_and_clears_bitmap_fills() {
    let mut ed = Editor::new(Document::default());
    let id = ed.add_bitmap(bitmap::BitmapAsset::from_rgba(0, "Tile", 2, 2, &[255; 16]).unwrap());
    execute(&mut ed, "draw.rect", &json!({"x": 0, "y": 0, "w": 10, "h": 10})).unwrap();
    if let ElementKind::Shape(s) = &mut ed.timeline_mut().layers[0].keyframes.get_mut(&0).unwrap().elements[0].kind {
        s.bitmap_fill = Some(BitmapFill { bitmap: id, matrix: Matrix::IDENTITY });
    }
    assert_eq!(ed.bitmap_usage(id), 1, "a bitmap fill is a use");
    ed.delete_bitmap(id);
    let ElementKind::Shape(s) = &ed.timeline().layers[0].keyframes[&0].elements[0].kind else { panic!() };
    assert!(s.bitmap_fill.is_none(), "no dangling bitmap reference after deleting it");
}
