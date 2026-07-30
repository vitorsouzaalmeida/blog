use chrono::NaiveDate;

use crate::frontmatter::{self, Value};

#[derive(Debug)]
pub struct Post {
    pub slug: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub pub_date: NaiveDate,
    pub tags: Vec<String>,
    pub thread: Option<String>,
    pub thread_order: Option<i64>,
    pub description: Option<String>,
    pub body: String,
    pub html: String,
}

impl Post {
    pub fn summary(&self) -> Option<&str> {
        self.description.as_deref().or(self.subtitle.as_deref())
    }
}

const KEYS: [&str; 7] = [
    "title",
    "subtitle",
    "description",
    "pubDate",
    "tags",
    "thread",
    "threadOrder",
];

fn split_frontmatter(raw: &str) -> Result<(&str, &str), String> {
    let after_open = ["---\n", "---\r\n"]
        .into_iter()
        .find_map(|fence| raw.strip_prefix(fence))
        .ok_or("missing opening `---` on a line of its own (frontmatter is required)")?;
    let open = raw.len() - after_open.len();

    after_open
        .split_inclusive('\n')
        .scan(open, |offset, line| {
            let start = *offset;
            *offset += line.len();
            Some((start, line))
        })
        .find(|(_, line)| line.trim_end() == "---")
        .map(|(start, line)| {
            (
                &raw[open..start],
                raw[start + line.len()..].trim_start_matches(['\r', '\n']),
            )
        })
        .ok_or_else(|| "missing closing `---` (frontmatter is never terminated)".to_string())
}

fn scalar<'a>(fields: &'a [(&str, Value)], key: &str) -> Result<Option<&'a str>, String> {
    match fields.iter().find(|(k, _)| *k == key) {
        None => Ok(None),
        Some((_, Value::Scalar(s))) => Ok(Some(s.as_ref())),
        Some(_) => Err(format!("`{key}` must be a single value, not a list")),
    }
}

pub fn parse(raw: &str, slug: &str) -> Result<Post, String> {
    let (front, body) = split_frontmatter(raw)?;
    let fields = frontmatter::parse(front, &KEYS).map_err(|e| e.to_string())?;
    let field = |key| scalar(&fields, key);

    let tags = match fields.iter().find(|(k, _)| *k == "tags") {
        None => Vec::new(),
        Some((_, Value::Seq(items))) => items.iter().map(|t| t.to_string()).collect(),
        Some(_) => return Err("`tags` must be a list".into()),
    };

    let thread_order = field("threadOrder")?
        .map(|v| {
            v.parse()
                .map_err(|_| format!("`threadOrder` must be a whole number, got {v:?}"))
        })
        .transpose()?;

    let pub_date = {
        let raw = field("pubDate")?.ok_or("missing `pubDate`")?;
        NaiveDate::parse_from_str(raw, "%Y-%m-%d")
            .map_err(|e| format!("invalid `pubDate` {raw:?}: {e}"))?
    };

    Ok(Post {
        slug: slug.to_string(),
        title: field("title")?.ok_or("missing `title`")?.to_string(),
        subtitle: field("subtitle")?.map(str::to_string),
        description: field("description")?.map(str::to_string),
        pub_date,
        tags,
        thread: field("thread")?.map(str::to_string),
        thread_order,
        body: body.to_string(),
        html: String::new(),
    })
}

pub fn tag_counts(posts: &[Post]) -> Vec<(&str, usize)> {
    let mut counts: Vec<(&str, usize)> = posts
        .iter()
        .flat_map(|p| p.tags.iter())
        .fold(std::collections::BTreeMap::new(), |mut acc, tag| {
            *acc.entry(tag.as_str()).or_insert(0) += 1;
            acc
        })
        .into_iter()
        .collect();
    counts.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    counts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn frontmatter_parses_both_tag_forms_and_unquotes() {
        let raw = "---\ntitle: \"Rust: a tour\"\npubDate: 2024-05-06\ntags:\n  - code\n  - math\nthread: t1\nthreadOrder: 2\n---\n\nBody text here\n";
        let p = parse(raw, "hello").unwrap();
        assert_eq!(p.title, "Rust: a tour");
        assert_eq!(p.slug, "hello");
        assert_eq!(p.tags, ["code", "math"]);
        assert_eq!(p.thread.as_deref(), Some("t1"));
        assert_eq!(p.thread_order, Some(2));
        assert_eq!(p.pub_date, date("2024-05-06"));
        assert_eq!(p.body, "Body text here\n");

        let inline = parse(
            "---\ntitle: X\npubDate: 2024-01-02\ntags: [\"c++\", 'type theory']\n---\nB\n",
            "x",
        )
        .unwrap();
        assert_eq!(inline.tags, ["c++", "type theory"]);
        assert!(inline.thread.is_none());
    }

    #[test]
    fn every_post_in_the_repository_parses() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("content/blog");
        let posts: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "md"))
            .collect();
        assert!(!posts.is_empty(), "no posts found in {}", dir.display());
        for path in posts {
            let raw = std::fs::read_to_string(&path).unwrap();
            let slug = path.file_stem().unwrap().to_string_lossy().to_string();
            let post = parse(&raw, &slug).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            assert!(!post.title.is_empty(), "{} has an empty title", slug);
        }
    }

    #[test]
    fn a_scaffolded_post_parses() {
        let raw = "---\ntitle: My Post\npubDate: 2026-07-28\ntags:\n  - tag1\n# Optional frontmatter:\n# subtitle: A short italic subtitle\n# description: Used for RSS + social meta\n# thread: some-thread-id\n# threadOrder: 1\n---\n\nContent\n";
        let p = parse(raw, "my-post").unwrap();
        assert_eq!(p.title, "My Post");
        assert_eq!(p.tags, ["tag1"]);
        assert_eq!(p.subtitle, None);
        assert_eq!(p.body, "Content\n");
    }

    #[test]
    fn a_setext_underline_in_the_body_does_not_close_the_frontmatter() {
        let raw = "---\ntitle: X\npubDate: 2024-01-02\n---\nHeading\n----\ntext\n";
        let p = parse(raw, "x").unwrap();
        assert_eq!(p.title, "X");
        assert_eq!(p.body, "Heading\n----\ntext\n");
    }

    #[test]
    fn a_malformed_post_is_an_error_not_a_blank_page() {
        let err = parse("---\ntitle: X\npubDate: 2024-01-02\n\nBody\n", "x").unwrap_err();
        assert!(err.contains("closing"), "got: {err}");
        assert!(parse("Just a body, no frontmatter.\n", "x").is_err());
        assert!(parse("---\npubDate: 2024-01-02\n---\nB\n", "x").is_err());
        assert!(parse("---\ntitle: X\n---\nB\n", "x").is_err());
    }

    fn tagged(slug: &str, tags: &[&str]) -> Post {
        Post {
            slug: slug.into(),
            title: slug.into(),
            subtitle: None,
            pub_date: date("2024-01-01"),
            tags: tags.iter().map(|t| t.to_string()).collect(),
            thread: None,
            thread_order: None,
            description: None,
            body: String::new(),
            html: String::new(),
        }
    }

    #[test]
    fn tag_counts_order_by_count_then_name() {
        let posts = [
            tagged("a", &["code", "math"]),
            tagged("b", &["code", "zed"]),
            tagged("c", &["code", "math"]),
        ];
        assert_eq!(tag_counts(&posts), [("code", 3), ("math", 2), ("zed", 1)]);
    }
}
