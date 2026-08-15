use std::fs;
use std::io::{BufRead, BufReader, Result, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime};

use chrono::{Datelike, Local};

use crate::render::{esc, fill};
use crate::Ctx;

const ERROR: &str = include_str!("../templates/error.html");

pub fn live_reload() -> String {
    "(function(){try{new EventSource('/__livereload').onmessage=function(){location.reload()}}catch(e){}})();"
        .to_string()
}

pub enum Resolved {
    File(PathBuf),
    Redirect(String),
    NotFound,
}

pub fn resolve(dist: &Path, url_path: &str) -> Resolved {
    let path = url_path.split('?').next().unwrap_or("/");
    if path.contains("..") || path.contains('\\') {
        return Resolved::NotFound;
    }
    let rel = path.trim_start_matches('/');

    if path.ends_with('/') {
        let index = dist.join(rel).join("index.html");
        return match index.is_file() {
            true => Resolved::File(index),
            false => Resolved::NotFound,
        };
    }

    let direct = dist.join(rel);
    if direct.is_file() {
        return Resolved::File(direct);
    }
    if direct.is_dir() {
        return Resolved::Redirect(format!("{path}/"));
    }

    let html = dist.join(format!("{rel}.html"));
    match html.is_file() {
        true => Resolved::File(html),
        false => Resolved::NotFound,
    }
}

pub fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "xml" => "application/xml; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "mp4" => "video/mp4",
        "txt" => "text/plain; charset=utf-8",
        "ico" => "image/x-icon",
        _ => "application/octet-stream",
    }
}

#[derive(Default)]
struct State {
    clients: Mutex<Vec<TcpStream>>,
    error: Mutex<Option<String>>,
}

fn error_page(msg: &str) -> String {
    fill(ERROR, &[("reload", &live_reload()), ("message", &esc(msg))])
}

fn write_head(stream: &mut TcpStream, status: &str, ctype: &str, len: usize) -> Result<()> {
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {len}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n"
    );
    stream.write_all(head.as_bytes())
}

fn handle(mut stream: TcpStream, dist: &Path, state: &State) -> Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let path = line.split_whitespace().nth(1).unwrap_or("/").to_string();

    if path == "/__livereload" {
        stream.write_all(
            b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: keep-alive\r\n\r\nretry: 1000\n\n",
        )?;
        stream.flush()?;
        state.clients.lock().unwrap().push(stream);
        return Ok(());
    }

    if let Some(err) = state.error.lock().unwrap().clone() {
        let body = error_page(&err);
        write_head(
            &mut stream,
            "500 Internal Server Error",
            "text/html; charset=utf-8",
            body.len(),
        )?;
        stream.write_all(body.as_bytes())?;
        return stream.flush();
    }

    match resolve(dist, &path) {
        Resolved::File(f) => {
            let body = fs::read(&f).unwrap_or_default();
            write_head(&mut stream, "200 OK", content_type(&f), body.len())?;
            stream.write_all(&body)?;
        }
        Resolved::Redirect(loc) => {
            let resp = format!(
                "HTTP/1.1 301 Moved Permanently\r\nLocation: {loc}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            );
            stream.write_all(resp.as_bytes())?;
        }
        Resolved::NotFound => {
            let body = b"404 Not Found";
            write_head(
                &mut stream,
                "404 Not Found",
                "text/plain; charset=utf-8",
                body.len(),
            )?;
            stream.write_all(body)?;
        }
    }
    stream.flush()
}

fn newest(dir: &Path) -> SystemTime {
    fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| {
            let path = entry.path();
            match path.is_dir() {
                true => newest(&path),
                false => entry
                    .metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(SystemTime::UNIX_EPOCH),
            }
        })
        .max()
        .unwrap_or(SystemTime::UNIX_EPOCH)
}

fn latest(dirs: &[PathBuf]) -> SystemTime {
    dirs.iter()
        .map(|d| newest(d))
        .max()
        .unwrap_or(SystemTime::UNIX_EPOCH)
}

fn broadcast(state: &State) {
    state.clients.lock().unwrap().retain_mut(|s| {
        s.write_all(b"data: reload\n\n")
            .and_then(|_| s.flush())
            .is_ok()
    });
}

fn rebuild(root: &Path, dist: &Path, ctx: Ctx, state: &State) {
    let result = crate::build(root, dist, ctx);
    match &result {
        Ok(()) => println!("rebuilt"),
        Err(e) => eprintln!("build failed: {e}"),
    }
    *state.error.lock().unwrap() = result.err().map(|e| e.to_string());
}

pub fn serve(root: &Path, dist: &Path, port: u16) -> Result<()> {
    let ctx = Ctx::dev(Local::now().year());
    let state = Arc::new(State::default());
    rebuild(root, dist, ctx, &state);

    {
        let state = state.clone();
        let (root, dist) = (root.to_path_buf(), dist.to_path_buf());
        let dirs = [root.join("content"), root.join("static")];
        thread::spawn(move || {
            let mut last = latest(&dirs);
            loop {
                thread::sleep(Duration::from_millis(250));
                let now = latest(&dirs);
                if now > last {
                    last = now;
                    rebuild(&root, &dist, ctx, &state);
                    broadcast(&state);
                }
            }
        });
    }

    let listener = TcpListener::bind(("127.0.0.1", port))?;
    println!("dev server on http://localhost:{port} (watching for changes)");
    for stream in listener.incoming().flatten() {
        let dist = dist.to_path_buf();
        let state = state.clone();
        thread::spawn(move || {
            let _ = handle(stream, &dist, &state);
        });
    }
    Ok(())
}
