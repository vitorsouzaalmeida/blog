---
title: How I isolated my work environment
pubDate: 2026-06-03
tags:
  - code
---

### A separate Linux user

I created a new Linux user called `work` for development, and I run it from my main user. The `work` user must **not** have sudo. It is a normal user with no privileges, and that is the whole point. Then I locked down my own home directory and some other paths:

```bash
sudo chmod 700 /home/vitor
```

From now on `work` can not read anything inside `/home/vitor`: SSH keys, browser data, GPG keyring, etc. So a malicious package running as `work` can not reach my real
account.

### A proper session

`sudo -iu work` drops you into the work shell, but it does not create a real systemd user session. `XDG_RUNTIME_DIR` is unset, dbus is unavailable, and tools like Podman start to complain, solved by running:

```bash
sudo loginctl enable-linger work
sudo machinectl shell work@ /usr/bin/zsh
```

`enable-linger` makes systemd start the work user manager at boot, and
`machinectl shell` gives you a fully-sessioned login. After that, `systemctl --user`
works, `XDG_RUNTIME_DIR` is set, and the warnings go away.

I aliased it in my main shell BTW:

```bash
alias work='sudo machinectl shell work@ /usr/bin/zsh'
```

So now I just type `work` and I am in.

### The shell

I copied my zsh config from `vitor` to `work` and cleaned it up, so nothing special.

### pnpm/npm

I installed pnpm directly on the work user, not inside containers. I did consider Podman but for my workflow the friction was not worth it. Currently PNPM and NPM already support a bunch of mitigations against supply-chain attacks: [Mitigating supply chain attacks](https://pnpm.io/supply-chain-security), so take a read.

### Mitigations without containers

I allowlist only the packages whose install scripts I actually need, through `pnpm.onlyBuiltDependencies` in `package.json`. Most projects need very few, and each one becomes a deliberate trust decision instead of an implicit one. Pretty obvious, but avoid putting production keys on the work user. I thought about using some kind of HTTP Proxy to avoid having any secrets on the work user at all, but I delayed that for now. About dev server, since I'm running the work user in the same host, I can just access localhost from my main user, so I don't need to share any session or cookie with the work user.

### My development workflow

I'm using Zed with Remote Development (SSH). I'm no longer using vscode, but if you do, there's an official extension by Microsoft to deal with it: [Visual Studio Code Remote - SSH](https://code.visualstudio.com/docs/remote/ssh) and if you're using something else, there might be similar tools available. I also created a dedicated Github SSH key for this work user, and I'm no longer using the one in my main user. Regardless AI stuff, I'm a Claude Code user, and I have it installed in the work user as well. Since it doesn't have root access, I'm always running Claude with the bypass permission enabled.

Docker works as expected, but worth mentioning that it is usually a long-running background service called `dockerd` that runs as root. Everything is a proxy to send something to this daemon, that receives the message and does the work, and since the daemon is root, it ends up being a child of a root process. So, you want to avoid adding the work user to the docker group. I will not deep dive into this, but you should probably use docker as a rootless user, and if you need Docker socket, it is possible to spawn a docker compatible socket with podman by running `podman system service` (or just use podman to replace docker, since it is compatible with the same CLI).
