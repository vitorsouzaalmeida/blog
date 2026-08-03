use chrono::{Datelike, Local};
use std::fs;
use std::io::{BufRead, BufReader, Result, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime};

use crate::Ctx;

/// Where the dev build leaves a copy of the inlined stylesheet, so a change to
/// one can be swapped into the open page instead of reloading it.
pub const CSS_PATH: &str = "/__css";

/// Injected into every page by a dev build, and into the build-failure page.
pub fn live_reload() -> String {
    format!(
        "(function(){{try{{new EventSource('/__livereload').onmessage=function(e){{\
         if(e.data==='css'){{fetch('{CSS_PATH}').then(function(r){{return r.text()}})\
         .then(function(t){{document.querySelector('style').textContent=t}})}}\
         else{{location.reload()}}}}}}catch(e){{}}}})();"
    )
}

#[derive(Debug)]
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
        let f = dist.join(rel).join("index.html");
        return if f.is_file() {
            Resolved::File(f)
        } else {
            Resolved::NotFound
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
    if html.is_file() {
        return Resolved::File(html);
    }
    Resolved::NotFound
}

pub fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "xml" => "application/xml; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "mp4" => "video/mp4",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "ttf" => "font/ttf",
        "txt" => "text/plain; charset=utf-8",
        "ico" => "image/x-icon",
        _ => "application/octet-stream",
    }
}

fn write_head(stream: &mut TcpStream, status: &str, ctype: &str, len: usize) -> Result<()> {
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {len}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n"
    );
    stream.write_all(head.as_bytes())
}

/// What the server threads share: who is listening for a reload, and why the
/// last build failed, if it did.
#[derive(Default)]
struct State {
    clients: Mutex<Vec<TcpStream>>,
    error: Mutex<Option<String>>,
}

/// Serving the error rather than printing it: otherwise the last good `dist/`
/// keeps being served and you edit against a stale page without noticing.
fn error_page(msg: &str) -> String {
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
         <title>build failed</title><style>\
         body{{margin:0;padding:40px;background:#1f1d1a;color:#e8e4da;\
         font:14px/1.6 ui-monospace,monospace}}\
         h1{{font-size:12px;letter-spacing:.14em;text-transform:uppercase;color:#c8946a}}\
         pre{{margin-top:20px;white-space:pre-wrap}}</style></head>\
         <body><h1>build failed</h1><pre>{}</pre><script>{}</script></body></html>",
        crate::fill::esc(msg),
        live_reload()
    )
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

/// The most recent modification under `dir` among the files `want` accepts.
fn newest(dir: &Path, want: &dyn Fn(&Path) -> bool) -> SystemTime {
    fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| {
            let path = e.path();
            match (path.is_dir(), want(&path)) {
                (true, _) => newest(&path, want),
                (false, true) => e
                    .metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(SystemTime::UNIX_EPOCH),
                (false, false) => SystemTime::UNIX_EPOCH,
            }
        })
        .max()
        .unwrap_or(SystemTime::UNIX_EPOCH)
}

fn latest(dirs: &[PathBuf], want: &dyn Fn(&Path) -> bool) -> SystemTime {
    dirs.iter()
        .map(|d| newest(d, want))
        .max()
        .unwrap_or(SystemTime::UNIX_EPOCH)
}

fn is_css(path: &Path) -> bool {
    path.extension().is_some_and(|e| e == "css")
}

fn broadcast(state: &State, event: &str) {
    let message = format!("data: {event}\n\n");
    state.clients.lock().unwrap().retain_mut(|s| {
        s.write_all(message.as_bytes())
            .and_then(|_| s.flush())
            .is_ok()
    });
}

/// Builds, and keeps the reason it could not; answers whether it worked.
fn rebuild(root: &Path, dist: &Path, ctx: Ctx, state: &State) -> bool {
    let result = crate::build(root, dist, ctx);
    match &result {
        Ok(()) => println!("rebuilt"),
        Err(e) => eprintln!("build failed: {e}"),
    }
    let failure = result.err().map(|e| e.to_string());
    let ok = failure.is_none();
    *state.error.lock().unwrap() = failure;
    ok
}

pub fn serve(root: &Path, dist: &Path, port: u16) -> Result<()> {
    let ctx = Ctx::dev(Local::now().year());
    let state = Arc::new(State::default());
    // A first build that fails still starts the server: the error is the page.
    rebuild(root, dist, ctx, &state);

    {
        let state = state.clone();
        let (root, dist) = (root.to_path_buf(), dist.to_path_buf());
        let dirs = [
            root.join("content"),
            root.join("static"),
            root.join("templates"),
        ];
        thread::spawn(move || {
            let mut last = latest(&dirs, &|_| true);
            loop {
                thread::sleep(Duration::from_millis(250));
                let now = latest(&dirs, &|_| true);
                if now > last {
                    last = now;
                    let was_broken = state.error.lock().unwrap().is_some();
                    let ok = rebuild(&root, &dist, ctx, &state);
                    // A stylesheet on its own is swapped into the open page,
                    // which keeps the scroll position mid-post.
                    let css_only = ok && !was_broken && latest(&dirs, &is_css) == now;
                    broadcast(&state, if css_only { "css" } else { "reload" });
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
