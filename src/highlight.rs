//! Deliberately partial syntax highlighting.
//!
//! Follows Tonsky's *Syntax Highlighting* <https://tonsky.me/blog/syntax-highlighting/>:
//! "if everything is highlighted, nothing is highlighted". Coloured are the
//! things a reader looks *for* -- strings and numbers, comments, constants, and
//! the name a definition introduces. Keywords, variables, calls and types are
//! not, which is most of every block, by design.
//!
//! Four classes, coloured by `static/highlight.css` from the site's palette:
//!
//! ```text
//! str  strings, characters, numbers
//! com  comments
//! con  constants
//! def  the name a definition introduces
//! ```
//!
//! `syntect` does the lexing, against Sublime's syntax definitions, and this
//! module only decides which of its scopes earn a class -- so a language is
//! correct because someone else maintains its grammar, while the palette stays
//! this site's. Every scope with no rule in `RULES` inherits `--ink`, which is
//! most of them.
//!
//! The mapping falls out of a property of those definitions worth naming: they
//! scope a *binding site* and leave a *use* alone. In OCaml `reduce` in
//! `let rec reduce` is `entity.name.function` and `reduce` in `reduce ~term:lt`
//! is nothing at all, so "colour definitions, not uses" needs no rule of its
//! own -- it is what the grammar already says.

use std::sync::OnceLock;

use syntect::parsing::{ParseState, Scope, ScopeStack, SyntaxReference, SyntaxSet};
use syntect::util::LinesWithEndings;

/// Scope prefixes that earn a class, in priority order: the first whose prefix
/// appears anywhere in a token's scope stack wins.
///
/// Order is what keeps a delimiter with the thing it delimits. A quote carries
/// both `string.quoted.double` and `punctuation.definition.string.begin`, and a
/// `{name}` inside a format string carries `constant.other.placeholder`; because
/// `string` is tested before either, the whole literal comes out one colour
/// rather than three.
const RULES: [(&str, &str); 8] = [
    ("comment", "com"),
    ("string", "str"),
    ("constant.numeric", "str"),
    ("constant.character", "str"),
    ("entity.name", "def"),
    ("variable.parameter", "def"),
    // A `let` binding in OCaml, which is a name a definition introduces even
    // though the grammar calls it a constant.
    ("variable.other.constant", "def"),
    ("constant", "con"),
];

/// Loaded once: the dump is some two megabytes to deserialise, and a build
/// highlights every block of every post.
fn syntaxes() -> &'static SyntaxSet {
    static SET: OnceLock<SyntaxSet> = OnceLock::new();
    SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

/// `RULES` with each selector interned, which is what `is_prefix_of` compares.
fn rules() -> &'static [(Scope, &'static str)] {
    static INTERNED: OnceLock<Vec<(Scope, &'static str)>> = OnceLock::new();
    INTERNED.get_or_init(|| {
        RULES
            .iter()
            .filter_map(|(sel, class)| Scope::new(sel).ok().map(|scope| (scope, *class)))
            .collect()
    })
}

/// Info-string words Sublime does not know by that name. Everything else is
/// looked up as written, and anything it still does not know is plain text.
fn alias(name: &str) -> &str {
    match name {
        "typescript" | "ts" | "tsx" | "jsx" | "mjs" | "cjs" => "js",
        "shell" | "console" | "shell-session" | "sh-session" | "zsh" => "bash",
        other => other,
    }
}

fn is_asm(name: &str) -> bool {
    matches!(name, "asm" | "nasm" | "x86asm")
}

/// The contents of one `<code>` element: escaped, and marked up if the language
/// is one we can lex.
pub fn render(code: &str, name: &str) -> String {
    let name = name.to_ascii_lowercase();
    if is_asm(&name) {
        return asm(code);
    }
    match syntaxes().find_syntax_by_token(alias(&name)) {
        Some(syntax) => lex(code, syntax),
        None => escaped(code),
    }
}

fn escaped(code: &str) -> String {
    let mut out = String::with_capacity(code.len());
    push_esc(&mut out, code);
    out
}

/// The class a token's scope stack earns, or `None` for the majority that earn
/// nothing.
fn class(stack: &ScopeStack) -> Option<&'static str> {
    rules()
        .iter()
        .find(|(prefix, _)| stack.as_slice().iter().any(|s| prefix.is_prefix_of(*s)))
        .map(|&(_, class)| class)
}

