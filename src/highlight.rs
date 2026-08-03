//! Deliberately partial syntax highlighting.
//!
//! Follows Tonsky's *Syntax Highlighting* <https://tonsky.me/blog/syntax-highlighting/>:
//! "if everything is highlighted, nothing is highlighted". Coloured are the
//! things a reader looks *for* -- strings and numbers, comments, constants, and
//! the name a definition introduces. Keywords, variables, calls and types are
//! not, which is most of every block, by design.
//!
//! Five classes, coloured by `static/highlight.css` from the site's palette:
//!
//! ```text
//! str  strings, characters, numbers
//! com  comments
//! con  ALL_CAPS constants
//! def  the name a definition introduces
//! pun  punctuation, dimmed so names stand out
//! ```
//!
//! Everything is still lexed properly even though most tokens end up unstyled:
//! a `#` inside a string does not open a comment, and a quote inside a comment
//! does not open a string.

/// Per-language data. Comment syntax is the only reason one fixed lexer will
/// not do -- OCaml nests `(* *)`, shell uses `#`, assembly uses `;` -- and
/// dropping keyword colouring removes the table that would otherwise dominate.
struct Lang {
    /// Prefixes that comment out the rest of the line.
    line: &'static [&'static str],
    block: Option<(&'static str, &'static str)>,
    /// OCaml and Rust nest block comments.
    nested: bool,
    /// Words that introduce a definition. The word itself stays unstyled --
    /// that is the point -- and the identifier following it is the name.
    defines: &'static [&'static str],
    /// Delimiters that open a string. A `'` that is *not* listed opens a
    /// character literal instead, which is what leaves a Rust lifetime and an
    /// OCaml type variable (`'a`, neither of them a literal) alone.
    quotes: &'static str,
    /// ALL_CAPS is a constant in every language here except assembly, where it
    /// is how mnemonics are written.
    caps: bool,
}

const RUST: Lang = Lang {
    line: &["//"],
    block: Some(("/*", "*/")),
    nested: true,
    defines: &[
        "fn", "struct", "enum", "trait", "type", "mod", "const", "static", "union",
    ],
    quotes: "\"",
    caps: true,
};

const OCAML: Lang = Lang {
    line: &[],
    block: Some(("(*", "*)")),
    nested: true,
    // `rec` is listed so `let rec f` names `f`: an introducer never names an
    // introducer, so the two chain.
    defines: &["let", "rec", "and", "type", "module", "val", "exception"],
    quotes: "\"",
    caps: true,
};

const JS: Lang = Lang {
    line: &["//"],
    block: Some(("/*", "*/")),
    nested: false,
    defines: &["function", "class", "const"],
    quotes: "\"'`",
    caps: true,
};

const GO: Lang = Lang {
    line: &["//"],
    block: Some(("/*", "*/")),
    nested: false,
    defines: &["func", "type", "package", "const", "var"],
    quotes: "\"`",
    caps: true,
};

const SHELL: Lang = Lang {
    line: &["#"],
    block: None,
    nested: false,
    defines: &[],
    quotes: "\"'",
    caps: true,
};

const ASM: Lang = Lang {
    line: &[";", "//"],
    block: None,
    nested: false,
    defines: &[],
    quotes: "\"",
    caps: false,
};

/// Info-string words that select a language. Anything else -- including the
/// untagged blocks, which are most of them -- is emitted as plain text.
const LANGS: [(&str, &Lang); 6] = [
    ("rust rs", &RUST),
    ("ocaml ml mli", &OCAML),
    ("javascript js jsx mjs cjs typescript ts tsx", &JS),
    ("go", &GO),
    ("bash sh shell zsh console shell-session sh-session", &SHELL),
    ("asm nasm x86asm", &ASM),
];

fn lang(name: &str) -> Option<&'static Lang> {
    LANGS
        .iter()
        .find(|(names, _)| names.split(' ').any(|n| n.eq_ignore_ascii_case(name)))
        .map(|&(_, l)| l)
}

/// The contents of one `<code>` element: escaped, and marked up if the language
/// is one we lex.
pub fn render(code: &str, name: &str) -> String {
    match lang(name) {
        Some(l) => lex(code, l),
        None => {
            let mut out = String::with_capacity(code.len());
            push_esc(&mut out, code);
            out
        }
    }
}

