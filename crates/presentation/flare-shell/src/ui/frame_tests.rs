use super::frame::*;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::window::ResizeDirection::*;
use winit::window::{CursorIcon, ResizeDirection};

const SIZE: PhysicalSize<u32> = PhysicalSize::new(1000, 700);

fn edge(x: f64, y: f64) -> Option<ResizeDirection> {
    resize_edge(PhysicalPosition::new(x, y), SIZE, 6.0)
}

#[test]
fn the_four_edges_resize_along_their_own_axis() {
    assert_eq!(edge(500.0, 2.0), Some(North));
    assert_eq!(edge(500.0, 697.0), Some(South));
    assert_eq!(edge(2.0, 350.0), Some(West));
    assert_eq!(edge(997.0, 350.0), Some(East));
}

#[test]
fn the_four_corners_resize_both_ways_and_reach_further_along_the_edges() {
    assert_eq!(edge(1.0, 1.0), Some(NorthWest));
    assert_eq!(edge(998.0, 1.0), Some(NorthEast));
    assert_eq!(edge(1.0, 698.0), Some(SouthWest));
    assert_eq!(edge(998.0, 698.0), Some(SouthEast));
    // Along the top edge, within twice the border of the corner, is the corner; beyond it is not.
    assert_eq!(edge(11.0, 2.0), Some(NorthWest));
    assert_eq!(edge(13.0, 2.0), Some(North));
    assert_eq!(edge(2.0, 11.0), Some(NorthWest));
    assert_eq!(edge(2.0, 13.0), Some(West));
}

#[test]
fn inside_the_window_or_outside_it_is_not_an_edge() {
    assert_eq!(edge(500.0, 350.0), None);
    assert_eq!(edge(6.0, 350.0), None, "just inside the border");
    assert_eq!(edge(-1.0, 350.0), None);
    assert_eq!(edge(500.0, 700.0), None, "the pixel past the bottom");
    assert_eq!(edge(1000.0, 350.0), None);
}

#[test]
fn a_window_with_no_size_has_no_edges() {
    let at = PhysicalPosition::new(0.0, 0.0);
    assert_eq!(resize_edge(at, PhysicalSize::new(0, 700), 6.0), None);
    assert_eq!(resize_edge(at, PhysicalSize::new(1000, 0), 6.0), None);
}

#[test]
fn every_direction_has_a_resize_pointer_and_opposite_ones_share_it() {
    assert_eq!(cursor_for(North), CursorIcon::NsResize);
    assert_eq!(cursor_for(South), CursorIcon::NsResize);
    assert_eq!(cursor_for(East), CursorIcon::EwResize);
    assert_eq!(cursor_for(West), CursorIcon::EwResize);
    assert_eq!(cursor_for(NorthWest), CursorIcon::NwseResize);
    assert_eq!(cursor_for(SouthEast), CursorIcon::NwseResize);
    assert_eq!(cursor_for(NorthEast), CursorIcon::NeswResize);
    assert_eq!(cursor_for(SouthWest), CursorIcon::NeswResize);
}

#[test]
fn the_system_title_bar_is_kept_only_when_asked_for() {
    assert!(!system_titlebar_wanted(None));
    assert!(!system_titlebar_wanted(Some("")));
    assert!(!system_titlebar_wanted(Some("0")));
    assert!(system_titlebar_wanted(Some("1")));
    assert!(system_titlebar_wanted(Some("yes")));
}
