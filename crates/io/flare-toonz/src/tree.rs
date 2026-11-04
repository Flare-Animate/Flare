//! A bounded element tree read from XML. Every bound is checked as the file is read, so a
//! hostile file is refused before it can exhaust memory.

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use crate::error::ToonzError;
use crate::limits::{MAX_DEPTH, MAX_INPUT_BYTES, MAX_NODE_TEXT, MAX_NODES};

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Node {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub text: String,
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

    pub fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Node> + 'a {
        self.children.iter().filter(move |c| c.name == name)
    }
}

/// Parses `input` and returns its root element.
pub fn parse(input: &str) -> Result<Node, ToonzError> {
    if input.len() > MAX_INPUT_BYTES {
        return Err(ToonzError::Limit {
            what: "file size",
            max: MAX_INPUT_BYTES,
        });
    }
    let mut reader = Reader::from_str(input);
    reader.config_mut().trim_text(true);
    // Index 0 is a synthetic document node that holds the root element.
    let mut stack = vec![Node::default()];
    let mut count = 0usize;
    loop {
        match reader.read_event()? {
            Event::Start(start) => open(&mut stack, &mut count, start_node(&start)?)?,
            Event::Empty(start) => {
                open(&mut stack, &mut count, start_node(&start)?)?;
                close(&mut stack)?;
            }
            Event::End(_) => close(&mut stack)?,
            Event::Text(text) => {
                let raw: &str = &text;
                let value = quick_xml::escape::unescape(raw).map_err(malformed)?;
                append_text(&mut stack, &value)?;
            }
            Event::CData(data) => {
                let value: &str = &data;
                append_text(&mut stack, value)?;
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if stack.len() != 1 {
        return Err(ToonzError::malformed(
            "xml",
            "element left open at end of file",
        ));
    }
    let document = stack.pop().unwrap_or_default();
    document
        .children
        .into_iter()
        .next()
        .ok_or(ToonzError::UnknownFormat("no root element"))
}

fn malformed(err: impl std::fmt::Display) -> ToonzError {
    ToonzError::malformed("xml", err.to_string())
}

fn start_node(start: &BytesStart<'_>) -> Result<Node, ToonzError> {
    let name = start.name().as_ref().to_owned();
    let mut attrs = Vec::new();
    for attr in start.attributes() {
        let attr = attr.map_err(malformed)?;
        let key = attr.key.as_ref().to_owned();
        let raw: &str = &attr.value;
        let value = quick_xml::escape::unescape(raw)
            .map_err(malformed)?
            .into_owned();
        attrs.push((key, value));
    }
    Ok(Node {
        name,
        attrs,
        text: String::new(),
        children: Vec::new(),
    })
}

fn open(stack: &mut Vec<Node>, count: &mut usize, node: Node) -> Result<(), ToonzError> {
    if stack.len() > MAX_DEPTH {
        return Err(ToonzError::Limit {
            what: "element depth",
            max: MAX_DEPTH,
        });
    }
    *count += 1;
    if *count > MAX_NODES {
        return Err(ToonzError::Limit {
            what: "element count",
            max: MAX_NODES,
        });
    }
    stack.push(node);
    Ok(())
}

fn close(stack: &mut Vec<Node>) -> Result<(), ToonzError> {
    if stack.len() < 2 {
        return Err(ToonzError::malformed("xml", "end tag without a start tag"));
    }
    let Some(node) = stack.pop() else {
        return Err(ToonzError::malformed("xml", "end tag without a start tag"));
    };
    if let Some(parent) = stack.last_mut() {
        parent.children.push(node);
    }
    Ok(())
}

fn append_text(stack: &mut [Node], value: &str) -> Result<(), ToonzError> {
    let Some(top) = stack.last_mut() else {
        return Ok(());
    };
    if top.text.len() + value.len() > MAX_NODE_TEXT {
        return Err(ToonzError::Limit {
            what: "text in one element",
            max: MAX_NODE_TEXT,
        });
    }
    top.text.push_str(value);
    Ok(())
}
