//! HTML Standard §13.2.5 tokenization
//! <https://html.spec.whatwg.org/multipage/parsing.html>
//!
//! Deliberately missing: character-reference decoding, foreign content, and `srcset`

use std::borrow::Cow;

const URL_ATTRS: [&str; 5] = ["href", "src", "poster", "cite", "data"];

const RAWTEXT: [&str; 4] = ["script", "style", "textarea", "title"];

fn is_space(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\x0c' | b'\r')
}

pub fn absolutize(html: &str, base: &str) -> String {
    let mut out = String::with_capacity(html.len() + 128);
    let mut rest = html;

    while let Some(lt) = rest.find('<') {
        out.push_str(&rest[..lt]);
        let tail = &rest[lt..];

        if tail.starts_with("<!--") {
            let end = tail[4..].find("-->").map_or(tail.len(), |i| 4 + i + 3);
            out.push_str(&tail[..end]);
            rest = &tail[end..];
            continue;
        }
        if tail.starts_with("<!") || tail.starts_with("<?") {
            let end = tail.find('>').map_or(tail.len(), |i| i + 1);
            out.push_str(&tail[..end]);
            rest = &tail[end..];
            continue;
        }
        let Some((name, closing)) = tag_name(tail) else {
            out.push('<');
            rest = &tail[1..];
            continue;
        };
        let end = tag_end(tail);
        out.push_str(&rewrite_tag(&tail[..end], base));
        rest = &tail[end..];

        if !closing && RAWTEXT.iter().any(|r| name.eq_ignore_ascii_case(r)) {
            let stop = rawtext_end(rest, name);
            out.push_str(&rest[..stop]);
            rest = &rest[stop..];
        }
    }
    out.push_str(rest);
    out
}

fn tag_name(tag: &str) -> Option<(&str, bool)> {
    let after = tag.strip_prefix('<')?;
    let (body, closing) = match after.strip_prefix('/') {
        Some(body) => (body, true),
        None => (after, false),
    };
    if !body.starts_with(|c: char| c.is_ascii_alphabetic()) {
        return None;
    }
    let end = body
        .find(|c: char| !c.is_ascii_alphanumeric())
        .unwrap_or(body.len());
    Some((&body[..end], closing))
}

fn tag_end(tag: &str) -> usize {
    let bytes = tag.as_bytes();
    let mut quote = 0u8;
    for (i, &b) in bytes.iter().enumerate().skip(1) {
        match b {
            _ if quote != 0 => quote = if b == quote { 0 } else { quote },
            b'"' | b'\'' => quote = b,
            b'>' => return i + 1,
            _ => {}
        }
    }
    tag.len()
}

fn rawtext_end(s: &str, name: &str) -> usize {
    s.match_indices('<')
        .find(|(i, _)| {
            let after = &s[i + 1..];
            after.starts_with('/')
                && after
                    .get(1..1 + name.len())
                    .is_some_and(|n| n.eq_ignore_ascii_case(name))
        })
        .map_or(s.len(), |(i, _)| i)
}

struct Attribute<'a> {
    name: &'a str,
    value: Option<(usize, usize)>,
}

fn attributes(tag: &str) -> Vec<Attribute<'_>> {
    let bytes = tag.as_bytes();
    let mut attrs = Vec::new();
    let mut i = 1;
    if bytes.get(i) == Some(&b'/') {
        i += 1;
    }
    while i < bytes.len() && !is_space(bytes[i]) && !matches!(bytes[i], b'>' | b'/') {
        i += 1;
    }

    while i < bytes.len() {
        while i < bytes.len() && is_space(bytes[i]) {
            i += 1;
        }
        match bytes.get(i) {
            None | Some(b'>') => break,
            Some(b'/') => {
                i += 1;
                continue;
            }
            _ => {}
        }
        let name_start = i;
        while i < bytes.len() && !is_space(bytes[i]) && !matches!(bytes[i], b'=' | b'>' | b'/') {
            i += 1;
        }
        let name = &tag[name_start..i];
        while i < bytes.len() && is_space(bytes[i]) {
            i += 1;
        }
        let value = match bytes.get(i) {
            Some(&b'=') => {
                i += 1;
                while i < bytes.len() && is_space(bytes[i]) {
                    i += 1;
                }
                match bytes.get(i) {
                    Some(&quote @ (b'"' | b'\'')) => {
                        i += 1;
                        let start = i;
                        while i < bytes.len() && bytes[i] != quote {
                            i += 1;
                        }
                        let span = (start, i);
                        i = (i + 1).min(bytes.len());
                        Some(span)
                    }
                    Some(_) => {
                        let start = i;
                        while i < bytes.len() && !is_space(bytes[i]) && bytes[i] != b'>' {
                            i += 1;
                        }
                        Some((start, i))
                    }
                    None => None,
                }
            }
            _ => None,
        };
        if !name.is_empty() {
            attrs.push(Attribute { name, value });
        }
    }
    attrs
}

