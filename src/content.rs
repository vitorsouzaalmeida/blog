use std::cmp::Reverse;
use std::collections::BTreeMap;

use chrono::NaiveDate;

use crate::markdown;

#[derive(Debug)]
pub struct Post {
    pub slug: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub pub_date: NaiveDate,
    pub tags: Vec<String>,
    pub description: Option<String>,
    pub draft: bool,
    pub html: String,
}

impl Post {
    pub fn summary(&self) -> Option<&str> {
        self.description.as_deref().or(self.subtitle.as_deref())
    }
}

const KEYS: [&str; 6] = [
    "title",
    "subtitle",
    "description",
    "draft",
    "pubDate",
    "tags",
];

#[derive(Debug)]
enum Value {
    Scalar(String),
    Seq(Vec<String>),
}

fn unquote(raw: &str) -> String {
    let s = raw.trim();
    let quoted = |q: char| {
        s.strip_prefix(q)
            .and_then(|rest| rest.strip_suffix(q))
            .filter(|_| s.len() >= 2)
    };

    match (quoted('"'), quoted('\'')) {
        (Some(inner), _) => {
            inner
                .chars()
                .fold((String::new(), false), |(mut out, escaped), c| {
                    match (escaped, c) {
                        (false, '\\') => (out, true),
                        _ => {
                            out.push(c);
                            (out, false)
                        }
                    }
                })
                .0
        }
        (_, Some(inner)) => inner.to_string(),
        _ => s.to_string(),
    }
}

fn split_items(inner: &str) -> Vec<String> {
    let (mut items, last, _) = inner.chars().fold(
        (Vec::new(), String::new(), None::<char>),
        |(mut items, mut current, quote), c| match (quote, c) {
            (Some(q), c) if c == q => {
                current.push(c);
                (items, current, None)
            }
            (Some(_), c) => {
                current.push(c);
                (items, current, quote)
            }
            (None, '"') | (None, '\'') => {
                current.push(c);
                (items, current, Some(c))
            }
            (None, ',') => {
                items.push(current);
                (items, String::new(), None)
            }
            (None, c) => {
                current.push(c);
                (items, current, None)
            }
        },
    );
    items.push(last);

    items
        .iter()
        .map(|item| unquote(item))
        .filter(|item| !item.is_empty())
        .collect()
}

fn parse_front(front: &str, allowed: &[&str]) -> Result<Vec<(String, Value)>, String> {
    front
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.trim_start().starts_with('#'))
        .try_fold(
            Vec::new(),
            |mut fields: Vec<(String, Value)>, line| match line.trim_start().strip_prefix("- ") {
                Some(item) => match fields.last_mut() {
                    Some((_, Value::Seq(items))) => {
                        items.push(unquote(item));
                        Ok(fields)
                    }
                    _ => Err(format!("list item with no key above it: {line:?}")),
                },
                None => {
                    let (key, rest) = line
                        .split_once(':')
                        .ok_or_else(|| format!("expected `key: value`, got {line:?}"))?;
                    let key = key.trim();

                    if !allowed.contains(&key) {
                        return Err(format!(
                            "unknown key `{key}` (allowed: {})",
                            allowed.join(", ")
                        ));
                    }
                    if fields.iter().any(|(k, _)| k == key) {
                        return Err(format!("duplicate key `{key}`"));
                    }

                    let rest = rest.trim();
                    let value = match rest.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
                        Some(inner) => Value::Seq(split_items(inner)),
                        None if rest.is_empty() => Value::Seq(Vec::new()),
                        None => Value::Scalar(unquote(rest)),
                    };

                    fields.push((key.to_string(), value));
                    Ok(fields)
                }
            },
        )
}

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

fn scalar<'a>(fields: &'a [(String, Value)], key: &str) -> Result<Option<&'a str>, String> {
    match fields.iter().find(|(k, _)| k == key) {
        None => Ok(None),
        Some((_, Value::Scalar(s))) => Ok(Some(s.as_str())),
        Some(_) => Err(format!("`{key}` must be a single value, not a list")),
    }
}

pub fn parse(raw: &str, slug: &str) -> Result<Post, String> {
    let (front, body) = split_frontmatter(raw)?;
    let fields = parse_front(front, &KEYS)?;
    let field = |key| scalar(&fields, key);

    let tags = match fields.iter().find(|(k, _)| k == "tags") {
        None => Vec::new(),
        Some((_, Value::Seq(items))) => items.clone(),
        Some(_) => return Err("`tags` must be a list".into()),
    };

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
        draft,
        html: markdown::render(body),
    })
}

pub fn newest_first(posts: Vec<Post>) -> Vec<Post> {
    let mut posts = posts;
    posts.sort_by_key(|p| (Reverse(p.pub_date), p.slug.clone()));
    posts
}

pub fn tag_counts(posts: &[Post]) -> Vec<(&str, usize)> {
    let mut counts: Vec<(&str, usize)> = posts
        .iter()
        .flat_map(|p| p.tags.iter())
        .fold(BTreeMap::new(), |mut acc, tag| {
            *acc.entry(tag.as_str()).or_insert(0) += 1;
            acc
        })
        .into_iter()
        .collect();
    counts.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    counts
}
