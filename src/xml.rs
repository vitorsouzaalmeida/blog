//! XML 1.0 (Fifth Edition) <https://www.w3.org/TR/xml/>
//! Deliberately missing: DTDs, entity declarations, namespaces, and processing
//! instructions.

use std::borrow::Cow;
use std::fmt::{self, Write};

fn is_char(c: char) -> bool {
    matches!(c,
        '\u{9}' | '\u{A}' | '\u{D}'
        | '\u{20}'..='\u{D7FF}'
        | '\u{E000}'..='\u{FFFD}'
        | '\u{10000}'..='\u{10FFFF}')
}

pub struct Text<'a>(&'a str);

pub struct Attr<'a>(&'a str);

pub struct CData<'a>(&'a str);

pub fn text(s: &str) -> Text<'_> {
    Text(s)
}

pub fn attr(s: &str) -> Attr<'_> {
    Attr(s)
}

pub fn cdata(s: &str) -> CData<'_> {
    CData(s)
}

fn chars(s: &str) -> impl Iterator<Item = char> + '_ {
    s.chars().filter(|c| is_char(*c))
}

impl fmt::Display for Text<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        chars(self.0).try_for_each(|c| match c {
            '&' => f.write_str("&amp;"),
            '<' => f.write_str("&lt;"),
            '>' => f.write_str("&gt;"),
            '\r' => f.write_str("&#xD;"),
            c => f.write_char(c),
        })
    }
}

impl fmt::Display for Attr<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        chars(self.0).try_for_each(|c| match c {
            '&' => f.write_str("&amp;"),
            '<' => f.write_str("&lt;"),
            '>' => f.write_str("&gt;"),
            '"' => f.write_str("&quot;"),
            '\t' => f.write_str("&#x9;"),
            '\n' => f.write_str("&#xA;"),
            '\r' => f.write_str("&#xD;"),
            c => f.write_char(c),
        })
    }
}

impl fmt::Display for CData<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let filtered: Cow<str> = match self.0.chars().all(is_char) {
            true => Cow::Borrowed(self.0),
            false => Cow::Owned(chars(self.0).collect()),
        };
        f.write_str("<![CDATA[")?;
        filtered
            .split("]]>")
            .enumerate()
            .try_for_each(|(i, part)| {
                match i {
                    0 => Ok(()),
                    _ => f.write_str("]]]]><![CDATA[>"),
                }?;
                f.write_str(part)
            })?;
        f.write_str("]]>")
    }
}

fn is_name(s: &str) -> bool {
    let start = |c: char| c.is_ascii_alphabetic() || c == '_' || c == ':' || !c.is_ascii();
    let rest = |c: char| start(c) || c.is_ascii_digit() || c == '-' || c == '.';
    s.starts_with(start) && s.chars().all(rest)
}

