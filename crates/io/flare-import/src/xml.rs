//! A small, bounded XML tree. Input is untrusted, so depth and node count are capped and
//! DTDs are refused (which also rules out entity-expansion attacks).

use quick_xml::Reader;
use quick_xml::events::{BytesRef, BytesStart, Event};
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
    #[error("unknown entity &{0};")]
    UnknownEntity(String),
    #[error("the document has no root element")]
    Empty,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Node {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Node>,
    /// Character data. Whitespace-only text between child elements is formatting and is
    /// dropped; text in an element without children is kept exactly.
    pub text: String,
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
        text: String::new(),
    })
}

fn decode_reference(reference: &BytesRef<'_>) -> Result<char, XmlError> {
    if let Ok(Some(c)) = reference.resolve_char_ref() {
        return Ok(c);
    }
    match reference.xml10_content().as_ref() {
        "amp" => Ok('&'),
        "lt" => Ok('<'),
        "gt" => Ok('>'),
        "quot" => Ok('"'),
        "apos" => Ok('\''),
        other => Err(XmlError::UnknownEntity(other.to_owned())),
    }
}

impl Node {
    /// This element and everything under it as XML text that parses back to the same tree.
    pub fn to_xml(&self) -> String {
        let mut out = String::new();
        self.write_xml(&mut out);
        out
    }

    fn write_xml(&self, out: &mut String) {
        out.push('<');
        out.push_str(&self.name);
        for (key, value) in &self.attrs {
            out.push(' ');
            out.push_str(key);
            out.push_str("=\"");
            escape_into(out, value, true);
            out.push('"');
        }
        if self.text.is_empty() && self.children.is_empty() {
            out.push_str("/>");
            return;
        }
        out.push('>');
        escape_into(out, &self.text, false);
        for child in &self.children {
            child.write_xml(out);
        }
        out.push_str("</");
        out.push_str(&self.name);
        out.push('>');
    }
}

fn escape_into(out: &mut String, text: &str, attribute: bool) {
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' if attribute => out.push_str("&quot;"),
            c => out.push(c),
        }
    }
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
                let mut node = stack
                    .pop()
                    .ok_or_else(|| XmlError::Malformed("unbalanced end tag".into()))?;
                // Whitespace between child elements is formatting, not content.
                if !node.children.is_empty() && node.text.trim().is_empty() {
                    node.text.clear();
                }
                match stack.last_mut() {
                    Some(parent) => parent.children.push(node),
                    None if root.is_none() => root = Some(node),
                    None => return Err(XmlError::Malformed("more than one root element".into())),
                }
            }
            Event::Text(text) => {
                if let Some(top) = stack.last_mut() {
                    top.text.push_str(&text.xml10_content());
                }
            }
            Event::CData(data) => {
                if let Some(top) = stack.last_mut() {
                    top.text.push_str(&data.into_inner());
                }
            }
            Event::GeneralRef(reference) => {
                let decoded = decode_reference(&reference)?;
                if let Some(top) = stack.last_mut() {
                    top.text.push(decoded);
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

    #[test]
    fn text_is_kept_decoded_and_formatting_whitespace_is_dropped() {
        let xml = br#"<a>
          <b>Hello &amp; &lt;world&gt; &#65;&#x42;</b>
          <c>  padded  </c>
          <d><![CDATA[1 < 2 && 3 > 2]]></d>
          <e/>
        </a>"#;
        let root = parse(xml).expect("parse");
        assert_eq!(root.text, "", "whitespace between children is formatting");
        assert_eq!(root.children[0].text, "Hello & <world> AB");
        assert_eq!(root.children[1].text, "  padded  ", "leaf text is exact");
        assert_eq!(root.children[2].text, "1 < 2 && 3 > 2");
        assert_eq!(root.children[3].text, "");
        assert_eq!(
            parse(b"<a>&bogus;</a>"),
            Err(XmlError::UnknownEntity("bogus".into()))
        );
    }

    #[test]
    fn serializing_and_parsing_again_is_a_fixed_point() {
        let corpus: [&str; 5] = [
            r#"<a x="1" y="a &amp; b &lt; c &gt; &quot;q&quot;"><b>text &amp; more</b><c/></a>"#,
            r#"<root><DOMStaticText><textRuns><DOMTextRun><characters>  spaced &lt;out&gt;  </characters></DOMTextRun></textRuns></DOMStaticText></root>"#,
            r#"<e a="é ü 日本語"/>"#,
            r#"<a><![CDATA[<raw & text>]]></a>"#,
            r#"<a><b><c><d k="v">deep</d></c></b></a>"#,
        ];
        for source in corpus {
            let tree = parse(source.as_bytes()).expect("parse");
            let again = parse(tree.to_xml().as_bytes()).expect("re-parse");
            assert_eq!(again, tree, "{source}");
            assert_eq!(again.to_xml(), tree.to_xml(), "serialization is stable");
        }
    }
}
