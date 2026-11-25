use super::*;

const RED: Rgba8 = Rgba8::new(255, 0, 0, 255);

fn corners_of(shape: &Shape) -> Vec<(f64, f64)> {
    shape
        .path()
        .iter()
        .filter_map(|s| match s {
            PathSegment::MoveTo(p) | PathSegment::LineTo(p) => Some((p.as_vec().x, p.as_vec().y)),
            _ => None,
        })
        .collect()
}

#[test]
fn the_corners_clicked_become_a_closed_filled_outline_in_order() {
    let corners = [(0.0, 0.0), (40.0, 0.0), (50.0, 30.0), (10.0, 40.0)];
    let shape = polygon_shape(&corners, RED, None).expect("a polygon");
    assert_eq!(corners_of(&shape), corners.to_vec());
    assert!(matches!(shape.path().last(), Some(PathSegment::Close)));
    assert!(matches!(shape.path().first(), Some(PathSegment::MoveTo(_))));
    assert_eq!(shape.fill(), RED);
    assert!(shape.stroke().is_none());
}

#[test]
fn a_stroke_given_goes_around_it() {
    let stroke = Stroke::new(3.0, Rgba8::new(0, 0, 0, 255)).expect("stroke");
    let shape = polygon_shape(&[(0.0, 0.0), (10.0, 0.0), (0.0, 10.0)], RED, Some(stroke))
        .expect("a triangle");
    assert_eq!(shape.stroke(), Some(stroke));
}

#[test]
fn repeated_corners_and_a_last_corner_on_the_first_count_once() {
    let corners = [
        (0.0, 0.0),
        (10.0, 0.0),
        (10.0, 0.0),
        (10.0, 10.0),
        (0.0, 0.0),
    ];
    let shape = polygon_shape(&corners, RED, None).expect("a triangle");
    assert_eq!(corners_of(&shape).len(), 3);
}

#[test]
fn too_few_corners_a_line_or_numbers_that_are_not_are_refused() {
    assert_eq!(
        polygon_shape(&[(0.0, 0.0), (5.0, 5.0)], RED, None),
        Err(PolygonError::TooFewCorners)
    );
    assert_eq!(
        polygon_shape(&[(0.0, 0.0), (0.0, 0.0), (0.0, 0.0)], RED, None),
        Err(PolygonError::TooFewCorners)
    );
    assert_eq!(
        polygon_shape(&[(0.0, 0.0), (5.0, 5.0), (10.0, 10.0)], RED, None),
        Err(PolygonError::NoArea)
    );
    assert_eq!(
        polygon_shape(&[(0.0, 0.0), (f64::NAN, 5.0), (10.0, 0.0)], RED, None),
        Err(PolygonError::NotFinite)
    );
    let many = vec![(0.0, 0.0); MAX_CORNERS + 1];
    assert_eq!(
        polygon_shape(&many, RED, None),
        Err(PolygonError::TooManyCorners)
    );
}
