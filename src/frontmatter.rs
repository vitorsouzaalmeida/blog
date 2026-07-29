//! A strict subset of YAML 1.2.2 <https://yaml.org/spec/1.2.2/>
//!
//! Deliberately missing because I don't want them: block scalars, flow
//! mappings, anchors, aliases, tags, merge keys, directives, document markers,
//! nested mappings, duplicate keys, unknown keys, and tabs for indentation.

use std::borrow::Cow;
use std::fmt;

#[derive(Debug, PartialEq, Eq)]
pub struct Error {
    pub line: usize,
    pub msg: String,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.msg)
    }
}

fn err<T>(line: usize, msg: impl Into<String>) -> Result<T, Error> {
    Err(Error {
        line,
        msg: msg.into(),
    })
}

#[derive(Debug, PartialEq, Eq)]
pub enum Value<'a> {
    Scalar(Cow<'a, str>),
    Seq(Vec<Cow<'a, str>>),
}

pub fn parse<'a>(front: &'a str, allowed: &[&str]) -> Result<Vec<(&'a str, Value<'a>)>, Error> {
    group(front)?
        .iter()
        .filter_map(|g| match allowed.contains(&g.key) {
            false => Some(err(g.line, format!("unknown key {:?}", g.key))),
            true => match value_of(g) {
                Ok(None) => None,
                Ok(Some(value)) => Some(Ok((g.key, value))),
                Err(e) => Some(Err(e)),
            },
        })
        .collect()
}

struct Group<'a> {
    line: usize,
    key: &'a str,
    inline: &'a str,
    items: Vec<(usize, &'a str)>,
}

fn group(front: &str) -> Result<Vec<Group<'_>>, Error> {
    let mut groups: Vec<Group> = Vec::new();
    for (i, raw) in front.lines().enumerate() {
        let line = i + 1;
        let text = raw.strip_suffix('\r').unwrap_or(raw);
        let body = text.trim_start();
        let indent = &text[..text.len() - body.len()];

        // §6.6: a `#` at the start of a line begins a comment.
        if body.is_empty() || body.starts_with('#') {
            continue;
        }
        if indent.contains('\t') {
            return err(line, "tabs may not be used for indentation");
        }
        if body.starts_with("---") || body.starts_with("...") {
            return err(line, "unexpected document marker inside the frontmatter");
        }
        if body.starts_with('%') {
            return err(line, "directives (%YAML, %TAG) are not supported");
        }
        if body.starts_with("<<:") {
            return err(line, "merge keys are not supported");
        }
        if body == "-" || body.starts_with("- ") {
            let item = body[1..].trim_start();
            if item.is_empty() {
                return err(line, "empty list item");
            }
            match groups.last_mut() {
                None => return err(line, "list item before any key"),
                Some(g) => g.items.push((line, item)),
            }
            continue;
        }
        if !indent.is_empty() {
            return err(line, "nested mappings are not supported");
        }
        let Some((key, inline)) = body.split_once(':') else {
            return err(line, "expected \"key: value\"");
        };
        if !is_key(key) {
            return err(line, format!("{key:?} is not a valid key"));
        }
        if groups.iter().any(|g| g.key == key) {
            return err(line, format!("duplicate key {key:?}"));
        }
        if inline.trim_start().starts_with(['|', '>']) {
            return err(
                line,
                "block scalars are not supported; use a quoted single-line string",
            );
        }
        groups.push(Group {
            line,
            key,
            inline,
            items: Vec::new(),
        });
    }
    Ok(groups)
}

fn is_key(s: &str) -> bool {
    let mut chars = s.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn value_of<'a>(g: &Group<'a>) -> Result<Option<Value<'a>>, Error> {
    let inline = g.inline.trim_start();
    let has_inline = !inline.is_empty() && !inline.starts_with('#');

    if has_inline && !g.items.is_empty() {
        return err(
            g.line,
            format!("key {:?} has both an inline value and a block list", g.key),
        );
    }
    if !g.items.is_empty() {
        return g
            .items
            .iter()
            .map(|&(line, text)| scalar(line, text).map(|(v, _)| v))
            .collect::<Result<Vec<_>, _>>()
            .map(|items| Some(Value::Seq(items)));
    }
    if !has_inline {
        return Ok(None);
    }
    // A value must be separated from its `:` by a space (§7.2).
    if !g.inline.starts_with([' ', '\t']) {
        return err(g.line, "expected a space after \":\"");
    }
    let (value, rest) = match inline.starts_with('[') {
        true => flow_seq(g.line, inline)?,
        false => scalar(g.line, inline).map(|(s, r)| (Value::Scalar(s), r))?,
    };
    end_of_line(g.line, rest)?;
    Ok(Some(value))
}

