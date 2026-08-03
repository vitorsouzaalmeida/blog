use chrono::NaiveDate;

use crate::frontmatter::{self, Value};
use crate::markdown;

#[derive(Debug)]
pub struct Post {
    pub slug: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub pub_date: NaiveDate,
    pub tags: Vec<String>,
    pub thread: Option<String>,
    pub thread_order: Option<i64>,
    pub thread_title: Option<String>,
    pub thread_description: Option<String>,
    pub description: Option<String>,
    /// Built in `dev`, skipped in `prod`: a post you are still writing.
    pub draft: bool,
    pub body: String,
    pub html: String,
}

impl Post {
    pub fn summary(&self) -> Option<&str> {
        self.description.as_deref().or(self.subtitle.as_deref())
    }
}

const KEYS: [&str; 10] = [
    "title",
    "subtitle",
    "description",
    "draft",
    "pubDate",
    "tags",
    "thread",
    "threadOrder",
    "threadTitle",
    "threadDescription",
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

    let draft = match field("draft")? {
        None => false,
        Some("true") => true,
        Some("false") => false,
        Some(other) => return Err(format!("`draft` must be true or false, got {other:?}")),
    };

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
        thread_title: field("threadTitle")?.map(str::to_string),
        thread_description: field("threadDescription")?.map(str::to_string),
        draft,
        // Rendered here rather than by a second pass over the loaded posts,
        // which would mean constructing every `Post` with an `html` it does not
        // have yet and overwriting it.
        html: markdown::render(body),
        body: body.to_string(),
    })
}

/// A series of posts. Threads are not declared anywhere central: a post joins
/// one with `thread`, and exactly one post per thread carries the `threadTitle`
/// and `threadDescription` that name it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Thread<'a> {
    pub id: &'a str,
    pub title: &'a str,
    pub description: &'a str,
}

