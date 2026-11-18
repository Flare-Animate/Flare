//! A bounded, typed, backend-independent shader graph.
//!
//! Graph nodes are evaluated in declaration order. This makes cycles impossible and keeps
//! evaluation deterministic on the CPU, including in the browser build.

use serde::{Deserialize, Serialize};

/// Maximum number of nodes accepted in one graph.
pub const MAX_SHADER_NODES: usize = 256;

/// An input to a shader node, identified by its zero-based position in the graph.
pub type ShaderInput = usize;

/// One operation in a shader graph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", content = "args", rename_all = "snake_case")]
pub enum ShaderNode {
    Constant(f32),
    UvX,
    UvY,
    Time,
    SourceColor,
    Channel(ShaderInput, u8),
    Add(ShaderInput, ShaderInput),
    Multiply(ShaderInput, ShaderInput),
    Sin(ShaderInput),
    Fract(ShaderInput),
    Noise2D(ShaderInput, ShaderInput),
    Color([ShaderInput; 4]),
}

/// Why a graph could not be constructed or evaluated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ShaderGraphError {
    #[error("shader graph has no nodes")]
    Empty,
    #[error("shader graph exceeds the {MAX_SHADER_NODES}-node limit")]
    TooManyNodes,
    #[error("shader node {node} refers to input {input}, which must be an earlier node")]
    InvalidInput { node: usize, input: usize },
    #[error("shader node {node} requires a float input, but node {input} produces a color")]
    ExpectedFloat { node: usize, input: usize },
    #[error("shader node {node} requires a color input, but node {input} produces a float")]
    ExpectedColor { node: usize, input: usize },
    #[error("shader output node {0} does not produce a color")]
    OutputIsNotColor(usize),
    #[error("shader channel node {node} selects invalid channel {channel}")]
    InvalidChannel { node: usize, channel: u8 },
    #[error("shader constant at node {0} is not finite")]
    NonFiniteConstant(usize),
    #[error("shader node {0} produced a non-finite value")]
    NonFiniteResult(usize),
    #[error("shader evaluation input is not finite")]
    NonFiniteContext,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Value {
    Float(f32),
    Color([f32; 4]),
}

/// A checked graph with a designated color output.
#[derive(Debug, Clone, PartialEq)]
pub struct ShaderGraph {
    nodes: Vec<ShaderNode>,
    output: ShaderInput,
}

impl ShaderGraph {
    /// Validates node references and types before retaining the graph.
    pub fn new(nodes: Vec<ShaderNode>, output: ShaderInput) -> Result<Self, ShaderGraphError> {
        if nodes.is_empty() {
            return Err(ShaderGraphError::Empty);
        }
        if nodes.len() > MAX_SHADER_NODES {
            return Err(ShaderGraphError::TooManyNodes);
        }

        let mut color_nodes: Vec<bool> = Vec::with_capacity(nodes.len());
        for (index, node) in nodes.iter().enumerate() {
            if matches!(node, ShaderNode::Constant(value) if !value.is_finite()) {
                return Err(ShaderGraphError::NonFiniteConstant(index));
            }
            let (inputs, input_count) = match node {
                ShaderNode::Constant(_)
                | ShaderNode::UvX
                | ShaderNode::UvY
                | ShaderNode::Time
                | ShaderNode::SourceColor => ([None; 4], 0),
                ShaderNode::Add(a, b) | ShaderNode::Multiply(a, b) | ShaderNode::Noise2D(a, b) => {
                    ([Some(*a), Some(*b), None, None], 2)
                }
                ShaderNode::Sin(input) | ShaderNode::Fract(input) => {
                    ([Some(*input), None, None, None], 1)
                }
                ShaderNode::Channel(input, channel) => {
                    if *channel > 3 {
                        return Err(ShaderGraphError::InvalidChannel {
                            node: index,
                            channel: *channel,
                        });
                    }
                    ([Some(*input), None, None, None], 1)
                }
                ShaderNode::Color(inputs) => (inputs.map(Some), 4),
            };
            for input in inputs.into_iter().flatten().take(input_count) {
                if input >= index {
                    return Err(ShaderGraphError::InvalidInput { node: index, input });
                }
                let needs_color = matches!(node, ShaderNode::Channel(_, _));
                if needs_color && !color_nodes[input] {
                    return Err(ShaderGraphError::ExpectedColor { node: index, input });
                }
                if !needs_color && color_nodes[input] {
                    return Err(ShaderGraphError::ExpectedFloat { node: index, input });
                }
            }
            color_nodes.push(matches!(
                node,
                ShaderNode::Color(_) | ShaderNode::SourceColor
            ));
        }
        if output >= nodes.len() {
            return Err(ShaderGraphError::InvalidInput {
                node: nodes.len(),
                input: output,
            });
        }
        if !color_nodes[output] {
            return Err(ShaderGraphError::OutputIsNotColor(output));
        }
        Ok(Self { nodes, output })
    }