fn end_of_line(line: usize, rest: &str) -> Result<(), Error> {
    let rest = rest.trim_start();
    match rest.is_empty() || rest.starts_with('#') {
        true => Ok(()),
        false => err(line, format!("unexpected text after the value: {rest:?}")),
    }
}

fn scalar(line: usize, body: &str) -> Result<(Cow<'_, str>, &str), Error> {
    match body.chars().next() {
        Some('"') => quoted_double(line, body),
        Some('\'') => quoted_single(line, body),
        Some('|') | Some('>') => err(
            line,
            "block scalars are not supported; use a quoted single-line string",
        ),
        Some('&') | Some('*') => err(line, "anchors and aliases are not supported"),
        Some('!') => err(line, "tags are not supported"),
        Some('{') => err(line, "flow mappings are not supported"),
        Some('[') => err(line, "a list is not valid here"),
        None => err(line, "expected a value"),
        Some(_) => plain(line, body),
    }
}

/// §7.3.3. Ends at a comment or end of input; may not contain `": "`, which
/// is how YAML keeps `key: a: b` from being ambiguous.
fn plain(line: usize, body: &str) -> Result<(Cow<'_, str>, &str), Error> {
    let end = body
        .char_indices()
        .find(|(i, c)| *c == '#' && body[..*i].ends_with([' ', '\t']))
        .map_or(body.len(), |(i, _)| i);
    let value = body[..end].trim_end();
    if value.contains(": ") {
        return err(
            line,
            format!("unquoted value contains \": \"; quote it: {value:?}"),
        );
    }
    match value.is_empty() {
        true => err(line, "expected a value"),
        false => Ok((Cow::Borrowed(value), &body[end..])),
    }
}

/// §7.3.2. The only escape is `''` for a literal apostrophe.
fn quoted_single(line: usize, body: &str) -> Result<(Cow<'_, str>, &str), Error> {
    let mut out = String::new();
    let mut i = 1;
    let mut doubled = false;
    loop {
        let Some(c) = body[i..].chars().next() else {
            return err(line, "unterminated quoted string");
        };
        if c != '\'' {
            out.push(c);
            i += c.len_utf8();
            continue;
        }
        if body[i + 1..].starts_with('\'') {
            out.push('\'');
            doubled = true;
            i += 2;
            continue;
        }
        return Ok((
            match doubled {
                true => Cow::Owned(out),
                false => Cow::Borrowed(&body[1..i]),
            },
            &body[i + 1..],
        ));
    }
}

/// §7.3.1 with the §5.7 escape table.
fn quoted_double(line: usize, body: &str) -> Result<(Cow<'_, str>, &str), Error> {
    let mut out = String::new();
    let mut i = 1;
    loop {
        match body[i..].chars().next() {
            None => return err(line, "unterminated quoted string"),
            Some('"') => {
                let borrowed = &body[1..i];
                return Ok((
                    match borrowed.contains('\\') {
                        true => Cow::Owned(out),
                        false => Cow::Borrowed(borrowed),
                    },
                    &body[i + 1..],
                ));
            }
            Some('\\') => {
                let (c, width) = escape(line, &body[i..])?;
                out.push(c);
                i += width;
            }
            Some(c) => {
                out.push(c);
                i += c.len_utf8();
            }
        }
    }
}

