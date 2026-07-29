A static site generator that turns Markdown into HTML, and HTMX for interactivity

No Node, bundler, or framework because I don't want to run JavaScript on my computer, and you shouldn't either. I'm using a few crates to do the building and wrote a half-baked HTML and CSS parser, as well as an XML writer to handle only this project's needs. They're all based on its specifications.

Development:

```bash
cargo run --release -- build
cargo run -- dev
cargo test
```

Code is under [LICENSE](LICENSE); the posts in `content/` are under [LICENSE-posts](LICENSE-posts).
