Static site built with HTMX and Rust.

I'm using Rust as a static site generator to turns Markdown into HTML, and HTMX to add interactivity. This project does not use Node, or a bundler, or a framework on purpose.

## Development

```bash
cargo run --release -- build
cargo run -- dev
cargo test
cargo fmt
./generateMarkdown.sh "Title"
```

## License

Code is under [LICENSE](LICENSE); the posts in `content/` are under [LICENSE-posts](LICENSE-posts).