/// §5.7. `s` starts at the backslash; returns the character and its width in
/// bytes of source.
fn escape(line: usize, s: &str) -> Result<(char, usize), Error> {
    let Some(kind) = s[1..].chars().next() else {
        return err(line, "unterminated escape sequence");
    };
    let simple = |c| Ok((c, 2));
    match kind {
        '"' => simple('"'),
        '\\' => simple('\\'),
        '/' => simple('/'),
        ' ' => simple(' '),
        '0' => simple('\0'),
        'a' => simple('\u{7}'),
        'b' => simple('\u{8}'),
        't' => simple('\t'),
        'n' => simple('\n'),
        'v' => simple('\u{b}'),
        'f' => simple('\u{c}'),
        'r' => simple('\r'),
        'e' => simple('\u{1b}'),
        'x' | 'u' | 'U' => {
            let digits = match kind {
                'x' => 2,
                'u' => 4,
                _ => 8,
            };
            let hex = s.get(2..2 + digits).unwrap_or("");
            match u32::from_str_radix(hex, 16).ok().and_then(char::from_u32) {
                Some(c) => Ok((c, 2 + digits)),
                None => err(line, format!("invalid \\{kind} escape: {hex:?}")),
            }
        }
        other => err(line, format!("unknown escape sequence \"\\{other}\"")),
    }
}

