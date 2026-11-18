//! A graph, and the kinds of node, as JSON for the command line and the MCP server.

use flare_fx::{EffectGraph, EffectKind, NodeId, ParamKind, Unit, Value};
use serde_json::{Map, Value as Json, json};

fn color_text(c: flare_core::Rgba8) -> String {
    format!("#{:02X}{:02X}{:02X}{:02X}", c.r, c.g, c.b, c.a)
}

fn unit_name(unit: Unit) -> &'static str {
    match unit {
        Unit::Length => "stage units",
        Unit::Speed => "stage units per frame",
        Unit::Angle => "degrees",
        Unit::Percent => "percent",
        Unit::Count => "count",
    }
}

/// The graph's nodes, each with its kind, inputs by name, settings and place in the view, and
/// whether the graph changes the picture and moves on its own. `null` for a layer with none.
pub fn describe(graph: Option<&EffectGraph>) -> Json {
    let Some(graph) = graph else {
        return Json::Null;
    };
    let nodes: Vec<Json> = graph
        .nodes()
        .iter()
        .map(|node| {
            let inputs: Vec<Json> = node
                .kind()
                .inputs()
                .iter()
                .zip(node.inputs())
                .map(|(name, from)| json!({ "name": name, "from": from.map(NodeId::get) }))
                .collect();
            let mut settings = Map::new();
            for (spec, value) in node.kind().params().iter().zip(node.values()) {
                let value = match value {
                    Value::Number(n) => json!(n),
                    Value::Color(c) => json!(color_text(*c)),
                };
                settings.insert(spec.name.to_owned(), value);
            }
            json!({
                "id": node.id().get(),
                "kind": node.kind().name(),
                "inputs": inputs,
                "settings": settings,
                "at": node.at(),
            })
        })
        .collect();
    json!({
        "nodes": nodes,
        "changes_picture": !graph.is_identity(),
        "animated": graph.is_animated(),
        "order": graph.order().iter().map(|id| id.get()).collect::<Vec<_>>(),
    })
}

/// Every kind of node that can be added, with its inputs and its settings' ranges and defaults.
pub fn kinds() -> Json {
    let kinds: Vec<Json> = EffectKind::ADDABLE
        .iter()
        .map(|kind| {
            let settings: Vec<Json> = kind
                .params()
                .iter()
                .map(|spec| match spec.kind {
                    ParamKind::Number {
                        min,
                        max,
                        default,
                        unit,
                    } => json!({
                        "name": spec.name,
                        "type": "number",
                        "min": min,
                        "max": max,
                        "default": default,
                        "unit": unit_name(unit),
                    }),
                    ParamKind::Color { default } => json!({
                        "name": spec.name,
                        "type": "color",
                        "default": color_text(default),
                    }),
                })
                .collect();
            json!({
                "kind": kind.name(),
                "inputs": kind.inputs(),
                "animated": kind.is_animated(),
                "settings": settings,
            })
        })
        .collect();
    Json::Array(kinds)
}
