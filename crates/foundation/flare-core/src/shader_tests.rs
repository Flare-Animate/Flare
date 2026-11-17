#![allow(clippy::expect_used)]

use super::*;

fn constant_color() -> ShaderGraph {
    ShaderGraph::new(
        vec![
            ShaderNode::Constant(0.2),
            ShaderNode::Constant(0.4),
            ShaderNode::Constant(0.6),
            ShaderNode::Constant(1.0),
            ShaderNode::Color([0, 1, 2, 3]),
        ],
        4,
    )
    .expect("valid graph")
}

#[test]
fn evaluates_color_and_clamps_channels() {
    let graph = ShaderGraph::new(
        vec![
            ShaderNode::Constant(-1.0),
            ShaderNode::Constant(0.5),
            ShaderNode::Constant(2.0),
            ShaderNode::Constant(1.0),
            ShaderNode::Color([0, 1, 2, 3]),
        ],
        4,
    )
    .expect("valid graph");
    assert_eq!(graph.evaluate([0.0, 0.0], 0.0), Ok([0.0, 0.5, 1.0, 1.0]));
}

#[test]
fn rejects_forward_edges_and_type_mismatches() {
    assert_eq!(
        ShaderGraph::new(vec![ShaderNode::Sin(0)], 0),
        Err(ShaderGraphError::InvalidInput { node: 0, input: 0 })
    );
    assert_eq!(
        ShaderGraph::new(
            vec![
                ShaderNode::Constant(0.0),
                ShaderNode::Constant(0.0),
                ShaderNode::Constant(0.0),
                ShaderNode::Constant(1.0),
                ShaderNode::Color([0, 1, 2, 3]),
                ShaderNode::Sin(4),
            ],
            4
        ),
        Err(ShaderGraphError::ExpectedFloat { node: 5, input: 4 })
    );
    assert_eq!(
        ShaderGraph::new(
            vec![ShaderNode::Constant(0.0), ShaderNode::Channel(0, 0)],
            1
        ),
        Err(ShaderGraphError::ExpectedColor { node: 1, input: 0 })
    );
    assert_eq!(
        ShaderGraph::new(vec![ShaderNode::SourceColor, ShaderNode::Channel(0, 4)], 1),
        Err(ShaderGraphError::InvalidChannel {
            node: 1,
            channel: 4
        })
    );
}

#[test]
fn evaluates_coordinates_and_time_deterministically() {
    let graph = ShaderGraph::new(
        vec![
            ShaderNode::UvX,
            ShaderNode::UvY,
            ShaderNode::Time,
            ShaderNode::Color([0, 1, 2, 2]),
        ],
        3,
    )
    .expect("valid graph");
    assert_eq!(
        graph.evaluate([0.25, 0.75], 0.5),
        Ok([0.25, 0.75, 0.5, 0.5])
    );
    assert_eq!(
        graph.evaluate([0.25, 0.75], 0.5),
        Ok([0.25, 0.75, 0.5, 0.5])
    );
}

#[test]
fn evaluates_arithmetic_trigonometry_fraction_and_noise_nodes() {
    let graph = ShaderGraph::new(
        vec![
            ShaderNode::UvX,
            ShaderNode::UvY,
            ShaderNode::Time,
            ShaderNode::Add(0, 1),
            ShaderNode::Multiply(3, 2),
            ShaderNode::Sin(4),
            ShaderNode::Fract(3),
            ShaderNode::Noise2D(0, 1),
            ShaderNode::Color([5, 6, 7, 2]),
        ],
        8,
    )
    .expect("valid graph");
    let output = graph.evaluate([0.25, 0.5], 0.5).expect("evaluate");
    assert!((output[0] - 0.375_f32.sin()).abs() < 0.000_001);
    assert_eq!(output[1], 0.75);
    assert!((0.0..1.0).contains(&output[2]));
    assert_eq!(output[3], 0.5);
    assert_eq!(graph.evaluate([0.25, 0.5], 0.5), Ok(output));
}

#[test]
fn rejects_non_finite_data_and_graphs_over_the_limit() {
    assert_eq!(
        ShaderGraph::new(vec![ShaderNode::Constant(f32::NAN)], 0),
        Err(ShaderGraphError::NonFiniteConstant(0))
    );
    assert_eq!(
        constant_color().evaluate([f32::INFINITY, 0.0], 0.0),
        Err(ShaderGraphError::NonFiniteContext)
    );
    assert_eq!(
        ShaderGraph::new(vec![ShaderNode::UvX; MAX_SHADER_NODES + 1], 0),
        Err(ShaderGraphError::TooManyNodes)
    );
}
