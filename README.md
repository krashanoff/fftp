# ff

`ff` is the "Fast File" client and server. It transfers files quickly between computers
on a network with low overhead.

## Why?

You can't easily access your server's FTP from behind an SSH reverse proxy. HTTP file servers
might be too much, and aren't as fast as FTP. This is my duct tape and chicken wire compromise.

```sh
# Forward port 30 of our server over SSH.
ssh -L 8080:127.0.0.1:8080 -N mycomputer.net

# List files available to download.
ff ls localhost:8080

# Download a file.
ff get localhost:8080 Cargo.toml
```

## Goals
* Minimal overhead
* Fast
* Maintainable

## Non-goals
* Security
