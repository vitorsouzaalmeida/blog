//! Binds the model to the `templates/` route tree.
//!
//! A template's path *is* its output path, so the directory is the route table:
//!
//! ```text
//! templates/index.html             -> dist/index.html              canonical /
//! templates/blog/index.html        -> dist/blog/index.html         canonical /blog
//! templates/blog/[slug]/index.html -> dist/blog/<slug>/index.html  one per post
//! ```
//!
//! Two rules: a basename starting with `_` is an include rather than a route,
//! and a route named `index.html` is wrapped in `_layout.html` while any other
//! file is emitted bare.
//!
//! A page declares the rest of itself in an HTML comment at the top of the
//! file -- invisible in a browser, so the template still opens standalone, and
//! parsed by the same frontmatter parser posts use:
//!
//! ```text
//! each:  a collection in the model; the page is emitted once per item, and the
//!        `[field]` segment of its path is filled from that item
//! title: the document title, where `{field}` interpolates the page's own
//!        fields and then the site's
//! class: goes on <body>; column width and footer layout co-vary with nothing
//!        but which kind of page this is
//! ```
//!
//! Any other key is an alias binding a page-scope name to a field of the site
//! map, which is how home shows `posts: recent`.
//!
//! Nothing here emits HTML -- the markup is in `templates/`, the values are in
//! `model`, and `fill` puts the two together.

use std::borrow::Cow;

use crate::fill::{self, Includes, Src, Value};
use crate::frontmatter;

/// Metadata keys a page uses for itself. Anything else is an alias.
const RESERVED: [&str; 3] = ["each", "title", "class"];

pub struct Route<'a> {
    /// Output path, still holding its `[name]` segment if it has one.
    pub out: &'a str,
    pub param: Option<&'a str>,
    pub layout: bool,
    src: &'a str,
    meta: Vec<(&'a str, Cow<'a, str>)>,
}

impl<'a> Route<'a> {
    fn meta(&self, key: &str) -> Option<&str> {
        self.meta
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| v.as_ref())
    }

    pub fn each(&self) -> Option<&str> {
        self.meta("each")
    }

    fn aliases(&self) -> impl Iterator<Item = (&'a str, &str)> {
        self.meta
            .iter()
            .filter(|(k, _)| !RESERVED.contains(k))
            .map(|(k, v)| (*k, v.as_ref()))
    }
}

/// The `[name]` segment of an output path, if it has one.
pub fn param_of(out: &str) -> Option<&str> {
    out.split('/')
        .find_map(|seg| seg.strip_prefix('[')?.strip_suffix(']'))
}

fn is_partial(out: &str) -> bool {
    out.rsplit('/')
        .next()
        .is_some_and(|name| name.starts_with('_'))
}

/// `blog/foo/index.html` -> `/blog/foo`; `index.html` -> `/`.
pub fn canonical(out: &str) -> String {
    let path = out.strip_suffix("index.html").unwrap_or(out);
    format!("/{}", path.trim_end_matches('/'))
}

pub struct Templates<'a> {
    pub routes: Vec<Route<'a>>,
    includes: Vec<(&'a str, &'a str)>,
    layout: Option<&'a str>,
}

/// Sorts the template files into routes, includes and the layout, reading each
/// route's metadata block. An include is looked up by file name where it is
/// used; nothing else is parsed here.
pub fn load(files: &[(String, String)]) -> Result<Templates<'_>, String> {
    let files: Vec<(&str, &str)> = files
        .iter()
        .map(|(path, src)| (path.as_str(), src.as_str()))
        .collect();

    let (includes, route_files): (Vec<_>, Vec<_>) =
        files.into_iter().partition(|(p, _)| is_partial(p));

    Ok(Templates {
        routes: route_files
            .into_iter()
            .map(|(out, src)| {
                Ok(Route {
                    out,
                    param: param_of(out),
                    layout: out == "index.html" || out.ends_with("/index.html"),
                    src,
                    meta: parse_meta(out, src)?,
                })
            })
            .collect::<Result<Vec<Route>, String>>()?,
        layout: includes
            .iter()
            .find(|(p, _)| *p == "_layout.html")
            .map(|(_, src)| *src),
        includes,
    })
}

