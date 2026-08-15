use crate::content::Post;
use crate::{AUTHOR, BIRTH_YEAR, DESCRIPTION, TITLE, WEBSITE};

const LAYOUT: &str = include_str!("../templates/layout.html");
const HOME: &str = include_str!("../templates/home.html");
const LISTING: &str = include_str!("../templates/listing.html");
const POST: &str = include_str!("../templates/post.html");
const ARTICLE: &str = include_str!("../templates/article.html");
const POST_ITEM: &str = include_str!("../templates/post_item.html");
const LINK_ITEM: &str = include_str!("../templates/link_item.html");
const LINK_ITEM_EXT: &str = include_str!("../templates/link_item_ext.html");
const TAG_ITEM: &str = include_str!("../templates/tag_item.html");
const TAG_LINK: &str = include_str!("../templates/tag_link.html");
const SUBTITLE: &str = include_str!("../templates/subtitle.html");
const RELOAD: &str = include_str!("../templates/reload.html");

pub fn fill(template: &str, holes: &[(&str, &str)]) -> String {
    template
        .split("{{")
        .enumerate()
        .map(|(i, chunk)| match i {
            0 => chunk.to_string(),
            _ => match chunk.split_once("}}") {
                Some((name, rest)) => {
                    let value = holes
                        .iter()
                        .find(|(hole, _)| *hole == name)
                        .unwrap_or_else(|| panic!("unknown template hole {{{{{name}}}}}"))
                        .1;
                    format!("{value}{rest}")
                }
                None => format!("{{{{{chunk}"),
            },
        })
        .collect()
}

pub fn esc(raw: &str) -> String {
    raw.chars()
        .map(|c| match c {
            '&' => "&amp;".into(),
            '<' => "&lt;".into(),
            '>' => "&gt;".into(),
            '"' => "&quot;".into(),
            '\'' => "&#39;".into(),
            other => other.to_string(),
        })
        .collect()
}

pub fn tag_slug(tag: &str) -> String {
    tag.chars()
        .map(
            |c| match c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                true => c.to_string(),
                false => c
                    .to_string()
                    .bytes()
                    .map(|b| format!("%{b:02X}"))
                    .collect::<String>(),
            },
        )
        .collect()
}

pub fn post_url(slug: &str) -> String {
    format!("/blog/{slug}/")
}

pub fn fragment_url(slug: &str) -> String {
    format!("/blog/{slug}/fragment")
}

pub fn tag_url(tag: &str) -> String {
    format!("/tag/{}/", tag_slug(tag))
}

fn boostable(href: &str) -> bool {
    href.starts_with('/') && href.ends_with('/')
}

fn date(post: &Post) -> String {
    post.pub_date.format("%Y · %m · %d").to_string()
}

pub struct Page<'a> {
    pub title: &'a str,
    pub description: &'a str,
    pub canonical: &'a str,
    pub body: String,
}

pub fn layout(page: &Page, year: i32, live_reload: Option<&str>) -> String {
    let reload = live_reload
        .map(|js| fill(RELOAD, &[("js", js)]))
        .unwrap_or_default();

    fill(
        LAYOUT,
        &[
            ("title", &esc(page.title)),
            ("description", &esc(page.description)),
            ("author", &esc(AUTHOR)),
            ("site_title", &esc(TITLE)),
            ("website", WEBSITE),
            ("canonical", page.canonical),
            ("year", &year.to_string()),
            ("reload", &reload),
            ("body", &page.body),
        ],
    )
}

fn post_item(post: &Post) -> String {
    fill(
        POST_ITEM,
        &[
            ("date", &date(post)),
            ("url", &post_url(&post.slug)),
            ("title", &esc(&post.title)),
            ("fragment", &fragment_url(&post.slug)),
        ],
    )
}

fn post_items(posts: &[&Post]) -> String {
    posts.iter().map(|post| post_item(post)).collect()
}

fn listing(back_url: &str, back_label: &str, heading: &str, class: &str, items: &str) -> String {
    fill(
        LISTING,
        &[
            ("back_url", back_url),
            ("back_label", back_label),
            ("heading", heading),
            ("class", class),
            ("items", items),
        ],
    )
}

pub fn home(posts: &[Post], year: i32) -> Page<'static> {
    let recent: Vec<&Post> = posts.iter().take(5).collect();
    let links = [
        ("github", "https://github.com/vitorsouzaalmeida/"),
        ("linkedin", "https://www.linkedin.com/in/vitorsalmeida/"),
        ("tags", "/tags/"),
        ("rss", "/rss.xml"),
    ];
    let link_items: String = links
        .iter()
        .map(|(name, href)| {
            let template = match boostable(href) {
                true => LINK_ITEM,
                false => LINK_ITEM_EXT,
            };
            fill(template, &[("href", href), ("name", name)])
        })
        .collect();

    let body = fill(
        HOME,
        &[
            ("title", &esc(TITLE)),
            ("age", &(year - BIRTH_YEAR).to_string()),
            ("links", &link_items),
            ("posts", &post_items(&recent)),
        ],
    );

    Page {
        title: TITLE,
        description: DESCRIPTION,
        canonical: "/",
        body,
    }
}

pub fn blog_index(posts: &[Post]) -> Page<'static> {
    let all: Vec<&Post> = posts.iter().collect();

    Page {
        title: "posts | vitor s. almeida",
        description: DESCRIPTION,
        canonical: "/blog/",
        body: listing("/", &esc(AUTHOR), "all posts", "posts", &post_items(&all)),
    }
}

fn article(post: &Post) -> String {
    let tags: String = post
        .tags
        .iter()
        .map(|tag| fill(TAG_LINK, &[("url", &tag_url(tag)), ("name", &esc(tag))]))
        .collect();
    let subtitle = post
        .subtitle
        .as_deref()
        .map(|text| fill(SUBTITLE, &[("text", &esc(text))]))
        .unwrap_or_default();

    fill(
        ARTICLE,
        &[
            ("title", &esc(&post.title)),
            ("date", &date(post)),
            ("tags", &tags),
            ("subtitle", &subtitle),
            ("html", &post.html),
        ],
    )
}

pub fn post_page<'a>(post: &'a Post, canonical: &'a str) -> Page<'a> {
    Page {
        title: &post.title,
        description: post.summary().unwrap_or(DESCRIPTION),
        canonical,
        body: fill(
            POST,
            &[("author", &esc(AUTHOR)), ("article", &article(post))],
        ),
    }
}

pub fn post_fragment(post: &Post) -> String {
    article(post)
}

pub fn tags_index(tags: &[(&str, usize)]) -> Page<'static> {
    let items: String = tags
        .iter()
        .map(|(tag, count)| {
            fill(
                TAG_ITEM,
                &[
                    ("url", &tag_url(tag)),
                    ("name", &esc(tag)),
                    ("count", &count.to_string()),
                ],
            )
        })
        .collect();

    Page {
        title: "tags | vitor s. almeida",
        description: DESCRIPTION,
        canonical: "/tags/",
        body: listing("/", &esc(AUTHOR), "tags", "links", &items),
    }
}

pub fn tag_page<'a>(tag: &'a str, posts: &[&Post], canonical: &'a str) -> Page<'a> {
    let heading = format!("#{}", esc(tag));

    Page {
        title: tag,
        description: DESCRIPTION,
        canonical,
        body: listing("/tags/", "tags", &heading, "posts", &post_items(posts)),
    }
}
