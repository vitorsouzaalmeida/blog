pub mod clock;
pub mod config;
pub mod content;
pub mod dev;
pub mod disk;
pub mod feeds;
pub mod highlight;
pub mod markdown;
pub mod og;
pub mod render;
pub mod threads;

use std::io::Result;
use std::path::Path;

use config::Ctx;
use content::Post;
use highlight::Highlighter;
use threads::THREADS;

pub fn build(root: &Path, dist: &Path, ctx: Ctx) -> Result<()> {
    let content = disk::load(root, &Highlighter::new())?;
    let posts = &content.posts;
    let all: Vec<&Post> = posts.iter().collect();
    let tm = threads::thread_map(posts);
    let tag_counts = content::tag_counts(posts);
    let tags: Vec<&str> = tag_counts.iter().map(|(t, _)| *t).collect();
    let thread_ids: Vec<&str> = THREADS.iter().map(|t| t.id).collect();

    disk::clean(dist)?;

    disk::write(dist, "index.html", render::home_page(ctx, posts, &tm))?;
    disk::write(
        dist,
        "blog/index.html",
        render::blog_index_page(ctx, posts, &tm),
    )?;
    disk::write(dist, "tags/index.html", render::tags_page(ctx, &tag_counts))?;
    disk::write(dist, "highlight.css", highlight::highlight_css())?;
    disk::write(dist, "rss.xml", feeds::rss(&all))?;
    disk::write(
        dist,
        "sitemap.xml",
        feeds::sitemap(&all, &tags, &thread_ids),
    )?;

    for post in posts {
        let nav = threads::thread_nav(post, posts);
        disk::write(
            dist,
            format!("blog/{}/index.html", post.slug),
            render::post_page(ctx, post, nav.as_ref()),
        )?;
    }

    for tag in &tags {
        let tagged: Vec<&Post> = posts.iter().filter(|p| has_tag(p, tag)).collect();
        let dir = render::tag_path(tag);
        disk::write(
            dist,
            format!("tag/{dir}/index.html"),
            render::tag_page(ctx, tag, &tagged, &tm),
        )?;
        disk::write(
            dist,
            format!("tag/{dir}/partial.html"),
            render::tag_partial(&tagged, &tm),
        )?;
    }

    for thread in THREADS {
        let parts = threads::thread_parts(posts, thread.id);
        if !parts.is_empty() {
            disk::write(
                dist,
                format!("thread/{}/index.html", thread.id),
                render::thread_page(ctx, thread, &parts),
            )?;
        }
    }

    if ctx.og_images {
        let fonts = og::Fonts::embedded();
        for post in posts {
            disk::write(
                dist,
                format!("blog/{}/og.png", post.slug),
                og::render(&fonts, post),
            )?;
        }
    }

    for asset in &content.assets {
        disk::copy(dist, &asset.rel, &asset.src)?;
    }

    Ok(())
}

fn has_tag(post: &Post, tag: &str) -> bool {
    post.tags.iter().any(|t| t == tag)
}
