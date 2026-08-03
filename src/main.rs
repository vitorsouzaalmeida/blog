//! `ssg build | dev | new "Title" | check`

use chrono::{Datelike, Local, NaiveDate};
use std::io::{Error, ErrorKind, Result};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use blog::{dev, Ctx};

fn root() -> PathBuf {
    std::env::current_dir().expect("current directory")
}

fn port() -> u16 {
    std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8788)
}

fn main() -> ExitCode {
    let root = root();
    let dist = root.join("dist");
    let today = Local::now().date_naive();
    let args: Vec<String> = std::env::args().collect();

    let result = match args.get(1).map(String::as_str) {
        Some("dev") => dev::serve(&root, &dist, port()),
        Some("new") => new(&root, args.get(2).map(String::as_str), today),
        Some("check") => check(&root, &dist, today.year()),
        _ => blog::build(&root, &dist, Ctx::prod(today.year())).map(|()| println!("built dist/")),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn bad(msg: impl Into<String>) -> Error {
    Error::new(ErrorKind::InvalidInput, msg.into())
}

/// Builds, then reports everything wrong with what it wrote.
fn check(root: &Path, dist: &Path, year: i32) -> Result<()> {
    blog::build(root, dist, Ctx::prod(year))?;
    match blog::check::run(root, dist)?.as_slice() {
        [] => Ok(println!("checked dist/")),
        problems => Err(bad(problems.join("\n"))),
    }
}

/// Scaffolds a post and opens it. Refuses to overwrite: the slug is the URL, so
/// a collision is a post you have already published.
fn new(root: &Path, title: Option<&str>, today: NaiveDate) -> Result<()> {
    let title = title.ok_or_else(|| bad("usage: ssg new \"post title\""))?;
    let slug = slugify(title);
    if slug.is_empty() {
        return Err(bad(format!("{title:?} has no letters to make a slug from")));
    }

    let path = root.join("content/blog").join(format!("{slug}.md"));
    if path.exists() {
        return Err(bad(format!("{} already exists", path.display())));
    }

    std::fs::write(&path, scaffold(title, today))?;
    println!("{}", path.display());

    match std::env::var("EDITOR").ok().filter(|e| !e.is_empty()) {
        None => Ok(()),
        Some(editor) => Command::new(editor).arg(&path).status().map(|_| ()),
    }
}

fn slugify(title: &str) -> String {
    title
        .chars()
        .map(|c| match c.is_ascii_alphanumeric() {
            true => c.to_ascii_lowercase(),
            false => '-',
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<&str>>()
        .join("-")
}

/// Every optional key, commented out: the frontmatter parser rejects unknown
/// ones, so this is also the reference for what a post may declare.
fn scaffold(title: &str, today: NaiveDate) -> String {
    // Always quoted: a title is prose, and `Rust: a tour` unquoted is a key.
    let quoted = format!("\"{}\"", title.replace('\\', "\\\\").replace('"', "\\\""));
    format!(
        "---\n\
         title: {quoted}\n\
         pubDate: {today}\n\
         tags:\n  - tag1\n\
         # draft: true                 -- built by `ssg dev`, left out of the site\n\
         # subtitle: A short italic subtitle\n\
         # description: Used for RSS and social meta\n\
         ---\n\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_title_becomes_the_slug_that_becomes_the_url() {
        assert_eq!(
            slugify("How I Isolated My Work Environment"),
            "how-i-isolated-my-work-environment"
        );
        assert_eq!(slugify("Rust: a tour"), "rust-a-tour");
        assert_eq!(slugify("  spaced  out  "), "spaced-out");
        assert_eq!(slugify("C++ & you"), "c-you");
        assert_eq!(slugify("λ"), "");
    }

    #[test]
    fn a_scaffolded_post_parses() {
        let today = NaiveDate::from_ymd_opt(2026, 7, 28).unwrap();
        let post = blog::content::parse(&scaffold("My Post", today), "my-post").unwrap();
        assert_eq!(post.title, "My Post");
        assert_eq!(post.pub_date, today);
        assert_eq!(post.tags, ["tag1"]);
        assert!(!post.draft);

        // A title is prose: unquoted, `: ` in one would be frontmatter syntax.
        let tricky = r#"Rust: a "tour""#;
        let post = blog::content::parse(&scaffold(tricky, today), "x").unwrap();
        assert_eq!(post.title, tricky);
    }
}
