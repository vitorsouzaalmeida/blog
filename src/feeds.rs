use crate::content::Post;
use crate::render::post_url;
use crate::WEBSITE;

fn esc_xml(raw: &str) -> String {
    raw.chars()
        .filter(|c| {
            matches!(c, '\t' | '\n' | '\r' | ' '..='\u{d7ff}' | '\u{e000}'..='\u{fffd}')
                || *c >= '\u{10000}'
        })
        .map(|c| match c {
            '&' => "&amp;".into(),
            '<' => "&lt;".into(),
            '>' => "&gt;".into(),
            '"' => "&quot;".into(),
            '\'' => "&apos;".into(),
            other => other.to_string(),
        })
        .collect()
}

fn cdata(raw: &str) -> String {
    format!("<![CDATA[{}]]>", raw.replace("]]>", "]]]]><![CDATA[>"))
}

fn replace_urls(chunk: &str, base: &str) -> String {
    chunk
        .replace("href=\"/", &format!("href=\"{base}/"))
        .replace("src=\"/", &format!("src=\"{base}/"))
}

fn absolutize(html: &str, base: &str) -> String {
    html.split_inclusive("</code>")
        .map(|chunk| match chunk.find("<code") {
            Some(at) => replace_urls(&chunk[..at], base) + &chunk[at..],
            None => replace_urls(chunk, base),
        })
        .collect()
}

fn item(post: &Post) -> String {
    let url = format!("{WEBSITE}{}", post_url(&post.slug));
    let description = post
        .summary()
        .map(|s| format!("<description>{}</description>", esc_xml(s)))
        .unwrap_or_default();

    format!(
        "<item><title>{title}</title><link>{url}</link><guid isPermaLink=\"true\">{url}</guid><pubDate>{date}</pubDate>{description}<content:encoded>{body}</content:encoded></item>",
        title = esc_xml(&post.title),
        date = post.pub_date.format("%a, %d %b %Y 00:00:00 GMT"),
        body = cdata(&absolutize(&post.html, WEBSITE)),
    )
}

pub fn rss(posts: &[&Post]) -> String {
    let items: String = posts.iter().map(|p| item(p)).collect();

    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<rss version=\"2.0\" xmlns:atom=\"http://www.w3.org/2005/Atom\" xmlns:content=\"http://purl.org/rss/1.0/modules/content/\"><channel><title>{title}</title><description>{description}</description><link>{WEBSITE}</link><atom:link href=\"{WEBSITE}/rss.xml\" rel=\"self\" type=\"application/rss+xml\"/>{items}</channel></rss>\n",
        title = esc_xml(crate::TITLE),
        description = esc_xml(crate::DESCRIPTION),
    )
}

pub fn sitemap(pages: &[String]) -> String {
    let urls: String = pages
        .iter()
        .map(|path| {
            format!(
                "<url><loc>{}</loc></url>",
                esc_xml(&format!("{WEBSITE}{path}"))
            )
        })
        .collect();

    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">{urls}</urlset>\n"
    )
}
