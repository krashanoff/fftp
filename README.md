# fftp

`fftp` is the "Fast File Transport Protocol". It transfers files quickly between
computers on a network with low overhead.

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
ffd 127.0.0.1 8080 test &

# List files available.
ff 127.0.0.1 8080 ls

# Download a file.
ff MY_LOCAL_IP:8080 get test.txt
```

## Goals
* Minimal communication overhead
* Fast
* Maintainable

## Protocol Details

It's fast and it's simple.
* Nonblocking
* Stateless
* Insecure

### V0

V0 was implemented in Rust and used `bincode` for sending data.

### V1

V1 transitioned the program to C. It simplified the protocol at expense of implementation complexity.

Request:
* 2 byte length of the packet
* 1 byte request/response packet
  * Higher 4 bits are used for request/response type
  * Lower 4 bits are used for request/response tagging
* 2 byte requested buffer size
  * Server will send packets of maximum size `min(server_buffer_size, requested_buffer_size)`.
* Args (maximum length of `255 * 4` bytes)

Response:
* 2 byte length of the packet
* 1 byte request/response packet
  * Higher 4 bits are used for request/response type
  * Lower 4 bits are used for request/response tagging
* Variable-length data
