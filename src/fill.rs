//! Fills marked regions of an ordinary HTML file with values.
//!
//! A template here is not a dialect -- it is HTML. Every dynamic region is
//! named by a `data-` attribute, so a template opens in a browser, validates as
//! HTML, and can be edited with HTML tooling. Six markers, all stripped from
//! the output:
//!
//! ```text
//! data-slot="path"       element content, escaped
//! data-html="path"       element content, raw
//! data-attr-NAME="path"  attribute NAME, escaped
//! data-each="path"       clone the first child element once per list item
//! data-include="_f.html" replace this element with that file
//! data-when="path"       keep this element only if `path` is present
//! ```
//!
//! There is no conditional. A binding that resolves to `Nil`, or a `data-each`
//! over an empty list, removes the element -- which is every condition this
//! site had: an absent subtitle, a post with no tags, the dev-only live-reload
//! script. `data-when` is that same rule and nothing more, for a container that
//! has no value of its own to bind: it is present exactly when its path is.
//!
//! Two rules carry the weight, as before. `data-slot` refuses to emit `Html`
//! and `data-html` is the only marker that emits it unescaped, so
//! `grep -rn data-html templates/` is the whole audit surface. And an unknown
//! field is an error rather than an empty string, so a typo fails the build
//! instead of shipping a blank page.

use std::borrow::Cow;
use std::fmt;

use crate::html;

#[derive(Debug, PartialEq, Eq)]
pub struct Error {
    pub file: String,
    pub line: usize,
    pub msg: String,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: line {}: {}", self.file, self.line, self.msg)
    }
}

/// 1-based line containing the byte at `at`.
pub fn line_at(src: &str, at: usize) -> usize {
    src[..at.min(src.len())].matches('\n').count() + 1
}

/// An include may not nest deeper than this. Templates are hand-written and
/// shallow; anything deeper is a cycle.
const MAX_DEPTH: usize = 16;

/// The file a span belongs to. Carried so an error inside an include names the
/// include, not the page that pulled it in.
#[derive(Clone, Copy)]
pub struct Src<'a> {
    pub name: &'a str,
    pub text: &'a str,
}

impl Src<'_> {
    fn at(&self, at: usize, msg: impl Into<String>) -> Error {
        Error {
            file: self.name.to_string(),
            line: line_at(self.text, at),
            msg: msg.into(),
        }
    }

    fn err<T>(&self, at: usize, msg: impl Into<String>) -> Result<T, Error> {
        Err(self.at(at, msg))
    }
}

pub type Includes<'a> = &'a [(&'a str, &'a str)];

/// What the walk carries: the file it is in, the files it may pull in, and how
/// deep the includes have nested.
#[derive(Clone, Copy)]
struct Ctx<'a> {
    src: Src<'a>,
    inc: Includes<'a>,
    depth: usize,
}

impl<'a> Ctx<'a> {
    fn text(&self) -> &'a str {
        self.src.text
    }

    fn err<T>(&self, at: usize, msg: impl Into<String>) -> Result<T, Error> {
        self.src.err(at, msg)
    }

    /// The same walk one file deeper.
    fn include(&self, name: &'a str, text: &'a str) -> Self {
        Ctx {
            src: Src { name, text },
            inc: self.inc,
            depth: self.depth + 1,
        }
    }
}

