#![allow(clippy::expect_used)]

use flare_core::{ShaderGraph, ShaderNode};

use super::*;

#[test]
fn renders_procedural_graph_at_pixel_centers() {
    let graph = ShaderGraph::new(
        vec![
            ShaderNode::UvX,
            ShaderNode::UvY,
            ShaderNode::Constant(0.0),
            ShaderNode::Constant(1.0),
            ShaderNode::Color([0, 1, 2, 3]),
        ],
        4,
    )
    .expect("valid graph");
    let image = render_shader(&graph, 2, 1, 0.0).expect("render");
    assert_eq!(
        image.pixel(0, 0).map(|p| p.to_array()),
        Some([64, 128, 0, 255])
    );
    assert_eq!(
        image.pixel(1, 0).map(|p| p.to_array()),
        Some([191, 128, 0, 255])
    );
}

#[test]
fn refuses_invalid_dimensions_and_non_finite_time() {
    let graph = ShaderGraph::new(
        vec![
            ShaderNode::Constant(0.0),
            ShaderNode::Constant(0.0),
            ShaderNode::Constant(0.0),
            ShaderNode::Constant(1.0),
            ShaderNode::Color([0, 1, 2, 3]),
        ],
        4,
    )
    .expect("valid graph");
    assert!(matches!(
        render_shader(&graph, 0, 1, 0.0),
        Err(ShaderRenderError::Render(RenderError::InvalidSize(0, 1)))
    ));
    assert!(matches!(
        render_shader(&graph, 1, 1, f32::NAN),
        Err(ShaderRenderError::Graph(
            flare_core::ShaderGraphError::NonFiniteContext
        ))
    ));
}

#[test]
fn applies_a_graph_to_source_pixels_without_changing_the_input() {
    let graph = ShaderGraph::new(
        vec![
            ShaderNode::SourceColor,
            ShaderNode::Channel(0, 0),
            ShaderNode::Constant(0.5),
            ShaderNode::Multiply(1, 2),
            ShaderNode::Channel(0, 1),
            ShaderNode::Channel(0, 2),
            ShaderNode::Channel(0, 3),
            ShaderNode::Color([3, 4, 5, 6]),
        ],
        7,
    )
    .expect("valid graph");
    let source = Image::from_rgba(1, 1, vec![200, 100, 50, 128]).expect("image");
    let result = apply_shader(&graph, &source, 0.0).expect("shader result");
    assert_eq!(
        source.pixel(0, 0).map(|p| p.to_array()),
        Some([200, 100, 50, 128])
    );
    assert_eq!(
        result.pixel(0, 0).map(|p| p.to_array()),
        Some([100, 100, 50, 128])
    );
}

#[test]
fn shader_stack_feeds_each_output_to_the_next_graph() {
    let first = ShaderGraph::new(
        vec![
            ShaderNode::Constant(0.25),
            ShaderNode::Constant(0.5),
            ShaderNode::Constant(0.75),
            ShaderNode::Constant(1.0),
            ShaderNode::Color([0, 1, 2, 3]),
        ],
        4,
    )
    .expect("first graph");
    let second = ShaderGraph::new(
        vec![
            ShaderNode::SourceColor,
            ShaderNode::Channel(0, 1),
            ShaderNode::Constant(0.0),
            ShaderNode::Constant(1.0),
            ShaderNode::Color([1, 2, 2, 3]),
        ],
        4,
    )
    .expect("second graph");
    let source = Image::from_rgba(1, 1, vec![0, 0, 0, 255]).expect("image");

    let result = apply_shader_stack(&[&first, &second], &source, 0.0).expect("stack");

    assert_eq!(
        result.pixel(0, 0).map(|p| p.to_array()),
        Some([128, 0, 0, 255])
    );
    assert_eq!(
        apply_shader_stack(&[], &source, 0.0).expect("empty stack"),
        source
    );
    let too_many = vec![&first; MAX_SHADER_STACK + 1];
    assert!(matches!(
        apply_shader_stack(&too_many, &source, 0.0),
        Err(ShaderRenderError::TooManyEffects)
    ));
}
