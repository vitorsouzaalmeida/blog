//! following CSS Syntax Module Level 3 <https://www.w3.org/TR/css-syntax-3/>
//! Deliberately omitted a few things, because I don't need the whole specification.

use std::borrow::Cow;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Token<'a> {
    Whitespace(&'a str),
    Comment(&'a str),
    Ident(&'a str),
    Function(&'a str),
    AtKeyword(&'a str),
    Hash(&'a str),
    Str(&'a str),
    BadStr(&'a str),
    Url(&'a str),
    BadUrl(&'a str),
    Numeric(&'a str),
    Delim(char),
    Colon,
    Semicolon,
    Comma,
    OpenParen,
    CloseParen,
    OpenSquare,
    CloseSquare,
    OpenCurly,
    CloseCurly,
}

impl<'a> Token<'a> {
    pub fn raw(&self) -> Cow<'a, str> {
        match *self {
            Token::Whitespace(s)
            | Token::Comment(s)
            | Token::Ident(s)
            | Token::Function(s)
            | Token::AtKeyword(s)
            | Token::Hash(s)
            | Token::Str(s)
            | Token::BadStr(s)
            | Token::Url(s)
            | Token::BadUrl(s)
            | Token::Numeric(s) => Cow::Borrowed(s),
            Token::Delim(c) => Cow::Owned(c.to_string()),
            Token::Colon => Cow::Borrowed(":"),
            Token::Semicolon => Cow::Borrowed(";"),
            Token::Comma => Cow::Borrowed(","),
            Token::OpenParen => Cow::Borrowed("("),
            Token::CloseParen => Cow::Borrowed(")"),
            Token::OpenSquare => Cow::Borrowed("["),
            Token::CloseSquare => Cow::Borrowed("]"),
            Token::OpenCurly => Cow::Borrowed("{"),
            Token::CloseCurly => Cow::Borrowed("}"),
        }
    }

    #[cfg(test)]
    fn is_trivia(&self) -> bool {
        matches!(self, Token::Whitespace(_) | Token::Comment(_))
    }
}

fn is_newline(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{c}')
}

fn is_ws(c: char) -> bool {
    is_newline(c) || matches!(c, '\t' | ' ')
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_' || c >= '\u{80}'
}

fn is_ident(c: char) -> bool {
    is_ident_start(c) || c.is_ascii_digit() || c == '-'
}

fn at(src: &str, i: usize) -> Option<char> {
    src.get(i..).and_then(|s| s.chars().next())
}

fn peek(src: &str, i: usize, n: usize) -> Option<char> {
    src.get(i..).and_then(|s| s.chars().nth(n))
}

fn valid_escape(src: &str, i: usize) -> bool {
    at(src, i) == Some('\\') && !matches!(peek(src, i, 1), Some(c) if is_newline(c))
}

fn starts_ident(src: &str, i: usize) -> bool {
    match at(src, i) {
        Some('-') => match peek(src, i, 1) {
            Some(c) if is_ident_start(c) || c == '-' => true,
            Some('\\') => valid_escape(src, i + 1),
            _ => false,
        },
        Some('\\') => valid_escape(src, i),
        Some(c) => is_ident_start(c),
        None => false,
    }
}

fn starts_number(src: &str, i: usize) -> bool {
    match at(src, i) {
        Some('+') | Some('-') => match peek(src, i, 1) {
            Some('.') => matches!(peek(src, i, 2), Some(c) if c.is_ascii_digit()),
            Some(c) => c.is_ascii_digit(),
            None => false,
        },
        Some('.') => matches!(peek(src, i, 1), Some(c) if c.is_ascii_digit()),
        Some(c) => c.is_ascii_digit(),
        None => false,
    }
}

fn scan_while(src: &str, i: usize, f: impl Fn(char) -> bool) -> usize {
    src[i..]
        .char_indices()
        .find(|(_, c)| !f(*c))
        .map_or(src.len(), |(o, _)| i + o)
}

fn escape_end(src: &str, i: usize) -> usize {
    let after = i + 1;
    match at(src, after) {
        None => after,
        Some(c) if c.is_ascii_hexdigit() => {
            let hex = src[after..]
                .char_indices()
                .take(6)
                .take_while(|(_, c)| c.is_ascii_hexdigit())
                .last()
                .map_or(after, |(o, c)| after + o + c.len_utf8());
            match (at(src, hex), peek(src, hex, 1)) {
                (Some('\r'), Some('\n')) => hex + 2,
                (Some(w), _) if is_ws(w) => hex + w.len_utf8(),
                _ => hex,
            }
        }
        Some(c) => after + c.len_utf8(),
    }
}

fn ident_end(src: &str, i: usize) -> usize {
    std::iter::successors(Some(i), |&j| match at(src, j)? {
        c if is_ident(c) => Some(j + c.len_utf8()),
        '\\' if valid_escape(src, j) => Some(escape_end(src, j)),
        _ => None,
    })
    .last()
    .unwrap_or(i)
}

fn digits_end(src: &str, i: usize) -> usize {
    scan_while(src, i, |c| c.is_ascii_digit())
}

fn number_end(src: &str, i: usize) -> usize {
    let signed = match at(src, i) {
        Some('+') | Some('-') => i + 1,
        _ => i,
    };
    let whole = digits_end(src, signed);
    let frac = match (at(src, whole), peek(src, whole, 1)) {
        (Some('.'), Some(c)) if c.is_ascii_digit() => digits_end(src, whole + 1),
        _ => whole,
    };
    match (at(src, frac), peek(src, frac, 1), peek(src, frac, 2)) {
        (Some('e' | 'E'), Some(c), _) if c.is_ascii_digit() => digits_end(src, frac + 1),
        (Some('e' | 'E'), Some('+' | '-'), Some(c)) if c.is_ascii_digit() => {
            digits_end(src, frac + 2)
        }
        _ => frac,
    }
}

fn comment_end(src: &str, i: usize) -> (usize, bool) {
    match src[i + 2..].find("*/") {
        Some(o) => (i + 2 + o + 2, true),
        None => (src.len(), false),
    }
}

fn string_end(src: &str, open: usize, quote: char) -> (usize, bool) {
    let mut j = open + quote.len_utf8();
    loop {
        match at(src, j) {
            None => return (j, false),
            Some(c) if c == quote => return (j + c.len_utf8(), true),
            Some(c) if is_newline(c) => return (j, false),
            Some('\\') if valid_escape(src, j) => j = escape_end(src, j),
            Some('\\') => {
                j = match peek(src, j, 1) {
                    Some(c) => j + 1 + c.len_utf8(),
                    None => j + 1,
                }
            }
            Some(c) => j += c.len_utf8(),
        }
    }
}

fn url_end(src: &str, from: usize) -> (usize, bool) {
    let mut j = from;
    loop {
        match at(src, j) {
            None => return (j, false),
            Some(')') => return (j + 1, true),
            Some('"') | Some('\'') | Some('(') => return (bad_url_end(src, j), false),
            Some(c) if is_ws(c) => {
                let ws = scan_while(src, j, is_ws);
                return match at(src, ws) {
                    Some(')') => (ws + 1, true),
                    None => (ws, false),
                    _ => (bad_url_end(src, ws), false),
                };
            }
            Some('\\') if valid_escape(src, j) => j = escape_end(src, j),
            Some('\\') => return (bad_url_end(src, j), false),
            Some(c) => j += c.len_utf8(),
        }
    }
}

fn bad_url_end(src: &str, i: usize) -> usize {
    let mut j = i;
    loop {
        match at(src, j) {
            None => return j,
            Some(')') => return j + 1,
            Some('\\') if valid_escape(src, j) => j = escape_end(src, j),
            Some(c) => j += c.len_utf8(),
        }
    }
}

fn ident_like_at(src: &str, i: usize) -> (Token<'_>, usize) {
    let end = ident_end(src, i);
    if at(src, end) != Some('(') {
        return (Token::Ident(&src[i..end]), end);
    }
    let open = end + 1;
    if !src[i..end].eq_ignore_ascii_case("url") {
        return (Token::Function(&src[i..open]), open);
    }

    let body = scan_while(src, open, is_ws);
    match at(src, body) {
        Some('"') | Some('\'') => (Token::Function(&src[i..open]), open),
        _ => match url_end(src, body) {
            (end, true) => (Token::Url(&src[i..end]), end),
            (end, false) => (Token::BadUrl(&src[i..end]), end),
        },
    }
}

fn numeric_at(src: &str, i: usize) -> (Token<'_>, usize) {
    let n = number_end(src, i);
    let end = match at(src, n) {
        _ if starts_ident(src, n) => ident_end(src, n),
        Some('%') => n + 1,
        _ => n,
    };
    (Token::Numeric(&src[i..end]), end)
}

fn token_at(src: &str, i: usize) -> Option<(Token<'_>, usize)> {
    let c = at(src, i)?;
    Some(match c {
        c if is_ws(c) => {
            let end = scan_while(src, i, is_ws);
            (Token::Whitespace(&src[i..end]), end)
        }
        '/' if peek(src, i, 1) == Some('*') => match comment_end(src, i) {
            (end, _) => (Token::Comment(&src[i..end]), end),
        },
        '"' | '\'' => match string_end(src, i, c) {
            (end, true) => (Token::Str(&src[i..end]), end),
            (end, false) => (Token::BadStr(&src[i..end]), end),
        },
        '#' => match peek(src, i, 1) {
            Some(n) if is_ident(n) => {
                let end = ident_end(src, i + 1);
                (Token::Hash(&src[i..end]), end)
            }
            _ if valid_escape(src, i + 1) => {
                let end = ident_end(src, i + 1);
                (Token::Hash(&src[i..end]), end)
            }
            _ => (Token::Delim('#'), i + 1),
        },
        '@' if starts_ident(src, i + 1) => {
            let end = ident_end(src, i + 1);
            (Token::AtKeyword(&src[i..end]), end)
        }
        '(' => (Token::OpenParen, i + 1),
        ')' => (Token::CloseParen, i + 1),
        '[' => (Token::OpenSquare, i + 1),
        ']' => (Token::CloseSquare, i + 1),
        '{' => (Token::OpenCurly, i + 1),
        '}' => (Token::CloseCurly, i + 1),
        ',' => (Token::Comma, i + 1),
        ':' => (Token::Colon, i + 1),
        ';' => (Token::Semicolon, i + 1),
        '+' | '-' | '.' if starts_number(src, i) => numeric_at(src, i),
        '-' if starts_ident(src, i) => ident_like_at(src, i),
        '\\' if valid_escape(src, i) => ident_like_at(src, i),
        c if c.is_ascii_digit() => numeric_at(src, i),
        c if is_ident_start(c) => ident_like_at(src, i),
        c => (Token::Delim(c), i + c.len_utf8()),
    })
}

fn scan(src: &str) -> impl Iterator<Item = (Token<'_>, usize)> {
    std::iter::successors(
        token_at(src, 0).map(|(t, e)| ((t, 0), e)),
        move |&(_, e)| token_at(src, e).map(|(t, next)| ((t, e), next)),
    )
    .map(|(pair, _)| pair)
}

fn tokenize(src: &str) -> impl Iterator<Item = Token<'_>> {
    scan(src).map(|(t, _)| t)
}

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    UnterminatedComment,
    UnterminatedString,
    BadUrl,
    UnclosedBlock,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(match self {
            Error::UnterminatedComment => "unterminated comment (`/*` with no `*/`)",
            Error::UnterminatedString => "unterminated string literal",
            Error::BadUrl => "malformed url()",
            Error::UnclosedBlock => "unclosed `{` block",
        })
    }
}

/// The parse errors §4.3.2 and §5.4.4 tell you to recover from silently.
/// Returns the byte offset of the construct that failed.
pub fn check(css: &str) -> Result<(), (Error, usize)> {
    let mut open_blocks: Vec<usize> = Vec::new();
    for (token, offset) in scan(css) {
        match token {
            Token::Comment(raw) if !raw.ends_with("*/") || raw.len() < 4 => {
                return Err((Error::UnterminatedComment, offset))
            }
            Token::BadStr(_) => return Err((Error::UnterminatedString, offset)),
            Token::BadUrl(_) => return Err((Error::BadUrl, offset)),
            Token::OpenCurly => open_blocks.push(offset),
            Token::CloseCurly => {
                open_blocks.pop();
            }
            _ => {}
        }
    }
    match open_blocks.first() {
        Some(&offset) => Err((Error::UnclosedBlock, offset)),
        None => Ok(()),
    }
}

fn suppress_after(t: &Token) -> bool {
    matches!(
        t,
        Token::OpenCurly | Token::CloseCurly | Token::Semicolon | Token::Colon | Token::Comma
    )
}

fn suppress_before(t: &Token) -> bool {
    matches!(
        t,
        Token::OpenCurly | Token::CloseCurly | Token::Semicolon | Token::Comma
    )
}

fn escape_lt(raw: &str) -> Cow<'_, str> {
    match raw.contains('<') {
        false => Cow::Borrowed(raw),
        true => Cow::Owned(raw.replace('<', "\\3c ")),
    }
}

pub fn minify(css: &str) -> String {
    let normalized: Vec<Token> = tokenize(css)
        .map(|t| match t {
            Token::Comment(_) => Token::Whitespace(" "),
            other => other,
        })
        .collect();

    let deduped: Vec<Token> = normalized
        .iter()
        .enumerate()
        .filter(|(k, t)| {
            !matches!(t, Token::Whitespace(_))
                || *k == 0
                || !matches!(normalized[k - 1], Token::Whitespace(_))
        })
        .map(|(_, t)| *t)
        .collect();

    let next_significant = |from: usize| {
        deduped[from..]
            .iter()
            .find(|t| !matches!(t, Token::Whitespace(_) | Token::Semicolon))
    };
    let trimmed: Vec<Token> = deduped
        .iter()
        .enumerate()
        .filter(|(k, t)| {
            !matches!(t, Token::Semicolon)
                || !matches!(next_significant(k + 1), Some(Token::CloseCurly))
        })
        .map(|(_, t)| *t)
        .collect();

    let kept: Vec<Token> = trimmed
        .iter()
        .enumerate()
        .filter(|(k, t)| match t {
            Token::Whitespace(_) => match (trimmed.get(k.wrapping_sub(1)), trimmed.get(k + 1)) {
                (Some(before), Some(after)) => !suppress_after(before) && !suppress_before(after),
                _ => false,
            },
            _ => true,
        })
        .map(|(_, t)| *t)
        .collect();

    let out =
        kept.iter()
            .enumerate()
            .fold(String::with_capacity(css.len()), |mut acc, (k, token)| {
                match token {
                    Token::Str(s) | Token::Url(s) | Token::BadStr(s) | Token::BadUrl(s) => {
                        acc.push_str(&escape_lt(s))
                    }
                    Token::Delim('<') if kept.get(k + 1) == Some(&Token::Delim('/')) => {
                        acc.push_str("\\3c ")
                    }
                    t => acc.push_str(&t.raw()),
                }
                acc
            });
    let out = out.trim().to_string();
    debug_assert!(
        !out.to_ascii_lowercase().contains("</style"),
        "minified CSS would close its own <style> element"
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(css: &str) -> Vec<Token<'_>> {
        tokenize(css).collect()
    }

    #[test]
    fn whitespace_inside_a_string_literal_is_preserved() {
        for css in [
            r#"a{content:"a, b"}"#,
            r#"a{content:"a  b"}"#,
            r#"a{font-family:"JetBrains Mono"}"#,
            r#"a{content:"x; y: z"}"#,
        ] {
            assert_eq!(minify(css), css, "mangled {css:?}");
        }
    }

    #[test]
    fn a_comment_delimiter_inside_a_string_is_not_a_comment() {
        assert_eq!(minify(r#"a{content:"/* x */"}"#), r#"a{content:"/* x */"}"#);
        assert_eq!(
            minify(r#"a{background:url(/a/*b.png)}"#),
            r#"a{background:url(/a/*b.png)}"#
        );
    }

    #[test]
    fn an_unterminated_comment_is_a_build_error_not_silent_data_loss() {
        assert_eq!(
            check("/* oops\np{top:0}").map_err(|(e, _)| e),
            Err(Error::UnterminatedComment)
        );
        assert_eq!(check("a{top:0}/* fine */").map_err(|(e, _)| e), Ok(()));
    }

    #[test]
    fn an_unclosed_block_or_string_is_a_build_error() {
        assert_eq!(
            check("a{top:0").map_err(|(e, _)| e),
            Err(Error::UnclosedBlock)
        );
        assert_eq!(
            check("a{content:\"x}").map_err(|(e, _)| e),
            Err(Error::UnterminatedString)
        );
        assert_eq!(check("a{top:0}").map_err(|(e, _)| e), Ok(()));
        assert_eq!(check("a{top:0}\nb{left:0"), Err((Error::UnclosedBlock, 10)));
    }

    #[test]
    fn a_brace_inside_a_string_or_url_does_not_open_a_block() {
        for css in [r#"a{content:"}"}"#, r#"a{background:url(a}b)}"#] {
            assert_eq!(check(css).map_err(|(e, _)| e), Ok(()), "for {css:?}");
        }
    }

    #[test]
    fn a_string_may_not_contain_a_raw_newline() {
        assert!(matches!(kinds("\"a\nb\"")[0], Token::BadStr(_)));
        assert!(matches!(kinds("\"a b\"")[0], Token::Str(_)));
        assert!(matches!(kinds("\"a\\\nb\"")[0], Token::Str(_)));
    }

    #[test]
    fn an_escaped_quote_does_not_end_a_string() {
        assert_eq!(kinds(r#""a\"b""#), [Token::Str(r#""a\"b""#)]);
    }

    #[test]
    fn url_is_a_token_but_url_with_a_quote_is_a_function() {
        assert_eq!(kinds("url(/a.woff2)"), [Token::Url("url(/a.woff2)")]);
        assert_eq!(
            kinds(r#"url("/a")"#),
            [
                Token::Function("url("),
                Token::Str(r#""/a""#),
                Token::CloseParen
            ]
        );
    }

    #[test]
    fn numbers_and_unicode_ranges_round_trip_byte_for_byte() {
        for src in [
            "+0000",
            "-00FF",
            "14.5px",
            ".5",
            "1e3",
            "-.5em",
            "100%",
            "U+0000-00FF, U+0131, U+02BB-02BC",
            "--paper",
            "-webkit-box",
            "\\32 x",
        ] {
            let out: String = tokenize(src).map(|t| t.raw().to_string()).collect();
            assert_eq!(out, src, "did not round-trip");
        }
    }

    #[test]
    fn tokenizing_the_real_stylesheets_round_trips() {
        for path in [
            "static/styles.css",
            "static/fonts/fonts.css",
            "static/highlight.css",
        ] {
            let css = std::fs::read_to_string(path).unwrap();
            let out: String = tokenize(&css).map(|t| t.raw().to_string()).collect();
            assert_eq!(out, css, "{path} did not round-trip");
            assert_eq!(
                check(&css).map_err(|(e, _)| e),
                Ok(()),
                "{path} failed check"
            );
        }
    }

    #[test]
    fn escapes_are_never_unescaped_for_output() {
        assert!(minify(".a\\23 {top:0}").contains("\\23 "));
    }

    #[test]
    fn minify_only_removes_comments_whitespace_and_redundant_semicolons() {
        let significant = |s: &str| -> Vec<String> {
            tokenize(s)
                .filter(|t| !t.is_trivia() && !matches!(t, Token::Semicolon))
                .map(|t| t.raw().to_string())
                .collect()
        };
        for css in [
            "a {\n  color: red;\n}",
            "@media (min-width: 768px) { p { top: 0 } }",
            "article :not(pre) > code { padding: 1px 5px; }",
            r#"@font-face { src: url(/a.woff2) format("woff2"); }"#,
            "a{b:c}/* trailing */",
            "a/*x*/b{top:0}",
            r#"a{content:"a, b";font-family:"JetBrains Mono"}"#,
            &std::fs::read_to_string("static/styles.css").unwrap(),
            &std::fs::read_to_string("static/fonts/fonts.css").unwrap(),
        ] {
            assert_eq!(
                significant(&minify(css)),
                significant(css),
                "for {:?}",
                &css[..css.len().min(60)]
            );
        }
    }

    #[test]
    fn a_trailing_semicolon_is_dropped_only_before_a_brace() {
        assert_eq!(minify("a{b:c;}"), "a{b:c}");
        assert_eq!(minify("a{b:c;d:e}"), "a{b:c;d:e}");
        assert_eq!(minify("a{b:c};"), "a{b:c};");
    }

    #[test]
    fn a_comment_between_two_idents_does_not_fuse_them() {
        assert_eq!(minify("a/*x*/b{top:0}"), "a b{top:0}");
    }

    #[test]
    fn minify_is_idempotent() {
        for css in [
            "a {\n  color: red;\n}",
            "@media (min-width: 768px) { p { top: 0 } }",
            r#"a{content:"a, b"}"#,
        ] {
            let once = minify(css);
            assert_eq!(minify(&once), once, "for {css:?}");
        }
    }

    #[test]
    fn minify_never_closes_the_style_element() {
        for css in [
            r#"a{content:"</style>"}"#,
            r#"a{content:"</STYLE >"}"#,
            r#"a{background:url(</style>)}"#,
            "a{x:< /style}",
            "a{x:</style}",
        ] {
            let out = minify(css);
            assert!(
                !out.to_ascii_lowercase().contains("</style"),
                "{css:?} produced {out:?}"
            );
        }
    }

    #[test]
    fn a_media_query_range_comparison_is_not_escaped() {
        assert!(minify("@media (400px <= width){a{top:0}}").contains("<="));
    }

    #[test]
    fn minify_collapses_without_breaking_selectors() {
        assert_eq!(minify("a {\n  color: red;\n}"), "a{color:red}");
        assert_eq!(minify("/* note */\np { margin: 0 }"), "p{margin:0}");
        assert_eq!(minify("h1,\nh2 { top: 0; }"), "h1,h2{top:0}");
    }

    #[test]
    fn minify_keeps_the_spaces_that_carry_meaning() {
        assert_eq!(
            minify("article :not(pre) > code { padding: 1px 5px; }"),
            "article :not(pre) > code{padding:1px 5px}"
        );
        assert_eq!(
            minify("@media (min-width: 768px) { p { top: 0 } }"),
            "@media (min-width:768px){p{top:0}}"
        );
        assert_eq!(
            minify("@font-face { src: url(/a.woff2) format(\"woff2\"); }"),
            "@font-face{src:url(/a.woff2) format(\"woff2\")}"
        );
    }
}