// -- Values -------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum Value<'a> {
    /// A field that is present but empty. Removes the element that binds it.
    Nil,
    Bool(bool),
    Num(i64),
    /// Escaped by `data-slot` and `data-attr-*`.
    Text(Cow<'a, str>),
    /// Markup. Only `data-html` may emit it.
    Html(Cow<'a, str>),
    List(Vec<Value<'a>>),
    Map(Vec<(&'a str, Value<'a>)>),
}

const NIL: Value<'static> = Value::Nil;

impl<'a> Value<'a> {
    pub fn text(s: impl Into<Cow<'a, str>>) -> Self {
        Value::Text(s.into())
    }

    pub fn html(s: impl Into<Cow<'a, str>>) -> Self {
        Value::Html(s.into())
    }

    pub fn num(n: usize) -> Self {
        Value::Num(n as i64)
    }

    /// `None` becomes `Nil`, so an absent optional is still a bound field.
    pub fn opt(s: Option<impl Into<Cow<'a, str>>>) -> Self {
        s.map_or(Value::Nil, Value::text)
    }

    pub fn list(items: impl IntoIterator<Item = Value<'a>>) -> Self {
        Value::List(items.into_iter().collect())
    }

    pub fn map(entries: impl IntoIterator<Item = (&'a str, Value<'a>)>) -> Self {
        Value::Map(entries.into_iter().collect())
    }

    pub fn get<'s>(&'s self, key: &str) -> Option<&'s Value<'a>> {
        match self {
            Value::Map(entries) => entries.iter().find(|(k, _)| *k == key).map(|(_, v)| v),
            _ => None,
        }
    }
}

/// Escapes the four characters that change meaning in element content or a
/// double-quoted attribute. Not `'`: every attribute this project emits is
/// double-quoted, and XML text does not need `&apos;`.
pub fn esc(s: &str) -> String {
    s.chars()
        .fold(String::with_capacity(s.len()), |mut out, c| {
            match c {
                '&' => out.push_str("&amp;"),
                '<' => out.push_str("&lt;"),
                '>' => out.push_str("&gt;"),
                '"' => out.push_str("&quot;"),
                _ => out.push(c),
            }
            out
        })
}

struct Scope<'s, 'a> {
    value: &'s Value<'a>,
    parent: Option<&'s Scope<'s, 'a>>,
}

impl<'s, 'a> Scope<'s, 'a> {
    /// Resolves `a.b.c` against the innermost scope that defines `a`, so a
    /// `data-each` prototype can still see the page's fields. A segment reached
    /// through a `Nil` is `Nil` rather than an error, which is what lets
    /// `data-attr-href="a.url"` drop the link of an item that has no `a`.
    fn lookup(&self, path: &str) -> Option<&'s Value<'a>> {
        let (head, rest) = path.split_once('.').unwrap_or((path, ""));
        let found =
            std::iter::successors(Some(self), |s| s.parent).find_map(|s| s.value.get(head))?;
        match rest {
            "" => Some(found),
            rest => rest.split('.').try_fold(found, |acc, k| match acc {
                Value::Nil => Some(&NIL),
                _ => acc.get(k),
            }),
        }
    }
}

// -- Markers ------------------------------------------------------------------

#[derive(Debug, PartialEq, Eq)]
enum Kind<'a> {
    Slot,
    Html,
    Each,
    Include,
    Attr(&'a str),
    /// Binds a path solely so the drop rule can see it; emits nothing.
    When,
}

struct Marker<'a> {
    kind: Kind<'a>,
    /// A field path, or for `Include` a file name.
    path: &'a str,
    /// Span within the opening tag, including the whitespace before it, so
    /// deleting it leaves the tag well-formed.
    span: (usize, usize),
}

fn marker_of<'a>(tag: &'a str, a: &html::Attribute<'a>) -> Option<Marker<'a>> {
    let kind = match a.name {
        "data-slot" => Kind::Slot,
        "data-html" => Kind::Html,
        "data-each" => Kind::Each,
        "data-include" => Kind::Include,
        "data-when" => Kind::When,
        name => Kind::Attr(name.strip_prefix("data-attr-")?),
    };
    Some(Marker {
        kind,
        path: a.value.map_or("", |(s, e)| tag[s..e].trim()),
        span: (tag[..a.span.0].trim_end().len(), a.span.1),
    })
}

fn markers<'a>(tag: &'a str) -> Vec<Marker<'a>> {
    html::attributes(tag)
        .iter()
        .filter_map(|a| marker_of(tag, a))
        .collect()
}

// -- Rendering ----------------------------------------------------------------

/// Two scopes: what the page itself binds, falling back to what the site
/// provides. A page shadows the shell rather than copying it, and `Nil` shadows
/// too -- a page that binds `description: Nil` deletes the element the shell
/// would have filled.
pub fn render(src: Src, page: &Value, site: &Value, inc: Includes) -> Result<String, Error> {
    let outer = Scope {
        value: site,
        parent: None,
    };
    let scope = Scope {
        value: page,
        parent: Some(&outer),
    };
    let ctx = Ctx { src, inc, depth: 0 };
    walk(ctx, (0, src.text.len()), &scope)
}

