use std::borrow::Cow;
use std::cmp::Reverse;

use crate::config;
use crate::content::Post;
use crate::html;
use crate::render::tag_path;
use crate::xml::{self, Node};

fn item<'a>(post: &'a Post) -> Node<'a> {
    let url = format!("{}/blog/{}", config::WEBSITE, post.slug);
    let head = [
        Node::line("title", post.title.as_str()),
        Node::line("link", url.clone()),
        Node::elem(
            "guid",
            [("isPermaLink", Cow::Borrowed("true"))],
            [Node::text(url)],
        ),
        Node::line("pubDate", post.pub_date.rfc822()),
    ];

    let description = post.summary().map(|s| Node::line("description", s));
    let body = Node::elem(
        "content:encoded",
        [],
        [Node::cdata(html::absolutize(&post.html, config::WEBSITE))],
    );
    Node::tag("item", head.into_iter().chain(description).chain([body]))
}

pub fn rss(posts: &[&Post]) -> String {
    let mut ps: Vec<&Post> = posts.to_vec();
    ps.sort_by_key(|p| Reverse(p.pub_date));

    let head = [
        Node::line("title", config::TITLE),
        Node::line("description", config::DESCRIPTION),
        Node::line("link", config::WEBSITE),
        Node::elem(
            "atom:link",
            [
                ("href", Cow::Owned(format!("{}/rss.xml", config::WEBSITE))),
                ("rel", Cow::Borrowed("self")),
                ("type", Cow::Borrowed("application/rss+xml")),
            ],
            [],
        ),
    ];
    let channel = Node::tag(
        "channel",
        head.into_iter().chain(ps.iter().map(|p| item(p))),
    );

    xml::document(&Node::elem(
        "rss",
        [
            ("version", Cow::Borrowed("2.0")),
            ("xmlns:atom", Cow::Borrowed("http://www.w3.org/2005/Atom")),
            (
                "xmlns:content",
                Cow::Borrowed("http://purl.org/rss/1.0/modules/content/"),
            ),
        ],
        [channel],
    ))
}

pub fn sitemap(posts: &[&Post], tags: &[&str], thread_ids: &[&str]) -> String {
    let site = config::WEBSITE;
    let fixed = ["".to_string(), "blog/".to_string(), "tags/".to_string()];
    let urls = fixed
        .into_iter()
        .chain(posts.iter().map(|p| format!("blog/{}/", p.slug)))
        .chain(tags.iter().map(|t| format!("tag/{}/", tag_path(t))))
        .chain(thread_ids.iter().map(|id| format!("thread/{id}/")))
        .map(|rel| Node::tag("url", [Node::line("loc", format!("{site}/{rel}"))]));

    xml::document(&Node::elem(
        "urlset",
        [(
            "xmlns",
            Cow::Borrowed("http://www.sitemaps.org/schemas/sitemap/0.9"),
        )],
        urls,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::date::Date;

    fn post(title: &str, summary: Option<&str>) -> Post {
        Post {
            slug: "s".into(),
            title: title.into(),
            subtitle: summary.map(str::to_string),
            pub_date: Date::parse("2024-01-02").unwrap(),
            tags: Vec::new(),
            thread: None,
            thread_order: None,
            description: None,
            body: String::new(),
            html: "<p>hi</p>".into(),
        }
    }

    #[test]
    fn a_title_with_a_control_character_still_produces_wellformed_xml() {
        let feed = rss(&[&post("we\u{8}ird & <sharp>", None)]);
        assert!(
            !feed.contains('\u{8}'),
            "control character reached the feed"
        );
        assert!(feed.contains("<title>weird &amp; &lt;sharp&gt;</title>"));
    }

    #[test]
    fn an_item_without_a_summary_omits_the_description_element() {
        assert!(!rss(&[&post("x", None)]).contains("<description></description>"));
        assert!(
            rss(&[&post("x", Some("a summary"))]).contains("<description>a summary</description>")
        );
    }
}
