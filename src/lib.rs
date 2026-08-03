pub mod check;
pub mod content;
pub mod css;
pub mod dev;
pub mod disk;
pub mod feeds;
pub mod fill;
pub mod frontmatter;
pub mod highlight;
pub mod html;
pub mod markdown;
pub mod model;
pub mod og;
pub mod routes;
pub mod xml;

use std::io::Result;
use std::path::Path;

use content::Post;

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
    pub og_images: bool,
    /// Unpublished posts: visible while you write them, absent from the site.
    pub drafts: bool,
}

impl Ctx {
    pub fn prod(year: i32) -> Self {
        Ctx {
            year,
            live_reload: false,
            og_images: true,
            drafts: false,
        }
    }

    pub fn dev(year: i32) -> Self {
        Ctx {
            year,
            live_reload: true,
            og_images: false,
            drafts: true,
        }
    }
}

pub fn build(root: &Path, dist: &Path, ctx: Ctx) -> Result<()> {
    let content = disk::load(root)?;
    let published: Vec<Post> = content
        .posts
        .into_iter()
        .filter(|p| ctx.drafts || !p.draft)
        .collect();
    let posts = &published;

    content.stylesheets.iter().try_for_each(|(name, source)| {
        css::check(source).map_err(|(err, at)| stylesheet_error(name, source, err, at))
    })?;
    let css = css::minify(
        &content
            .stylesheets
            .iter()
            .map(|(_, source)| source.as_str())
            .collect::<Vec<&str>>()
            .join("\n"),
    );
    let templates = routes::load(&content.templates).map_err(invalid_data)?;
    let collections = routes::collections(&templates).map_err(invalid_data)?;
    let site = model::build(posts, &css, ctx, &collections).map_err(invalid_data)?;

    disk::clean(dist)?;

    let written: Vec<String> = templates
        .routes
        .iter()
        .map(|route| write_pages(route, &templates, &site, dist))
        .collect::<Result<Vec<Vec<String>>>>()?
        .concat();

    let all: Vec<&Post> = posts.iter().collect();
    disk::write(dist, "rss.xml", feeds::rss(&all))?;
    disk::write(dist, "sitemap.xml", feeds::sitemap(&written))?;

    // The stylesheet is inlined into every page, so the dev server needs a copy
    // it can hand back on its own to swap one in place (`dev::CSS_PATH`).
    if ctx.live_reload {
        disk::write(dist, dev::CSS_PATH.trim_start_matches('/'), &css)?;
    }

    if ctx.og_images {
        let fonts = og::Fonts::embedded();
        for post in posts {
            let url = model::url_of(&collections, "posts", &post.slug).map_err(invalid_data)?;
            let rel = model::og_image(&url);
            disk::write(dist, rel.trim_start_matches('/'), og::render(&fonts, post))?;
        }
    }

    for asset in &content.assets {
        disk::copy(dist, &asset.rel, &asset.src)?;
    }

    Ok(())
}

/// Writes every page one route expands to and answers with the canonical URL of
/// each, which is what the sitemap lists -- so it can neither miss a page nor
/// name one that was never written. A route emitted without the layout is a
/// fragment rather than a page, and is not a URL to advertise.
fn write_pages(
    route: &routes::Route,
    templates: &routes::Templates,
    site: &fill::Value,
    dist: &Path,
) -> Result<Vec<String>> {
    let urls = routes::expand(route, site)
        .map_err(invalid_data)?
        .iter()
        .map(|page| {
            let html = routes::render(route, templates, site, page).map_err(invalid_data)?;
            disk::write(dist, &page.out, html)?;
            Ok(routes::canonical(&page.out))
        })
        .collect::<Result<Vec<String>>>()?;

    Ok(match route.layout {
        true => urls,
        false => Vec::new(),
    })
}

fn invalid_data(msg: String) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, msg)
}

fn stylesheet_error(name: &str, css: &str, err: css::Error, at: usize) -> std::io::Error {
    let line = fill::line_at(css, at);
    std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        format!("stylesheet: {err} at line {line} of {name}"),
    )
}
