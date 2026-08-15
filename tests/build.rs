use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use blog::dev::{resolve, Resolved};
use blog::Ctx;

fn dist() -> &'static Path {
    static DIST: OnceLock<PathBuf> = OnceLock::new();
    DIST.get_or_init(|| {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let out = std::env::temp_dir().join("blog-integration-dist");
        blog::build(&root, &out, Ctx::prod(2026)).expect("build");
        out
    })
}

fn pages() -> Vec<(String, String)> {
    blog::list_files(dist())
        .unwrap()
        .into_iter()
        .map(|(rel, _)| rel.to_string_lossy().replace('\\', "/"))
        .filter(|rel| rel.ends_with(".html"))
        .map(|rel| {
            let body = std::fs::read_to_string(dist().join(&rel)).unwrap();
            (rel, body)
        })
        .collect()
}

fn attribute_values<'a>(body: &'a str, attribute: &str) -> Vec<&'a str> {
    body.match_indices(attribute)
        .filter_map(|(at, _)| {
            let rest = &body[at + attribute.len()..];
            rest.find('"').map(|end| &rest[..end])
        })
        .collect()
}

#[test]
fn every_internal_link_and_expander_resolves_to_something_the_build_wrote() {
    for (page, body) in pages() {
        for attribute in ["href=\"", "hx-get=\"", "src=\""] {
            for value in attribute_values(&body, attribute) {
                if !value.starts_with('/') {
                    continue;
                }
                assert!(
                    matches!(resolve(dist(), value), Resolved::File(_)),
                    "{page}: {attribute}{value}\" does not resolve"
                );
            }
        }
    }
}

#[test]
fn an_expander_asks_for_the_url_the_host_serves_rather_than_the_file() {
    for (page, body) in pages() {
        for value in attribute_values(&body, "hx-get=\"") {
            assert!(
                !value.ends_with(".html"),
                "{page}: {value} would be redirected by auto-trailing-slash"
            );
        }
    }
}

#[test]
fn nothing_boosted_points_at_a_url_that_is_not_a_page() {
    for (page, body) in pages() {
        for anchor in body.split("<a ").skip(1) {
            let Some((tag, _)) = anchor.split_once('>') else {
                continue;
            };
            let Some(href) = attribute_values(tag, "href=\"").first().copied() else {
                continue;
            };
            if !href.starts_with('/') || href.ends_with('/') {
                continue;
            }
            assert!(
                tag.contains("hx-boost=\"false\""),
                "{page}: boosted <a {tag}> would swap a non-page into the body"
            );
        }
    }
}