type Meta<'a> = Vec<(&'a str, Cow<'a, str>)>;

fn parse_meta<'a>(out: &str, text: &'a str) -> Result<Meta<'a>, String> {
    let Some(rest) = text.strip_prefix("<!--") else {
        return Ok(Vec::new());
    };
    let Some(end) = rest.find("-->") else {
        return Err(format!("{out}: the metadata comment is never closed"));
    };

    // No key whitelist: a key is either reserved or an alias, and an alias is
    // checked against the site map, so `clas: post` still fails the build.
    frontmatter::parse(&rest[..end], &[])
        .map_err(|e| format!("{out}: {e}"))?
        .into_iter()
        .map(|(key, value)| match value {
            frontmatter::Value::Scalar(s) => Ok((key, s)),
            frontmatter::Value::Seq(_) => Err(format!("{out}: `{key}` must be a single value")),
        })
        .collect()
}

/// The rendered page without its metadata block. `fill` copies comments through
/// verbatim, so the block is dropped from the output rather than the input --
/// which keeps the line numbers in an error the ones in the file.
fn without_meta(html: &str) -> &str {
    match html.strip_prefix("<!--") {
        None => html,
        Some(rest) => match rest.find("-->") {
            None => html,
            Some(end) => rest[end + 3..].trim_start_matches('\n'),
        },
    }
}

/// The collection each page expands, which is where every URL in the model
/// comes from. Only `index.html` routes define one, and two routes claiming the
/// same collection would make an item's URL ambiguous.
pub fn collections<'a>(templates: &'a Templates) -> Result<Vec<(&'a str, &'a str)>, String> {
    let claims: Vec<(&str, &str)> = templates
        .routes
        .iter()
        .filter(|r| r.layout)
        .filter_map(|r| r.each().map(|name| (name, r.out)))
        .collect();

    match claims
        .iter()
        .enumerate()
        .find(|(i, (name, _))| claims[..*i].iter().any(|(other, _)| other == name))
    {
        None => Ok(claims),
        Some((_, (name, out))) => Err(format!(
            "{out}: `{name}` is already claimed by another page"
        )),
    }
}

/// One output file: where it goes, and the item it is about.
pub struct Page<'a> {
    pub out: String,
    item: Option<&'a Value<'a>>,
}

pub fn expand<'a>(route: &Route, site: &'a Value<'a>) -> Result<Vec<Page<'a>>, String> {
    match (route.each(), route.param) {
        (None, None) => Ok(vec![Page {
            out: route.out.to_string(),
            item: None,
        }]),

        (Some(name), Some(field)) => {
            let Some(Value::List(items)) = site.get(name) else {
                return Err(format!("{}: no collection named `{name}`", route.out));
            };
            items
                .iter()
                .map(|item| {
                    let Some(Value::Text(seg)) = item.get(field) else {
                        let msg = format!("items of `{name}` have no text field `{field}`");
                        return Err(format!("{}: {msg}", route.out));
                    };
                    Ok(Page {
                        out: route.out.replace(&format!("[{field}]"), seg),
                        item: Some(item),
                    })
                })
                .collect()
        }

        (Some(_), None) => Err(format!(
            "{}: `each:` needs a [field] segment in the path",
            route.out
        )),
        (None, Some(f)) => Err(format!(
            "{}: [{f}] segment but no `each:` to fill it",
            route.out
        )),
    }
}

