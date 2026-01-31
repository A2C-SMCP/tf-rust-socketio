# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

A Rust Socket.IO/Engine.IO client library organized as a two-crate workspace:
- **engineio** - Low-level Engine.IO protocol implementation
- **socketio** - High-level Socket.IO client built on engineio

This is a fork of rust-socketio with A2C-SMCP protocol enhancements including server-to-client ACK support and reconnect header updates.

## Build Commands

```bash
# Build entire workspace
cargo build --all-features

# Quick tests (packet parsing, no Docker needed)
make test-fast

# Full test suite (requires Docker)
make keys                    # Generate TLS certificates (first time only)
make run-test-servers        # Start test servers in Docker
make test-all                # Run all tests
docker stop socketio_test    # Stop test servers when done

# Code quality
make clippy                  # Linting
make format                  # Format check
make checks                  # Build + test-fast + clippy + format
make pipeline                # Full CI (build + test-all + clippy + format)
```

## Running Individual Tests

```bash
# Run a specific test
cargo test --package tf-rust-socketio test_name

# Run tests with output
cargo test --package tf-rust-socketio -- --nocapture

# Run async tests
cargo test --package tf-rust-socketio --features async
```

## Architecture

### Dual Sync/Async Design
Each crate has parallel sync and async modules. The async version uses Tokio runtime and is enabled via the `async` feature flag (default on).

### Transport Layer (engineio)
- Auto-upgrades from HTTP long-polling to WebSocket when possible
- Transports: `polling.rs`, `websocket.rs`, `websocket_secure.rs`
- Async variants in `asynchronous/async_transports/`

### Client Layer (socketio)
- **ClientBuilder** (`client/builder.rs`) - Fluent configuration with `.on()`, `.namespace()`, `.auth()`, `.tls_config()`
- **RawClient** (`client/raw_client.rs`) - Core API: `emit()`, `emit_with_ack()`, `ack()`, `disconnect()`
- Event callbacks registered via closures, stored in `Arc<Mutex<HashMap<Event, Callback>>>`

### Packet Protocol
- **Engine.IO** (`engineio/src/packet.rs`): Open, Close, Ping, Pong, Message, MessageBinary, Upgrade, Noop
- **Socket.IO** (`socketio/src/packet.rs`): Connect, Disconnect, Event, Ack, ConnectError, BinaryEvent, BinaryAck

### Fork-Specific Enhancements
- **Server-to-Client ACK**: `Payload` includes optional `ack_id`, client has `ack()` and `ack_with_id()` methods
- **Reconnect Headers**: `ReconnectSettings::opening_header()` for token refresh during reconnection
- **CloseReason enum**: `IOServerDisconnect`, `IOClientDisconnect`, `TransportClose`

## Test Infrastructure

Integration tests require Docker test servers (Node.js Socket.IO/Engine.IO):
- Port 4200: Socket.IO default namespace
- Port 4201: Engine.IO polling
- Port 4202: Engine.IO secure (HTTPS)
- Port 4203: Engine.IO polling-only (no upgrade)
- Port 4204: Socket.IO with auth
- Port 4205-4206: Socket.IO with restart capability

Test servers configured in `ci/` directory with `Dockerfile` and JavaScript server implementations.

## Key Files

- `socketio/src/client/raw_client.rs` - Main client implementation
- `socketio/src/packet.rs` - Socket.IO protocol parsing
- `socketio/src/payload.rs` - Payload types with ACK ID support
- `engineio/src/client/client.rs` - Transport-level client
- `engineio/src/transports/` - Transport implementations
