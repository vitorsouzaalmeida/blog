My blog!

No Node, bundler, or framework because I don't want to run JavaScript on my computer, and you shouldn't either.

```bash
cargo run --release -- build      # -> dist/
cargo run -- dev                  # localhost:8788, live reload, drafts included
cargo run -- new "Post title"     # scaffolds content/blog/post-title.md, opens $EDITOR
cargo run --release -- check      # builds, then verifies links and class names
```

Code is under [LICENSE](LICENSE); the posts in `content/` are under [LICENSE-posts](LICENSE-posts).
