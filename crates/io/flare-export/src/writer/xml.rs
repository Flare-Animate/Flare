//! Building XFL with quick-xml's writer, so escaping and well-formedness are the library's job.

use quick_xml::Writer;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, Event};

/// An attribute: its name and its value as text (already cleaned by [`a`]).
pub(super) type Attr = (&'static str, String);

/// An attribute with `value` as its text. Characters XML 1.0 cannot carry at all (controls, and
/// the noncharacters U+FFFE and U+FFFF) become spaces here; the writer escapes the rest.
pub(super) fn a(name: &'static str, value: impl ToString) -> Attr {
    (name, clean(&value.to_string()))
}

fn clean(text: &str) -> String {
    text.chars()
        .map(|c| {
            if (c as u32) < 0x20 || matches!(c, '\u{fffe}' | '\u{ffff}') {
                ' '
            } else {
                c
            }
        })
        .collect()
}

/// One XML document being written into memory.
pub(super) struct Xml {
    writer: Writer<Vec<u8>>,
}

impl Xml {
    /// A document that starts with the XML declaration.
    pub(super) fn new() -> Self {
        let mut xml = Self {
            writer: Writer::new(Vec::new()),
        };
        xml.event(Event::Decl(BytesDecl::new(
            "1.0",
            Some("UTF-8"),
            Some("no"),
        )));
        xml.raw("\n");
        xml
    }

    // Writing into a `Vec` cannot fail, so the results of the writer calls are not reported.
    fn event(&mut self, event: Event<'_>) {
        let _ = self.writer.write_event(event);
    }

    fn start(name: &str, attrs: impl IntoIterator<Item = Attr>) -> BytesStart<'_> {
        let mut start = BytesStart::new(name);
        for (key, value) in attrs {
            // A (&str, &str) pair is an attribute whose value the library escapes.
            start.push_attribute((key, value.as_str()));
        }
        start
    }

    pub(super) fn open(&mut self, name: &str, attrs: impl IntoIterator<Item = Attr>) {
        self.event(Event::Start(Self::start(name, attrs)));
    }

    pub(super) fn empty(&mut self, name: &str, attrs: impl IntoIterator<Item = Attr>) {
        self.event(Event::Empty(Self::start(name, attrs)));
    }

    pub(super) fn close(&mut self, name: &str) {
        self.event(Event::End(BytesEnd::new(name)));
    }

    /// Text that is already XML, written as it is: preserved source data, checked to be
    /// well-formed before it gets here, and line breaks.
    pub(super) fn raw(&mut self, xml: &str) {
        self.writer.get_mut().extend_from_slice(xml.as_bytes());
    }

    pub(super) fn finish(self) -> String {
        String::from_utf8_lossy(&self.writer.into_inner()).into_owned()
    }
}
