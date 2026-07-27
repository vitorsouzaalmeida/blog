use latex2mathml::{latex_to_mathml, DisplayStyle};
use pulldown_cmark::{html, CodeBlockKind, CowStr, Event, Options, Parser, Tag, TagEnd};

use crate::content::Post;
use crate::highlight::Highlighter;

fn options() -> Options {
    Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_MATH
}

fn mathml(latex: &str, style: DisplayStyle) -> Result<String, ()> {
    latex_to_mathml(latex, style).map_err(|_| ())
}

pub fn render(src: &str, hl: &Highlighter) -> String {
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
                let inner = hl.highlight(&code, &lang);
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

pub fn render_posts(posts: Vec<Post>, hl: &Highlighter) -> Vec<Post> {
    posts
        .into_iter()
        .map(|p| Post {
            html: render(&p.body, hl),
            ..p
        })
        .collect()
}
