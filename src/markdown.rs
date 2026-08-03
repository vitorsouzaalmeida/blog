use latex2mathml::{latex_to_mathml, DisplayStyle};
use pulldown_cmark::{html, CodeBlockKind, CowStr, Event, Options, Parser, Tag, TagEnd};

use crate::highlight;

fn options() -> Options {
    Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_MATH
}

fn mathml(latex: &str, style: DisplayStyle) -> Result<String, ()> {
    latex_to_mathml(latex, style).map_err(|_| ())
}

pub fn render(src: &str) -> String {
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
                let inner = highlight::render(&code, &lang);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn md(src: &str) -> String {
        render(src)
    }

    #[test]
    fn a_fenced_block_becomes_a_classed_pre() {
        let out = md("```rust\nfn main() {}\n```\n");
        assert!(out.starts_with("<pre class=\"hl\"><code>"), "got: {out}");
        assert!(out.ends_with("</code></pre>"), "got: {out}");
        // `highlight` marks the name a definition introduces, and nothing else
        // about this line.
        assert!(out.contains("class=\"def\">main<"), "got: {out}");
    }

    #[test]
    fn an_unknown_language_still_renders_as_a_code_block() {
        let out = md("```wolof\nsome text\n```\n");
        assert!(out.contains("<pre class=\"hl\">"), "got: {out}");
        assert!(out.contains("some text"), "got: {out}");
    }

    #[test]
    fn an_indented_block_is_a_code_block_with_no_language() {
        let out = md("    indented code\n");
        assert!(out.contains("<pre class=\"hl\">"), "got: {out}");
        assert!(out.contains("indented code"), "got: {out}");
    }

    #[test]
    fn only_the_first_word_of_the_info_string_is_the_language() {
        let tagged = md("```rust,ignore\nfn main() {}\n```\n");
        assert_eq!(tagged, md("```rust\nfn main() {}\n```\n"));
    }

    #[test]
    fn code_is_escaped_not_injected() {
        let out = md("```html\n<script>alert(1)</script>\n```\n");
        assert!(!out.contains("<script>"), "raw script tag survived: {out}");
        assert!(!out.contains("</script>"), "raw close tag survived: {out}");
        assert!(out.contains("&lt;"), "nothing was escaped: {out}");
    }

    #[test]
    fn math_becomes_mathml() {
        assert!(md("$x + 1$\n").contains("<math"));
        assert!(md("$$\\frac{1}{2}$$\n").contains("<math"));
    }

    #[test]
    fn malformed_latex_does_not_fail_the_build() {
        // Recorded because it is surprising: `latex2mathml` reports a bad
        // expression *inside* the MathML rather than returning `Err`, so the
        // `Event::Text` fallback above almost never fires and a typo ships a
        // visible "[PARSE ERROR: ...]" instead of breaking the page.
        let out = md("$\\frac{1}$\n");
        assert!(out.contains("PARSE ERROR"), "got: {out}");
    }
}
