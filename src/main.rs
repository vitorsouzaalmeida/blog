use std::path::PathBuf;
use std::process::ExitCode;

use blog::{clock, config::Ctx, dev};

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
    let year = clock::current_year();

    let result = match std::env::args().nth(1).as_deref() {
        Some("dev") => dev::serve(&root, &dist, port()),
        _ => blog::build(&root, &dist, Ctx::prod(year)).map(|()| println!("built dist/")),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