    pub fn nodes(&self) -> &[ShaderNode] {
        &self.nodes
    }

    pub fn output(&self) -> ShaderInput {
        self.output
    }

    /// Evaluates a graph at normalized image coordinates and a time in seconds.
    pub fn evaluate(&self, uv: [f32; 2], time: f32) -> Result<[f32; 4], ShaderGraphError> {
        self.evaluate_with_source(uv, time, [0.0; 4])
    }

    /// Evaluates a graph with one straight-alpha source pixel, normalized to 0..=1.
    pub fn evaluate_with_source(
        &self,
        uv: [f32; 2],
        time: f32,
        source: [f32; 4],
    ) -> Result<[f32; 4], ShaderGraphError> {
        if !uv.into_iter().all(f32::is_finite)
            || !time.is_finite()
            || !source.into_iter().all(f32::is_finite)
        {
            return Err(ShaderGraphError::NonFiniteContext);
        }
        let mut values = [Value::Float(0.0); MAX_SHADER_NODES];
        for (index, node) in self.nodes.iter().enumerate() {
            let float = |input: usize| match values.get(input) {
                Some(Value::Float(value)) => Ok(*value),
                _ => Err(ShaderGraphError::ExpectedFloat { node: index, input }),
            };
            let value = match node {
                ShaderNode::Constant(value) => Value::Float(*value),
                ShaderNode::UvX => Value::Float(uv[0]),
                ShaderNode::UvY => Value::Float(uv[1]),
                ShaderNode::Time => Value::Float(time),
                ShaderNode::SourceColor => Value::Color(source),
                ShaderNode::Channel(input, channel) => match values.get(*input) {
                    Some(Value::Color(color)) => Value::Float(color[usize::from(*channel)]),
                    _ => {
                        return Err(ShaderGraphError::ExpectedColor {
                            node: index,
                            input: *input,
                        });
                    }
                },
                ShaderNode::Add(a, b) => Value::Float(float(*a)? + float(*b)?),
                ShaderNode::Multiply(a, b) => Value::Float(float(*a)? * float(*b)?),
                ShaderNode::Sin(input) => Value::Float(float(*input)?.sin()),
                ShaderNode::Fract(input) => {
                    let value = float(*input)?;
                    Value::Float(value - value.floor())
                }
                ShaderNode::Noise2D(x, y) => Value::Float(noise(float(*x)?, float(*y)?)),
                ShaderNode::Color(channels) => Value::Color([
                    float(channels[0])?,
                    float(channels[1])?,
                    float(channels[2])?,
                    float(channels[3])?,
                ]),
            };
            let finite = match value {
                Value::Float(value) => value.is_finite(),
                Value::Color(color) => color.into_iter().all(f32::is_finite),
            };
            if !finite {
                return Err(ShaderGraphError::NonFiniteResult(index));
            }
            values[index] = value;
        }
        match values[self.output] {
            Value::Color(color) => Ok(color.map(|channel| channel.clamp(0.0, 1.0))),
            Value::Float(_) => Err(ShaderGraphError::OutputIsNotColor(self.output)),
        }
    }
}

fn noise(x: f32, y: f32) -> f32 {
    let value = (x.mul_add(127.1, y * 311.7)).sin() * 43_758.547;
    value - value.floor()
}

#[cfg(test)]
#[path = "shader_tests.rs"]
mod tests;