fn is_root_relative(value: &str) -> bool {
    value.starts_with('/') && !value.starts_with("//")
}

fn rewrite_tag<'a>(tag: &'a str, base: &str) -> Cow<'a, str> {
    let insert_at: Vec<usize> = attributes(tag)
        .iter()
        .filter(|a| URL_ATTRS.iter().any(|u| a.name.eq_ignore_ascii_case(u)))
        .filter_map(|a| a.value)
        .filter(|&(start, end)| is_root_relative(&tag[start..end]))
        .map(|(start, _)| start)
        .collect();

    if insert_at.is_empty() {
        return Cow::Borrowed(tag);
    }
    let (mut out, last) = insert_at.iter().fold(
        (
            String::with_capacity(tag.len() + insert_at.len() * base.len()),
            0,
        ),
        |(mut out, last), &start| {
            out.push_str(&tag[last..start]);
            out.push_str(base);
            (out, start)
        },
    );
    out.push_str(&tag[last..]);
    Cow::Owned(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = "https://example.com";

    fn abs(html: &str) -> String {
        absolutize(html, BASE)
    }

    #[test]
    fn absolutize_rewrites_root_relative_urls_only() {
        assert_eq!(
            abs(r#"<img src="/introbigo/bigo.jpg" /><a href="/blog/x">x</a>"#),
            format!(r#"<img src="{BASE}/introbigo/bigo.jpg" /><a href="{BASE}/blog/x">x</a>"#)
        );

        let untouched = r#"<img src="//cdn.example.com/pic.png" /><img src="https://x.com/a.png" /><img src="img/b.png" />"#;
        assert_eq!(abs(untouched), untouched);
    }

    #[test]
    fn absolutize_ignores_urls_inside_an_inline_code_span() {
        let html = r#"<p><code>&lt;img src="/x"&gt;</code></p>"#;
        assert_eq!(abs(html), html);

        let json = r#"<p><code>{ "src": "/x" }</code></p>"#;
        assert_eq!(abs(json), json);
    }

    #[test]
    fn absolutize_handles_every_attribute_quoting_form() {
        assert_eq!(abs(r#"<a href="/a">"#), format!(r#"<a href="{BASE}/a">"#));
        assert_eq!(abs("<a href='/a'>"), format!("<a href='{BASE}/a'>"));
        assert_eq!(abs("<a href=/a>"), format!("<a href={BASE}/a>"));
        assert_eq!(abs("<a href = \"/a\">"), format!("<a href = \"{BASE}/a\">"));
    }

    #[test]
    fn absolutize_rewrites_every_url_attribute_it_claims_to() {
        assert_eq!(
            abs(r#"<video src="/a.mp4" poster="/p.jpg"></video>"#),
            format!(r#"<video src="{BASE}/a.mp4" poster="{BASE}/p.jpg"></video>"#)
        );
        assert_eq!(
            abs(r#"<blockquote cite="/src">q</blockquote>"#),
            format!(r#"<blockquote cite="{BASE}/src">q</blockquote>"#)
        );
    }

    #[test]
    fn srcset_is_left_alone_on_purpose() {
        let html = r#"<img src="/a.png" srcset="/a.png 1x, /b.png 2x">"#;
        assert_eq!(
            abs(html),
            format!(r#"<img src="{BASE}/a.png" srcset="/a.png 1x, /b.png 2x">"#)
        );
    }

    #[test]
    fn markup_inside_rawtext_elements_is_not_markup() {
        let html = r#"<script>var a = '<img src="/x">';</script><img src="/y">"#;
        assert_eq!(
            abs(html),
            format!(r#"<script>var a = '<img src="/x">';</script><img src="{BASE}/y">"#)
        );
    }

    #[test]
    fn comments_and_declarations_pass_through_untouched() {
        let html = r#"<!-- <img src="/x"> --><!DOCTYPE html><img src="/y">"#;
        assert_eq!(
            abs(html),
            format!(r#"<!-- <img src="/x"> --><!DOCTYPE html><img src="{BASE}/y">"#)
        );
    }

    #[test]
    fn a_bare_angle_bracket_is_text_not_a_tag() {
        for html in ["a < b", "1<2 and 3>2", "<3"] {
            assert_eq!(abs(html), html, "for {html:?}");
        }
    }

    #[test]
    fn a_greater_than_inside_an_attribute_does_not_end_the_tag() {
        assert_eq!(
            abs(r#"<a title="a > b" href="/x">t</a>"#),
            format!(r#"<a title="a > b" href="{BASE}/x">t</a>"#)
        );
    }

    #[test]
    fn output_is_unchanged_when_there_is_nothing_to_rewrite() {
        for html in ["", "<p>plain</p>", "<a href=\"https://x/\">x</a>"] {
            assert_eq!(abs(html), html, "for {html:?}");
        }
    }
}
