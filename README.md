# ff

`ff` is the "Fast File" client and server. It transfers files quickly between
computers on a network with low overhead.

FTP uses two ports which makes it inconvenient to reverse proxy. HTTP file servers
might be too bulky for certain things, and they aren't as fast as FTP. This is my
duct tape and chicken wire compromise.

```sh
# Create a file.
mkdir test
echo "hi" > test/test.txt

# Start running a server.
ffd -d localhost:8080 test

# List files available to download.
ff localhost:8080 ls

# Download a file.
ff localhost:8080 get test.txt
```

## Goals
* Minimal communication overhead
* Fast
* Maintainable