/// Emits the given range of the current file, filling any marked element it
/// meets. Elements without markers are copied through; the scan meets their
/// children on its own, so only a marked element needs its subtree recursed
/// into.
fn walk(ctx: Ctx, (start, end): (usize, usize), scope: &Scope) -> Result<String, Error> {
    let text = ctx.text();
    let mut out = String::new();
    let mut i = start;

    while let Some(lt) = text[i..end].find('<') {
        let at = i + lt;
        out.push_str(&text[i..at]);
        let tail = &text[at..end];

        // Comments and declarations are not tags and hold no markers.
        if tail.starts_with("<!--") {
            let stop = tail[4..].find("-->").map_or(tail.len(), |k| 4 + k + 3);
            out.push_str(&tail[..stop]);
            i = at + stop;
            continue;
        }
        if tail.starts_with("<!") || tail.starts_with("<?") {
            let stop = tail.find('>').map_or(tail.len(), |k| k + 1);
            out.push_str(&tail[..stop]);
            i = at + stop;
            continue;
        }
        let Some((name, closing)) = html::tag_name(tail) else {
            out.push('<');
            i = at + 1;
            continue;
        };

        let tag_end = at + html::tag_end(tail);
        let ms = markers(&text[at..tag_end]);

        if closing || ms.is_empty() {
            out.push_str(&text[at..tag_end]);
            i = tag_end;
            // Rawtext content is not markup: the inline theme script is full of
            // braces and quotes, and a `<` in it does not open a tag.
            if !closing && html::named_rawtext(name) {
                if let Some(e) = html::extent(text, at) {
                    out.push_str(&text[tag_end..e.content_end]);
                    i = e.content_end;
                }
            }
            continue;
        }

        let Some(e) = html::extent(text, at) else {
            return ctx.err(at, format!("<{name}> is never closed"));
        };
        out.push_str(&element(ctx, at, &e, &ms, scope)?);
        i = e.end;
    }

    out.push_str(&text[i..end]);
    Ok(out)
}

fn element(
    ctx: Ctx,
    at: usize,
    e: &html::Extent,
    ms: &[Marker],
    scope: &Scope,
) -> Result<String, Error> {
    let (src, text) = (ctx.src, ctx.text());
    let tag = &text[at..e.open_end];
    let offset = |m: &Marker| at + m.span.0;

    // An include replaces its element rather than filling it, so a fragment
    // starts with its own markup instead of a wrapper the caller chose.
    if let Some(m) = ms.iter().find(|m| m.kind == Kind::Include) {
        if ctx.depth >= MAX_DEPTH {
            let msg = format!(
                "includes nested more than {MAX_DEPTH} deep; `{}` is probably a cycle",
                m.path
            );
            return ctx.err(offset(m), msg);
        }
        let Some(&(name, text)) = ctx.inc.iter().find(|(n, _)| *n == m.path) else {
            return ctx.err(offset(m), format!("no file named `{}`", m.path));
        };
        return walk(ctx.include(name, text), (0, text.len()), scope);
    }

    // Resolve every binding first: one `Nil` removes the element, and that is
    // the only conditional this engine has.
    let bound = ms
        .iter()
        .map(|m| {
            let msg = || format!("unknown field `{}`", m.path);
            let v = scope
                .lookup(m.path)
                .ok_or_else(|| src.at(offset(m), msg()))?;
            Ok((m, v))
        })
        .collect::<Result<Vec<(&Marker, &Value)>, Error>>()?;

    let dropped = bound.iter().any(|(m, v)| match v {
        Value::Nil => true,
        Value::List(items) => m.kind == Kind::Each && items.is_empty(),
        _ => false,
    });
    if dropped {
        return Ok(String::new());
    }

    let attrs = bound
        .iter()
        .filter_map(|(m, v)| match m.kind {
            Kind::Attr(name) => Some((m, name, v)),
            _ => None,
        })
        .map(|(m, name, v)| Ok((name, scalar(src, offset(m), m.path, v)?)))
        .collect::<Result<Vec<(&str, String)>, Error>>()?;

    let open = open_tag(tag, ms, &attrs);
    let close = &text[e.content_end..e.end];
    let inner = (e.open_end, e.content_end);

    // At most one marker writes the content; the rest are attributes.
    let writer = bound
        .iter()
        .find(|(m, _)| matches!(m.kind, Kind::Slot | Kind::Html | Kind::Each));

    let content = match writer {
        None => walk(ctx, inner, scope)?,
        Some((m, v)) => match m.kind {
            Kind::Slot => scalar(src, offset(m), m.path, v)?,
            Kind::Html => raw(src, offset(m), m.path, v)?,
            Kind::Each => each(ctx, offset(m), m.path, v, inner, scope)?,
            Kind::Attr(_) | Kind::Include | Kind::When => unreachable!("not a content writer"),
        },
    };

    Ok(format!("{open}{content}{close}"))
}

