//! HTML Standard §13.2.5 tokenization
//! <https://html.spec.whatwg.org/multipage/parsing.html>
//!
//! Deliberately missing: character-reference decoding, foreign content, and `srcset`

use std::borrow::Cow;

const URL_ATTRS: [&str; 5] = ["href", "src", "poster", "cite", "data"];

const RAWTEXT: [&str; 4] = ["script", "style", "textarea", "title"];

/// §13.1.2 void elements: they have no closing tag, so they have no content and
/// cannot nest.
const VOID: [&str; 14] = [
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

fn is_space(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\x0c' | b'\r')
}

fn named(name: &str, set: &[&str]) -> bool {
    set.iter().any(|n| name.eq_ignore_ascii_case(n))
}

/// An element whose content is text, not markup: `<script>`, `<style>`,
/// `<textarea>`, `<title>`.
pub fn named_rawtext(name: &str) -> bool {
    named(name, &RAWTEXT)
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

        if !closing && named(name, &RAWTEXT) {
            let stop = rawtext_end(rest, name);
            out.push_str(&rest[..stop]);
            rest = &rest[stop..];
        }
    }
    out.push_str(rest);
    out
}

pub fn tag_name(tag: &str) -> Option<(&str, bool)> {
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

pub fn tag_end(tag: &str) -> usize {
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

pub struct Attribute<'a> {
    pub name: &'a str,
    /// Byte span of the value inside its quotes, relative to the tag.
    pub value: Option<(usize, usize)>,
    /// Byte span of the whole `name="value"`, relative to the tag. `fill`
    /// deletes this range to strip a marker from the output.
    pub span: (usize, usize),
}

pub fn attributes(tag: &str) -> Vec<Attribute<'_>> {
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
            attrs.push(Attribute {
                name,
                value,
                span: (name_start, i),
            });
        }
    }
    attrs
}

/// Where an element begins, where its content ends, and where it ends. A void
/// or self-closing element has no content, so all three collapse to the byte
/// after `>`.
#[derive(Debug, PartialEq, Eq)]
pub struct Extent {
    pub open_end: usize,
    pub content_end: usize,
    pub end: usize,
}

fn self_closing(tag: &str) -> bool {
    tag.strip_suffix('>')
        .unwrap_or(tag)
        .trim_end()
        .ends_with('/')
}

fn close_len(s: &str, name: &str) -> usize {
    match tag_name(s) {
        Some((n, true)) if n.eq_ignore_ascii_case(name) => tag_end(s),
        _ => 0,
    }
}

/// The extent of the element whose opening tag starts at `at`, found by
/// counting depth over same-named tags. `None` if `at` is not an opening tag or
/// the element is never closed.
pub fn extent(src: &str, at: usize) -> Option<Extent> {
    let tail = src.get(at..)?;
    let (name, closing) = tag_name(tail)?;
    if closing {
        return None;
    }
    let open_end = at + tag_end(tail);
    let empty = Extent {
        open_end,
        content_end: open_end,
        end: open_end,
    };

    if named(name, &VOID) || self_closing(&src[at..open_end]) {
        return Some(empty);
    }
    if named(name, &RAWTEXT) {
        let content_end = open_end + rawtext_end(&src[open_end..], name);
        return Some(Extent {
            open_end,
            content_end,
            end: content_end + close_len(&src[content_end..], name),
        });
    }

    let mut i = open_end;
    let mut depth = 1usize;
    while let Some(lt) = src[i..].find('<') {
        let at = i + lt;
        let tail = &src[at..];

        if tail.starts_with("<!--") {
            i = at + tail[4..].find("-->").map_or(tail.len(), |k| 4 + k + 3);
            continue;
        }
        if tail.starts_with("<!") || tail.starts_with("<?") {
            i = at + tail.find('>').map_or(tail.len(), |k| k + 1);
            continue;
        }
        let Some((n, close)) = tag_name(tail) else {
            i = at + 1;
            continue;
        };
        let end = at + tag_end(tail);

        if n.eq_ignore_ascii_case(name) {
            match close {
                true => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(Extent {
                            open_end,
                            content_end: at,
                            end,
                        });
                    }
                }
                false if !self_closing(&src[at..end]) => depth += 1,
                false => {}
            }
        }

        i = match !close && named(n, &RAWTEXT) {
            true => end + rawtext_end(&src[end..], n),
            false => end,
        };
    }
    None
}

fn is_root_relative(value: &str) -> bool {
    value.starts_with('/') && !value.starts_with("//")
}

fn rewrite_tag<'a>(tag: &'a str, base: &str) -> Cow<'a, str> {
    let insert_at: Vec<usize> = attributes(tag)
        .iter()
        .filter(|a| named(a.name, &URL_ATTRS))
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

    /// The source between an element's `>` and its matching `</`.
    fn content(src: &str) -> &str {
        let e = extent(src, src.find('<').unwrap()).unwrap();
        &src[e.open_end..e.content_end]
    }

    #[test]
    fn extent_finds_the_matching_close_not_the_first_one() {
        assert_eq!(
            content("<ul><li>a</li><li>b</li></ul>"),
            "<li>a</li><li>b</li>"
        );
        assert_eq!(content("<div><div>in</div></div>"), "<div>in</div>");
        assert_eq!(content("<p>plain</p>"), "plain");
    }

    #[test]
    fn extent_consumes_the_close_tag() {
        let src = "<p>x</p>tail";
        assert_eq!(&src[extent(src, 0).unwrap().end..], "tail");
    }

    #[test]
    fn a_void_or_self_closing_element_has_no_content() {
        for src in ["<meta name=\"a\" />", "<br>", "<img src=\"/x\">", "<hr/>"] {
            let e = extent(src, 0).unwrap();
            assert_eq!(e.open_end, e.content_end, "for {src:?}");
            assert_eq!(e.end, src.len(), "for {src:?}");
        }
        // A trailing slash inside an attribute value does not self-close a tag.
        assert_eq!(content("<a href=\"/blog/\">t</a>"), "t");
    }

    #[test]
    fn a_tag_inside_rawtext_does_not_count_toward_depth() {
        // The inline theme script contains braces and quotes, and `title` shows
        // up in both the head and the templates.
        assert_eq!(
            content("<script>var a = '</div>';</script>"),
            "var a = '</div>';"
        );
        assert_eq!(content("<title>a > b</title>"), "a > b");
        assert_eq!(
            content("<div><script>'<div>'</script></div>"),
            "<script>'<div>'</script>"
        );
    }

    #[test]
    fn extent_ignores_comments_and_declarations() {
        assert_eq!(content("<div><!-- </div> -->x</div>"), "<!-- </div> -->x");
    }

    #[test]
    fn an_unclosed_element_has_no_extent() {
        assert_eq!(extent("<div>x", 0), None);
        assert_eq!(
            extent("</div>", 0),
            None,
            "a close tag is not an opening tag"
        );
    }

    #[test]
    fn attribute_spans_cover_the_whole_pair() {
        let tag = r#"<a data-slot="title" href="/x">"#;
        let attrs = attributes(tag);
        let span = |n: &str| {
            let a = attrs.iter().find(|a| a.name == n).unwrap();
            &tag[a.span.0..a.span.1]
        };
        assert_eq!(span("data-slot"), r#"data-slot="title""#);
        assert_eq!(span("href"), r#"href="/x""#);
    }

    #[test]
    fn output_is_unchanged_when_there_is_nothing_to_rewrite() {
        for html in ["", "<p>plain</p>", "<a href=\"https://x/\">x</a>"] {
            assert_eq!(abs(html), html, "for {html:?}");
        }
    }
}
