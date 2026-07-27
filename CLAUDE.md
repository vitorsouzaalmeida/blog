```bash
cargo run --release -- build   # build the project and output it to /dist
cargo run -- dev               # build + serve dist/ with live-reload (default port 8788; PORT to override)
cargo test                     # unit tests (see Tests below — the suite is deliberately small)
cargo fmt                      # format
./generateMarkdown.sh "Title"  # scaffold a new post in content/blog/
```

Code:

- Functional style: pure, side-effect free, no mutable state
- Prefer borrowing over ownership
- Avoid unnecessary allocations; prefer `&str` over `String`
