use std::collections::BTreeSet;

use syntect::html::{css_for_theme_with_class_style, ClassStyle, ClassedHTMLGenerator};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

use crate::content::Post;

pub struct Highlighter {
    ss: SyntaxSet,
}

impl Default for Highlighter {
    fn default() -> Self {
        Self::new()
    }
}

impl Highlighter {
    pub fn new() -> Self {
        Highlighter {
            ss: SyntaxSet::load_defaults_newlines(),
        }
    }

    pub fn highlight(&self, code: &str, lang: &str) -> String {
        let lc = lang.to_ascii_lowercase();
        let syntax = self
            .ss
            .find_syntax_by_token(alias(&lc))
            .unwrap_or_else(|| self.ss.find_syntax_plain_text());
        let mut gen =
            ClassedHTMLGenerator::new_with_class_style(syntax, &self.ss, ClassStyle::Spaced);
        for line in LinesWithEndings::from(code) {
            let _ = gen.parse_html_for_line_which_includes_newline(line);
        }
        gen.finalize()
    }
}

fn alias(lang: &str) -> &str {
    match lang {
        "typescript" | "ts" | "tsx" | "jsx" | "mjs" | "cjs" => "js",
        "shell" | "console" | "shell-session" | "sh-session" | "zsh" => "bash",
        other => other,
    }
}

pub fn used_classes(posts: &[Post]) -> BTreeSet<&str> {
    posts
        .iter()
        .flat_map(|p| {
            p.html.match_indices("class=\"").filter_map(|(i, m)| {
                let rest = &p.html[i + m.len()..];
                rest.find('"').map(|end| &rest[..end])
            })
        })
        .flat_map(str::split_whitespace)
        .collect()
}

fn reachable(selector: &str, used: &BTreeSet<&str>) -> bool {
    selector.split('.').skip(1).all(|frag| {
        let class = frag
            .split(|c: char| !(c.is_alphanumeric() || c == '-' || c == '_'))
            .next()
            .unwrap_or("");
        class.is_empty() || used.contains(class)
    })
}

fn scope(css: &str, prefix: &str, used: &BTreeSet<&str>) -> String {
    css.split('}')
        .filter_map(|block| block.split_once('{'))
        .filter_map(|(head, body)| {
            let selectors = head.split_once("*/").map_or(head, |(_, rest)| rest);
            let kept: Vec<String> = selectors
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty() && reachable(s, used))
                .map(|s| format!("{prefix} {s}"))
                .collect();
            let body = body.trim();
            (!kept.is_empty() && !body.is_empty())
                .then(|| format!("{} {{\n{body}\n}}\n", kept.join(", ")))
        })
        .collect()
}

pub fn highlight_css(used: &BTreeSet<&str>) -> String {
    let ts = syntect::highlighting::ThemeSet::load_defaults();
    let light = css_for_theme_with_class_style(&ts.themes["InspiredGitHub"], ClassStyle::Spaced)
        .unwrap_or_default();
    let dark = css_for_theme_with_class_style(&ts.themes["base16-ocean.dark"], ClassStyle::Spaced)
        .unwrap_or_default();
    format!(
        "{}\n{}",
        scope(&light, ".hl", used),
        scope(&dark, "[data-theme=\"dark\"] .hl", used)
    )
}