/// The prototype is the element's first child element, cloned once per item.
/// Whitespace around it is layout in the template, not output.
fn each(
    ctx: Ctx,
    at: usize,
    path: &str,
    value: &Value,
    (start, end): (usize, usize),
    scope: &Scope,
) -> Result<String, Error> {
    let Value::List(items) = value else {
        return ctx.err(at, format!("`{path}` is not a list; `data-each` needs one"));
    };
    let text = ctx.text();
    let proto = text[start..end]
        .find('<')
        .map(|o| start + o)
        .filter(|&p| matches!(html::tag_name(&text[p..]), Some((_, false))))
        .and_then(|p| html::extent(text, p).map(|e| (p, e.end)));

    let Some(proto) = proto else {
        let msg = format!("`data-each` over `{path}` has no child element to repeat");
        return ctx.err(at, msg);
    };

    items.iter().try_fold(String::new(), |acc, item| {
        let inner = Scope {
            value: item,
            parent: Some(scope),
        };
        Ok(acc + &walk(ctx, proto, &inner)?)
    })
}

fn scalar(src: Src, at: usize, path: &str, v: &Value) -> Result<String, Error> {
    match v {
        Value::Nil => Ok(String::new()),
        Value::Num(n) => Ok(n.to_string()),
        Value::Bool(b) => Ok(b.to_string()),
        Value::Text(s) => Ok(esc(s)),
        Value::Html(_) => src.err(
            at,
            format!("`{path}` is markup; write data-html=\"{path}\" to emit it unescaped"),
        ),
        Value::List(_) | Value::Map(_) => {
            src.err(at, format!("`{path}` is a collection; it has no text form"))
        }
    }
}

fn raw(src: Src, at: usize, path: &str, v: &Value) -> Result<String, Error> {
    match v {
        Value::Html(s) | Value::Text(s) => Ok(s.to_string()),
        Value::List(_) | Value::Map(_) => {
            src.err(at, format!("`{path}` is a collection; it has no text form"))
        }
        other => scalar(src, at, path, other),
    }
}

/// The opening tag with every marker removed and every `data-attr-*` applied:
/// replacing the attribute's value if it already has one, otherwise appended.
fn open_tag(tag: &str, ms: &[Marker], attrs: &[(&str, String)]) -> String {
    let existing = html::attributes(tag);
    let mut edits: Vec<((usize, usize), &str)> = ms.iter().map(|m| (m.span, "")).collect();

    let mut append = String::new();
    for (name, value) in attrs {
        match existing
            .iter()
            .find(|a| a.name.eq_ignore_ascii_case(name))
            .and_then(|a| a.value)
        {
            Some(span) => edits.push((span, value)),
            None => append.push_str(&format!(" {name}=\"{value}\"")),
        }
    }
    edits.sort_by_key(|(span, _)| span.0);

    let (mut out, last) = edits
        .iter()
        .fold((String::new(), 0), |(mut out, last), (span, to)| {
            out.push_str(&tag[last..span.0]);
            out.push_str(to);
            (out, span.1)
        });
    out.push_str(&tag[last..]);

    match append.is_empty() {
        true => out,
        false => {
            let at = insert_point(&out);
            format!("{}{}{}", &out[..at], append, &out[at..])
        }
    }
}

