//! Phase 3 gap fixes: Break Apart on text and bitmaps, bitmap fills on merge
//! shapes, the Magic Wand, brush shapes and Ctrl-drag corners.

use ac_core::bitmap::BitmapAsset;
use ac_core::commands::execute;
use ac_core::merge::{nib_outline, BrushShape, MergeShape, Part};
use ac_core::{pt, BitmapFill, Color, Editor, ElementKind, Matrix, Path, Stroke};
use serde_json::json;

fn two_tone() -> BitmapAsset {
    // 40 × 20: left half red, right half blue
    let mut rgba = vec![];
    for _y in 0..20 {
        for x in 0..40 {
            rgba.extend_from_slice(if x < 20 { &[255, 0, 0, 255] } else { &[0, 0, 255, 255] });
        }
    }
    BitmapAsset::from_rgba(0, "two-tone", 40, 20, &rgba).unwrap()
}

fn kinds(ed: &Editor) -> Vec<&'static str> {
    let l = ed.layer;
    let (els, _) = ac_core::model::layer_elements_at(&ed.timeline().layers[l], ed.frame).unwrap();
    els.iter()
        .map(|e| match &e.kind {
            ElementKind::Shape(_) => "shape",
            ElementKind::Text(_) => "text",
            ElementKind::Bitmap(_) => "bitmap",
            ElementKind::Canvas(_) => "merge",
            _ => "other",
        })
        .collect()
}

#[test]
fn break_apart_text_goes_letters_then_shapes() {
    let mut ed = Editor::new(Default::default());
    execute(&mut ed, "draw.text", &json!({"x": 10, "y": 10, "text": "AB", "size": 40})).unwrap();
    execute(&mut ed, "select.all", &json!({})).unwrap();
    execute(&mut ed, "selection.break_apart", &json!({})).unwrap();
    assert_eq!(kinds(&ed), ["text", "text"], "first Break Apart: one text object per letter");
    execute(&mut ed, "select.all", &json!({})).unwrap();
    execute(&mut ed, "selection.break_apart", &json!({})).unwrap();
    assert_eq!(kinds(&ed), ["merge"], "second Break Apart: the letters join the layer's merge shape");
    assert!(ed.canvas(ed.layer).unwrap().fills.len() >= 2, "both letters are fills");
    // one undo goes back to the letters
    assert!(ed.undo().is_some());
    assert_eq!(kinds(&ed), ["text", "text"]);
}

#[test]
fn break_apart_bitmap_then_magic_wand_selects_one_colour() {
    let mut ed = Editor::new(Default::default());
    let id = ed.add_bitmap(two_tone());
    ed.place_bitmap(id, pt(100.0, 100.0)).unwrap();
    execute(&mut ed, "select.all", &json!({})).unwrap();
    execute(&mut ed, "selection.break_apart", &json!({})).unwrap();
    let c = ed.canvas(ed.layer).expect("broken-apart bitmap is a merge shape");
    assert_eq!(c.fills.len(), 1);
    assert_eq!(c.fills[0].bitmap.map(|b| b.bitmap), Some(id));

    // wand on the red half
    let r = execute(&mut ed, "select.magic_wand", &json!({"x": 105, "y": 110})).unwrap();
    assert_ne!(r, json!(false));
    execute(&mut ed, "selection.delete", &json!({})).unwrap();
    ed.select(None, false);
    let c = ed.canvas(ed.layer).unwrap();
    let xs: Vec<f32> = c.fills.iter().flat_map(|f| f.contours.iter().flatten().map(|p| p.x)).collect();
    let lo = xs.iter().cloned().fold(f32::MAX, f32::min);
    assert!((lo - 120.0).abs() < 1.5, "the red half is gone, blue remains from x=120 (got {lo})");
}

#[test]
fn merge_shape_bitmap_fills_merge_and_survive_conversion() {
    let sq = |x: f32| vec![pt(x, 0.), pt(x + 50., 0.), pt(x + 50., 50.), pt(x, 50.)];
    let mut m = MergeShape::default();
    m.add_fill(&[sq(0.0)], Some(Color::rgb(0, 200, 0)), None, true);
    let bf = BitmapFill { bitmap: 7, matrix: Matrix::IDENTITY };
    m.set_bitmap_fill(&[Part::Fill(0)], bf);
    assert_eq!(m.fills[0].bitmap.map(|b| b.bitmap), Some(7));
    // the same bitmap fill painted next to it merges into one region
    let paint = m.fills[0].clone();
    m.add_region(&[sq(50.0)], &paint, true);
    assert_eq!(m.fills.len(), 1, "same bitmap fill merges");
    // a solid fill next to it does not
    m.add_fill(&[sq(100.0)], Some(Color::rgb(0, 200, 0)), None, true);
    assert_eq!(m.fills.len(), 2);
    let shapes = m.to_shapes();
    assert_eq!(shapes.iter().filter(|s| s.bitmap_fill.is_some()).count(), 1);
}

