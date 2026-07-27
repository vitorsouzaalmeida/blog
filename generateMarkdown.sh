#!/bin/bash

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

if [ "$#" -ne 1 ]; then
  echo "Use: $0 <post title>"
  exit 1
fi

BLOG_TITLE="$1"
CURRENT_DATE=$(date +'%Y-%m-%d')

FRONT_MATTER="---
title: $BLOG_TITLE
pubDate: $CURRENT_DATE
tags:
  - tag1
# Optional frontmatter:
# subtitle: A short italic subtitle
# description: Used for RSS + social meta
# thread: some-thread-id
# threadOrder: 1
---

Content
"

FILE_NAME=$(echo "$BLOG_TITLE" | sed 's/ /-/g' | tr '[:upper:]' '[:lower:]')
FILE_PATH="$SCRIPT_DIR/content/blog/${FILE_NAME}.md"

echo -e "$FRONT_MATTER" > "$FILE_PATH"

echo "File \"$FILE_NAME\" created at \"$FILE_PATH\""