/// Just before the `>` or `/>` that ends a tag.
fn insert_point(tag: &str) -> usize {
    let close = tag.rfind('>').unwrap_or(tag.len());
    let body = tag[..close].trim_end();
    body.strip_suffix('/').unwrap_or(body).trim_end().len()
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOTHING: Value<'static> = Value::Map(Vec::new());

    fn run(text: &str, root: &Value) -> Result<String, Error> {
        render(
            Src {
                name: "t.html",
                text,
            },
            root,
            &NOTHING,
            &[],
        )
    }

    fn ok(text: &str, root: &Value) -> String {
        run(text, root).unwrap()
    }

    fn root<'a>(entries: impl IntoIterator<Item = (&'a str, Value<'a>)>) -> Value<'a> {
        Value::map(entries)
    }

    #[test]
    fn escaping() {
        assert_eq!(
            esc("a & b < c > \"d\""),
            "a &amp; b &lt; c &gt; &quot;d&quot;"
        );
        // Deliberate: every attribute we emit is double-quoted, and XML text
        // does not need &apos;.
        assert_eq!(esc("it's"), "it's");
    }

    #[test]
    fn a_slot_is_escaped_and_data_html_is_not() {
        let v = root([
            ("t", Value::text("a & <b>")),
            ("h", Value::html("<em>hi</em>")),
        ]);
        assert_eq!(
            ok(r#"<p data-slot="t"></p>"#, &v),
            "<p>a &amp; &lt;b&gt;</p>"
        );
        assert_eq!(ok(r#"<p data-html="h"></p>"#, &v), "<p><em>hi</em></p>");
    }

    #[test]
    fn a_slot_replaces_whatever_placeholder_the_file_held() {
        // The point of the design: the file is viewable HTML on its own.
        let v = root([("title", Value::text("real"))]);
        assert_eq!(
            ok(r#"<h1 data-slot="title">Example headline</h1>"#, &v),
            "<h1>real</h1>"
        );
    }

    #[test]
    fn markup_cannot_be_emitted_by_an_escaping_slot() {
        let v = root([("h", Value::html("<em>hi</em>"))]);
        let e = run("x\n<p data-slot=\"h\"></p>", &v).unwrap_err();
        assert_eq!(e.line, 2);
        assert!(e.msg.contains("is markup"), "got: {}", e.msg);
        assert!(
            e.msg.contains("data-html=\"h\""),
            "should show the fix: {}",
            e.msg
        );
    }

    #[test]
    fn an_unknown_field_is_an_error_not_a_blank() {
        let v = root([("title", Value::text("x"))]);
        let e = run("<p>one</p>\n<p data-slot=\"titel\"></p>", &v).unwrap_err();
        assert_eq!(e.line, 2);
        assert!(e.msg.contains("titel"), "got: {}", e.msg);
        assert_eq!(e.file, "t.html");
    }

    #[test]
    fn a_nil_binding_removes_the_element() {
        let v = root([("subtitle", Value::Nil), ("title", Value::text("t"))]);
        assert_eq!(
            ok(
                r#"<h1 data-slot="title"></h1><p data-slot="subtitle"></p>"#,
                &v
            ),
            "<h1>t</h1>"
        );
        // ...including everything nested inside it.
        assert_eq!(
            ok(r#"<div data-html="subtitle"><span>gone</span></div>"#, &v),
            ""
        );
    }

    #[test]
    fn data_when_keeps_or_drops_a_container_that_binds_nothing_else() {
        let box_ = r#"<div class="tag-box" data-when="tag"><p>filed under something</p></div>"#;
        assert_eq!(
            ok(
                box_,
                &root([("tag", Value::map([("id", Value::text("t"))]))])
            ),
            r#"<div class="tag-box"><p>filed under something</p></div>"#
        );
        assert_eq!(ok(box_, &root([("tag", Value::Nil)])), "");
        // It never emits its value, so binding a map is not an error.
        assert!(run(box_, &root([("tag", Value::list([Value::Num(1)]))])).is_ok());
    }

    #[test]
    fn markers_are_stripped_from_the_output() {
        let v = root([("t", Value::text("x")), ("u", Value::text("/p"))]);
        assert_eq!(
            ok(r#"<a class="k" data-slot="t" data-attr-href="u"></a>"#, &v),
            r#"<a class="k" href="/p">x</a>"#
        );
        // The only attribute left is a real one, in its original position.
        assert!(!ok(r#"<i data-slot="t"></i>"#, &v).contains("data-"));
    }

    #[test]
    fn data_attr_replaces_an_existing_value_and_appends_a_missing_one() {
        let v = root([("u", Value::text("/x")), ("c", Value::text("home"))]);
        assert_eq!(
            ok(r#"<a href="/placeholder" data-attr-href="u">t</a>"#, &v),
            r#"<a href="/x">t</a>"#
        );
        assert_eq!(
            ok(r#"<body data-attr-class="c">b</body>"#, &v),
            r#"<body class="home">b</body>"#
        );
        // Void and self-closing tags keep their shape.
        assert_eq!(
            ok(r#"<meta name="d" data-attr-content="c" />"#, &v),
            r#"<meta name="d" content="home" />"#
        );
    }

    #[test]
    fn an_attribute_value_is_escaped() {
        let v = root([("u", Value::text(r#"/a"b&c"#))]);
        assert_eq!(
            ok(r#"<a data-attr-href="u">t</a>"#, &v),
            r#"<a href="/a&quot;b&amp;c">t</a>"#
        );
    }

    #[test]
    fn each_clones_the_first_child_and_scopes_to_the_item() {
        let v = root([
            ("site", Value::text("blog")),
            (
                "posts",
                Value::list([
                    Value::map([("title", Value::text("a"))]),
                    Value::map([("title", Value::text("b"))]),
                ]),
            ),
        ]);
        // The prototype still sees the page scope, and surrounding whitespace
        // is template layout rather than output.
        assert_eq!(
            ok(
                "<ul data-each=\"posts\">\n  <li data-slot=\"title\"></li>\n</ul>",
                &v
            ),
            "<ul><li>a</li><li>b</li></ul>"
        );
        assert_eq!(
            ok(
                r#"<ul data-each="posts"><li data-slot="site"></li></ul>"#,
                &v
            ),
            "<ul><li>blog</li><li>blog</li></ul>"
        );
    }

    #[test]
    fn each_over_an_empty_list_removes_the_element() {
        let v = root([("posts", Value::List(Vec::new()))]);
        assert_eq!(ok(r#"<ul data-each="posts"><li>x</li></ul>"#, &v), "");
    }

    #[test]
    fn the_drop_rule_applies_inside_a_repeated_item() {
        let item = |t: &'static str, tag: Value<'static>| {
            Value::map([("t", Value::text(t)), ("tag", tag)])
        };
        let v = root([(
            "xs",
            Value::list([
                item("a", Value::map([("id", Value::text("z"))])),
                item("b", Value::Nil),
            ]),
        )]);
        assert_eq!(
            ok(
                r#"<ul data-each="xs"><li data-slot="t"><a data-attr-href="tag.id"></a></li></ul>"#,
                &v
            ),
            "<ul><li>a</li><li>b</li></ul>",
            "a slot writes the content, so the nested anchor is replaced either way"
        );
        assert_eq!(
            ok(
                r#"<ul data-each="xs"><li><a data-slot="tag.id"></a></li></ul>"#,
                &v
            ),
            "<ul><li><a>z</a></li><li></li></ul>",
            "the untagged item drops only the anchor"
        );
    }

    #[test]
    fn dotted_paths_walk_into_maps_and_stop_at_nil() {
        let v = root([
            (
                "tag",
                Value::map([("name", Value::text("t")), ("count", Value::num(2))]),
            ),
            ("none", Value::Nil),
        ]);
        assert_eq!(
            ok(
                r#"<p data-slot="tag.name"></p><i data-slot="tag.count"></i>"#,
                &v
            ),
            "<p>t</p><i>2</i>"
        );
        // A typo under a real map is still an error...
        assert!(run(r#"<p data-slot="tag.nope"></p>"#, &v).is_err());
        // ...but a path through a Nil is Nil, which drops the element.
        assert_eq!(ok(r#"<p data-slot="none.url"></p>"#, &v), "");
    }

    #[test]
    fn a_collection_has_no_text_form_and_a_scalar_is_not_a_list() {
        let v = root([("xs", Value::list([Value::Num(1)])), ("n", Value::Num(1))]);
        assert!(run(r#"<p data-slot="xs"></p>"#, &v)
            .unwrap_err()
            .msg
            .contains("collection"));
        assert!(run(r#"<ul data-each="n"><li>x</li></ul>"#, &v)
            .unwrap_err()
            .msg
            .contains("not a list"));
    }

    #[test]
    fn an_include_replaces_its_element_and_renders_in_the_callers_scope() {
        let v = root([(
            "posts",
            Value::list([
                Value::map([("title", Value::text("a"))]),
                Value::map([("title", Value::text("b"))]),
            ]),
        )]);
        let inc = [(
            "_list.html",
            r#"<ul data-each="posts"><li data-slot="title"></li></ul>"#,
        )];
        let out = render(
            Src {
                name: "t.html",
                text: r#"<main><div data-include="_list.html"></div></main>"#,
            },
            &v,
            &NOTHING,
            &inc,
        )
        .unwrap();
        // The placeholder element is gone, not merely emptied: the fragment has
        // to start with the list itself.
        assert_eq!(out, "<main><ul><li>a</li><li>b</li></ul></main>");
    }

    #[test]
    fn a_missing_include_is_an_error() {
        let e = run(
            "<p>a</p>\n<div data-include=\"_nope.html\"></div>",
            &root([]),
        )
        .unwrap_err();
        assert_eq!(e.line, 2);
        assert!(e.msg.contains("_nope.html"), "got: {}", e.msg);
    }

    #[test]
    fn an_include_cycle_is_caught_rather_than_overflowing_the_stack() {
        let inc = [("_a.html", r#"<div data-include="_a.html"></div>"#)];
        let e = render(
            Src {
                name: "t.html",
                text: r#"<div data-include="_a.html"></div>"#,
            },
            &root([]),
            &NOTHING,
            &inc,
        )
        .unwrap_err();
        assert!(e.msg.contains("cycle"), "got: {}", e.msg);
        assert_eq!(e.file, "_a.html", "should name the file that recurses");
    }

    #[test]
    fn an_error_inside_an_include_names_the_include() {
        let inc = [("_a.html", "<p>ok</p>\n<p data-slot=\"nope\"></p>")];
        let e = render(
            Src {
                name: "page.html",
                text: r#"<div data-include="_a.html"></div>"#,
            },
            &root([]),
            &NOTHING,
            &inc,
        )
        .unwrap_err();
        assert_eq!(e.file, "_a.html");
        assert_eq!(e.line, 2);
    }

    #[test]
    fn rawtext_is_copied_through_untouched() {
        // The inline theme script is full of braces, quotes and `<`.
        let script = r#"<script>if(a<b){var s='<div data-slot="x">'}</script>"#;
        assert_eq!(ok(script, &root([])), script);
        assert_eq!(
            ok("<style>a{b:c}</style>", &root([])),
            "<style>a{b:c}</style>"
        );
    }

    #[test]
    fn a_marker_on_a_rawtext_element_still_fills_it() {
        let v = root([("css", Value::html("a{b:c}")), ("js", Value::Nil)]);
        assert_eq!(
            ok(r#"<style data-html="css"></style>"#, &v),
            "<style>a{b:c}</style>"
        );
        // The dev-only live-reload script, absent in a production build.
        assert_eq!(ok(r#"<script data-html="js"></script>"#, &v), "");
    }

    #[test]
    fn unmarked_markup_passes_through_byte_for_byte() {
        for src in [
            "<!doctype html>\n<html><body><p>hi</p></body></html>",
            "<!-- a comment --><p>x</p>",
            "a < b and 1<2",
            "<p>plain</p>",
        ] {
            assert_eq!(ok(src, &root([])), src, "for {src:?}");
        }
    }

    #[test]
    fn multibyte_text_does_not_split_a_character() {
        let v = root([("a", Value::text("λ"))]);
        assert_eq!(
            ok("λ → <p data-slot=\"a\"></p> · ×", &v),
            "λ → <p>λ</p> · ×"
        );
    }

    #[test]
    fn a_page_shadows_the_site_and_falls_back_to_it() {
        let site = root([
            ("title", Value::text("the site")),
            ("author", Value::text("me")),
        ]);
        let page = root([("title", Value::text("this page"))]);
        let text = r#"<h1 data-slot="title"></h1><p data-slot="author"></p>"#;
        let out = render(
            Src {
                name: "t.html",
                text,
            },
            &page,
            &site,
            &[],
        )
        .unwrap();
        assert_eq!(out, "<h1>this page</h1><p>me</p>");
    }
}
