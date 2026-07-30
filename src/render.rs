use std::cmp::Reverse;
use std::collections::HashMap;

use crate::config::{self, Ctx};
use crate::content::Post;
use crate::threads::{Placement, Thread, ThreadNav};

const PRELOAD_FONTS: [&str; 2] = [
    "/fonts/newsreader-300-700-normal-latin.woff2",
    "/fonts/jetbrainsmono-400-normal-latin.woff2",
];

/// The inlined assets every page needs, minified once per build.
#[derive(Clone, Copy)]
pub struct Page<'a> {
    pub css: &'a str,
    pub highlight: &'a str,
    pub script: &'a str,
}

fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |h, b| {
        (h ^ *b as u64).wrapping_mul(0x100000001b3)
    })
}

/// Content-addressed so the script can be cached forever.
pub fn script_path(body: &str) -> String {
    format!("/vendor/app.{:016x}.js", fnv1a(body.as_bytes()))
}

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

fn url_enc(s: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    s.bytes()
        .fold(String::with_capacity(s.len()), |mut out, b| {
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

pub fn tag_path(tag: &str) -> String {
    url_enc(tag)
}

pub fn tag_href(tag: &str) -> String {
    format!("/tag/{}", tag_path(tag))
}

fn link(href: &str, inner: &str, external: bool, class: Option<&str>) -> String {
    let cls = class.map(|c| format!(" class=\"{c}\"")).unwrap_or_default();
    if external {
        format!(
            "<a href=\"{}\"{} target=\"_blank\" rel=\"noreferrer noopener\" hx-boost=\"false\">{}</a>",
            esc(href), cls, inner
        )
    } else {
        format!("<a href=\"{}\"{}>{}</a>", esc(href), cls, inner)
    }
}

fn nav_back() -> String {
    format!(
        "<div class=\"nav-back\">{}</div>",
        link("/", &format!("&larr; {}", esc(config::AUTHOR)), false, None)
    )
}

const THEME_TOGGLE: &str = r#"
  <button id="theme-toggle" aria-label="Toggle theme">
    <svg class="sun-icon" width="13" height="13" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
      <circle cx="12" cy="12" r="5" stroke="currentColor" stroke-width="2"></circle>
      <path d="M12 1v6M12 17v6M23 12h-6M7 12H1M20.07 3.93l-4.24 4.24M8.17 15.83l-4.24 4.24M20.07 20.07l-4.24-4.24M8.17 8.17L3.93 3.93" stroke="currentColor" stroke-width="2" stroke-linecap="round"></path>
    </svg>
    <svg class="moon-icon" width="13" height="13" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
      <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"></path>
    </svg>
  </button>"#;

#[derive(Clone, Copy)]
pub enum Footer {
    Split,
    Centered,
}

fn footer(variant: Footer, max_width: &str, year: i32) -> String {
    let nav_links = [
        ("Github", "https://github.com/vitorsouzaalmeida/", true),
        (
            "LinkedIn",
            "https://www.linkedin.com/in/vitorsalmeida/",
            true,
        ),
        ("Blog", "/blog", false),
        ("Tags", "/tags", false),
        ("RSS", "/rss.xml", true),
    ];
    let meta = format!("<div class=\"footer-meta\"><span>&copy; {year}</span>{THEME_TOGGLE}</div>");

    match variant {
        Footer::Centered => {
            let items: String = nav_links
                .iter()
                .map(|(t, h, e)| format!("<li>{}</li>", link(h, t, *e, None)))
                .collect();
            format!(
                "<footer class=\"site-footer footer-centered\" style=\"max-width:{max_width}\">\n      <nav><ul class=\"footer-nav\">{items}</ul></nav>\n      {meta}\n    </footer>"
            )
        }
        Footer::Split => {
            let items: String = nav_links
                .iter()
                .map(|(t, h, e)| {
                    let hide = if *t == "Tags" || *t == "RSS" {
                        " class=\"mobile-hidden\""
                    } else {
                        ""
                    };
                    format!("<li{hide}>{}</li>", link(h, t, *e, None))
                })
                .collect();
            format!(
                "<footer class=\"site-footer footer-split\" style=\"max-width:{max_width}\">\n      {meta}\n      <nav><ul class=\"footer-nav footer-nav-end\">{items}</ul></nav>\n    </footer>"
            )
        }
    }
}

pub fn post_list(posts: &[&Post], tm: &HashMap<&str, Placement>) -> String {
    let mut sorted: Vec<&Post> = posts.to_vec();
    sorted.sort_by_key(|p| Reverse(p.pub_date));
    let items: String = sorted
        .iter()
        .map(|post| {
            let thread = tm
                .get(post.slug.as_str())
                .map(|pl| {
                    link(
                        &format!("/thread/{}", pl.thread.id),
                        &format!(
                            "&#8627; {} &middot; part {}",
                            esc(pl.thread.title),
                            pl.index
                        ),
                        false,
                        Some("thread-tag"),
                    )
                })
                .unwrap_or_default();
            format!(
                "<li class=\"post-item\">\n        <time class=\"post-date\">{}</time>\n        <div class=\"post-item-body\">\n          {}\n          {}\n        </div>\n      </li>",
                post.pub_date,
                link(&format!("/blog/{}", post.slug), &esc(&post.title), false, Some("post-link")),
                thread
            )
        })
        .collect();
    format!("<ul class=\"post-list\">{items}</ul>")
}

const THEME_SCRIPT: &str = r#"(function(){
  try{
    var t = localStorage.getItem('theme') || (window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light');
    document.documentElement.setAttribute('data-theme', t === 'dark' ? 'dark' : 'light');
  }catch(e){}
  if(!window.__themeBound){
    window.__themeBound = true;
    document.addEventListener('click', function(e){
      var btn = e.target.closest && e.target.closest('#theme-toggle');
      if(!btn) return;
      var el = document.documentElement;
      var dark = el.getAttribute('data-theme') === 'dark';
      var next = dark ? 'light' : 'dark';
      el.setAttribute('data-theme', next);
      try{ localStorage.setItem('theme', next); }catch(e){}
    });
  }
})();"#;

const LIVE_RELOAD: &str = r#"<script>(function(){try{new EventSource('/__livereload').onmessage=function(e){if(e.data==='reload')location.reload()}}catch(e){}})();</script>"#;

pub struct Layout {
    pub title: String,
    pub description: String,
    pub og_image: String,
    pub path: String,
    pub max_width: String,
    pub footer: Footer,
    pub body: String,
    pub highlighted: bool,
}

impl Default for Layout {
    fn default() -> Self {
        Layout {
            title: config::TITLE.to_string(),
            description: config::DESCRIPTION.to_string(),
            og_image: "/og_default.jpg".to_string(),
            path: "/".to_string(),
            max_width: "640px".to_string(),
            footer: Footer::Split,
            body: String::new(),
            highlighted: false,
        }
    }
}

pub fn layout(ctx: Ctx, page: Page, o: Layout) -> String {
    let canonical_e = esc(&format!("{}{}", config::WEBSITE, o.path));
    let og_url_e = esc(&format!("{}{}", config::WEBSITE, o.og_image));
    let title = esc(&o.title);
    let description = esc(&o.description);
    let author = esc(config::AUTHOR);
    let site_title = esc(config::TITLE);
    let footer = footer(o.footer, &o.max_width, ctx.year);
    let live = if ctx.live_reload { LIVE_RELOAD } else { "" };
    let body = o.body;
    let max_width = o.max_width;

    let preloads: String = PRELOAD_FONTS
        .iter()
        .map(|href| {
            format!(
                "\n    <link rel=\"preload\" href=\"{href}\" as=\"font\" type=\"font/woff2\" crossorigin />"
            )
        })
        .collect();
    let css = page.css;
    let highlight = if o.highlighted {
        format!("\n    <style>{}</style>", page.highlight)
    } else {
        String::new()
    };
    let script = page.script;

    format!(
        r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    {preloads}
    <script>{THEME_SCRIPT}</script>

    <title>{title}</title>
    <meta name="description" content="{description}" />
    <meta name="author" content="{author}" />

    <meta property="og:image" content="{og_url_e}" />
    <meta property="og:image:width" content="1200" />
    <meta property="og:image:height" content="630" />
    <meta property="og:title" content="{title}" />
    <meta property="og:description" content="{description}" />
    <meta property="og:url" content="{canonical_e}" />
    <meta property="og:type" content="website" />

    <meta property="twitter:card" content="summary_large_image" />
    <meta property="twitter:image" content="{og_url_e}" />
    <meta property="twitter:url" content="{canonical_e}" />
    <meta property="twitter:title" content="{title}" />
    <meta property="twitter:description" content="{description}" />

    <link rel="canonical" href="{canonical_e}" />
    <link rel="alternate" type="application/rss+xml" title="{site_title}" href="/rss.xml" />
    <link rel="sitemap" href="/sitemap.xml" />

    <style>{css}</style>{highlight}

    <script defer src="{script}"></script>
  </head>
  <body hx-boost="true" hx-ext="head-support,preload,morph" hx-swap="morph:innerHTML" preload="mouseover">
    <main style="max-width:{max_width}">
{body}
    </main>
    {footer}
    {live}
  </body>
</html>"#
    )
}

pub fn home_page(ctx: Ctx, page: Page, posts: &[Post], tm: &HashMap<&str, Placement>) -> String {
    let mut recent: Vec<&Post> = posts.iter().collect();
    recent.sort_by_key(|p| Reverse(p.pub_date));
    recent.truncate(5);
    let yo = ctx.year - config::BIRTH_YEAR;

    let nav_items = format!(
        "<li>{}</li><li>{}</li><li>{}</li>",
        link("/blog", "posts", false, None),
        link("/tags", "tags", false, None),
        link("/rss.xml", "rss", true, None),
    );

    let body = format!(
        r#"
    <div class="home-hero">
      <h1 class="site-title">{author}</h1>
      <p class="tagline">Thoughts on PL, CS &amp; Math</p>
      <nav class="home-nav"><ul>{nav_items}</ul></nav>
      <p class="bio">Living in Brazil, {yo}yo. In my free time, I like to learn about CS, philosophy, read books/mangas, care for my health, and occasionally write something here.</p>
    </div>
    <div class="home-posts">
      <div class="rule"></div>
      <p class="section-label">Posts</p>
      {list}
      <div class="see-all">{see_all}</div>
    </div>"#,
        author = esc(config::AUTHOR),
        list = post_list(&recent, tm),
        see_all = link("/blog", "see all posts &rarr;", false, Some("accent-link")),
    );

    layout(
        ctx,
        page,
        Layout {
            title: config::TITLE.to_string(),
            path: "/".to_string(),
            max_width: "760px".to_string(),
            footer: Footer::Centered,
            body,
            ..Default::default()
        },
    )
}

pub fn blog_index_page(
    ctx: Ctx,
    page: Page,
    posts: &[Post],
    tm: &HashMap<&str, Placement>,
) -> String {
    let all: Vec<&Post> = posts.iter().collect();
    let body = format!(
        "\n    {}\n    <h1 class=\"page-title\">all posts</h1>\n    {}",
        nav_back(),
        post_list(&all, tm)
    );
    layout(
        ctx,
        page,
        Layout {
            title: format!("posts | {}", config::TITLE),
            path: "/blog".to_string(),
            body,
            ..Default::default()
        },
    )
}

pub fn post_page(ctx: Ctx, page: Page, post: &Post, nav: Option<&ThreadNav>) -> String {
    let date_part = post.pub_date.format("%Y · %m · %d");
    let tags_html = if post.tags.is_empty() {
        String::new()
    } else {
        let joined: String = post
            .tags
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let sep = if i > 0 { " · " } else { "" };
                format!("{sep}{}", link(&tag_href(t), &esc(t), false, None))
            })
            .collect();
        format!(" — {joined}")
    };

    let subtitle = post
        .subtitle
        .as_ref()
        .map(|s| format!("<p class=\"post-subtitle\">{}</p>", esc(s)))
        .unwrap_or_default();

    let thread_box = nav
        .map(|n| {
            let step = |p: Option<&Post>, arrow_before: bool, class| {
                p.map(|p| {
                    let t = esc(&p.title);
                    let label = if arrow_before {
                        format!("&larr; {t}")
                    } else {
                        format!("{t} &rarr;")
                    };
                    link(&format!("/blog/{}", p.slug), &label, false, class)
                })
                .unwrap_or_else(|| "<span></span>".to_string())
            };
            let nav_row = if n.prev.is_some() || n.next.is_some() {
                format!(
                    "<div class=\"thread-box-nav\">{}{}</div>",
                    step(n.prev, true, None),
                    step(n.next, false, Some("thread-next"))
                )
            } else {
                String::new()
            };
            format!(
                "\n      <div class=\"thread-box\">\n        <p class=\"thread-box-title\">\n          {}\n          <span> &middot; part {} of {}</span>\n        </p>\n        {nav_row}\n      </div>",
                link(
                    &format!("/thread/{}", n.thread.id),
                    &format!("&#8627; {}", esc(n.thread.title)),
                    false,
                    Some("accent-link")
                ),
                n.index,
                n.total,
            )
        })
        .unwrap_or_default();

    let body = format!(
        "\n    {}\n    <header class=\"post-header\">\n      <p class=\"post-meta\">{date_part}{tags_html}</p>\n      <h1 class=\"post-title\">{}</h1>\n      {subtitle}\n      {thread_box}\n    </header>\n    <article>{}</article>",
        nav_back(),
        esc(&post.title),
        post.html
    );

    layout(
        ctx,
        page,
        Layout {
            title: format!("{} | {}", post.title, config::TITLE),
            description: post.summary().unwrap_or(config::DESCRIPTION).to_string(),
            og_image: format!("/blog/{}/og.png", post.slug),
            path: format!("/blog/{}", post.slug),
            highlighted: post.html.contains("<pre class=\"hl\">"),
            body,
            ..Default::default()
        },
    )
}

