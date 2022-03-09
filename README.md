[![Build Status](https://travis-ci.org/earthengine/rocks.svg?branch=master)](https://travis-ci.org/earthengine/rocks)

# Rocks

A tunneling proxy based on SOCKS5 and Websocket, in Rust

# Features (planned)

- Can run as a local SOCKS5 proxy, directally connect to Internet
- Can run as a remote proxy, use Rocks protocol and directally connect to Internet
- Can run as a local SOCKS5 proxy, and use Rocks protocol to connect to the remote proxy

# The Rocks protocol (Draft)

- HTTPS+WebSocket protocol based
- No custom encryption algorithms
- Allow to configure a "login" page for random HTTPS clients, where the login always fail
- Use standard JWT authentication

# Roadmap

| Feature                             | Status       |
| ----------------------------------- | ------------ |
| Run as local SOCKS5 client (direct) | Ready to use |
| Run as remote Rocks server          | Planning     |
| Run as local SOCKS client (rocks)   | Not started  |

# Requirement

For windows, download [rustup-init.exe](https://win.rustup.rs/x86_64).

For Linux or MacOS, run the following command in a console:

```
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

For more information, check [rustup.rs](https://rustup.rs/)

# Build & run

In a console, enter the working folder and run the following command to build:

```
cargo build
```

Then you can run rocks with

```
cargo run
```

If you want to see more logging/debugging information, in Linux/MacOS run

```
RUST_LOG=debug cargo run
```

(set the value to `info` outputs less information)

In windows you need to

```
$env:RUST_LOG="info"
cargo run
```

(Note: all subsequence `cargo run` will automatically output the log)

# Docker

To make testing easier, a simple Dockerfile was created to test the code in a container.

To build the docker image run

```
docker build -t rocks:latest .
```

To run a docker container run

```
docker run -p 8443:8443 rocks:latest
```

# Contribute

Please let me know your thoughts or advises. You can post them in the issues. Thank you very much!
