//! One node of a graph: its kind, where its inputs come from, its settings and its place in the
//! graph view.

use flare_core::Rgba8;

use crate::{EffectKind, FxError, ParamKind, ParamSpec};

/// A node's number in its graph. The Layer node is always 1 and the Output always 2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(u32);

impl NodeId {
    pub const LAYER: Self = Self(1);
    pub const OUTPUT: Self = Self(2);

    /// A node number; 0 is none.
    pub fn new(raw: u32) -> Option<Self> {
        (raw > 0).then_some(Self(raw))
    }

    pub fn get(self) -> u32 {
        self.0
    }
}

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// A setting's value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Value {
    Number(f32),
    Color(Rgba8),
}

impl Value {
    /// The value `spec` starts at.
    pub fn default_for(spec: &ParamSpec) -> Self {
        match spec.kind {
            ParamKind::Number { default, .. } => Self::Number(default),
            ParamKind::Color { default } => Self::Color(default),
        }
    }

    /// The value if `spec` accepts it: the right kind, finite and in range. A count is rounded.
    pub fn checked(self, spec: &ParamSpec) -> Result<Self, FxError> {
        match (self, spec.kind) {
            (Self::Number(value), ParamKind::Number { min, max, unit, .. }) => {
                if !value.is_finite() || value < min || value > max {
                    return Err(FxError::OutOfRange {
                        name: spec.name,
                        min,
                        max,
                        value,
                    });
                }
                Ok(Self::Number(if unit == crate::Unit::Count {
                    value.round()
                } else {
                    value
                }))
            }
            (Self::Color(color), ParamKind::Color { .. }) => Ok(Self::Color(color)),
            (Self::Number(value), ParamKind::Color { .. }) => Err(FxError::BadColor {
                name: spec.name,
                value: value.to_string(),
            }),
            (Self::Color(_), ParamKind::Number { .. }) => Err(FxError::NotANumber {
                name: spec.name,
                value: "a color".to_owned(),
            }),
        }
    }
}

/// One node. Built freely; a graph checks its nodes when it is made from them.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    id: NodeId,
    kind: EffectKind,
    inputs: Vec<Option<NodeId>>,
    values: Vec<Value>,
    at: [f32; 2],
}

impl Node {
    /// A node of `kind` with its default settings and nothing connected, at `at` in the view.
    pub fn new(id: NodeId, kind: EffectKind, at: [f32; 2]) -> Self {
        Self {
            id,
            kind,
            inputs: vec![None; kind.inputs().len()],
            values: kind.params().iter().map(Value::default_for).collect(),
            at,
        }
    }

    /// A node from stored parts, checked only when a graph is made from it.
    pub fn from_parts(
        id: NodeId,
        kind: EffectKind,
        inputs: Vec<Option<NodeId>>,
        values: Vec<Value>,
        at: [f32; 2],
    ) -> Self {
        Self {
            id,
            kind,
            inputs,
            values,
            at,
        }
    }

    pub fn id(&self) -> NodeId {
        self.id
    }

    pub fn kind(&self) -> EffectKind {
        self.kind
    }

    /// Where each input comes from, in the order of [`EffectKind::inputs`].
    pub fn inputs(&self) -> &[Option<NodeId>] {
        &self.inputs
    }

    /// The settings, in the order of [`EffectKind::params`].
    pub fn values(&self) -> &[Value] {
        &self.values
    }

    /// The node's place in the graph view, in the view's own units.
    pub fn at(&self) -> [f32; 2] {
        self.at
    }

    /// The setting named `name`.
    pub fn value(&self, name: &str) -> Option<Value> {
        let (index, _) = self.kind.param(name)?;
        self.values.get(index).copied()
    }

    /// The number setting at `index`, 0 when it is missing or a color (a checked graph never
    /// has either).
    pub(crate) fn number(&self, index: usize) -> f32 {
        match self.values.get(index) {
            Some(Value::Number(value)) => *value,
            _ => 0.0,
        }
    }

    /// The color setting at `index`.
    pub(crate) fn color(&self, index: usize) -> Rgba8 {
        match self.values.get(index) {
            Some(Value::Color(color)) => *color,
            _ => Rgba8::TRANSPARENT,
        }
    }

    /// Checks the node on its own: inputs and settings that fit its kind, a finite place.
    pub(crate) fn check(&self) -> Result<(), FxError> {
        let ports = self.kind.inputs().len();
        if self.inputs.len() != ports {
            return Err(FxError::WrongInputs {
                node: self.id.0,
                kind: self.kind.name(),
                expected: ports,
                got: self.inputs.len(),
            });
        }
        let specs = self.kind.params();
        if self.values.len() != specs.len() {
            return Err(FxError::WrongValues(self.id.0));
        }
        for (value, spec) in self.values.iter().zip(specs) {
            if value.checked(spec)? != *value {
                return Err(FxError::WrongValues(self.id.0));
            }
        }
        if !self
            .at
            .iter()
            .all(|c| c.is_finite() && c.abs() <= 100_000.0)
        {
            return Err(FxError::BadPosition);
        }
        Ok(())
    }

    pub(crate) fn set_input(&mut self, port: usize, from: Option<NodeId>) {
        if let Some(slot) = self.inputs.get_mut(port) {
            *slot = from;
        }
    }

    pub(crate) fn set_value(&mut self, index: usize, value: Value) {
        if let Some(slot) = self.values.get_mut(index) {
            *slot = value;
        }
    }

    pub(crate) fn set_at(&mut self, at: [f32; 2]) {
        self.at = at;
    }
}