pub fn render(
    route: &Route,
    templates: &Templates,
    site: &Value,
    page: &Page,
) -> Result<String, String> {
    // The page's own fields: the item it is about, then whatever it aliases.
    // Both shadow the site map, which is the fallback scope.
    let item = match page.item {
        Some(Value::Map(fields)) => fields.clone(),
        _ => Vec::new(),
    };
    let fields: Vec<(&str, Value)> = item.into_iter().chain(aliases(route, site)?).collect();

    let title = interpolate(route.meta("title").unwrap_or("{site_title}"), &fields, site)
        .map_err(|e| format!("{}: {e}", route.out))?;
    let scope = Value::Map(
        [
            // `head_title`, not `title`: a post page binds its own `title` for
            // the <h1>, and the two must not shadow each other.
            ("head_title", Value::text(title)),
            (
                "canonical",
                Value::text(format!("{}{}", crate::WEBSITE, canonical(&page.out))),
            ),
            (
                "body_class",
                Value::text(route.meta("class").unwrap_or("page")),
            ),
        ]
        .into_iter()
        .chain(fields)
        .collect(),
    );

    let inc: Includes = &templates.includes;
    let filled =
        fill::render(src(route.out, route.src), &scope, site, inc).map_err(|e| e.to_string())?;
    let body = without_meta(&filled);

    match (route.layout, templates.layout, scope) {
        (false, _, _) | (_, None, _) => Ok(body.to_string()),
        (true, Some(layout), Value::Map(fields)) => {
            let scope = Value::Map(
                fields
                    .into_iter()
                    .chain([("body", Value::html(body))])
                    .collect(),
            );
            fill::render(src("_layout.html", layout), &scope, site, inc).map_err(|e| e.to_string())
        }
        (true, Some(_), _) => unreachable!("scope is always a map"),
    }
}

fn src<'a>(name: &'a str, text: &'a str) -> Src<'a> {
    Src { name, text }
}

/// A metadata key that is not reserved binds a page-scope name to a field of
/// the site map: home writes `posts: recent` and gets the five newest under the
/// name `_post_list.html` already binds.
fn aliases<'a, 's>(
    route: &Route<'a>,
    site: &'s Value<'s>,
) -> Result<Vec<(&'a str, Value<'s>)>, String> {
    route
        .aliases()
        .map(|(key, target)| match site.get(target) {
            Some(value) => Ok((key, value.clone())),
            None => Err(format!(
                "{}: `{key}: {target}` -- the site has no `{target}`",
                route.out
            )),
        })
        .collect()
}

