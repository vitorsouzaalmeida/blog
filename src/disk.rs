use std::fs;
use std::io::{Error, ErrorKind, Result};
use std::path::{Path, PathBuf};

use crate::assets;
use crate::content::{self, Post};
use crate::highlight::Highlighter;
use crate::markdown;

#[derive(Debug)]
pub struct Asset {
    pub rel: PathBuf,
    pub src: PathBuf,
}

#[derive(Debug)]
pub struct Content {
    pub posts: Vec<Post>,
    pub assets: Vec<Asset>,
    pub css: String,
    pub script: String,
}

fn invalid(path: &Path, msg: impl std::fmt::Display) -> Error {
    Error::new(ErrorKind::InvalidData, format!("{}: {msg}", path.display()))
}

fn load_posts(dir: &Path, hl: &Highlighter) -> Result<Vec<Post>> {
    let mut paths: Vec<PathBuf> = fs::read_dir(dir)
        .map_err(|e| invalid(dir, format!("cannot read content directory ({e})")))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "md"))
        .collect();
    paths.sort();

    let posts = paths
        .iter()
        .map(|path| {
            let raw = fs::read_to_string(path)?;
            let slug = path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .ok_or_else(|| invalid(path, "no filename"))?;
            content::parse(&raw, &slug).map_err(|e| invalid(path, e))
        })
        .collect::<Result<Vec<Post>>>()?;

    Ok(markdown::render_posts(posts, hl))
}

fn concat(dir: &Path, names: &[&str], sep: &str) -> Result<String> {
    let parts = names
        .iter()
        .map(|name| {
            let path = dir.join(name);
            fs::read_to_string(&path).map_err(|e| invalid(&path, e))
        })
        .collect::<Result<Vec<String>>>()?;
    Ok(parts.join(sep))
}

fn is_inlined(rel: &Path) -> bool {
    let rel = rel.to_string_lossy().replace('\\', "/");
    assets::STYLESHEETS.contains(&rel.as_str()) || assets::SCRIPTS.contains(&rel.as_str())
}

fn list_assets(dir: &Path) -> Result<Vec<Asset>> {
    fn walk(dir: &Path, prefix: &Path, into: &mut Vec<Asset>) -> Result<()> {
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
            if path.is_dir() {
                walk(&path, &rel, into)?;
            } else if !is_inlined(&rel) {
                into.push(Asset { rel, src: path });
            }
        }
        Ok(())
    }

    let mut assets = Vec::new();
    if dir.is_dir() {
        walk(dir, Path::new(""), &mut assets)?;
    }
    Ok(assets)
}

pub fn load(root: &Path, hl: &Highlighter) -> Result<Content> {
    let static_dir = root.join("static");
    Ok(Content {
        posts: load_posts(&root.join("content/blog"), hl)?,
        assets: list_assets(&static_dir)?,
        css: concat(&static_dir, &assets::STYLESHEETS, "\n")?,
        script: concat(&static_dir, &assets::SCRIPTS, ";\n")?,
    })
}

pub fn clean(dist: &Path) -> Result<()> {
    match fs::remove_dir_all(dist) {
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

pub fn write(dist: &Path, rel: impl AsRef<Path>, contents: impl AsRef<[u8]>) -> Result<()> {
    fs::write(prepare(dist, rel)?, contents)
}

pub fn copy(dist: &Path, rel: impl AsRef<Path>, src: &Path) -> Result<()> {
    fs::copy(src, prepare(dist, rel)?).map(|_| ())
}

fn prepare(dist: &Path, rel: impl AsRef<Path>) -> Result<PathBuf> {
    let path = dist.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(path)
}
