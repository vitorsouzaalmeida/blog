---
title: An Obsidian vault as internal docs for my isolated environment
pubDate: 2026-06-24
tags:
  - code
thread: isolated-env
draft: false
---

I've been using the SSH environment described in the previous article for a while now, but only recently something came up that required me to change my initial setup a bit. I needed to build an internal Obsidian vault for documenting a project I work on, which means granting access to the project files, at least in part.

I researched and found some solutions to keep using SSH with Obsidian, but I didn't like them much, and they weren't ergonomic, so I ended up granting my user `rw` access to the /docs directory. Since vanilla Obsidian is fairly inert with Markdown and we won't add any third-party plugins, it's safe enough.

Linux resolves paths in a loop, as described on its [path_resolution(7)](https://man7.org/linux/man-pages/man7/path_resolution.7.html) page. This loop checks each folder along the way and returns an error if the process doesn't have search permission. To deal with it, I granted `--x` permission to my user on each directory, so it executes/searches without read access, which lets me reach _docs/_ without being able to list (`ls`) my home, my work directory, or the rest of the repository.

```bash
setfacl -m u:vitor:--x /home/work
setfacl -m u:vitor:--x /home/work/...
setfacl -m u:vitor:--x /home/work/.../repository

setfacl -R -m u:vitor:rwX /home/work/.../repository/docs

# Defaults so new files inherit access for both users, whoever creates them
setfacl -R -d -m u:vitor:rwX /home/work/.../repository/docs
setfacl -R -d -m u:work:rwX  /home/work/.../repository/docs
```

And that's it!
