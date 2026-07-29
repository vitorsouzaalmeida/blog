pub const STYLESHEETS: [&str; 2] = ["fonts/fonts.css", "styles.css"];

pub const SCRIPTS: [&str; 4] = [
    "vendor/htmx.min.js",
    "vendor/head-support.min.js",
    "vendor/preload.min.js",
    "vendor/idiomorph-ext.min.js",
];

pub const PRELOAD_FONTS: [&str; 2] = [
    "/fonts/newsreader-300-700-normal-latin.woff2",
    "/fonts/jetbrainsmono-400-normal-latin.woff2",
];

#[derive(Clone, Copy)]
pub struct Page<'a> {
    pub css: &'a str,
    pub highlight: &'a str,
    pub script: &'a str,
}

pub fn script_path(body: &str) -> String {
    format!("/vendor/app.{:016x}.js", fnv1a(body.as_bytes()))
}

fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |h, b| {
        (h ^ *b as u64).wrapping_mul(0x100000001b3)
    })
}

pub fn minify(css: &str) -> String {
    crate::css::minify(css)
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn script_path_tracks_content() {
        assert_eq!(script_path("a"), script_path("a"));
        assert_ne!(script_path("a"), script_path("b"));
        assert!(script_path("a").starts_with("/vendor/app."));
    }
}
