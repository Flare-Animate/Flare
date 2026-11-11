//! Small Flash movies built tag by tag, shared by the SWF import tests.

#![allow(clippy::expect_used)]

use swf::{
    CharacterId, Color, FillStyle, Fixed8, Header, Matrix, PlaceObject, PlaceObjectAction, Point,
    PointDelta, Rectangle, Shape, ShapeFlag, ShapeRecord, ShapeStyles, StyleChangeData, Tag, Twips,
};

pub fn px(v: f64) -> Twips {
    Twips::from_pixels(v)
}

pub fn header(width: f64, height: f64, fps: f32) -> Header {
    Header {
        compression: swf::Compression::None,
        version: 10,
        stage_size: Rectangle {
            x_min: Twips::ZERO,
            x_max: px(width),
            y_min: Twips::ZERO,
            y_max: px(height),
        },
        frame_rate: Fixed8::from_f32(fps),
        num_frames: 1,
    }
}

pub fn bytes(header: &Header, tags: &[Tag<'_>]) -> Vec<u8> {
    let mut out = Vec::new();
    swf::write_swf(header, tags, &mut out).expect("write");
    out
}

pub fn red() -> Color {
    Color {
        r: 255,
        g: 0,
        b: 0,
        a: 255,
    }
}

/// A square of `side` pixels filled with `fill`, from the origin.
pub fn square(id: CharacterId, side: f64, fill: FillStyle) -> Tag<'static> {
    let edge = |dx: f64, dy: f64| ShapeRecord::StraightEdge {
        delta: PointDelta {
            dx: px(dx),
            dy: px(dy),
        },
    };
    let bounds = Rectangle {
        x_min: Twips::ZERO,
        x_max: px(side),
        y_min: Twips::ZERO,
        y_max: px(side),
    };
    Tag::DefineShape(Box::new(Shape {
        version: 1,
        id,
        shape_bounds: bounds,
        edge_bounds: bounds,
        flags: ShapeFlag::empty(),
        styles: ShapeStyles {
            fill_styles: vec![fill],
            line_styles: vec![],
        },
        shape: vec![
            ShapeRecord::StyleChange(Box::new(StyleChangeData {
                move_to: Some(Point {
                    x: Twips::ZERO,
                    y: Twips::ZERO,
                }),
                fill_style_0: None,
                fill_style_1: Some(1),
                line_style: None,
                new_styles: None,
            })),
            edge(side, 0.0),
            edge(0.0, side),
            edge(-side, 0.0),
            edge(0.0, -side),
        ],
    }))
}

pub fn placement(action: PlaceObjectAction, depth: u16, at: Option<(f64, f64)>) -> Tag<'static> {
    Tag::PlaceObject(Box::new(PlaceObject {
        version: 2,
        action,
        depth,
        matrix: at.map(|(x, y)| Matrix::translate(px(x), px(y))),
        color_transform: None,
        ratio: None,
        name: None,
        clip_depth: None,
        class_name: None,
        filters: None,
        background_color: None,
        blend_mode: None,
        clip_actions: None,
        has_image: false,
        is_bitmap_cached: None,
        is_visible: None,
        amf_data: None,
    }))
}

pub fn moving_square() -> Vec<u8> {
    let mut header = header(200.0, 100.0, 24.0);
    header.num_frames = 3;
    bytes(
        &header,
        &[
            Tag::SetBackgroundColor(Color {
                r: 16,
                g: 32,
                b: 48,
                a: 255,
            }),
            square(1, 10.0, FillStyle::Color(red())),
            placement(PlaceObjectAction::Place(1), 1, Some((10.0, 20.0))),
            Tag::ShowFrame,
            Tag::ShowFrame,
            placement(PlaceObjectAction::Modify, 1, Some((30.0, 20.0))),
            Tag::ShowFrame,
            Tag::End,
        ],
    )
}
