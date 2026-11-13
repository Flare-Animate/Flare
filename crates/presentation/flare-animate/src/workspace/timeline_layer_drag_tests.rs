#![allow(clippy::expect_used)]

use super::*;
use flare_editor::timeline_view::visible_layers;

fn named(name: &str) -> Layer {
    Layer::new(LayerId::new(), name, vec![]).expect("layer")
}

/// A, folder F holding X and Y, then B.
fn stack() -> Vec<Layer> {
    let f = named("F").with_folder(true);
    let (x, y) = (
        named("X").with_parent(Some(f.id())),
        named("Y").with_parent(Some(f.id())),
    );
    vec![named("A"), f, x, y, named("B")]
}

fn drop_at(layers: &[Layer], collapsed: &HashSet<LayerId>, moving: usize, gap: usize) -> LayerDrop {
    let rows = visible_layers(layers, collapsed);
    layer_drop(layers, &rows, collapsed, layers[moving].id(), gap)
}

#[test]
fn a_drop_goes_above_the_row_under_the_line_and_into_its_folder() {
    let layers = stack();
    let id = |i: usize| layers[i].id();
    let none = HashSet::new();
    // B dropped between X and Y: into F, above Y.
    let between = drop_at(&layers, &none, 4, 3);
    assert_eq!((between.before, between.folder), (Some(id(3)), Some(id(1))));
    assert_eq!(between.depth, 1);
    // Just under F's row: inside F, at the top.
    let under_folder = drop_at(&layers, &none, 0, 2);
    assert_eq!(
        (under_folder.before, under_folder.folder),
        (Some(id(2)), Some(id(1)))
    );
    // Under F's last layer: out of F, above B.
    let out = drop_at(&layers, &none, 2, 4);
    assert_eq!((out.before, out.folder), (Some(id(4)), None));
    // Past the last row: the bottom.
    let bottom = drop_at(&layers, &none, 0, 99);
    assert_eq!((bottom.before, bottom.folder, bottom.gap), (None, None, 5));
    // Above the first row: the top.
    let top = drop_at(&layers, &none, 4, 0);
    assert_eq!((top.before, top.folder), (Some(id(0)), None));
}

#[test]
fn a_closed_folder_is_passed_over_and_a_layer_dropped_beside_itself_stays() {
    let layers = stack();
    let id = |i: usize| layers[i].id();
    let closed = HashSet::from([id(1)]);
    // Rows: A, F, B. Under the closed F is under all it holds, not inside it.
    let under = drop_at(&layers, &closed, 0, 2);
    assert_eq!((under.before, under.folder), (Some(id(4)), None));
    // A dragged folder skips its own rows: just under itself is where it already is.
    let none = HashSet::new();
    let itself = drop_at(&layers, &none, 1, 3);
    assert_eq!((itself.before, itself.folder), (Some(id(4)), None));
}

#[test]
fn the_gap_is_the_row_edge_nearest_the_pointer() {
    let rect = egui::Rect::from_min_size(Pos2::ZERO, egui::vec2(400.0, ROW * 5.0));
    let l = Layout::new(rect, rect, 10.0, 140.0);
    assert_eq!(l.row_gap_at(l.row_top(0) - 30.0, 4), 0, "over the ruler");
    assert_eq!(l.row_gap_at(l.row_top(1) + ROW * 0.4, 4), 1);
    assert_eq!(l.row_gap_at(l.row_top(1) + ROW * 0.6, 4), 2);
    assert_eq!(l.row_gap_at(l.row_top(9), 4), 4, "below the last row");
    assert_eq!(l.row_gap_at(f32::NAN, 4), 0);
}
