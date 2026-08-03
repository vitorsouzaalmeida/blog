//! `ssg check`: the two things about the output that the output cannot state.
//!
//! Every internal href must resolve to a file the build wrote, and every class
//! name in the markup must be one the stylesheets define. Both survive a green
//! build -- a renamed route leaves working links behind, a renamed class leaves
//! unstyled markup -- and neither is visible until someone clicks.

use std::collections::BTreeSet;
use std::io::Result;
use std::path::Path;

use crate::dev::{self, Resolved};
use crate::{css, disk};

/// Everything wrong with `dist`, or an empty list.
pub fn run(root: &Path, dist: &Path) -> Result<Vec<String>> {
    let pages = pages(dist)?;
    if pages.is_empty() {
        return Ok(vec![format!("{}: no pages to check", dist.display())]);
    }
    let defined = defined_classes(root)?;

    Ok(pages
        .iter()
        .flat_map(|(page, body)| {
            let links = attrs(body, "href=\"")
                .into_iter()
                .filter(|href| href.starts_with('/') && !href.starts_with("//"))
                .filter(|href| matches!(dev::resolve(dist, href), Resolved::NotFound))
                .map(|href| format!("{page} links to {href}, which was never written"));

            // Highlighted code carries the highlighter's own classes, which are
            // styled by rule rather than by name.
            let classes = attrs(&outside_code(body), "class=\"")
                .into_iter()
                .flat_map(|value| {
                    value
                        .split_whitespace()
                        .map(str::to_string)
                        .collect::<Vec<String>>()
                })
                .filter(|class| !defined.contains(class))
                .map(|class| format!("{page} uses `.{class}`, which no stylesheet defines"));

            links.chain(classes).collect::<Vec<String>>()
        })
        .collect())
}

fn pages(dist: &Path) -> Result<Vec<(String, String)>> {
    disk::list_files(dist)?
        .into_iter()
        .filter(|(rel, _)| rel.extension().is_some_and(|e| e == "html"))
        .map(|(rel, path)| {
            let name = rel.to_string_lossy().replace('\\', "/");
            Ok((name, std::fs::read_to_string(&path)?))
        })
        .collect()
}

/// Every value of `attr` in `html`. Crude on purpose: it reads the output of a
/// build, not arbitrary markup.
fn attrs(html: &str, attr: &str) -> Vec<String> {
    html.match_indices(attr)
        .filter_map(|(i, _)| {
            let rest = &html[i + attr.len()..];
            rest.find('"').map(|end| rest[..end].to_string())
        })
        .collect()
}

fn outside_code(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(start) = rest.find("<pre class=\"hl\">") {
        out.push_str(&rest[..start]);
        rest = match rest[start..].find("</pre>") {
            Some(end) => &rest[start + end + "</pre>".len()..],
            None => "",
        };
    }
    out.push_str(rest);
    out
}

fn defined_classes(root: &Path) -> Result<BTreeSet<String>> {
    let dir = root.join("static");
    Ok(disk::STYLESHEETS
        .iter()
        .map(|name| std::fs::read_to_string(dir.join(name)))
        .collect::<Result<Vec<String>>>()?
        .iter()
        .flat_map(|source| {
            css::tokenize(source)
                .collect::<Vec<css::Token>>()
                .windows(2)
                .filter_map(|pair| match pair {
                    [css::Token::Delim('.'), css::Token::Ident(ident)] => Some(ident.to_string()),
                    _ => None,
                })
                .collect::<Vec<String>>()
        })
        .collect())
}
