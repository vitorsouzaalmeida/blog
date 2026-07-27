```bash
cargo run --release -- build   # build the project and output it to /dist
cargo run -- dev               # build + serve dist/ with live-reload (default port 8788; PORT to override)
cargo test                     # unit tests (see Tests below — the suite is deliberately small)
cargo fmt                      # format
./generateMarkdown.sh "Title"  # scaffold a new post in content/blog/
```

## Architecture

The crate is a library (`src/lib.rs`) with a thin `ssg` binary on top. A build
is two steps:

```
disk::load  ->  build() renders each page and writes it
   (I/O)        (pure renderers + disk::write / disk::copy)
```

`build()` walks posts, tags, threads, OG images and static assets in that order,
writing each file as it is rendered. There is no intermediate description of
`dist/`.

**Only `disk.rs`, `dev.rs` and `main.rs` may name `std::fs`, `std::env` or
`SystemTime::now`.** Every renderer is a function of its arguments, which keeps
the output byte-for-byte reproducible. `Ctx { year, live_reload, og_images }`
carries the values that used to be read from a `BLOG_DEV` env var and the
system clock.

## Modules (`src/`)

- `lib.rs` — module list + `build(root, dist, ctx)`: renders and writes all of
  `dist/`.
- `config.rs` — site constants and `Ctx`.
- `clock.rs` — `current_year()`, over a hand-rolled Gregorian year walk.
- `content.rs` — pure parser: frontmatter + dates (no yaml/date crate).
  `Post`, `Date`, `tag_counts`. Returns `Err` on a malformed post rather than
  publishing a blank page.
- `markdown.rs` — `pulldown-cmark` -> HTML. Math (`$…$`, `$$…$$`) -> MathML via
  `latex2mathml`; fenced code -> `highlight.rs`. Raw HTML passes through.
- `highlight.rs` — `syntect` (pure-Rust regex) class-based highlighting +
  `highlight_css()` generating light + `[data-theme="dark"]` rules.
- `render.rs` — HTML as `String` builders (layout, components, pages). Same class
  names as `static/styles.css`. `tag_path`/`tag_href` are the _only_ places a tag
  becomes a URL segment — the link, the htmx endpoint, the canonical URL, the
  sitemap entry and the output directory must all agree.
- `og.rs` — OG PNGs drawn with `image` + `ab_glyph` (fonts embedded via
  `include_bytes!` from `assets/og-fonts/`). 1200×630. `Fonts` is built once per
  build. Note: Merriweather has no glyph for `β`/`α`, so those render as notdef
  boxes on the card.
- `threads.rs` — post "threads" (series); pure functions over `&[Post]`.
  `THREADS` is a static slice.
- `disk.rs` — the only filesystem module: `load` (posts + sorted assets),
  `clean`, `write`, `copy`. `write`/`copy` create parent directories.
- `dev.rs` — std-only dev server: static file serving (dir index, extensionless
  `.html`, trailing-slash 301), SSE `/__livereload`, mtime-poll watcher.
  `resolve()` mirrors Cloudflare's asset resolution.

There is no `draft` concept: every post in `content/blog/` is published.

## Tests

This is a personal blog with one consumer, so the bar for a test is high. The
suite is intentionally ~8 tests, all beside the code they cover:

- `content.rs` — date rejection, frontmatter parsing, tag counting
- `feeds.rs` — URL absolutization and CDATA splitting
- `render.rs` — HTML escaping, URL encoding, `tag_href`/`tag_path` agreement

That list is the whole policy: test **hand-written string surgery with no
library behind it**, where a silent bug ships a broken feed or a 404 link.

Do **not** add tests that re-test `pulldown-cmark`, `syntect`, `image`,
`latex2mathml` or the standard library, that assert on produced site output, or
that require production code to grow accessors, wrappers or fixtures to be
reachable. If verifying a change means looking at the built site, build it and
look at it — do not encode it as an assertion.

## Code

- Functional style: pure, side-effect free, no mutable state
- Prefer borrowing over ownership
- Avoid unnecessary allocations; prefer `&str` over `String`