pub fn tags_page(ctx: Ctx, page: Page, tag_counts: &[(&str, usize)]) -> String {
    let items: String = tag_counts
        .iter()
        .map(|(tag, count)| {
            let path = tag_path(tag);
            let a = format!(
                "<a href=\"/tag/{path}\" class=\"tag-link\" hx-get=\"/tag/{path}/partial\" hx-target=\"#tag-results\" hx-swap=\"innerHTML\" hx-push-url=\"/tag/{path}/\">{}</a>",
                esc(tag)
            );
            format!("<li>{a}<span class=\"tag-count\"> ({count})</span></li>")
        })
        .collect();
    let body = format!(
        "\n    {}\n    <h1 class=\"page-title\">tags</h1>\n    <ul class=\"tag-cloud\">{items}</ul>\n    <div id=\"tag-results\" class=\"tag-results\"></div>",
        nav_back()
    );
    layout(
        ctx,
        page,
        Layout {
            title: format!("tags | {}", config::TITLE),
            path: "/tags".to_string(),
            body,
            ..Default::default()
        },
    )
}

pub fn tag_page(
    ctx: Ctx,
    page: Page,
    tag: &str,
    posts: &[&Post],
    tm: &HashMap<&str, Placement>,
) -> String {
    let body = format!(
        "\n    {}\n    <p class=\"section-label section-label-left\">Tag &middot; {}</p>\n    <div id=\"tag-results\" class=\"tag-results\">{}</div>",
        nav_back(),
        esc(tag),
        post_list(posts, tm)
    );
    layout(
        ctx,
        page,
        Layout {
            title: format!("{tag} | {}", config::TITLE),
            path: tag_href(tag),
            body,
            ..Default::default()
        },
    )
}