pub enum Node<'a> {
    Elem {
        name: &'a str,
        attrs: Vec<(&'a str, Cow<'a, str>)>,
        children: Vec<Node<'a>>,
    },
    Text(Cow<'a, str>),
    CData(Cow<'a, str>),
}

impl<'a> Node<'a> {
    pub fn elem(
        name: &'a str,
        attrs: impl IntoIterator<Item = (&'a str, Cow<'a, str>)>,
        children: impl IntoIterator<Item = Node<'a>>,
    ) -> Node<'a> {
        debug_assert!(is_name(name), "not an XML Name: {name:?}");
        Node::Elem {
            name,
            attrs: attrs.into_iter().collect(),
            children: children.into_iter().collect(),
        }
    }

    pub fn tag(name: &'a str, children: impl IntoIterator<Item = Node<'a>>) -> Node<'a> {
        Node::elem(name, [], children)
    }

    pub fn line(name: &'a str, s: impl Into<Cow<'a, str>>) -> Node<'a> {
        Node::elem(name, [], [Node::text(s)])
    }

    pub fn text(s: impl Into<Cow<'a, str>>) -> Node<'a> {
        Node::Text(s.into())
    }

    pub fn cdata(s: impl Into<Cow<'a, str>>) -> Node<'a> {
        Node::CData(s.into())
    }
}

fn render(node: &Node, depth: usize) -> String {
    let (name, attrs, children) = match node {
        Node::Text(s) => return text(s).to_string(),
        Node::CData(s) => return cdata(s).to_string(),
        Node::Elem {
            name,
            attrs,
            children,
        } => (name, attrs, children),
    };

    let head: String = attrs
        .iter()
        .map(|(k, v)| format!(" {k}=\"{}\"", attr(v)))
        .collect();

    if children.is_empty() {
        return format!("<{name}{head} />");
    }

    let parts: Vec<String> = children.iter().map(|c| render(c, depth + 1)).collect();
    let character_data = children.iter().all(|c| !matches!(c, Node::Elem { .. }));
    let single_line = parts.len() == 1 && !parts[0].contains('\n');

    match character_data || single_line {
        true => format!("<{name}{head}>{}</{name}>", parts.concat()),
        false => {
            let pad = "  ".repeat(depth + 1);
            let inner: String = parts.iter().map(|p| format!("{pad}{p}\n")).collect();
            format!("<{name}{head}>\n{inner}{}</{name}>", "  ".repeat(depth))
        }
    }
}

pub fn document(root: &Node) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n{}\n",
        render(root, 0)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unescape(s: &str) -> String {
        s.replace("&#xD;", "\r")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&amp;", "&")
    }

    #[test]
    fn control_characters_are_stripped_not_escaped() {
        for bad in [
            '\u{0}', '\u{8}', '\u{B}', '\u{C}', '\u{1F}', '\u{FFFE}', '\u{FFFF}',
        ] {
            let input = format!("a{bad}b");
            for got in [
                text(&input).to_string(),
                attr(&input).to_string(),
                cdata(&input).to_string(),
            ] {
                assert!(!got.contains(bad), "{bad:?} survived in {got:?}");
                assert!(got.contains("ab"), "{bad:?} took a neighbour: {got:?}");
            }
        }
    }

    #[test]
    fn legal_characters_the_production_allows_are_kept() {
        for good in ['\t', '\n', '\r', '\u{FFFD}', '\u{1FFFE}', '\u{10FFFF}'] {
            let input = format!("a{good}b");
            assert!(
                cdata(&input).to_string().contains(good),
                "{good:?} should survive CDATA"
            );
        }
        assert_eq!(text("a\tb").to_string(), "a\tb");
        assert_eq!(text("a\nb").to_string(), "a\nb");
    }

    #[test]
    fn text_escapes_exactly_the_characters_section_2_4_requires() {
        assert_eq!(text("a & b").to_string(), "a &amp; b");
        assert_eq!(text("<i>").to_string(), "&lt;i&gt;");
        assert_eq!(text("a\rb").to_string(), "a&#xD;b");
        assert_eq!(
            text(r#"he said "hi" and 'bye'"#).to_string(),
            r#"he said "hi" and 'bye'"#
        );
    }

    #[test]
    fn attribute_values_survive_normalization() {
        assert_eq!(attr("a\tb\nc\rd").to_string(), "a&#x9;b&#xA;c&#xD;d");
        assert_eq!(attr("a b").to_string(), "a b");
        assert_eq!(
            attr(r#"say "hi" & <go>"#).to_string(),
            "say &quot;hi&quot; &amp; &lt;go&gt;"
        );
    }

    #[test]
    fn text_roundtrips_to_the_original_content() {
        for inner in ["", "plain", "a & b", "<p>hi</p>", "&amp;", "a\rb", "\"'"] {
            assert_eq!(unescape(&text(inner).to_string()), inner, "for {inner:?}");
        }
    }

    #[test]
    fn cdata_survives_a_literal_terminator() {
        for inner in ["a ]]> b", "]]>]]>", "trailing ]]>", "]]> leading"] {
            let out = cdata(inner).to_string();
            assert_eq!(
                out.matches("<![CDATA[").count(),
                out.matches("]]>").count(),
                "unbalanced for {inner:?}: {out}"
            );
        }
    }

    #[test]
    fn cdata_roundtrips_to_the_original_content() {
        for inner in ["<p>hi</p>", "a ]]> b", "]]>]]>", "]]>"] {
            let out = cdata(inner).to_string();
            let rejoined: String = out
                .strip_prefix("<![CDATA[")
                .unwrap()
                .strip_suffix("]]>")
                .unwrap()
                .split("]]><![CDATA[")
                .collect();
            assert_eq!(rejoined, inner, "roundtrip failed for {inner:?}");
        }
    }

    #[test]
    fn an_element_with_no_children_is_self_closing() {
        let n = Node::elem("atom:link", [("rel", "self".into())], []);
        assert_eq!(render(&n, 0), r#"<atom:link rel="self" />"#);
    }

    #[test]
    fn nested_elements_indent_and_leaf_elements_stay_inline() {
        let url = Node::tag("url", [Node::line("loc", "https://x/")]);
        assert_eq!(render(&url, 1), "<url><loc>https://x/</loc></url>");

        let item = Node::tag("item", [Node::line("a", "1"), Node::line("b", "2")]);
        assert_eq!(
            render(&item, 2),
            "<item>\n      <a>1</a>\n      <b>2</b>\n    </item>"
        );
    }

    #[test]
    fn a_document_declares_its_version_and_ends_with_a_newline() {
        let doc = document(&Node::line("a", "x"));
        assert_eq!(
            doc,
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<a>x</a>\n"
        );
    }
}
