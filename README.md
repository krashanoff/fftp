# fftp

`fftp` is the "Fast File Transport Protocol". It transfers files quickly between
computers on a network with as little overhead as possible.

## Motivation

FTP uses two ports which makes it inconvenient to reverse proxy. HTTP file servers
might be too bulky for certain things, and they aren't as fast as FTP. Both use TCP.
This is my duct tape and chicken wire compromise.

## Use

The client is `ff`. The server is `ffd`.

```sh
# Create a file.
mkdir test
echo "hi" > test/test.txt

# Start running a server.
ffd MY_LOCAL_IP:8080 test &

# List files available.
ff MY_LOCAL_IP:8080 ls

# Download a file.
ff MY_LOCAL_IP:8080 get test.txt
```

## Goals
* Minimal communication overhead
* Fast
* Maintainable

## Proto

There is only one version of the FFTP application-layer protocol.

* First byte is metadata for the packet.
* Second byte of the packet is data.
* Last few bytes are a checksum of the entire packet (with the checksum zeroed).
