pub mod content;
pub mod dev;
pub mod feeds;
pub mod markdown;
pub mod render;

use std::fs;
use std::io::{Error, ErrorKind, Result};
use std::path::{Path, PathBuf};

use content::Post;
use render::{fragment_url, post_url, tag_url, Page};

pub const TITLE: &str = "vitor s. almeida";
pub const WEBSITE: &str = "https://vitorsalmeida.com";
pub const DESCRIPTION: &str =
    "A dedicated space to share part of me. You will find some articles, essays and some links";
pub const AUTHOR: &str = "vitor s. almeida";
pub const BIRTH_YEAR: i32 = 2004;

#[derive(Clone, Copy, Debug)]
pub struct Ctx {
    pub year: i32,
    pub live_reload: bool,
    pub drafts: bool,
}

impl Ctx {
    pub fn prod(year: i32) -> Self {
        Ctx {
            year,
            live_reload: false,
            drafts: false,
        }
    }

    pub fn dev(year: i32) -> Self {
        Ctx {
            year,
            live_reload: true,
            drafts: true,
        }
    }
}

fn invalid(path: &Path, msg: impl std::fmt::Display) -> Error {
    Error::new(ErrorKind::InvalidData, format!("{}: {msg}", path.display()))
}

fn load_posts(dir: &Path) -> Result<Vec<Post>> {
    let mut paths: Vec<PathBuf> = fs::read_dir(dir)
        .map_err(|e| invalid(dir, format!("cannot read content directory ({e})")))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "md"))
        .collect();
    paths.sort();

    paths
        .iter()
        .map(|path| {
            let raw = fs::read_to_string(path)?;
            let slug = path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .ok_or_else(|| invalid(path, "no filename"))?;
            content::parse(&raw, &slug).map_err(|e| invalid(path, e))
        })
        .collect()
}

pub fn list_files(dir: &Path) -> Result<Vec<(PathBuf, PathBuf)>> {
    fn walk(dir: &Path, prefix: &Path, into: &mut Vec<(PathBuf, PathBuf)>) -> Result<()> {
        let mut entries: Vec<PathBuf> = fs::read_dir(dir)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .collect();
        entries.sort();
        for path in entries {
            let Some(name) = path.file_name() else {
                continue;
            };
            let rel = prefix.join(name);
            match path.is_dir() {
                true => walk(&path, &rel, into)?,
                false => into.push((rel, path)),
            }
        }
        Ok(())
    }

    let mut files = Vec::new();
    if dir.is_dir() {
        walk(dir, Path::new(""), &mut files)?;
    }
    Ok(files)
}

fn clean(dist: &Path) -> Result<()> {
    match fs::remove_dir_all(dist) {
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

fn prepare(dist: &Path, rel: impl AsRef<Path>) -> Result<PathBuf> {
    let path = dist.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(path)
}

fn write(dist: &Path, rel: impl AsRef<Path>, contents: impl AsRef<[u8]>) -> Result<()> {
    fs::write(prepare(dist, rel)?, contents)
}

fn write_page(dist: &Path, ctx: Ctx, page: &Page) -> Result<String> {
    let reload = ctx.live_reload.then(dev::live_reload);
    let html = render::layout(page, ctx.year, reload.as_deref());
    let path = format!("{}index.html", page.canonical.trim_start_matches('/'));
    write(dist, path, html)?;
    Ok(page.canonical.to_string())
}

pub fn build(root: &Path, dist: &Path, ctx: Ctx) -> Result<()> {
    let posts = content::newest_first(
        load_posts(&root.join("content/blog"))?
            .into_iter()
            .filter(|p| ctx.drafts || !p.draft)
            .collect(),
    );

    clean(dist)?;

    let tags = content::tag_counts(&posts);
    let pages = [
        render::home(&posts, ctx.year),
        render::blog_index(&posts),
        render::tags_index(&tags),
    ];
    let listed: Vec<String> = pages
        .iter()
        .map(|page| write_page(dist, ctx, page))
        .collect::<Result<Vec<String>>>()?;

    let post_urls: Vec<String> = posts
        .iter()
        .map(|post| {
            let url = post_url(&post.slug);
            write_page(dist, ctx, &render::post_page(post, &url))?;
            write(
                dist,
                format!("{}.html", fragment_url(&post.slug).trim_start_matches('/')),
                render::post_fragment(post),
            )?;
            Ok(url)
        })
        .collect::<Result<Vec<String>>>()?;

    let tag_urls: Vec<String> = tags
        .iter()
        .map(|(tag, _)| {
            let url = tag_url(tag);
            let tagged: Vec<&Post> = posts
                .iter()
                .filter(|p| p.tags.iter().any(|t| t == tag))
                .collect();
            write_page(dist, ctx, &render::tag_page(tag, &tagged, &url))
        })
        .collect::<Result<Vec<String>>>()?;

    let all: Vec<&Post> = posts.iter().collect();
    write(dist, "rss.xml", feeds::rss(&all))?;
    write(
        dist,
        "sitemap.xml",
        feeds::sitemap(&[listed, post_urls, tag_urls].concat()),
    )?;

    let static_dir = root.join("static");
    list_files(&static_dir)?
        .iter()
        .try_for_each(|(rel, src)| fs::copy(src, prepare(dist, rel)?).map(|_| ()))
}
