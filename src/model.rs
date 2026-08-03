//! The one value every page is rendered against.
//!
//! Deliberately denormalised: each tag carries its own `posts`. A tag page then
//! writes `data-each="posts"` and the tag's list shadows the site-wide one
//! through `fill`'s scope chain, which is what removes the need for a query
//! language. At six posts the duplication is pointer copies.
//!
//! Ordering is baked in here -- `posts` is always newest first -- so no
//! template ever expresses a sort. Exactly one field is `Value::Html`, a post's
//! `html`, which keeps `grep -rn data-html templates/` the whole audit surface
//! for unescaped output.
//!
//! Every URL is built from the route that emits it (see `Routes`), so a link
//! cannot point somewhere the build did not write.

use std::cmp::Reverse;

use crate::content::{self, Post};
use crate::fill::Value;
use crate::routes;
use crate::Ctx;

/// Where each collection's pages go, as `(collection, output path)`, read off
/// the route table before anything is rendered.
pub type Routes<'a> = [(&'a str, &'a str)];

/// An output path and the item field its `[segment]` names.
type Pattern<'a> = (&'a str, &'a str);

/// The route patterns the model builds URLs from.
struct Where<'a> {
    posts: Pattern<'a>,
    tags: Pattern<'a>,
}

fn pattern<'a>(routes: &Routes<'a>, collection: &str) -> Result<Pattern<'a>, String> {
    let (_, out) = routes
        .iter()
        .find(|(name, _)| *name == collection)
        .ok_or_else(|| format!("no page declares `each: {collection}`"))?;
    let param = routes::param_of(out)
        .ok_or_else(|| format!("{out}: `each: {collection}` needs a [field] path segment"))?;
    Ok((out, param))
}

fn path((out, param): Pattern, value: &str) -> String {
    routes::canonical(&out.replace(&format!("[{param}]"), value))
}

/// The URL of one item, for a caller outside the model -- `build` writes an OG
/// image next to the page it belongs to.
pub fn url_of(routes: &Routes, collection: &str, slug: &str) -> Result<String, String> {
    Ok(path(pattern(routes, collection)?, slug))
}

/// Percent-encoded, because a tag is prose: `c++` becomes `c%2B%2B`.
pub fn tag_slug(tag: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    tag.bytes()
        .fold(String::with_capacity(tag.len()), |mut out, b| {
            if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' {
                out.push(b as char);
            } else {
                out.push('%');
                out.push(HEX[(b >> 4) as usize] as char);
                out.push(HEX[(b & 0xf) as usize] as char);
            }
            out
        })
}

pub fn build<'a>(
    posts: &'a [Post],
    css: &'a str,
    ctx: Ctx,
    routes: &Routes,
) -> Result<Value<'a>, String> {
    let at = Where {
        posts: pattern(routes, "posts")?,
        tags: pattern(routes, "tags")?,
    };

    let items: Vec<Value<'a>> = newest_first(posts)
        .into_iter()
        .map(|p| post_item(p, &at))
        .collect();

    Ok(Value::map([
        ("site_title", Value::text(crate::TITLE)),
        ("author", Value::text(crate::AUTHOR)),
        ("description", Value::text(crate::DESCRIPTION)),
        (
            "image",
            Value::text(format!("{}/og_default.jpg", crate::WEBSITE)),
        ),
        ("year", Value::Num(ctx.year as i64)),
        ("age", Value::Num((ctx.year - crate::BIRTH_YEAR) as i64)),
        ("css", Value::html(css)),
        ("live_reload", live_reload(ctx.live_reload)),
        (
            "recent",
            Value::List(items.iter().take(5).cloned().collect()),
        ),
        (
            "tags",
            Value::list(
                content::tag_counts(posts)
                    .into_iter()
                    .map(|(tag, count)| tag_item(tag, count, posts, &at)),
            ),
        ),
        ("posts", Value::List(items)),
    ]))
}

/// `Nil` in a production build, which removes the `<script>` that binds it.
fn live_reload<'a>(on: bool) -> Value<'a> {
    match on {
        false => Value::Nil,
        true => Value::html(crate::dev::live_reload()),
    }
}

fn newest_first<'a>(posts: impl IntoIterator<Item = &'a Post>) -> Vec<&'a Post> {
    let mut sorted: Vec<&Post> = posts.into_iter().collect();
    sorted.sort_by_key(|p| Reverse(p.pub_date));
    sorted
}

/// One date format, the one every template's placeholder already shows.
fn date(post: &Post) -> String {
    post.pub_date.format("%Y · %m · %d").to_string()
}

fn post_item<'a>(post: &'a Post, at: &Where) -> Value<'a> {
    let url = path(at.posts, &post.slug);
    let tags = post.tags.iter().map(|tag| {
        Value::map([
            ("url", Value::text(path(at.tags, &tag_slug(tag)))),
            ("name", Value::text(tag.as_str())),
        ])
    });

    Value::map([
        ("slug", Value::text(post.slug.as_str())),
        ("title", Value::text(post.title.as_str())),
        ("subtitle", Value::opt(post.subtitle.as_deref())),
        ("date", Value::text(date(post))),
        // Always text, never `Nil`: a `Nil` would shadow the site default and
        // delete the `<meta name="description">` element instead of falling
        // back to it.
        (
            "description",
            Value::text(post.summary().unwrap_or(crate::DESCRIPTION)),
        ),
        (
            "image",
            Value::text(format!("{}{}", crate::WEBSITE, og_image(&url))),
        ),
        ("tags", Value::list(tags)),
        ("html", Value::html(post.html.as_str())),
        ("url", Value::text(url)),
    ])
}

