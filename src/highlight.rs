use std::collections::BTreeSet;

use syntect::html::{css_for_theme_with_class_style, ClassStyle, ClassedHTMLGenerator};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

use crate::content::Post;
use crate::css::{self, Token};

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

fn reachable(selector: &[Token], used: &BTreeSet<&str>) -> bool {
    css::selector_classes(selector).all(|class| used.contains(class.as_ref()))
}

fn scope(css_src: &str, prefix: &str, used: &BTreeSet<&str>) -> String {
    let tokens: Vec<Token> = css::tokenize(css_src).collect();
    scope_rules(&css::rules(&tokens), prefix, used)
}

fn scope_rules(rules: &[css::Rule], prefix: &str, used: &BTreeSet<&str>) -> String {
    rules
        .iter()
        .map(|rule| match (rule.is_at_rule(), rule.block) {
            (true, Some(block)) => {
                let inner = scope_rules(&css::rules(block), prefix, used);
                match inner.is_empty() {
                    true => String::new(),
                    false => format!("{} {{\n{inner}}}\n", css::serialize(rule.prelude).trim()),
                }
            }
            (true, None) => format!("{};\n", css::serialize(rule.prelude).trim()),
            (false, Some(block)) => {
                let kept: Vec<String> = css::selectors(rule.prelude)
                    .into_iter()
                    .filter(|selector| reachable(selector, used))
                    .map(|selector| format!("{prefix} {}", css::serialize(selector)))
                    .collect();
                let body = css::serialize(block);
                let body = body.trim();
                match kept.is_empty() || body.is_empty() {
                    true => String::new(),
                    false => format!("{} {{\n{body}\n}}\n", kept.join(", ")),
                }
            }
            (false, None) => String::new(),
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
