//! Properties of the document model over many generated inputs.
#![allow(clippy::expect_used)]

use flare_core::{DocumentId, FrameRate, LayerId, Rgba8, SceneId, ShapeId};
use flare_document::{
    Document, Gradient, Layer, MAX_STROKE_WIDTH, PathSegment, Scene, Shape, Stroke, Timeline,
};
use proptest::prelude::*;

fn color() -> impl Strategy<Value = Rgba8> {
    any::<[u8; 4]>().prop_map(|[r, g, b, a]| Rgba8::new(r, g, b, a))
}

proptest! {
    #[test]
    fn strokes_exist_exactly_for_widths_in_range(width in any::<f64>(), color in color()) {
        let made = Stroke::new(width, color);
        prop_assert_eq!(made.is_ok(), width.is_finite() && width > 0.0 && width <= MAX_STROKE_WIDTH);
    }

    #[test]
    fn polygons_and_stars_exist_for_three_to_sixty_four_corners_and_are_closed(
        sides in 0_u32..100, radius in -10.0_f64..1000.0, rotation in -7.0_f64..7.0,
        inner in prop::option::of(0.0_f64..2.0), cx in -1e4_f64..1e4, cy in -1e4_f64..1e4,
    ) {
        let shape = Shape::polygon(ShapeId::new(), (cx, cy), radius, sides, rotation, inner, Rgba8::WHITE);
        let valid = (3..=64).contains(&sides) && radius > 0.0;
        prop_assert_eq!(shape.is_some(), valid);
        if let Some(shape) = shape {
            let corners = if inner.is_some() { sides * 2 } else { sides } as usize;
            prop_assert_eq!(shape.path().len(), corners + 1);
            prop_assert!(matches!(shape.path().last(), Some(PathSegment::Close)));
        }
    }

    #[test]
    fn rounded_rectangles_are_always_valid_whatever_the_radius(
        w in 1.0_f64..500.0, h in 1.0_f64..500.0, r in -50.0_f64..800.0,
    ) {
        let shape = Shape::rounded_rect(ShapeId::new(), 0.0, 0.0, w, h, r, Rgba8::WHITE)
            .expect("finite numbers always make a rectangle");
        prop_assert!(matches!(shape.path().first(), Some(PathSegment::MoveTo(_))));
        prop_assert!(matches!(shape.path().last(), Some(PathSegment::Close)));
    }

    #[test]
    fn a_gradient_needs_two_different_points(
        ax in -100.0_f64..100.0, ay in -100.0_f64..100.0, same in any::<bool>(), c in color(),
    ) {
        let a = flare_core::DocumentPoint::new(ax, ay).expect("finite");
        let b = if same { a } else { flare_core::DocumentPoint::new(ax + 1.0, ay).expect("finite") };
        prop_assert_eq!(Gradient::linear(a, b, c, c).is_ok(), !same);
    }

    /// Folders sit above what they hold: a document is valid exactly when every layer's parent is
    /// a folder earlier in the list, whatever shape of nesting was generated.
    #[test]
    fn a_folder_structure_is_valid_exactly_when_every_parent_is_a_folder_above_its_child(
        spec in prop::collection::vec((any::<bool>(), prop::option::of(0_usize..10)), 1..10),
    ) {
        let ids: Vec<LayerId> = spec.iter().map(|_| LayerId::new()).collect();
        let mut expected_valid = true;
        let layers: Vec<Layer> = spec
            .iter()
            .enumerate()
            .map(|(i, (folder, parent))| {
                let parent_index = parent.map(|p| p % 10).filter(|p| *p < spec.len());
                if let Some(p) = parent_index {
                    // Valid only if the parent is earlier and is itself a folder.
                    expected_valid &= p < i && spec[p].0;
                }
                Layer::new(ids[i], format!("L{i}"), vec![])
                    .expect("layer")
                    .with_folder(*folder)
                    .with_parent(parent_index.map(|p| ids[p]))
            })
            .collect();
        let scene = Scene::new(SceneId::new(), "S", Timeline::new(layers));
        let built = Document::new(DocumentId::new(), FrameRate::new(24, 1).expect("rate"), vec![scene], vec![]);
        prop_assert_eq!(built.is_ok(), expected_valid);
    }
}
