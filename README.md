# Raw-Redis-Rust

A Redis-compatible server implementation written entirely in Rust from scratch.

## What is this?

This is an experimental Redis server that implements the RESP (REdis Serialization Protocol) and supports basic Redis commands. Built with async Rust using Tokio for high-performance concurrent connections.

## 🛠️ Technical Highlights

- **Memory Management**: Implements a "trash janitor" pattern to move deallocation to background tasks, keeping the main request path $O(1)$.
- **Concurrency**: Built on Tokio with a semaphore-backed connection pool to prevent resource exhaustion.
- **Observability**: Integrated Prometheus exporter for real-time tracking of lock contention and latency.

## Quick Start

### Build and Run

```bash
# Build the project
cargo build --release

# Run the server
cargo run --release
```

The server will start on `0.0.0.0:6379` (Redis port) and expose Prometheus metrics on `127.0.0.1:9091/metrics`.
Set it to local host for testing.

## 🔌 Compatibility

Raw-Redis-Rust implements a subset of the RESP (REdis Serialization Protocol). It is compatible with any standard Redis client (redis-cli, redis-py, etc.).

## Supported Commands

| Command | Description |
|---------|-------------|
| `PING` | Test server connectivity |
| `ECHO` | Echo back a message |
| `GET` | Retrieve a value by key |
| `SET` | Set a key-value pair (supports `EX` and `PX` for expiration) |
| `RPUSH` | Push values to a list |
| `DEL` | Delete keys (blocking) |
| `UNLINK` | Delete keys (non-blocking, uses background cleanup) |
| `FLUSHALL` | Clear all keys |

## Project Structure

```
src/
├── main.rs          # Server entry point & connection handling
├── lib.rs           # Module exports
├── api.rs           # Prometheus metrics API server
├── db.rs            # Core database with expiration & cleanup
├── frame.rs         # RESP protocol frame encoding/decoding
├── parse.rs         # RESP protocol parser
├── connection.rs    # TCP connection wrapper
├── types.rs         # Data types (RedisData, CacheValue)
├── metrics.rs       # Prometheus metrics definitions
├── errors.rs        # Error types
└── cmd/             # Command implementations
    ├── mod.rs
    ├── ping.rs
    ├── echo.rs
    ├── get.rs
    ├── set.rs
    ├── rpush.rs
    ├── del.rs
    ├── unlink.rs
    └── flushall.rs
```

## Notes

This is a work-in-progress experimental project. I update this periodically as experiments succeed.

---

Architecture and logic designed by a human, with every line of code hand-written with 🦀