fn lex(src: &str, l: &Lang) -> String {
    let mut out = String::with_capacity(src.len() * 2);
    let mut i = 0;

    while i < src.len() {
        let rest = &src[i..];
        let c = rest.chars().next().unwrap_or('\0');

        if let Some(n) = comment(rest, l) {
            span(&mut out, "com", &rest[..n]);
            i += n;
        } else if let Some(n) = quoted(rest, l) {
            span(&mut out, "str", &rest[..n]);
            i += n;
        } else if c.is_ascii_digit() {
            let n = number(rest);
            span(&mut out, "str", &rest[..n]);
            i += n;
        } else if is_ident_start(c) {
            i += word(&mut out, rest, l);
        } else if is_pun(c) {
            let n = pun(rest, l);
            span(&mut out, "pun", &rest[..n]);
            i += n;
        } else {
            // Whitespace, and anything else a keyboard produces.
            let n = c.len_utf8();
            push_esc(&mut out, &rest[..n]);
            i += n;
        }
    }
    out
}

fn span(out: &mut String, class: &str, text: &str) {
    out.push_str("<span class=\"");
    out.push_str(class);
    out.push_str("\">");
    push_esc(out, text);
    out.push_str("</span>");
}

/// The three characters that change meaning in element content. Attributes are
/// `fill::esc`'s business; nothing here is written into one.
fn push_esc(out: &mut String, s: &str) {
    s.chars().for_each(|c| match c {
        '&' => out.push_str("&amp;"),
        '<' => out.push_str("&lt;"),
        '>' => out.push_str("&gt;"),
        _ => out.push(c),
    })
}

// -- Token kinds --------------------------------------------------------------

/// The length of the comment starting `rest`, if one does. An unterminated
/// block comment runs to the end of the block, which is what the author wrote.
fn comment(rest: &str, l: &Lang) -> Option<usize> {
    if l.line.iter().any(|p| rest.starts_with(p)) {
        return Some(rest.find('\n').unwrap_or(rest.len()));
    }

    let (open, close) = l.block.filter(|(open, _)| rest.starts_with(open))?;
    let mut i = open.len();
    let mut depth = 1usize;

    while i < rest.len() {
        if rest[i..].starts_with(close) {
            i += close.len();
            depth -= 1;
            if depth == 0 {
                return Some(i);
            }
        } else if l.nested && rest[i..].starts_with(open) {
            i += open.len();
            depth += 1;
        } else {
            i += char_len(&rest[i..]);
        }
    }
    Some(rest.len())
}

/// A string, or -- where `'` is not a string delimiter -- a character literal.
fn quoted(rest: &str, l: &Lang) -> Option<usize> {
    let c = rest.chars().next()?;
    if c == '\'' && !l.quotes.contains('\'') {
        return char_literal(rest);
    }
    if !l.quotes.contains(c) {
        return None;
    }
    // A backtick spans lines (Go's raw strings, JS templates); a quote does
    // not, so one stray quote colours a line rather than the rest of the block.
    let stop = match c {
        '`' => rest.len(),
        _ => rest.find('\n').unwrap_or(rest.len()),
    };
    let mut i = c.len_utf8();
    while i < stop {
        let ch = rest[i..].chars().next().unwrap_or('\0');
        if ch == '\\' {
            i += 1 + char_len(&rest[i + 1..]);
            continue;
        }
        i += ch.len_utf8();
        if ch == c {
            return Some(i);
        }
    }
    Some(stop)
}

/// `'x'`, `'\n'`, `'\x41'` -- but not `'a`, which is a Rust lifetime or an
/// OCaml type variable and stays unstyled.
fn char_literal(rest: &str) -> Option<usize> {
    let body = &rest[1..];
    let after = match body.strip_prefix('\\') {
        None => &body[char_len(body)..],
        Some(esc) => {
            let head = char_len(esc);
            let digits = esc[head..].bytes().take_while(u8::is_ascii_alphanumeric);
            &esc[head + digits.count()..]
        }
    };
    after.strip_prefix('\'').map(|tail| rest.len() - tail.len())
}

/// Digits, then whatever letters and underscores belong to the literal
/// (`0x1f`, `34h`, `1_000`). A `.` only when a digit follows it, so Rust's
/// `0..5` lexes as a number, a range and a number.
fn number(rest: &str) -> usize {
    let b = rest.as_bytes();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            c if c.is_ascii_alphanumeric() || c == b'_' => i += 1,
            b'.' if b.get(i + 1).is_some_and(u8::is_ascii_digit) => i += 1,
            _ => break,
        }
    }
    i
}

