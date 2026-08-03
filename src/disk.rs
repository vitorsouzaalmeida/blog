use std::fs;
use std::io::{Error, ErrorKind, Result};
use std::path::{Path, PathBuf};

use crate::content::{self, Post};

/// Concatenated and inlined into every page, in cascade order.
pub const STYLESHEETS: [&str; 3] = ["fonts/fonts.css", "styles.css", "highlight.css"];

#[derive(Debug)]
pub struct Asset {
    pub rel: PathBuf,
    pub src: PathBuf,
}

#[derive(Debug)]
pub struct Content {
    pub posts: Vec<Post>,
    pub assets: Vec<Asset>,
    /// Kept per file so a syntax error can name the file it is in.
    pub stylesheets: Vec<(&'static str, String)>,
    pub templates: Vec<(String, String)>,
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

fn read_all(dir: &Path, names: &[&'static str]) -> Result<Vec<(&'static str, String)>> {
    names
        .iter()
        .map(|name| {
            let path = dir.join(name);
            fs::read_to_string(&path)
                .map(|source| (*name, source))
                .map_err(|e| invalid(&path, e))
        })
        .collect()
}

/// Stylesheets are inlined into every page, so they are not also copied out.
fn is_inlined(rel: &Path) -> bool {
    STYLESHEETS.contains(&slashed(rel).as_str())
}

/// Every file under `dir`, as `(path relative to dir, path on disk)`, sorted so
/// the build is deterministic.
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
            if path.is_dir() {
                walk(&path, &rel, into)?;
            } else {
                into.push((rel, path));
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

fn list_assets(dir: &Path) -> Result<Vec<Asset>> {
    Ok(list_files(dir)?
        .into_iter()
        .filter(|(rel, _)| !is_inlined(rel))
        .map(|(rel, src)| Asset { rel, src })
        .collect())
}

fn slashed(rel: &Path) -> String {
    rel.to_string_lossy().replace('\\', "/")
}

/// Route templates, as `(path relative to templates/, source)`. A template's
/// final newline is not part of its output -- it is an artifact of the file
/// ending in one -- so it is stripped here rather than in every template.
fn load_templates(dir: &Path) -> Result<Vec<(String, String)>> {
    list_files(dir)?
        .into_iter()
        .filter(|(rel, _)| rel.extension().is_some_and(|e| e == "html"))
        .map(|(rel, src)| {
            let source = fs::read_to_string(&src).map_err(|e| invalid(&src, e))?;
            let trimmed = source.strip_suffix('\n').unwrap_or(&source).to_string();
            Ok((slashed(&rel), trimmed))
        })
        .collect()
}

pub fn load(root: &Path) -> Result<Content> {
    let static_dir = root.join("static");
    Ok(Content {
        posts: load_posts(&root.join("content/blog"))?,
        assets: list_assets(&static_dir)?,
        stylesheets: read_all(&static_dir, &STYLESHEETS)?,
        templates: load_templates(&root.join("templates"))?,
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