#[test]
fn brush_shapes_sweep_their_tip() {
    let line = [pt(0.0, 0.0), pt(100.0, 0.0)];
    let area = |polys: &[Vec<ac_core::Pt>]| -> f32 {
        polys
            .iter()
            .map(|c| {
                let n = c.len();
                (0..n).map(|i| c[i].x * c[(i + 1) % n].y - c[(i + 1) % n].x * c[i].y).sum::<f32>().abs() / 2.0
            })
            .sum()
    };
    let square = nib_outline(&line, &BrushShape::Square.nib(10.0));
    assert!((area(&square) - 110.0 * 10.0).abs() < 2.0, "square tip: 110 × 10, got {}", area(&square));
    // a vertical flat tip moving sideways paints wide, a horizontal one barely at all
    let v = area(&nib_outline(&line, &BrushShape::FlatVertical.nib(10.0)));
    let h = area(&nib_outline(&line, &BrushShape::FlatHorizontal.nib(10.0)));
    assert!(v > h * 3.0, "flat vertical {v} vs flat horizontal {h}");
    // one dab = the tip itself
    assert_eq!(nib_outline(&line[..1], &BrushShape::Slant.nib(10.0)).len(), 1);
}

#[test]
fn ctrl_drag_adds_a_corner() {
    let mut m = MergeShape::default();
    let st = Stroke { color: Color::rgb(0, 0, 0), width: 1.0 };
    m.add_path(&Path::rect(0.0, 0.0, 100.0, 100.0), Some(Color::rgb(255, 0, 0)), None, Some(st));
    let b = m.begin_bend_ex(pt(30.0, 0.0), 2.0, true).unwrap();
    let out = b.apply(pt(0.0, -30.0));
    let c = &out.fills[0].contours[0];
    // the new corner is exactly where the edge was grabbed
    assert!(c.iter().any(|q| q.dist(pt(30.0, -30.0)) < 0.5), "corner at the grab point");
    // the segments to the old corners are straight: the point a third of the way
    // from (0,0) to the new corner lies on that line
    let mid = pt(10.0, -10.0);
    assert!(c.iter().any(|q| q.dist(mid) < 1.0), "straight edge from the old corner");
}

#[test]
fn lifting_a_fill_keeps_its_outline() {
    let st = Stroke { color: Color::rgb(0, 0, 0), width: 1.0 };
    for bend in [None, Some(false), Some(true)] {
        let mut m = MergeShape::default();
        m.add_path(&Path::rect(0.0, 0.0, 100.0, 70.0), Some(Color::rgb(255, 0, 0)), None, Some(st));
        if let Some(sharp) = bend {
            m = m.begin_bend_ex(pt(50.0, 0.0), 2.0, sharp).unwrap().apply(pt(0.0, -40.0));
        }
        let lines = m.strokes.len();
        assert!(lines > 0);
        m.lift(&[Part::Fill(0)], false);
        m.commit_float();
        assert!(!m.strokes.is_empty(), "bend {bend:?}: outline lost ({lines} lines before)");
    }
}

#[test]
fn merge_lines_take_width_profiles_that_survive_splitting() {
    use ac_core::merge::StrokeEdge;
    let st = Stroke { color: Color::rgb(0, 0, 0), width: 4.0 };
    let mut m = MergeShape::default();
    m.add_stroke(&[pt(0.0, 50.0), pt(200.0, 50.0)], false, st);
    m.strokes[0].set_width(0.75, 5.0); // thick near x = 150
    assert!((m.strokes[0].width_at(0.75) - 5.0).abs() < 1e-3);
    // a vertical line crosses at x = 100: the right half keeps the bulge
    m.add_stroke(&[pt(100.0, 0.0), pt(100.0, 100.0)], false, st);
    let right = m.strokes.iter().find(|s| s.pts.iter().any(|p| p.x > 190.0) && s.pts.iter().all(|p| p.x >= 99.0 && (p.y - 50.0).abs() < 1e-3)).expect("right half");
    assert!((right.width_at(right.t_of(pt(150.0, 50.0))) - 5.0).abs() < 0.05, "bulge moved with the piece");
    let left = m.strokes.iter().find(|s| s.pts.iter().any(|p| p.x < 10.0)).expect("left half");
    assert!(left.width_at(0.0) < 1.01);
    // StrokeEdge::new is plain
    assert!(StrokeEdge::new(vec![pt(0.0, 0.0), pt(1.0, 0.0)], false, st).width.is_none());
}

#[test]
fn width_tool_on_merge_lines_through_the_editor() {
    let mut ed = Editor::new(Default::default());
    ed.script_merge = true;
    ac_core::commands::execute(&mut ed, "draw.line", &serde_json::json!({"x1": 0, "y1": 50, "x2": 200, "y2": 50, "stroke_width": 4})).unwrap();
    let (li, line, t) = ed.merge_line_at(pt(150.0, 51.0), 3.0).expect("the line");
    ed.merge_set_width(li, line, t, 6.0, true);
    assert_eq!(ed.merge_width_points().len(), 1);
    // the evaluated line is a filled outline, wider at x = 150
    let items = ed.evaluate_current(0);
    let it = items.iter().find(|i| i.fill.is_some()).expect("outline item");
    let (lo, hi) = it.path.bounds().unwrap();
    assert!(hi.y - lo.y > 20.0, "about 24 px tall there: {}", hi.y - lo.y);
    ed.merge_remove_width_point(li, line, 1);
    assert!(ed.canvas(li).unwrap().strokes[line].width.is_none());
}