/// A post's social image sits next to the page it belongs to.
pub fn og_image(post_url: &str) -> String {
    format!("{post_url}/og.png")
}

fn tag_item<'a>(tag: &'a str, count: usize, posts: &'a [Post], at: &Where) -> Value<'a> {
    let slug = tag_slug(tag);
    let tagged = posts.iter().filter(|p| p.tags.iter().any(|t| t == tag));
    Value::map([
        ("url", Value::text(path(at.tags, &slug))),
        ("slug", Value::text(slug)),
        ("name", Value::text(tag)),
        ("count", Value::num(count)),
        (
            "posts",
            Value::list(newest_first(tagged).into_iter().map(|p| post_item(p, at))),
        ),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn post(slug: &str, tags: &[&str]) -> Post {
        Post {
            slug: slug.into(),
            title: slug.into(),
            subtitle: None,
            pub_date: NaiveDate::parse_from_str("2024-01-02", "%Y-%m-%d").unwrap(),
            tags: tags.iter().map(|t| t.to_string()).collect(),
            description: None,
            draft: false,
            body: String::new(),
            html: "<p>hi</p>".into(),
        }
    }

    const ROUTES: [(&str, &str); 2] = [
        ("posts", "blog/[slug]/index.html"),
        ("tags", "tag/[slug]/index.html"),
    ];

    fn site(posts: &[Post]) -> Value<'_> {
        build(posts, "", Ctx::prod(2026), &ROUTES).unwrap()
    }

    fn text<'a>(v: &'a Value, path: &str) -> &'a str {
        match path.split('.').try_fold(v, |acc, key| acc.get(key)) {
            Some(Value::Text(s)) => s.as_ref(),
            other => panic!("{path}: expected text, got {other:?}"),
        }
    }

    fn first<'a>(v: &'a Value, key: &str) -> &'a Value<'a> {
        match v.get(key) {
            Some(Value::List(items)) => &items[0],
            other => panic!("{key}: expected a list, got {other:?}"),
        }
    }

    #[test]
    fn urls_come_from_the_route_that_emits_the_page() {
        let posts = [post("a-post", &["c++"])];
        let site = site(&posts);
        assert_eq!(text(first(&site, "posts"), "url"), "/blog/a-post");
        assert_eq!(text(first(&site, "tags"), "url"), "/tag/c%2B%2B");
        // ...and the OG image sits next to the page.
        assert!(text(first(&site, "posts"), "image").ends_with("/blog/a-post/og.png"));
    }

    #[test]
    fn a_page_with_no_route_is_a_build_error_not_a_dead_link() {
        let posts = [post("a", &[])];
        let err = build(&posts, "", Ctx::prod(2026), &ROUTES[..1]).unwrap_err();
        assert!(err.contains("each: tags"), "got: {err}");
    }

    #[test]
    fn posts_are_newest_first_everywhere_they_appear() {
        let dated = |slug: &str, d: &str| Post {
            pub_date: NaiveDate::parse_from_str(d, "%Y-%m-%d").unwrap(),
            ..post(slug, &["code"])
        };
        let posts = [
            dated("old", "2024-01-01"),
            dated("new", "2026-01-01"),
            dated("mid", "2025-01-01"),
        ];
        let site = site(&posts);
        assert_eq!(text(first(&site, "posts"), "slug"), "new");
        assert_eq!(text(first(&site, "recent"), "slug"), "new");
        assert_eq!(text(first(first(&site, "tags"), "posts"), "slug"), "new");
    }

    #[test]
    fn a_post_without_a_summary_inherits_the_site_description() {
        // The `Nil` trap: an absent description must not shadow the site's, or
        // the drop rule deletes the <meta> element instead of falling back.
        let posts = [post("a", &[])];
        let site = site(&posts);
        assert_eq!(
            text(first(&site, "posts"), "description"),
            crate::DESCRIPTION
        );
        assert_eq!(first(&site, "posts").get("subtitle"), Some(&Value::Nil));
    }

    #[test]
    fn a_tag_carries_its_own_posts_so_a_tag_page_needs_no_query() {
        let posts = [post("a", &["code"]), post("b", &["math"])];
        let site = site(&posts);
        let tag = first(&site, "tags");
        assert_eq!(text(tag, "name"), "code");
        match tag.get("posts") {
            Some(Value::List(items)) => assert_eq!(items.len(), 1),
            other => panic!("expected the tag's own posts, got {other:?}"),
        }
    }

    #[test]
    fn tag_slugs_are_percent_encoded() {
        assert_eq!(tag_slug("computer-science"), "computer-science");
        assert_eq!(tag_slug("c++"), "c%2B%2B");
        assert_eq!(tag_slug("a b"), "a%20b");
        assert_eq!(tag_slug("R&D"), "R%26D");
        assert_eq!(tag_slug("λ"), "%CE%BB");
    }
}
