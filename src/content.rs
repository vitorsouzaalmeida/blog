use std::cmp::Ordering;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Date {
    pub y: i32,
    pub m: u32,
    pub d: u32,
}

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
const WEEKDAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

impl Date {
    pub fn parse(s: &str) -> Result<Date, String> {
        let date_part = s.split(['T', ' ']).next().unwrap_or(s);
        let [y, m, d] = date_part.split('-').collect::<Vec<_>>()[..] else {
            return Err(format!("expected date as YYYY-MM-DD, got {s:?}"));
        };
        let bad = |name: &str| format!("invalid {name} in date {s:?}");
        let date = Date {
            y: y.trim().parse().map_err(|_| bad("year"))?,
            m: m.trim().parse().map_err(|_| bad("month"))?,
            d: d.trim().parse().map_err(|_| bad("day"))?,
        };
        match date {
            Date {
                m: 1..=12,
                d: 1..=31,
                ..
            } => Ok(date),
            _ => Err(format!("date out of range: {s:?}")),
        }
    }

    pub fn iso(&self) -> String {
        format!("{:04}-{:02}-{:02}", self.y, self.m, self.d)
    }

    pub fn dotted(&self) -> String {
        format!("{:04} · {:02} · {:02}", self.y, self.m, self.d)
    }

    fn weekday(&self) -> usize {
        let (y, m) = (self.y, self.m as i32);
        let mm = if m < 3 { m + 12 } else { m };
        let y = if m < 3 { y - 1 } else { y };
        let (k, j) = (y % 100, y / 100);
        let h = (self.d as i32 + (13 * (mm + 1)) / 5 + k + k / 4 + j / 4 + 5 * j) % 7;
        ((h + 6) % 7) as usize
    }

    pub fn rfc822(&self) -> String {
        format!(
            "{}, {:02} {} {:04} 00:00:00 GMT",
            WEEKDAYS[self.weekday()],
            self.d,
            MONTHS[(self.m.clamp(1, 12) - 1) as usize],
            self.y
        )
    }
}

impl Ord for Date {
    fn cmp(&self, o: &Self) -> Ordering {
        (self.y, self.m, self.d).cmp(&(o.y, o.m, o.d))
    }
}
impl PartialOrd for Date {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

#[derive(Debug)]
pub struct Post {
    pub slug: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub pub_date: Date,
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

fn unquote(s: &str) -> String {
    let t = s.trim();
    let quoted = |q: char| t.len() >= 2 && t.starts_with(q) && t.ends_with(q);
    if quoted('"') || quoted('\'') {
        t[1..t.len() - 1].to_string()
    } else {
        t.to_string()
    }
}

fn split_frontmatter(raw: &str) -> Result<(&str, &str), String> {
    let rest = raw
        .strip_prefix("---")
        .ok_or("missing opening `---` (frontmatter is required)")?;
    let end = rest
        .find("\n---")
        .ok_or("missing closing `---` (frontmatter is never terminated)")?;
    Ok((
        &rest[..end],
        rest[end + 4..].trim_start_matches(['\r', '\n']),
    ))
}

fn is_key(line: &str, key: &str) -> bool {
    !line.starts_with([' ', '\t', '-'])
        && line.split_once(':').is_some_and(|(k, _)| k.trim() == key)
}

fn scalar<'a>(front: &'a str, key: &str) -> Option<&'a str> {
    front
        .lines()
        .find(|l| is_key(l, key))
        .and_then(|l| l.split_once(':'))
        .map(|(_, v)| v.trim())
        .filter(|v| !v.is_empty())
}

fn tag_list(front: &str) -> Vec<String> {
    let from_key = front.lines().skip_while(|l| !is_key(l, "tags"));
    let value = from_key
        .clone()
        .next()
        .and_then(|l| l.split_once(':'))
        .map(|(_, v)| v.trim());
    match value {
        None => Vec::new(),
        Some(v) if v.starts_with('[') => v
            .trim_matches(['[', ']'])
            .split(',')
            .map(unquote)
            .filter(|t| !t.is_empty())
            .collect(),
        Some(_) => from_key
            .skip(1)
            .map_while(|l| l.trim_start().strip_prefix("- "))
            .map(unquote)
            .collect(),
    }
}

pub fn parse(raw: &str, slug: &str) -> Result<Post, String> {
    let (front, body) = split_frontmatter(raw)?;
    let field = |key| scalar(front, key).map(unquote);
    Ok(Post {
        slug: slug.to_string(),
        title: field("title").ok_or("missing `title`")?,
        subtitle: field("subtitle"),
        description: field("description"),
        pub_date: Date::parse(&field("pubDate").ok_or("missing `pubDate`")?)?,
        tags: tag_list(front),
        thread: field("thread"),
        thread_order: field("threadOrder").and_then(|v| v.parse().ok()),
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

    fn date(s: &str) -> Date {
        Date::parse(s).unwrap()
    }

    #[test]
    fn bad_dates_are_rejected_not_defaulted() {
        // Each of these used to become 1970-01-01 and sort to the bottom.
        for bad in [
            "",
            "not-a-date",
            "2023",
            "2023-08",
            "2023-8x-13",
            "2023-13-01",
        ] {
            assert!(Date::parse(bad).is_err(), "{bad:?} should not parse");
        }
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