fn thread<'a>(posts: &'a [Post], id: &'a str) -> Result<Thread<'a>, String> {
    let declared: Vec<&Post> = posts
        .iter()
        .filter(|p| p.thread.as_deref() == Some(id))
        .filter(|p| p.thread_title.is_some() || p.thread_description.is_some())
        .collect();

    match declared.as_slice() {
        [] => Err(format!(
            "thread {id:?}: no post declares `threadTitle` and `threadDescription`; exactly one must"
        )),
        [p] => match (&p.thread_title, &p.thread_description) {
            (Some(title), Some(description)) => Ok(Thread {
                id,
                title,
                description,
            }),
            (None, _) => Err(format!("{}: `threadDescription` without `threadTitle`", p.slug)),
            (_, None) => Err(format!("{}: `threadTitle` without `threadDescription`", p.slug)),
        },
        many => Err(format!(
            "thread {id:?}: declared by {} posts ({}); exactly one must",
            many.len(),
            many.iter()
                .map(|p| p.slug.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

/// Every thread any post belongs to, ordered by id so the build is deterministic.
pub fn threads(posts: &[Post]) -> Result<Vec<Thread<'_>>, String> {
    posts
        .iter()
        .filter_map(|p| p.thread.as_deref())
        .collect::<std::collections::BTreeSet<&str>>()
        .into_iter()
        .map(|id| thread(posts, id))
        .collect()
}

pub fn thread_parts<'a>(posts: &'a [Post], id: &str) -> Vec<&'a Post> {
    let mut parts: Vec<&Post> = posts
        .iter()
        .filter(|p| p.thread.as_deref() == Some(id))
        .collect();
    parts.sort_by_key(|p| (p.thread_order.unwrap_or(i64::MAX), p.pub_date));
    parts
}

pub struct ThreadNav<'a> {
    pub thread: Thread<'a>,
    pub index: usize,
    pub total: usize,
    pub prev: Option<&'a Post>,
    pub next: Option<&'a Post>,
}

pub fn thread_nav<'a>(
    post: &Post,
    posts: &'a [Post],
    threads: &[Thread<'a>],
) -> Option<ThreadNav<'a>> {
    let id = post.thread.as_deref()?;
    let thread = *threads.iter().find(|t| t.id == id)?;
    let parts = thread_parts(posts, id);
    let i = parts.iter().position(|p| p.slug == post.slug)?;
    (parts.len() >= 2).then(|| ThreadNav {
        thread,
        index: i + 1,
        total: parts.len(),
        prev: i.checked_sub(1).and_then(|j| parts.get(j)).copied(),
        next: parts.get(i + 1).copied(),
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
    fn a_draft_says_so_and_anything_but_a_boolean_is_an_error() {
        let front = |extra: &str| format!("---\ntitle: X\npubDate: 2024-01-02\n{extra}---\nB\n");
        assert!(!parse(&front(""), "x").unwrap().draft);
        assert!(parse(&front("draft: true\n"), "x").unwrap().draft);
        assert!(!parse(&front("draft: false\n"), "x").unwrap().draft);
        assert!(parse(&front("draft: yes\n"), "x")
            .unwrap_err()
            .contains("true or false"));
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
            thread_title: None,
            thread_description: None,
            description: None,
            draft: false,
            body: String::new(),
            html: String::new(),
        }
    }

    fn threaded(slug: &str, id: &str, names: Option<(&str, &str)>) -> Post {
        Post {
            thread: Some(id.into()),
            thread_title: names.map(|(t, _)| t.into()),
            thread_description: names.map(|(_, d)| d.into()),
            ..tagged(slug, &[])
        }
    }

    #[test]
    fn a_thread_is_named_by_exactly_one_of_its_posts() {
        let posts = [
            threaded("a", "t", Some(("A Thread", "About things"))),
            threaded("b", "t", None),
        ];
        assert_eq!(
            threads(&posts).unwrap(),
            [Thread {
                id: "t",
                title: "A Thread",
                description: "About things"
            }]
        );
    }

    #[test]
    fn a_thread_nobody_names_is_an_error_not_a_blank_heading() {
        let posts = [threaded("a", "t", None), threaded("b", "t", None)];
        let err = threads(&posts).unwrap_err();
        assert!(err.contains("no post declares"), "got: {err}");
    }

    #[test]
    fn two_posts_naming_the_same_thread_is_an_error() {
        let posts = [
            threaded("a", "t", Some(("One", "d"))),
            threaded("b", "t", Some(("Two", "d"))),
        ];
        let err = threads(&posts).unwrap_err();
        assert!(err.contains("declared by 2 posts"), "got: {err}");
        assert!(err.contains("a, b"), "the error should name them: {err}");
    }

    #[test]
    fn half_a_declaration_is_an_error() {
        let posts = [Post {
            thread_description: None,
            ..threaded("a", "t", Some(("One", "d")))
        }];
        assert!(threads(&posts).unwrap_err().contains("without"));
    }

    #[test]
    fn a_single_part_thread_places_no_posts() {
        // One post is not a series, so it gets no "part 1 of 1" tag anywhere.
        let posts = [threaded("a", "t", Some(("One", "d")))];
        let ts = threads(&posts).unwrap();
        assert!(thread_nav(&posts[0], &posts, &ts).is_none());
        // ...but the thread page is still built, so the id must survive.
        assert_eq!(ts.len(), 1);
    }

    #[test]
    fn parts_order_by_thread_order_then_date() {
        let ordered = |slug: &str, order: Option<i64>, date: &str| Post {
            thread_order: order,
            pub_date: self::date(date),
            ..threaded(slug, "t", None)
        };
        let posts = [
            ordered("c", None, "2024-03-01"),
            ordered("a", Some(1), "2024-02-01"),
            ordered("b", None, "2024-01-01"),
        ];
        let slugs: Vec<&str> = thread_parts(&posts, "t")
            .iter()
            .map(|p| p.slug.as_str())
            .collect();
        assert_eq!(slugs, ["a", "b", "c"]);
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
