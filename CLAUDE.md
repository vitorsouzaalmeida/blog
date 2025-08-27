# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a personal portfolio/blog website built with Astro, deployed on Vercel. The site features blog posts, a photo gallery, and a reading list.

## Development Commands

```bash
# Install dependencies
npm install

# Start development server
npm run dev

# Build for production
npm run build

# Preview production build
npm run preview

# Create a new blog post (interactive script)
npm run write
```

## Architecture

### Tech Stack
- **Framework**: Astro with MDX support
- **Deployment**: Vercel (static site)
- **Styling**: CSS with Astro scoped styles
- **Content**: Markdown/MDX files in `src/content/blog/`
- **Images**: Photos in `src/pics/`, public assets in `public/`

### Key Directories
- `src/content/blog/`: Blog posts in Markdown
- `src/pages/`: Astro pages with file-based routing
  - `/blog/[slug]/`: Dynamic blog post routes
  - `/reads/`: Reading list with hardcoded entries
  - `/pics/`: Photo gallery
- `src/components/`: Reusable Astro components
- `src/layout/`: Layout wrapper components

### Content Management
- Blog posts use frontmatter with `title`, `pubDate`, and `tags`
- Reading list is managed directly in `src/pages/reads/index.astro` as a typed array
- RSS feed is auto-generated at `/rss.xml`
- Blog slugs redirect from root (e.g., `/post-name` → `/blog/post-name`)

### Special Features
- Math rendering with KaTeX (remark-math + rehype-katex)
- Code syntax highlighting with rehype-pretty-code (GitHub Light theme)
- Open Graph images generated for blog posts (`index.png.ts`)
- Tag-based filtering at `/tags` and `/tag/[tag]`

## Important Notes
- No linting or testing scripts configured - verify code manually
- Uses TypeScript with strict config extending Astro's defaults
- Post creation script (`generateMarkdown.sh`) creates posts with placeholder tags
- Site is configured for simplicity and speed - avoid adding complex features or heavy dependencies