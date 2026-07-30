use latex2mathml::{latex_to_mathml, DisplayStyle};
use pulldown_cmark::{html, CodeBlockKind, CowStr, Event, Options, Parser, Tag, TagEnd};
use syntect::html::{ClassStyle, ClassedHTMLGenerator};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

use crate::content::Post;

fn options() -> Options {
    Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_MATH
}

fn mathml(latex: &str, style: DisplayStyle) -> Result<String, ()> {
    latex_to_mathml(latex, style).map_err(|_| ())
}

fn alias(lang: &str) -> &str {
    match lang {
        "typescript" | "ts" | "tsx" | "jsx" | "mjs" | "cjs" => "js",
        "shell" | "console" | "shell-session" | "sh-session" | "zsh" => "bash",
        other => other,
    }
}

/// Emits `<span class="...">` wrappers named after syntect scopes; the colors
/// live in `static/highlight.css`.
fn highlight(ss: &SyntaxSet, code: &str, lang: &str) -> String {
    let lc = lang.to_ascii_lowercase();
    let syntax = ss
        .find_syntax_by_token(alias(&lc))
        .unwrap_or_else(|| ss.find_syntax_plain_text());
    let mut gen = ClassedHTMLGenerator::new_with_class_style(syntax, ss, ClassStyle::Spaced);
    for line in LinesWithEndings::from(code) {
        let _ = gen.parse_html_for_line_which_includes_newline(line);
    }
    gen.finalize()
}

pub fn render(src: &str, ss: &SyntaxSet) -> String {
    let mut iter = Parser::new_ext(src, options());
    let mut events: Vec<Event> = Vec::new();

    while let Some(ev) = iter.next() {
        match ev {
            Event::Start(Tag::CodeBlock(kind)) => {
                let lang = match &kind {
                    CodeBlockKind::Fenced(l) => {
                        l.split([' ', ',']).next().unwrap_or("").to_string()
                    }
                    CodeBlockKind::Indented => String::new(),
                };
                let code: String = iter
                    .by_ref()
                    .take_while(|e| !matches!(e, Event::End(TagEnd::CodeBlock)))
                    .filter_map(|e| match e {
                        Event::Text(t) => Some(t.to_string()),
                        _ => None,
                    })
                    .collect();
                let inner = highlight(ss, &code, &lang);
                events.push(Event::Html(CowStr::from(format!(
                    "<pre class=\"hl\"><code>{inner}</code></pre>"
                ))));
            }
            Event::InlineMath(latex) => events.push(match mathml(&latex, DisplayStyle::Inline) {
                Ok(m) => Event::InlineHtml(CowStr::from(m)),
                Err(()) => Event::Text(latex),
            }),
            Event::DisplayMath(latex) => events.push(match mathml(&latex, DisplayStyle::Block) {
                Ok(m) => Event::Html(CowStr::from(m)),
                Err(()) => Event::Text(latex),
            }),
            other => events.push(other),
        }
    }

    let mut out = String::new();
    html::push_html(&mut out, events.into_iter());
    out
}

pub fn render_posts(posts: Vec<Post>) -> Vec<Post> {
    let ss = SyntaxSet::load_defaults_newlines();
    posts
        .into_iter()
        .map(|p| Post {
            html: render(&p.body, &ss),
            ..p
        })
        .collect()
}
