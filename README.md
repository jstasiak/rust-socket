socket
======

> Low-level networking interface for Rust modeled after Python's socket module

Homepage and repository: https://github.com/jstasiak/rust-socket
Documentation: http://www.rustdox.com/github.com/jstasiak/rust-socket.git/socket/

## Example

```rust
use socket::{AF_INET, SO_REUSEADDR, SOCK_DGRAM, Socket, SOL_SOCKET};

let socket = Socket::new(AF_INET, SOCK_DGRAM, 0).unwrap();
socket.setsockopt(SOL_SOCKET, SO_REUSEADDR, 1).unwrap();
socket.bind("0.0.0.0:5353").unwrap();
```

## Status

Experimental

## License

MIT