fn flow_seq(line: usize, body: &str) -> Result<(Value<'_>, &str), Error> {
    let mut items = Vec::new();
    let mut rest = body[1..].trim_start();
    loop {
        if let Some(after) = rest.strip_prefix(']') {
            return Ok((Value::Seq(items), after));
        }
        if rest.is_empty() {
            return err(line, "unterminated flow sequence");
        }
        let (value, tail) = match rest.chars().next() {
            Some('"') => quoted_double(line, rest)?,
            Some('\'') => quoted_single(line, rest)?,
            _ => match rest.find([',', ']']) {
                None => return err(line, "unterminated flow sequence"),
                Some(end) => (Cow::Borrowed(rest[..end].trim_end()), &rest[end..]),
            },
        };
        if !value.is_empty() {
            items.push(value);
        }
        rest = tail.trim_start();
        match rest.chars().next() {
            Some(',') => rest = rest[1..].trim_start(),
            Some(']') => {}
            _ => return err(line, "expected \",\" or \"]\" in flow sequence"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEYS: [&str; 7] = [
        "title",
        "subtitle",
        "description",
        "pubDate",
        "tags",
        "thread",
        "threadOrder",
    ];

    fn get<'a>(front: &'a str, key: &str) -> Option<Value<'a>> {
        parse(front, &KEYS)
            .unwrap()
            .into_iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| v)
    }

    fn scalar_of(front: &str, key: &str) -> Option<String> {
        match get(front, key) {
            Some(Value::Scalar(s)) => Some(s.into_owned()),
            _ => None,
        }
    }

    fn seq_of(front: &str, key: &str) -> Vec<String> {
        match get(front, key) {
            Some(Value::Seq(items)) => items.into_iter().map(Cow::into_owned).collect(),
            _ => Vec::new(),
        }
    }

    #[test]
    fn escapes_in_double_quoted_scalars_are_decoded() {
        assert_eq!(
            scalar_of("title: \"He said \\\"hi\\\"\"\n", "title").as_deref(),
            Some(r#"He said "hi""#)
        );
        assert_eq!(
            scalar_of("title: \"a\\tb\\u00e9\"\n", "title").as_deref(),
            Some("a\tbé")
        );
    }

    #[test]
    fn a_trailing_comment_is_not_part_of_the_value() {
        assert_eq!(
            scalar_of("title: Draft # todo\n", "title").as_deref(),
            Some("Draft")
        );
    }

    #[test]
    fn a_hash_only_starts_a_comment_after_whitespace() {
        // §6.6: a `#` only opens a comment when it follows whitespace.
        assert_eq!(
            scalar_of("title: C# for beginners\n", "title").as_deref(),
            Some("C# for beginners")
        );
    }

    #[test]
    fn a_blank_line_does_not_terminate_a_block_list() {
        assert_eq!(
            seq_of("tags:\n  - code\n\n  - math\n", "tags"),
            ["code", "math"]
        );
    }

    #[test]
    fn a_comment_inside_a_block_list_is_skipped() {
        assert_eq!(
            seq_of("tags:\n  # main\n  - code\n  - math\n", "tags"),
            ["code", "math"]
        );
    }

    #[test]
    fn a_block_list_may_be_followed_by_more_keys() {
        let front = "tags:\n  - code\nthread: t1\ntitle: X\n";
        assert_eq!(seq_of(front, "tags"), ["code"]);
        assert_eq!(scalar_of(front, "thread").as_deref(), Some("t1"));
        assert_eq!(scalar_of(front, "title").as_deref(), Some("X"));
    }

    #[test]
    fn every_rejected_construct_names_its_line() {
        for (front, line, needle) in [
            ("title: X\ndescription: >\n  long\n", 2, "block scalar"),
            ("title: X\ndescription: |\n  long\n", 2, "block scalar"),
            ("title: X\nthread: {a: b}\n", 2, "flow mapping"),
            ("title: X\nthread: &anchor\n", 2, "anchors"),
            ("title: X\nthread: *alias\n", 2, "anchors"),
            ("title: X\nthread: !tag\n", 2, "tags are not supported"),
            ("title: X\n<<: base\n", 2, "merge key"),
            ("title: X\n---\n", 2, "document marker"),
            ("title: X\n%YAML 1.2\n", 2, "directive"),
            ("title: X\nog:\n  image: y\n", 3, "nested mapping"),
            ("title: X\ntitle: Y\n", 2, "duplicate key"),
            ("title: X\ntitel: Y\n", 2, "unknown key"),
            ("title: X\n\tthread: y\n", 2, "tab"),
            ("title: a: b\n", 1, "quote it"),
            ("title: \"unterminated\n", 1, "unterminated quoted string"),
            ("title: \"bad \\q\"\n", 1, "unknown escape"),
            ("tags: [a, b\n", 1, "unterminated flow sequence"),
            ("title:X\n", 1, "space after"),
            ("just some text\n", 1, "expected \"key: value\""),
            (
                "tags: one\n  - two\n",
                1,
                "both an inline value and a block list",
            ),
            ("  - orphan\n", 1, "before any key"),
        ] {
            let got = parse(front, &KEYS).unwrap_err();
            assert_eq!(got.line, line, "wrong line for {front:?}: {got}");
            assert!(
                got.msg.contains(needle),
                "expected {needle:?} in {:?} for {front:?}",
                got.msg
            );
        }
    }

    #[test]
    fn indentation_width_does_not_matter() {
        assert_eq!(seq_of("tags:\n  - code\n", "tags"), ["code"]);
        assert_eq!(seq_of("tags:\n    - code\n", "tags"), ["code"]);
    }

    #[test]
    fn both_list_forms_parse() {
        assert_eq!(
            seq_of("tags:\n  - code\n  - math\n", "tags"),
            ["code", "math"]
        );
        assert_eq!(
            seq_of("tags: [\"c++\", 'type theory']\n", "tags"),
            ["c++", "type theory"]
        );
        assert_eq!(seq_of("tags: [a, b,]\n", "tags"), ["a", "b"]);
        assert_eq!(seq_of("tags: []\n", "tags"), Vec::<String>::new());
    }

    #[test]
    fn quoted_scalars_keep_characters_that_would_otherwise_be_syntax() {
        assert_eq!(
            scalar_of("title: \"Rust: a tour\"\n", "title").as_deref(),
            Some("Rust: a tour")
        );
        assert_eq!(
            scalar_of("title: 'it''s fine'\n", "title").as_deref(),
            Some("it's fine")
        );
        assert_eq!(
            scalar_of("title: \"trailing # hash\"\n", "title").as_deref(),
            Some("trailing # hash")
        );
    }

    #[test]
    fn a_key_with_no_value_is_absent_rather_than_empty() {
        assert_eq!(scalar_of("title: X\nsubtitle:\n", "subtitle"), None);
        assert_eq!(seq_of("title: X\ntags:\n", "tags"), Vec::<String>::new());
    }

    #[test]
    fn whole_line_comments_are_ignored() {
        let front = "title: X\npubDate: 2024-01-02\ntags:\n  - tag1\n# Optional frontmatter:\n# subtitle: A short italic subtitle\n# thread: some-thread-id\n";
        assert_eq!(scalar_of(front, "title").as_deref(), Some("X"));
        assert_eq!(seq_of(front, "tags"), ["tag1"]);
        assert_eq!(scalar_of(front, "subtitle"), None);
    }

    #[test]
    fn plain_scalars_borrow_rather_than_allocate() {
        let front = "title: plain text\n";
        match parse(front, &KEYS).unwrap().pop().unwrap().1 {
            Value::Scalar(Cow::Borrowed(_)) => {}
            other => panic!("expected a borrow, got {other:?}"),
        }
    }
}