/// Emits an identifier and, when it is a word that introduces a definition, the
/// name that follows it. Returns how much of `rest` was consumed.
fn word(out: &mut String, rest: &str, l: &Lang) -> usize {
    let n = ident_len(rest, l);
    let (name, tail) = rest.split_at(n);

    if !l.defines.contains(&name) {
        match l.caps && is_const(name) {
            true => span(out, "con", name),
            false => push_esc(out, name),
        }
        return n;
    }

    push_esc(out, name);
    let gap = tail.len() - tail.trim_start_matches([' ', '\t']).len();
    let after = &tail[gap..];
    let m = ident_len(after, l);

    // An introducer never names an introducer: `let rec f` names `f`.
    let names = after.chars().next().is_some_and(is_ident_start);
    match names && !l.defines.contains(&&after[..m]) {
        false => n,
        true => {
            out.push_str(&tail[..gap]);
            span(out, "def", &after[..m]);
            n + gap + m
        }
    }
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

/// A trailing `'` belongs to the name where it is not a string delimiter:
/// OCaml's `lt'` is one identifier, not an identifier and an open quote.
fn ident_len(rest: &str, l: &Lang) -> usize {
    let prime = !l.quotes.contains('\'');
    rest.bytes()
        .take_while(|b| b.is_ascii_alphanumeric() || *b == b'_' || (prime && *b == b'\''))
        .count()
}

/// Entirely uppercase and underscores. A purely lexical rule, so constants need
/// no per-language data at all.
fn is_const(name: &str) -> bool {
    name.len() >= 2
        && name.bytes().any(|b| b.is_ascii_uppercase())
        && name
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
}

fn is_pun(c: char) -> bool {
    c.is_ascii_punctuation() && !matches!(c, '"' | '\'' | '`' | '_')
}

/// A run of punctuation, stopping where a comment begins so `;` in `x);` and
/// `;` opening an assembly comment are not one token.
fn pun(rest: &str, l: &Lang) -> usize {
    let b = rest.as_bytes();
    let mut i = 1;
    while i < b.len() && is_pun(b[i] as char) && comment(&rest[i..], l).is_none() {
        i += 1;
    }
    i
}

fn char_len(s: &str) -> usize {
    s.chars().next().map_or(0, char::len_utf8)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hl(code: &str, lang: &str) -> String {
        render(code, lang)
    }

    #[test]
    fn an_unknown_language_is_escaped_and_nothing_else() {
        assert_eq!(hl("a < b & c", ""), "a &lt; b &amp; c");
        assert_eq!(hl("plain text", "wolof"), "plain text");
        assert_eq!(hl("<script>", "text"), "&lt;script&gt;");
    }

    #[test]
    fn code_is_escaped_even_inside_a_span() {
        let out = hl("// <script>alert(1)</script>", "rust");
        assert!(!out.contains("<script>"), "raw tag survived: {out}");
        assert_eq!(
            out,
            "<span class=\"com\">// &lt;script&gt;alert(1)&lt;/script&gt;</span>"
        );
    }

    #[test]
    fn comments_are_recognised_per_language() {
        assert!(hl("# a comment", "bash").starts_with("<span class=\"com\">#"));
        assert!(hl("; a comment", "asm").starts_with("<span class=\"com\">;"));
        assert!(hl("(* a comment *)", "ocaml").starts_with("<span class=\"com\">(*"));
        assert!(hl("// a comment", "go").starts_with("<span class=\"com\">//"));
        // A `#` is not a comment in a language that has no line comment.
        assert!(!hl("#[derive(Debug)]", "rust").contains("class=\"com\""));
    }

    #[test]
    fn ocaml_nests_block_comments() {
        let out = hl("(* a (* b *) c *) x", "ocaml");
        assert_eq!(
            out, "<span class=\"com\">(* a (* b *) c *)</span> x",
            "the inner close must not end the outer comment"
        );
    }

    #[test]
    fn a_comment_marker_inside_a_string_is_not_a_comment() {
        assert_eq!(
            hl("\"a # b\"", "bash"),
            "<span class=\"str\">\"a # b\"</span>"
        );
        assert_eq!(
            hl("db \"a ; b\"", "asm"),
            "db <span class=\"str\">\"a ; b\"</span>"
        );
        // ...and a quote inside a comment does not open a string.
        assert_eq!(hl("# don't", "bash"), "<span class=\"com\"># don't</span>");
    }

    #[test]
    fn a_string_stops_at_the_end_of_its_line() {
        let out = hl("\"unterminated\nlet x = 1", "ocaml");
        assert!(
            out.contains("<span class=\"str\">\"unterminated</span>"),
            "got: {out}"
        );
        assert!(
            out.contains("class=\"def\">x<"),
            "the next line still lexes: {out}"
        );
    }

    #[test]
    fn a_lifetime_is_not_a_character_literal_but_a_character_is() {
        assert_eq!(hl("'a", "rust"), "'a");
        assert_eq!(hl("'x'", "rust"), "<span class=\"str\">'x'</span>");
        assert_eq!(hl("'\\n'", "rust"), "<span class=\"str\">'\\n'</span>");
        assert_eq!(hl("'0'", "asm"), "<span class=\"str\">'0'</span>");
        // OCaml primes belong to the name.
        assert_eq!(hl("lt'", "ocaml"), "lt'");
        // Where `'` is a string delimiter it stays one.
        assert_eq!(hl("'abc'", "bash"), "<span class=\"str\">'abc'</span>");
    }

    #[test]
    fn a_definition_names_the_identifier_after_the_introducer() {
        assert!(hl("fn main() {}", "rust").contains("class=\"def\">main<"));
        assert!(hl("let rec to_string t =", "ocaml").contains("class=\"def\">to_string<"));
        assert!(hl("type term =", "ocaml").contains("class=\"def\">term<"));
        assert!(hl("func Add(a int)", "go").contains("class=\"def\">Add<"));
        // The introducer itself stays unstyled -- keywords are not coloured.
        assert!(!hl("fn main() {}", "rust").contains(">fn<"));
        // Nothing to name is not an error.
        assert_eq!(hl("let", "ocaml"), "let");
        assert!(hl("func (r *R) f()", "go").contains("func <span class=\"pun\">("));
    }

    #[test]
    fn all_caps_identifiers_are_constants_except_in_assembly() {
        assert!(hl("MAX_SIZE", "rust").contains("class=\"con\">MAX_SIZE<"));
        assert!(!hl("maxSize", "rust").contains("class=\"con\""));
        // A single capital is a type variable far more often than a constant.
        assert!(!hl("T", "rust").contains("class=\"con\""));
        // Uppercase mnemonics would colour every token of an assembly block.
        assert_eq!(hl("MOV AL", "asm"), "MOV AL");
    }

    #[test]
    fn numbers_share_the_string_colour_and_stop_where_the_literal_does() {
        assert_eq!(hl("34h", "asm"), "<span class=\"str\">34h</span>");
        assert_eq!(
            hl("0..5", "rust"),
            "<span class=\"str\">0</span><span class=\"pun\">..</span><span class=\"str\">5</span>"
        );
        // A digit inside a name is part of the name, not a literal.
        assert_eq!(hl("t1", "ocaml"), "t1");
    }

    #[test]
    fn punctuation_runs_are_one_span_and_stop_before_a_comment() {
        assert_eq!(hl("->", "ocaml"), "<span class=\"pun\">-&gt;</span>");
        assert_eq!(
            hl("x); ", "asm"),
            "x<span class=\"pun\">)</span><span class=\"com\">; </span>"
        );
    }

    #[test]
    fn multibyte_source_is_never_split_mid_character() {
        assert_eq!(
            hl("\"λ\" ^ x", "ocaml"),
            "<span class=\"str\">\"λ\"</span> <span class=\"pun\">^</span> x"
        );
        assert_eq!(hl("(* λ *)", "ocaml"), "<span class=\"com\">(* λ *)</span>");
    }

    #[test]
    fn every_code_block_in_the_repository_lexes_to_the_same_text() {
        // The lexer may only add markup: strip it and the source must come back
        // byte for byte, which is the one invariant a hand-written lexer can
        // silently break.
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("content/blog");
        let mut blocks = 0;
        for entry in std::fs::read_dir(&dir).unwrap().filter_map(Result::ok) {
            let src = std::fs::read_to_string(entry.path()).unwrap();
            // Odd chunks are the fenced blocks; the first line of one is its
            // info string.
            for chunk in src.split("```").skip(1).step_by(2) {
                let (info, code) = chunk.split_once('\n').unwrap_or((chunk, ""));
                let name = info.split(' ').next().unwrap_or("");
                blocks += 1;
                assert_eq!(unmark(&render(code, name)), esc_all(code), "in {name:?}");
            }
        }
        assert!(blocks > 50, "expected to see every block, saw {blocks}");
    }

    /// Removes every `<span>` the highlighter adds.
    fn unmark(html: &str) -> String {
        let mut out = String::new();
        let mut rest = html;
        while let Some(lt) = rest.find('<') {
            out.push_str(&rest[..lt]);
            let end = rest[lt..].find('>').map_or(rest.len(), |i| lt + i + 1);
            rest = &rest[end..];
        }
        out.push_str(rest);
        out
    }

    fn esc_all(s: &str) -> String {
        let mut out = String::new();
        push_esc(&mut out, s);
        out
    }
}
