//! End-to-end checks on a real build of the repository's own content.
//!
//! The URL surface is pinned here; link integrity and class coverage are
//! `blog::check`, which is also `ssg check`, so the test and the command cannot
//! disagree about what a good build looks like.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use blog::Ctx;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Built once and shared: `Ctx::prod` rasterizes an OG image per post.
fn dist() -> &'static Path {
    static DIST: OnceLock<PathBuf> = OnceLock::new();
    DIST.get_or_init(|| {
        let out = std::env::temp_dir().join("blog-integration-dist");
        blog::build(&root(), &out, Ctx::prod(2026)).expect("build");
        out
    })
}

fn files() -> Vec<String> {
    fn walk(dir: &Path, prefix: &str, into: &mut Vec<String>) {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| e.path())
            .collect();
        entries.sort();
        for path in entries {
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            let rel = match prefix {
                "" => name,
                p => format!("{p}/{name}"),
            };
            if path.is_dir() {
                walk(&path, &rel, into);
            } else {
                into.push(rel);
            }
        }
    }
    let mut out = Vec::new();
    walk(dist(), "", &mut out);
    out
}

fn pages() -> Vec<(String, String)> {
    files()
        .into_iter()
        .filter(|f| f.ends_with(".html"))
        .map(|f| {
            let body = std::fs::read_to_string(dist().join(&f)).unwrap();
            (f, body)
        })
        .collect()
}

#[test]
fn the_url_surface_is_exactly_this() {
    let got: BTreeSet<String> = files().into_iter().collect();

    let slugs = [
        "building-json-parser-from-scratch",
        "intro-bigo",
        "introduction-to-machine-code",
        "introduction-untyped-lc",
        "is-data-struct-about-memory",
        "proving-naturals-infinity",
    ];
    let expected: BTreeSet<String> = [
        "_headers",
        "index.html",
        "blog/index.html",
        "tags/index.html",
        "rss.xml",
        "sitemap.xml",
        "robots.txt",
        "og_default.jpg",
    ]
    .iter()
    .map(|s| s.to_string())
    .chain(
        slugs
            .iter()
            .flat_map(|s| [format!("blog/{s}/index.html"), format!("blog/{s}/og.png")]),
    )
    .chain(
        ["code", "computer-science", "math"]
            .iter()
            .map(|t| format!("tag/{t}/index.html")),
    )
    .collect();

    let missing: Vec<&String> = expected.difference(&got).collect();
    assert!(missing.is_empty(), "not written: {missing:?}");

    // Post media and fonts are the remainder; spot-check their shape rather
    // than pinning every file in static/.
    let extra: Vec<&String> = got.difference(&expected).collect();
    assert!(
        extra.iter().all(|f| f.contains('/')),
        "unexpected top-level output: {extra:?}"
    );
    assert!(
        !got.iter().any(|f| f.ends_with(".js")),
        "the site ships no JavaScript files: {got:?}"
    );
}

#[test]
fn the_build_passes_its_own_checks() {
    // `blog::check` is `ssg check`: every internal link resolves to a file that
    // was written, and every class in the markup is one a stylesheet defines.
    let problems = blog::check::run(&root(), dist()).unwrap();
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn no_page_carries_a_script_the_browser_has_to_fetch() {
    // The whole point of dropping htmx: navigation is the platform's job now.
    for (page, body) in pages() {
        assert!(
            !body.contains("<script src") && !body.contains("<script defer"),
            "{page} loads an external script"
        );
        assert!(
            !body.contains("hx-"),
            "{page} still carries an htmx attribute"
        );
    }
}
