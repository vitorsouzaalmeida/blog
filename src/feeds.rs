use std::cmp::Reverse;

use crate::config;
use crate::content::Post;
use crate::render::{esc, tag_path};

fn absolutize(html: &str) -> String {
    ["src", "href"].iter().fold(html.to_string(), |out, attr| {
        let needle = format!("{attr}=\"/");
        let replacement = format!("{attr}=\"{}/", config::WEBSITE);
        out.split(&needle)
            .enumerate()
            .map(|(i, part)| match i {
                0 => part.to_string(),
                _ if part.starts_with('/') => format!("{needle}{part}"),
                _ => format!("{replacement}{part}"),
            })
            .collect()
    })
}

fn cdata(inner: &str) -> String {
    format!("<![CDATA[{}]]>", inner.replace("]]>", "]]]]><![CDATA[>"))
}

pub fn rss(posts: &[&Post]) -> String {
    let mut ps: Vec<&Post> = posts.to_vec();
    ps.sort_by_key(|p| Reverse(p.pub_date));
    let items: String = ps
        .iter()
        .map(|p| {
            let url = format!("{}/blog/{}", config::WEBSITE, p.slug);
            format!(
                "    <item>\n      <title>{}</title>\n      <link>{url}</link>\n      <guid isPermaLink=\"true\">{url}</guid>\n      <pubDate>{}</pubDate>\n      <description>{}</description>\n      <content:encoded>{}</content:encoded>\n    </item>",
                esc(&p.title),
                p.pub_date.rfc822(),
                esc(p.summary().unwrap_or_default()),
                cdata(&absolutize(&p.html))
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<rss version=\"2.0\" xmlns:atom=\"http://www.w3.org/2005/Atom\" xmlns:content=\"http://purl.org/rss/1.0/modules/content/\">\n  <channel>\n    <title>{}</title>\n    <description>{}</description>\n    <link>{}</link>\n    <atom:link href=\"{}/rss.xml\" rel=\"self\" type=\"application/rss+xml\" />\n{items}\n  </channel>\n</rss>\n",
        esc(config::TITLE),
        esc(config::DESCRIPTION),
        config::WEBSITE,
        config::WEBSITE
    )
}

pub fn sitemap(posts: &[&Post], tags: &[&str], thread_ids: &[&str]) -> String {
    let site = config::WEBSITE;
    let fixed = ["".to_string(), "blog/".to_string(), "tags/".to_string()];
    let locs = fixed
        .iter()
        .cloned()
        .chain(posts.iter().map(|p| format!("blog/{}/", p.slug)))
        .chain(tags.iter().map(|t| format!("tag/{}/", tag_path(t))))
        .chain(thread_ids.iter().map(|id| format!("thread/{id}/")))
        .map(|rel| format!("  <url><loc>{}</loc></url>", esc(&format!("{site}/{rel}"))))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n{locs}\n</urlset>\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absolutize_rewrites_root_relative_urls_only() {
        assert_eq!(
            absolutize(r#"<img src="/introbigo/bigo.jpg" /><a href="/blog/x">x</a>"#),
            format!(
                r#"<img src="{w}/introbigo/bigo.jpg" /><a href="{w}/blog/x">x</a>"#,
                w = config::WEBSITE
            )
        );

        let untouched = r#"<img src="//cdn.example.com/pic.png" /><img src="https://x.com/a.png" /><img src="img/b.png" />"#;
        assert_eq!(absolutize(untouched), untouched);
    }

    #[test]
    fn cdata_survives_a_literal_terminator() {
        for inner in ["a ]]> b", "]]>]]>", "trailing ]]>", "]]> leading"] {
            let out = cdata(inner);
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
            let out = cdata(inner);
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
}