/// The only interpolation in the project, and it is confined to metadata
/// strings: `{field}` is replaced from the page's own fields, then the site's.
/// "There is no template language" stays true of the HTML body, which is where
/// it matters.
fn interpolate(text: &str, fields: &[(&str, Value)], site: &Value) -> Result<String, String> {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;

    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let Some(len) = rest[open..].find('}') else {
            return Err(format!("unclosed `{{` in {text:?}"));
        };
        let key = &rest[open + 1..open + len];
        let found = fields
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| v)
            .or_else(|| site.get(key));

        match found {
            Some(Value::Text(s)) => out.push_str(s),
            Some(Value::Num(n)) => out.push_str(&n.to_string()),
            _ => return Err(format!("`{{{key}}}` names no text field")),
        }
        rest = &rest[open + len + 1..];
    }
    Ok(out + rest)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn templates(files: &[(&str, &str)]) -> Vec<(String, String)> {
        files
            .iter()
            .map(|(p, s)| (p.to_string(), s.to_string()))
            .collect()
    }

    #[test]
    fn a_templates_path_is_its_output_path() {
        assert_eq!(canonical("index.html"), "/");
        assert_eq!(canonical("blog/index.html"), "/blog");
        assert_eq!(canonical("blog/a-post/index.html"), "/blog/a-post");
        assert_eq!(canonical("tag/c%2B%2B/index.html"), "/tag/c%2B%2B");
    }

    #[test]
    fn dynamic_segments_are_recognised_anywhere_in_the_path() {
        assert_eq!(param_of("blog/[slug]/index.html"), Some("slug"));
        assert_eq!(param_of("blog/index.html"), None);
        assert_eq!(param_of("index.html"), None);
    }

    #[test]
    fn underscored_files_are_partials_not_routes() {
        assert!(is_partial("_layout.html"));
        assert!(is_partial("_post_list.html"));
        assert!(!is_partial("index.html"));
        assert!(!is_partial("blog/[slug]/index.html"));
    }

    #[test]
    fn only_index_files_get_the_layout() {
        let files = templates(&[
            ("index.html", "a"),
            ("tag/[slug]/index.html", "b"),
            ("feed/preview.html", "c"),
            ("_layout.html", "L"),
        ]);
        let t = load(&files).unwrap();
        let layout_of = |out: &str| t.routes.iter().find(|r| r.out == out).unwrap().layout;
        assert!(layout_of("index.html"));
        assert!(layout_of("tag/[slug]/index.html"));
        assert!(!layout_of("feed/preview.html"));
        assert_eq!(t.routes.len(), 3, "the layout is not a route");
        assert_eq!(t.layout, Some("L"));
    }

    #[test]
    fn a_metadata_comment_declares_the_page_and_is_not_copied_to_the_output() {
        let files = templates(&[(
            "blog/[slug]/index.html",
            "<!--\neach: posts\nclass: post\n-->\n<h1>x</h1>",
        )]);
        let t = load(&files).unwrap();
        let route = &t.routes[0];
        assert_eq!(route.each(), Some("posts"));
        assert_eq!(route.meta("class"), Some("post"));

        let site = Value::map([("site_title", Value::text("blog"))]);
        let page = Page {
            out: "blog/a/index.html".to_string(),
            item: None,
        };
        let out = render(route, &t, &site, &page).unwrap();
        assert_eq!(out, "<h1>x</h1>", "the build config must not ship");
    }

    #[test]
    fn an_error_names_the_line_of_the_file_it_is_on() {
        // The metadata block is stripped from the output rather than the input,
        // so what the compiler counts is what the editor shows.
        let files = templates(&[(
            "index.html",
            "<!--\nclass: page\n-->\n<p>ok</p>\n<p data-slot=\"nope\"></p>",
        )]);
        let t = load(&files).unwrap();
        let page = Page {
            out: "index.html".to_string(),
            item: None,
        };
        let site = Value::map([("site_title", Value::text("blog"))]);
        let err = render(&t.routes[0], &t, &site, &page).unwrap_err();
        assert!(err.contains("line 5"), "got: {err}");
    }

    #[test]
    fn a_page_with_no_metadata_is_still_a_page() {
        let files = templates(&[("index.html", "<h1>x</h1>")]);
        let t = load(&files).unwrap();
        assert_eq!(t.routes[0].each(), None);
        assert_eq!(without_meta(t.routes[0].src), "<h1>x</h1>");
    }

    #[test]
    fn two_pages_may_not_claim_one_collection() {
        let each = "<!--\neach: posts\n-->";
        let files = templates(&[
            ("blog/[slug]/index.html", each),
            ("post/[slug]/index.html", each),
        ]);
        let err = collections(&load(&files).unwrap()).unwrap_err();
        assert!(err.contains("already claimed"), "got: {err}");
    }

    #[test]
    fn expansion_fills_the_path_segment_from_each_item() {
        let files = templates(&[("blog/[slug]/index.html", "<!--\neach: posts\n-->\n<p>x</p>")]);
        let t = load(&files).unwrap();
        let site = Value::map([(
            "posts",
            Value::list([
                Value::map([("slug", Value::text("a"))]),
                Value::map([("slug", Value::text("b"))]),
            ]),
        )]);
        let outs: Vec<String> = expand(&t.routes[0], &site)
            .unwrap()
            .into_iter()
            .map(|p| p.out)
            .collect();
        assert_eq!(outs, ["blog/a/index.html", "blog/b/index.html"]);
    }

    #[test]
    fn a_route_and_its_collection_have_to_agree() {
        let site = Value::map([("posts", Value::list([]))]);
        let one = |out: &'static str, src: &'static str| {
            let files = templates(&[(out, src)]);
            let t = load(&files).unwrap();
            expand(&t.routes[0], &site).map(|_| ()).unwrap_err()
        };
        assert!(one("x/[slug]/index.html", "<!--\neach: nope\n-->").contains("no collection"));
        assert!(one("x/index.html", "<!--\neach: posts\n-->").contains("[field] segment"));
        assert!(one("x/[slug]/index.html", "<p>x</p>").contains("no `each:`"));
    }

    #[test]
    fn metadata_interpolates_the_page_scope_and_then_the_site() {
        let site = Value::map([("site_title", Value::text("blog"))]);
        let fields = [("title", Value::text("A post"))];
        assert_eq!(
            interpolate("{title} | {site_title}", &fields, &site).unwrap(),
            "A post | blog"
        );
        assert_eq!(interpolate("about", &fields, &site).unwrap(), "about");
        assert!(interpolate("{nope}", &fields, &site).is_err());
        assert!(interpolate("{title", &fields, &site).is_err());
    }
}
