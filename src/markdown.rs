use std::sync::OnceLock;

use latex2mathml::{latex_to_mathml, DisplayStyle};
use pulldown_cmark::{html, CodeBlockKind, CowStr, Event, Options, Parser, Tag, TagEnd};
use syntect::html::{ClassStyle, ClassedHTMLGenerator};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

use crate::render::fill;

const CODE_BLOCK: &str = include_str!("../templates/code_block.html");

const CLASS_STYLE: ClassStyle = ClassStyle::SpacedPrefixed { prefix: "hl-" };

fn options() -> Options {
    Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_MATH
}

fn syntaxes() -> &'static SyntaxSet {
    static SET: OnceLock<SyntaxSet> = OnceLock::new();
    SET.get_or_init(two_face::syntax::extra_newlines)
}

fn language(kind: &CodeBlockKind) -> String {
    let raw = match kind {
        CodeBlockKind::Fenced(info) => info.split([' ', ',']).next().unwrap_or(""),
        CodeBlockKind::Indented => "",
    };
    raw.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '+' || *c == '#' || *c == '-')
        .collect()
}

fn token(lang: &str) -> &str {
    match lang {
        "shell" | "console" => "bash",
        other => other,
    }
}

fn highlight(code: &str, lang: &str) -> String {
    let syntaxes = syntaxes();
    let syntax = syntaxes
        .find_syntax_by_token(token(lang))
        .unwrap_or_else(|| syntaxes.find_syntax_plain_text());

    let inner = LinesWithEndings::from(code)
        .fold(
            ClassedHTMLGenerator::new_with_class_style(syntax, syntaxes, CLASS_STYLE),
            |mut generator, line| {
                generator
                    .parse_html_for_line_which_includes_newline(line)
                    .expect("syntax definition");
                generator
            },
        )
        .finalize();

    fill(CODE_BLOCK, &[("lang", lang), ("code", &inner)])
}

fn mathml(latex: &str, style: DisplayStyle) -> Result<String, ()> {
    latex_to_mathml(latex, style).map_err(|_| ())
}

pub fn render(src: &str) -> String {
    let (events, _) = Parser::new_ext(src, options()).fold(
        (Vec::new(), None::<(String, String)>),
        |(mut events, pending), event| match (event, pending) {
            (Event::Start(Tag::CodeBlock(kind)), _) => {
                (events, Some((language(&kind), String::new())))
            }
            (Event::Text(text), Some((lang, code))) => (events, Some((lang, code + &text))),
            (Event::End(TagEnd::CodeBlock), Some((lang, code))) => {
                events.push(Event::Html(CowStr::from(highlight(&code, &lang))));
                (events, None)
            }
            (Event::InlineMath(latex), pending) => {
                events.push(match mathml(&latex, DisplayStyle::Inline) {
                    Ok(math) => Event::InlineHtml(CowStr::from(math)),
                    Err(()) => Event::Text(latex),
                });
                (events, pending)
            }
            (Event::DisplayMath(latex), pending) => {
                events.push(match mathml(&latex, DisplayStyle::Block) {
                    Ok(math) => Event::Html(CowStr::from(math)),
                    Err(()) => Event::Text(latex),
                });
                (events, pending)
            }
            (other, pending) => {
                events.push(other);
                (events, pending)
            }
        },
    );

    let mut out = String::new();
    html::push_html(&mut out, events.into_iter());
    out
}
