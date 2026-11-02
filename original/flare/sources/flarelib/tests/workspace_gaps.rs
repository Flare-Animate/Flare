//! Gaps closed with the referenced sources (Oct 15 2026): History steps,
//! Convert Lines to Fills, Expand Fill and Soften Fill Edges (i_overlay
//! outlines and offsets), on merge shapes and drawing objects.

use ac_core::commands::execute;
use ac_core::merge::{MergeShape, StrokeEdge};
use ac_core::{pt, Color, Editor, ElementKind, Stroke};
use serde_json::json;

fn area(polys: &[Vec<ac_core::Pt>]) -> f32 {
    polys
        .iter()
        .map(|c| {
            let n = c.len();
            (0..n).map(|i| c[i].x * c[(i + 1) % n].y - c[(i + 1) % n].x * c[i].y).sum::<f32>() / 2.0
        })
        .sum::<f32>()
        .abs()
}

#[test]
fn history_jumps_back_and_forward() {
    let mut ed = Editor::new(Default::default());
    for i in 0..3 {
        execute(&mut ed, "draw.rect", &json!({"x": 10 + i * 30, "y": 10, "w": 20, "h": 20})).unwrap();
    }
    let (done, undone) = ed.history();
    assert_eq!((done.len(), undone.len()), (3, 0));
    let count = |ed: &Editor| ed.timeline().layers[0].keyframes[&0].elements.len();
    assert_eq!(count(&ed), 3);
    ed.go_to_step(1);
    assert_eq!(count(&ed), 1);
    assert_eq!(ed.history().1.len(), 2);
    ed.go_to_step(3);
    assert_eq!(count(&ed), 3);
    ed.go_to_step(0);
    assert_eq!(count(&ed), 0);
    ed.clear_history();
    assert_eq!(ed.history(), (vec![], vec![]));
}

#[test]
fn merge_lines_become_fills_and_fills_expand_and_soften() {
    let black = Color::rgb(0, 0, 0);
    let mut m = MergeShape::default();
    m.strokes.push(StrokeEdge::new(vec![pt(0.0, 0.0), pt(100.0, 0.0)], false, Stroke { color: black, width: 10.0 }));
    assert_eq!(m.lines_to_fills(), 1);
    assert!(m.strokes.is_empty());
    assert_eq!(m.fills.len(), 1);
    assert_eq!(m.fills[0].fill, Some(black));
    // a 100 × 10 bar with round ends ≈ 1000 + π·25
    let a = area(&m.fills[0].contours);
    assert!((a - (1000.0 + std::f32::consts::PI * 25.0)).abs() < 15.0, "{a}");

    let red = Color::rgb(255, 0, 0);
    let square = vec![vec![pt(0.0, 0.0), pt(100.0, 0.0), pt(100.0, 100.0), pt(0.0, 100.0)]];
    let mut g = MergeShape::default();
    g.add_fill(&square, Some(red), None, false);
    g.expand_fills(10.0);
    let a = area(&g.fills[0].contours);
    assert!(a > 14_000.0 && a < 14_400.0, "grown with round corners: {a}");
    g.expand_fills(-20.0);
    let a = area(&g.fills[0].contours);
    assert!((a - 6400.0).abs() < 100.0, "120 wide inset by 20 each side → 80 × 80: {a}");

    let mut s = MergeShape::default();
    s.add_fill(&square, Some(red), None, false);
    s.soften_fills(12.0, 4, false);
    let alphas: Vec<u8> = s.fills.iter().filter_map(|f| f.fill).map(|c| c.a).collect();
    assert!(alphas.contains(&255) && alphas.iter().filter(|&&a| a < 255).count() == 4, "{alphas:?}");
    assert!(alphas.iter().all(|&a| a > 0));
}

#[test]
fn drawing_objects_convert_expand_and_soften() {
    let mut ed = Editor::new(Default::default());
    execute(&mut ed, "draw.line", &json!({"x1": 0, "y1": 0, "x2": 100, "y2": 0, "stroke": "#00FF00", "stroke_width": 8})).unwrap();
    execute(&mut ed, "select.all", &json!({})).unwrap();
    ed.shape_fill_op("lines_to_fills", 0.0, 0, false);
    let els = &ed.timeline().layers[0].keyframes[&0].elements;
    match &els[0].kind {
        ElementKind::Shape(s) => {
            assert!(s.stroke.is_none());
            assert_eq!(s.fill, Some(Color::rgb(0, 255, 0)));
            let (lo, hi) = s.path.bounds().unwrap();
            assert!((hi.y - lo.y - 8.0).abs() < 0.5 && (hi.x - lo.x - 108.0).abs() < 0.5, "{lo:?} {hi:?}");
        }
        k => panic!("{k:?}"),
    }
    let mut ed = Editor::new(Default::default());
    execute(&mut ed, "draw.rect", &json!({"x": 0, "y": 0, "w": 100, "h": 100, "fill": "#FF0000"})).unwrap();
    execute(&mut ed, "select.all", &json!({})).unwrap();
    ed.shape_fill_op("expand", 5.0, 0, false);
    let b = match &ed.timeline().layers[0].keyframes[&0].elements[0].kind {
        ElementKind::Shape(s) => s.path.bounds().unwrap(),
        k => panic!("{k:?}"),
    };
    assert!((b.1.x - b.0.x - 110.0).abs() < 0.5, "{b:?}");
    ed.shape_fill_op("soften", 6.0, 3, false);
    match &ed.timeline().layers[0].keyframes[&0].elements[0].kind {
        ElementKind::Group(parts) => assert_eq!(parts.len(), 4, "core + 3 rings"),
        k => panic!("{k:?}"),
    }
    assert_eq!(ed.history().0.len(), 3, "rectangle, expand, soften: one step each");
}