pub fn tag_partial(posts: &[&Post], tm: &HashMap<&str, Placement>) -> String {
    post_list(posts, tm)
}

pub fn thread_page(ctx: Ctx, page: Page, thread: &Thread, parts: &[&Post]) -> String {
    let items: String = parts
        .iter()
        .enumerate()
        .map(|(i, post)| {
            format!(
                "\n      <li class=\"post-item\">\n        <span class=\"post-date\">part {}</span>\n        <div class=\"post-item-body\">\n          {}\n          <span class=\"thread-part-date\">{}</span>\n        </div>\n      </li>",
                i + 1,
                link(&format!("/blog/{}", post.slug), &esc(&post.title), false, Some("post-link")),
                post.pub_date
            )
        })
        .collect();
    let body = format!(
        "\n    {}\n    <p class=\"section-label section-label-left\">Thread</p>\n    <h1 class=\"thread-title\">{}</h1>\n    <p class=\"thread-desc\">{}</p>\n    <ol class=\"post-list thread-list\">{items}</ol>",
        nav_back(),
        esc(thread.title),
        esc(thread.description)
    );
    layout(
        ctx,
        page,
        Layout {
            title: format!("{} | {}", thread.title, config::TITLE),
            path: format!("/thread/{}", thread.id),
            body,
            ..Default::default()
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn url_encoding() {
        assert_eq!(url_enc("computer-science"), "computer-science");
        assert_eq!(url_enc("c++"), "c%2B%2B");
        assert_eq!(url_enc("a b"), "a%20b");
        assert_eq!(url_enc("R&D"), "R%26D");
        assert_eq!(url_enc("λ"), "%CE%BB");
    }

    #[test]
    fn tag_href_and_tag_path_agree() {
        // The href and the output directory must be the same string, or the
        // link 404s. This is the invariant the old code broke.
        for tag in ["code", "c++", "type theory", "R&D"] {
            assert_eq!(tag_href(tag), format!("/tag/{}", tag_path(tag)));
        }
    }

    #[test]
    fn script_path_tracks_content() {
        assert_eq!(script_path("a"), script_path("a"));
        assert_ne!(script_path("a"), script_path("b"));
        assert!(script_path("a").starts_with("/vendor/app."));
    }
}
