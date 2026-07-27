use syntect::html::{css_for_theme_with_class_style, ClassStyle, ClassedHTMLGenerator};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

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

fn scope(css: &str, prefix: &str) -> String {
    let mut out = String::new();
    for line in css.lines() {
        if let Some(sel) = line.trim_end().strip_suffix('{') {
            let scoped: Vec<String> = sel
                .split(',')
                .map(|s| format!("{} {}", prefix, s.trim()))
                .collect();
            out.push_str(&scoped.join(", "));
            out.push_str(" {\n");
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

pub fn highlight_css() -> String {
    let ts = syntect::highlighting::ThemeSet::load_defaults();
    let light = css_for_theme_with_class_style(&ts.themes["InspiredGitHub"], ClassStyle::Spaced)
        .unwrap_or_default();
    let dark = css_for_theme_with_class_style(&ts.themes["base16-ocean.dark"], ClassStyle::Spaced)
        .unwrap_or_default();
    format!(
        "{}\n{}",
        scope(&light, ".hl"),
        scope(&dark, "[data-theme=\"dark\"] .hl")
    )
}
