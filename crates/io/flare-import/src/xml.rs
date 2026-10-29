//! A small, bounded XML tree. Input is untrusted, so depth and node count are capped and
//! DTDs are refused (which also rules out entity-expansion attacks).

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use thiserror::Error;

pub const MAX_DEPTH: usize = 64;
pub const MAX_NODES: usize = 500_000;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum XmlError {
    #[error("malformed XML: {0}")]
    Malformed(String),
    #[error("XML nesting deeper than {MAX_DEPTH}")]
    TooDeep,
    #[error("XML has more than {MAX_NODES} elements")]
    TooLarge,
    #[error("DTDs are not allowed")]
    DocType,
    #[error("the document has no root element")]
    Empty,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Node {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Node>,
}

impl Node {
    pub fn attr(&self, key: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    pub fn child(&self, name: &str) -> Option<&Node> {
        self.children.iter().find(|c| c.name == name)
    }

    pub fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Node> {
        self.children.iter().filter(move |c| c.name == name)
    }
}

fn node_from(start: &BytesStart<'_>) -> Result<Node, XmlError> {
    let name = start.name().as_ref().to_owned();
    let mut attrs = Vec::new();
    for attr in start.attributes() {
        let attr = attr.map_err(|e| XmlError::Malformed(e.to_string()))?;
        let key = attr.key.as_ref().to_owned();
        let value = attr
            .normalized_value(quick_xml::XmlVersion::Implicit1_0)
            .map_err(|e| XmlError::Malformed(e.to_string()))?;
        attrs.push((key, value.into_owned()));
    }
    Ok(Node {
        name,
        attrs,
        children: Vec::new(),
    })
}

/// Parses `bytes` into a tree of elements and attributes. Text content is ignored: the
/// formats read here carry everything in attributes.
pub fn parse(bytes: &[u8]) -> Result<Node, XmlError> {
    let mut reader = Reader::from_reader(bytes);
    let mut buffer = Vec::new();
    let mut stack: Vec<Node> = Vec::new();
    let mut root: Option<Node> = None;
    let mut count = 0usize;
    loop {
        let event = reader
            .read_event_into(&mut buffer)
            .map_err(|e| XmlError::Malformed(e.to_string()))?;
        match event {
            Event::Start(start) => {
                count += 1;
                if count > MAX_NODES {
                    return Err(XmlError::TooLarge);
                }
                if stack.len() >= MAX_DEPTH {
                    return Err(XmlError::TooDeep);
                }
                stack.push(node_from(&start)?);
            }
            Event::Empty(start) => {
                count += 1;
                if count > MAX_NODES {
                    return Err(XmlError::TooLarge);
                }
                let node = node_from(&start)?;
                match stack.last_mut() {
                    Some(parent) => parent.children.push(node),
                    None if root.is_none() => root = Some(node),
                    None => return Err(XmlError::Malformed("more than one root element".into())),
                }
            }
            Event::End(_) => {
                let node = stack
                    .pop()
                    .ok_or_else(|| XmlError::Malformed("unbalanced end tag".into()))?;
                match stack.last_mut() {
                    Some(parent) => parent.children.push(node),
                    None if root.is_none() => root = Some(node),
                    None => return Err(XmlError::Malformed("more than one root element".into())),
                }
            }
            Event::DocType(_) => return Err(XmlError::DocType),
            Event::Eof => break,
            _ => {}
        }
        buffer.clear();
    }
    if !stack.is_empty() {
        return Err(XmlError::Malformed("unclosed element".into()));
    }
    root.ok_or(XmlError::Empty)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn builds_a_tree_of_elements_and_attributes() {
        let xml = br#"<?xml version="1.0"?><a x="1" y="&amp;"><b/><c k="v"><d/></c></a>"#;
        let root = parse(xml).expect("parse");
        assert_eq!(root.name, "a");
        assert_eq!((root.attr("x"), root.attr("y")), (Some("1"), Some("&")));
        assert_eq!(root.children.len(), 2);
        assert_eq!(root.child("c").and_then(|c| c.attr("k")), Some("v"));
        assert_eq!(root.child("c").map(|c| c.children.len()), Some(1));
        assert_eq!(root.children_named("b").count(), 1);
        assert!(root.attr("missing").is_none());
    }

    #[test]
    fn hostile_or_broken_input_is_rejected_not_expanded() {
        assert_eq!(parse(b""), Err(XmlError::Empty));
        assert!(matches!(parse(b"<a><b></a>"), Err(XmlError::Malformed(_))));
        assert!(matches!(parse(b"<a>"), Err(XmlError::Malformed(_))));
        assert!(matches!(parse(b"<a/><b/>"), Err(XmlError::Malformed(_))));
        let bomb = br#"<!DOCTYPE a [<!ENTITY x "xxxxxxxxxx">]><a v="&x;"/>"#;
        assert_eq!(parse(bomb), Err(XmlError::DocType));
        let deep = "<a>".repeat(MAX_DEPTH + 1);
        assert_eq!(parse(deep.as_bytes()), Err(XmlError::TooDeep));
    }

    #[test]
    fn node_count_is_capped() {
        let mut xml = String::from("<r>");
        xml.push_str(&"<n/>".repeat(MAX_NODES));
        xml.push_str("</r>");
        assert_eq!(parse(xml.as_bytes()), Err(XmlError::TooLarge));
    }
}
