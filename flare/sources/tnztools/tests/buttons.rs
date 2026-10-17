//! Button symbols: published as DefineButton2 with their Up, Over, Down and Hit frames, and drawn
//! hovered and pressed as a player shows them.

use std::collections::{BTreeSet, HashMap};

use minnamate_core::button::{self, Live};
use minnamate_core::display::display_list;
use minnamate_core::doc::*;
use minnamate_core::render::render_live;
use minnamate_core::{draw, swf_export};
use swf::{ButtonState, Tag};

const COLORS: [Rgba; 4] = [[1.0, 0.0, 0.0, 1.0], [0.0, 1.0, 0.0, 1.0], [0.0, 0.0, 1.0, 1.0], [0.0, 0.0, 0.0, 1.0]];

/// A 100 × 100 stage with a button at (10, 10) whose Up, Over, Down and Hit frames hold squares 10,
/// 20, 30 and 40 px wide: red, green, blue and black.
fn doc() -> Document {
    let mut timeline = Timeline::new("Button");
    let layer = &mut timeline.layers[0];
    for state in 0..4 {
        if state > 0 {
            layer.insert_blank_keyframe(state as u32);
        }
        let side = 10.0 * (state + 1) as f32;
        draw::add(&mut layer.frames[state], draw::rectangle([(0.0, 0.0), (side, side)], 0.0, None, Some(Paint::Solid(COLORS[state]))));
    }
    let mut doc = Document::new(100.0, 100.0, 24.0);
    doc.library.insert("Button".into(), Symbol { kind: SymbolKind::Button, timeline, extra: Extra::default() });
    let instance = Instance {
        symbol: "Button".into(),
        kind: SymbolKind::Button,
        matrix: Matrix { tx: 10.0, ty: 10.0, ..Matrix::IDENTITY },
        pivot: (0.0, 0.0),
        color: ColorTransform::IDENTITY,
        looping: Loop::Loop,
        first_frame: 0,
        extra: Extra::default(),
    };
    doc.scenes[0].layers[0].frames[0].elements.push(Element::Instance(instance));
    doc
}

#[test]
fn buttons_publish_their_four_states() {
    let (bytes, skipped) = swf_export::export(&doc()).unwrap();
    assert!(skipped.is_empty(), "{skipped:?}");
    let data = swf::decompress_swf(&bytes[..]).unwrap();
    let movie = swf::parse_swf(&data).unwrap();
    let mut sprites = HashMap::new();
    let mut shapes = HashMap::new();
    let mut button = None;
    for tag in &movie.tags {
        match tag {
            Tag::DefineSprite(s) => drop(sprites.insert(s.id, &s.tags)),
            Tag::DefineShape(s) => drop(shapes.insert(s.id, s.shape_bounds.clone())),
            Tag::DefineButton2(b) => button = Some(&**b),
            _ => {}
        }
    }
    let button = button.expect("a DefineButton2");
    let states: Vec<ButtonState> = button.records.iter().map(|r| r.states).collect();
    assert_eq!(states, [ButtonState::UP, ButtonState::OVER, ButtonState::DOWN, ButtonState::HIT_TEST]);
    // Each state's sprite places its frame's square: the hit area is the 40 px one.
    let widths: Vec<i32> = button
        .records
        .iter()
        .map(|r| {
            let placed = sprites[&r.id].iter().find_map(|t| match t {
                Tag::PlaceObject(p) => match p.action {
                    swf::PlaceObjectAction::Place(id) => Some(id),
                    _ => None,
                },
                _ => None,
            });
            let b = &shapes[&placed.expect("a shape placed")];
            (b.x_max - b.x_min).get()
        })
        .collect();
    assert_eq!(widths, [200, 400, 600, 800]);
    // The stage places the button itself.
    let placed = movie.tags.iter().any(|t| matches!(t, Tag::PlaceObject(p) if p.action == swf::PlaceObjectAction::Place(button.id)));
    assert!(placed);
}

#[test]
fn a_player_shows_over_and_down_where_the_hit_area_reaches() {
    let doc = doc();
    let rgb = |live: Option<Live>, (x, y): (u32, u32)| {
        let c = render_live(&doc, 0, 1.0, live).unwrap().0.pixel(x, y).unwrap().demultiply();
        [c.red(), c.green(), c.blue()]
    };
    let at = |x: f32, y: f32, down: bool| Some(Live { pointer: (x, y), down });
    assert_eq!(rgb(None, (12, 12)), [255, 0, 0], "Up, left alone");
    assert_eq!(rgb(None, (25, 25)), [255, 255, 255]);
    // (45, 45) is outside every frame drawn but the 40 px Hit square from (10, 10).
    assert_eq!(rgb(at(45.0, 45.0, false), (25, 25)), [0, 255, 0], "Over");
    assert_eq!(rgb(at(45.0, 45.0, true), (35, 35)), [0, 0, 255], "Down");
    assert_eq!(rgb(at(55.0, 55.0, true), (12, 12)), [255, 0, 0], "outside the hit area: Up");

    let list = display_list(&doc, &doc.scenes[0], 0, &mut BTreeSet::new());
    assert!(button::under(&doc, &list, (45.0, 45.0)) && !button::under(&doc, &list, (55.0, 55.0)));
}