fn lex(code: &str, syntax: &SyntaxReference) -> String {
    let set = syntaxes();
    let mut state = ParseState::new(syntax);
    let mut stack = ScopeStack::new();
    let mut out = String::with_capacity(code.len() * 2);
    // The class of the span currently open, so that a run of tokens sharing one
    // is a single element rather than one per token.
    let mut open: Option<&'static str> = None;

    for line in LinesWithEndings::from(code) {
        // A grammar that fails mid-block leaves the rest of the block plain
        // rather than failing the build: the code is still readable, which is
        // the whole point of printing it.
        let Ok(ops) = state.parse_line(line, set) else {
            emit(&mut out, &mut open, None, line);
            continue;
        };

        let mut last = 0;
        for (at, op) in ops {
            if at > last {
                emit(&mut out, &mut open, class(&stack), &line[last..at]);
            }
            if stack.apply(&op).is_err() {
                break;
            }
            last = at;
        }
        if last < line.len() {
            emit(&mut out, &mut open, class(&stack), &line[last..]);
        }
    }

    emit(&mut out, &mut open, None, "");
    out
}

/// Writes `text` under `class`, opening and closing spans only where the class
/// changes. Called with an empty `text` and no class to close the last one.
fn emit(
    out: &mut String,
    open: &mut Option<&'static str>,
    class: Option<&'static str>,
    text: &str,
) {
    if *open != class {
        if open.is_some() {
            out.push_str("</span>");
        }
        if let Some(class) = class {
            out.push_str("<span class=\"");
            out.push_str(class);
            out.push_str("\">");
        }
        *open = class;
    }
    push_esc(out, text);
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

// -- Assembly -----------------------------------------------------------------

/// Sublime ships no assembler, and the blocks that use one here are mostly `;`
/// comments, so assembly is the one language still lexed by hand.
///
/// Comments, strings and numbers only. No names: uppercase in assembly is how a
/// mnemonic is written rather than how a constant is, so the rule that finds
/// `MAX_SIZE` in every other language would colour every token of a block.
fn asm(code: &str) -> String {
    let mut out = String::with_capacity(code.len() * 2);
    let mut i = 0;

    while i < code.len() {
        let rest = &code[i..];
        let c = rest.chars().next().unwrap_or('\0');

        let (class, n) = if c == ';' || rest.starts_with("//") {
            ("com", rest.find('\n').unwrap_or(rest.len()))
        } else if c == '"' || c == '\'' {
            ("str", quoted(rest, c))
        } else if c.is_ascii_digit() {
            ("str", number(rest))
        } else {
            let n = c.len_utf8();
            push_esc(&mut out, &rest[..n]);
            i += n;
            continue;
        };

        span(&mut out, class, &rest[..n]);
        i += n;
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

/// A quoted run, ending at its closing delimiter or the end of the line, so one
/// stray quote colours a line rather than the rest of the block.
fn quoted(rest: &str, delim: char) -> usize {
    let mut i = delim.len_utf8();
    while i < rest.len() {
        let c = rest[i..].chars().next().unwrap_or('\0');
        if c == '\n' {
            return i;
        }
        i += c.len_utf8();
        if c == delim {
            return i;
        }
    }
    rest.len()
}

/// Digits, then whatever letters and underscores belong to the literal, which
/// is how `34h` and `0x1f` are written.
fn number(rest: &str) -> usize {
    rest.bytes()
        .take_while(|b| b.is_ascii_alphanumeric() || *b == b'_')
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hl(code: &str, lang: &str) -> String {
        render(code, lang)
    }

    /// Whether `text` is exactly what one `class` span holds, which is what
    /// every test below asks.
    fn classed(html: &str, class: &str, text: &str) -> bool {
        html.contains(&format!("<span class=\"{class}\">{text}</span>"))
    }

    #[test]
    fn an_unknown_language_is_escaped_and_nothing_else() {
        assert_eq!(hl("a < b & c", ""), "a &lt; b &amp; c");
        assert_eq!(hl("plain text", "wolof"), "plain text");
        assert_eq!(hl("<script>", "text"), "&lt;script&gt;");
    }

    #[test]
    fn code_is_escaped_even_inside_a_span() {
        let out = hl("// <script>alert(1)</script>\n", "rust");
        assert!(!out.contains("<script>"), "raw tag survived: {out}");
        assert!(out.contains("&lt;script&gt;"), "got: {out}");
        assert!(out.contains("class=\"com\""), "got: {out}");
    }

    #[test]
    fn comments_are_recognised_per_language() {
        assert!(hl("# a comment\n", "bash").contains("class=\"com\""));
        assert!(hl("; a comment\n", "asm").contains("class=\"com\""));
        assert!(hl("(* a comment *)\n", "ocaml").contains("class=\"com\""));
        assert!(hl("// a comment\n", "go").contains("class=\"com\""));
        // A `#` is not a comment in a language that has no line comment.
        assert!(!hl("#[derive(Debug)]\n", "rust").contains("class=\"com\""));
    }

    #[test]
    fn a_comment_marker_inside_a_string_is_not_a_comment() {
        let out = hl("echo \"a # b\"\n", "bash");
        assert!(!out.contains("class=\"com\""), "got: {out}");
        let out = hl("db \"a ; b\"\n", "asm");
        assert!(!out.contains("class=\"com\""), "got: {out}");
    }

    #[test]
    fn ocaml_names_every_binding_a_pattern_introduces() {
        // The reason for this whole module: a hand lexer names `reduce` and
        // stops, because a binder in ML binds a pattern rather than one name.
        let out = hl(
            "let rec reduce ~term =\n  match term with\n  | App (lt, rt) -> lt\n",
            "ocaml",
        );
        assert!(classed(&out, "def", "reduce"), "got: {out}");
        assert!(classed(&out, "def", "lt"), "got: {out}");
        assert!(classed(&out, "def", "rt"), "got: {out}");
    }

    #[test]
    fn ocaml_constructors_are_the_landmarks_of_a_block() {
        let out = hl("type term = Var of string | App of term * term\n", "ocaml");
        assert!(classed(&out, "def", "Var"), "got: {out}");
        assert!(classed(&out, "def", "App"), "got: {out}");
    }

    #[test]
    fn a_use_is_not_a_definition() {
        // `reduce` is named where it is defined and left alone where it is
        // called, which is the rule the grammar already encodes. `t` is named
        // for the same reason: the parameter is a binding, its use is not.
        let out = hl("let rec reduce t = reduce t\n", "ocaml");
        assert!(classed(&out, "def", "reduce"), "got: {out}");
        assert!(classed(&out, "def", "t"), "got: {out}");
        assert!(
            out.ends_with("= reduce t\n"),
            "the call is not marked: {out}"
        );
    }

    #[test]
    fn a_definition_names_the_identifier_it_introduces() {
        assert!(classed(&hl("fn main() {}\n", "rust"), "def", "main"));
        assert!(classed(
            &hl("struct Point { x: f64 }\n", "rust"),
            "def",
            "Point"
        ));
        assert!(classed(&hl("func Add(a int) int {}\n", "go"), "def", "Add"));
        // The introducer itself stays unstyled -- keywords are not coloured.
        assert!(!hl("fn main() {}\n", "rust").contains(">fn<"));
    }

    #[test]
    fn strings_and_numbers_share_a_class_and_swallow_their_delimiters() {
        // The quotes belong to the literal, not to a separate punctuation span:
        // `string` outranks `punctuation.definition.string` in `RULES`.
        let out = hl("let s = \"hi\";\n", "rust");
        assert!(classed(&out, "str", "\"hi\""), "got: {out}");
        assert!(classed(&hl("let n = 10;\n", "rust"), "str", "10"));
    }

    #[test]
    fn all_caps_constants_are_named() {
        assert!(classed(
            &hl("const MAX: usize = 10;\n", "rust"),
            "con",
            "MAX"
        ));
    }

    #[test]
    fn assembly_is_lexed_here_and_mnemonics_stay_unstyled() {
        assert_eq!(
            hl("MOV AL, 34h\n", "asm"),
            "MOV AL, <span class=\"str\">34h</span>\n"
        );
        assert!(hl("mov rax, 1 ; a comment\n", "asm").contains("<span class=\"com\">; a comment"));
        assert!(hl("db '0'\n", "asm").contains("<span class=\"str\">'0'</span>"));
    }

    #[test]
    fn an_alias_reaches_the_syntax_it_names() {
        // TypeScript and the shell session names are not Sublime tokens; the
        // blocks that use them would otherwise come out plain.
        assert!(hl("const x = 1;\n", "typescript").contains("class="));
        assert!(hl("# comment\n", "shell").contains("class=\"com\""));
        assert!(hl("# comment\n", "console").contains("class=\"com\""));
    }

    #[test]
    fn every_code_block_in_the_repository_lexes_to_the_same_text() {
        // The highlighter may only add markup: strip it and the source must come
        // back byte for byte, which is the one invariant a swap of lexers can
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
