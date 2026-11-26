//! A small XML tree for documents read, changed here and there, and written back: a DASH manifest.

use quick_xml::XmlVersion;
use quick_xml::escape::resolve_predefined_entity;
use quick_xml::events::{BytesStart, BytesText, Event};

#[derive(Clone, Debug)]
pub(crate) enum Node {
    Element(Element),
    Other(Event<'static>),
}

#[derive(Clone, Debug)]
pub(crate) struct Element {
    start: BytesStart<'static>,
    pub children: Vec<Node>,
}

impl Element {
    pub fn new(name: &str) -> Element {
        Element { start: BytesStart::new(name.to_string()), children: Vec::new() }
    }

    /// The name without its namespace prefix.
    pub fn name(&self) -> String {
        self.start.local_name().as_ref().to_string()
    }

    pub fn qualified_name(&self) -> String {
        self.start.name().as_ref().to_string()
    }

    pub fn attr(&self, name: &str) -> Option<String> {
        self.start.attributes().flatten().find(|a| a.key.local_name().as_ref() == name).and_then(|a| a.normalized_value(XmlVersion::Implicit1_0).ok().map(|v| v.into_owned()))
    }

    fn attrs(&self) -> Vec<(String, String)> {
        self.start
            .attributes()
            .flatten()
            .filter_map(|a| Some((a.key.as_ref().to_string(), a.normalized_value(XmlVersion::Implicit1_0).ok()?.into_owned())))
            .collect()
    }

    pub fn set_attr(&mut self, name: &str, value: &str) {
        let mut attrs = self.attrs();
        match attrs.iter_mut().find(|(k, _)| k == name) {
            Some(attr) => attr.1 = value.to_string(),
            None => attrs.push((name.to_string(), value.to_string())),
        }
        self.rebuild(attrs);
    }

    fn rebuild(&mut self, attrs: Vec<(String, String)>) {
        let mut start = BytesStart::new(self.qualified_name());
        for (k, v) in &attrs {
            start.push_attribute((k.as_str(), v.as_str()));
        }
        self.start = start.into_owned();
    }

    pub fn elements(&self) -> impl Iterator<Item = &Element> {
        self.children.iter().filter_map(|n| match n {
            Node::Element(e) => Some(e),
            Node::Other(_) => None,
        })
    }

    pub fn elements_mut(&mut self) -> impl Iterator<Item = &mut Element> {
        self.children.iter_mut().filter_map(|n| match n {
            Node::Element(e) => Some(e),
            Node::Other(_) => None,
        })
    }

    pub fn child(&self, name: &str) -> Option<&Element> {
        self.elements().find(|e| e.name() == name)
    }

    /// The text it holds, entities resolved.
    pub fn text(&self) -> String {
        let mut text = String::new();
        for node in &self.children {
            match node {
                Node::Other(Event::Text(t)) => text.push_str(&t.xml10_content()),
                Node::Other(Event::CData(t)) => text.push_str(&t.xml10_content()),
                Node::Other(Event::GeneralRef(r)) => {
                    let resolved = if r.is_char_ref() { r.resolve_char_ref().ok().flatten().map(String::from) } else { resolve_predefined_entity(&r.xml10_content()).map(String::from) };
                    text.push_str(&resolved.unwrap_or_default());
                }
                _ => {}
            }
        }
        text
    }

    pub fn set_text(&mut self, text: &str) {
        self.children = vec![Node::Other(Event::Text(BytesText::new(text).into_owned()))];
    }

    /// Drops the child elements named `name`.
    pub fn remove(&mut self, name: &str) {
        self.children.retain(|n| !matches!(n, Node::Element(e) if e.name() == name));
    }

    pub fn retain(&mut self, mut keep: impl FnMut(&Element) -> bool) {
        self.children.retain(|n| match n {
            Node::Element(e) => keep(e),
            Node::Other(_) => true,
        });
    }

    pub fn prepend(&mut self, element: Element) {
        self.children.insert(0, Node::Element(element));
    }

    pub fn append(&mut self, element: Element) {
        self.children.push(Node::Element(element));
    }
}

pub(crate) fn parse(text: &str) -> Option<Vec<Node>> {
    let mut reader = quick_xml::Reader::from_str(text);
    let mut stack: Vec<Element> = Vec::new();
    let mut top = Vec::new();
    loop {
        let event = reader.read_event().ok()?;
        let node = match event {
            Event::Eof => break,
            Event::Start(start) => {
                stack.push(Element { start: start.into_owned(), children: Vec::new() });
                continue;
            }
            Event::End(_) => Node::Element(stack.pop()?),
            Event::Empty(start) => Node::Element(Element { start: start.into_owned(), children: Vec::new() }),
            other => Node::Other(other.into_owned()),
        };
        match stack.last_mut() {
            Some(parent) => parent.children.push(node),
            None => top.push(node),
        }
    }
    stack.is_empty().then_some(top)
}

pub(crate) fn root_mut(nodes: &mut [Node]) -> Option<&mut Element> {
    nodes.iter_mut().find_map(|n| match n {
        Node::Element(e) => Some(e),
        Node::Other(_) => None,
    })
}

pub(crate) fn write(nodes: &[Node]) -> String {
    let mut writer = quick_xml::Writer::new(Vec::new());
    for node in nodes {
        write_node(&mut writer, node);
    }
    String::from_utf8(writer.into_inner()).unwrap_or_default()
}

fn write_node(writer: &mut quick_xml::Writer<Vec<u8>>, node: &Node) {
    let _ = match node {
        Node::Other(event) => writer.write_event(event.borrow()),
        Node::Element(e) if e.children.is_empty() => writer.write_event(Event::Empty(e.start.borrow())),
        Node::Element(e) => {
            let _ = writer.write_event(Event::Start(e.start.borrow()));
            for child in &e.children {
                write_node(writer, child);
            }
            writer.write_event(Event::End(e.start.to_end()))
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn documents_come_back_as_they_were_but_for_what_changed() {
        let text = r#"<?xml version="1.0"?><!-- c --><MPD xmlns:cenc="urn:mpeg:cenc:2013" a="1"><BaseURL>http://x.test/?a=1&amp;b=2</BaseURL><cenc:pssh>QUJD</cenc:pssh><P/></MPD>"#;
        let mut nodes = parse(text).unwrap();
        assert_eq!(write(&nodes), text);
        let root = root_mut(&mut nodes).unwrap();
        assert_eq!((root.name(), root.attr("a").as_deref()), ("MPD".into(), Some("1")));
        assert_eq!(root.child("BaseURL").unwrap().text(), "http://x.test/?a=1&b=2");
        assert_eq!(root.child("pssh").unwrap().qualified_name(), "cenc:pssh");
        root.set_attr("a", "x&y");
        root.remove("pssh");
        root.elements_mut().find(|e| e.name() == "BaseURL").unwrap().set_text("http://127.0.0.1/?q=<1>");
        assert_eq!(write(&nodes), r#"<?xml version="1.0"?><!-- c --><MPD xmlns:cenc="urn:mpeg:cenc:2013" a="x&amp;y"><BaseURL>http://127.0.0.1/?q=&lt;1&gt;</BaseURL><P/></MPD>"#);
        assert!(parse("<a><b></a>").is_none());
    }
}
