# tf-rust-socketio

> **This is a fork of [rust-socketio](https://github.com/1c3t3a/rust-socketio) with additional features for the [A2C-SMCP](https://github.com/A2C-SMCP) protocol.**

## Acknowledgments

This project is based on the excellent work of [Bastian Kersting](https://github.com/1c3t3a) and the rust-socketio community. We are grateful for their contributions to the Rust ecosystem.

We plan to contribute these enhancements back to the upstream project via PR when time permits. For now, this fork is maintained for internal use in the A2C-SMCP project.

## Enhancements over upstream

This fork includes the following enhancements:

### 1. Server-to-Client ACK Support (emitWithAck)

The official `rust_socketio` does not support responding to server's `emitWithAck` calls. This fork adds:

- `Payload` enum now includes an optional `ack_id: Option<i32>` field
- New `ack()` and `ack_with_id()` methods on both sync and async clients
- New `Packet::ack_from_payload()` method for constructing ACK response packets

```rust
// Example: Responding to server's emitWithAck
socket.on("foo", |payload, socket| {
    async move {
        if let Payload::Text(values, ack_id) = payload {
            if let Some(id) = ack_id {
                socket.ack_with_id(id, json!({"status": "ok"})).await;
            }
        }
    }.boxed()
});
```

### 2. Reconnect Header Updates

Added ability to update HTTP headers during reconnection (useful for token refresh scenarios):

- `ReconnectSettings` now has `opening_header()` method
- `ClientBuilder.opening_headers` visibility changed to `pub(crate)`

```rust
// Example: Update auth header on reconnect
settings.opening_header("Authorization", "Bearer new_token");
```

### 3. Close Reason Enhancement

The `Event::Close` payload now includes the disconnect reason:

- New `CloseReason` enum with variants:
  - `IOServerDisconnect` - Server initiated disconnect
  - `IOClientDisconnect` - Client initiated disconnect
  - `TransportClose` - Transport layer closed

This aligns with the official Socket.IO client disconnect reasons.

### 4. Minor Fixes

- Added lifetime annotation `Iter<'_>` in `engineio/client.rs` (compiler warning fix)
- Additional test coverage for concurrent ACK scenarios

---

## Original README

[![Latest Version](https://img.shields.io/crates/v/tf_rust_socketio)](https://crates.io/crates/tf_rust_socketio)
[![docs.rs](https://docs.rs/tf_rust_socketio/badge.svg)](https://docs.rs/tf_rust_socketio)
[![Build and code style](https://github.com/1c3t3a/rust-socketio/actions/workflows/build.yml/badge.svg)](https://github.com/1c3t3a/rust-socketio/actions/workflows/build.yml)
[![Test](https://github.com/1c3t3a/rust-socketio/actions/workflows/test.yml/badge.svg)](https://github.com/1c3t3a/rust-socketio/actions/workflows/test.yml)
[![codecov](https://codecov.io/gh/1c3t3a/rust-socketio/branch/main/graph/badge.svg?token=GUF406K0KL)](https://codecov.io/gh/1c3t3a/rust-socketio)

# Rust-socketio-client

An implementation of a socket.io client written in the rust programming language. This implementation currently supports revision 5 of the socket.io protocol and therefore revision 4 of the engine.io protocol. If you have any connection issues with this client, make sure the server uses at least revision 4 of the engine.io protocol.
Information on the [`async`](#async) version can be found below.

## Example usage

Add the following to your `Cargo.toml` file:

```toml
tf_rust_socketio = "*"
```

Then you're able to run the following example code:

``` rust
use tf_rust_socketio::{ClientBuilder, Payload, RawClient};
use serde_json::json;
use std::time::Duration;

// define a callback which is called when a payload is received
// this callback gets the payload as well as an instance of the
// socket to communicate with the server
let callback = |payload: Payload, socket: RawClient| {
       match payload {
           Payload::String(str) => println!("Received: {}", str),
           Payload::Binary(bin_data) => println!("Received bytes: {:#?}", bin_data),
       }
       socket.emit("test", json!({"got ack": true})).expect("Server unreachable")
};

// get a socket that is connected to the admin namespace
let socket = ClientBuilder::new("http://localhost:4200")
     .namespace("/admin")
     .on("test", callback)
     .on("error", |err, _| eprintln!("Error: {:#?}", err))
     .connect()
     .expect("Connection failed");

// emit to the "foo" event
let json_payload = json!({"token": 123});
socket.emit("foo", json_payload).expect("Server unreachable");

// define a callback, that's executed when the ack got acked
let ack_callback = |message: Payload, _| {
    println!("Yehaa! My ack got acked?");
    println!("Ack data: {:#?}", message);
};

let json_payload = json!({"myAckData": 123});
// emit with an ack
socket
    .emit_with_ack("test", json_payload, Duration::from_secs(2), ack_callback)
    .expect("Server unreachable");

socket.disconnect().expect("Disconnect failed")

```

The main entry point for using this crate is the `ClientBuilder` which provides a way to easily configure a socket in the needed way. When the `connect` method is called on the builder, it returns a connected client which then could be used to emit messages to certain events. One client can only be connected to one namespace. If you need to listen to the messages in different namespaces you need to allocate multiple sockets.

## Documentation

Documentation of this crate can be found up on [docs.rs](https://docs.rs/tf_rust_socketio).

## Current features

This implementation now supports all of the features of the socket.io protocol mentioned [here](https://github.com/socketio/socket.io-protocol).
It generally tries to make use of websockets as often as possible. This means most times
only the opening request uses http and as soon as the server mentions that he is able to upgrade to
websockets, an upgrade  is performed. But if this upgrade is not successful or the server
does not mention an upgrade possibility, http-long polling is used (as specified in the protocol specs).
Here's an overview of possible use-cases:
- connecting to a server.
- register callbacks for the following event types:
    - open
    - close
    - error
    - message
    - custom events like "foo", "on_payment", etc.
- send JSON data to the server (via `serde_json` which provides safe
handling).
- send JSON data to the server and receive an `ack`.
- send and handle Binary data.
- **respond to server's `emitWithAck` calls with client-side ack messages**.

### Server-to-Client ACK Support

This crate now supports responding to server's `emitWithAck` calls. When the server emits an event with an acknowledgment request, the client can respond using the `ack` method:

#### Sync Example:
```rust
use tf_rust_socketio::{ClientBuilder, Payload, RawClient};
use serde_json::json;

let ack_callback = |message: Payload, socket: RawClient| {
    match message {
        Payload::Text(values, ack_id) => {
            println!("{:#?}", values);
            // Respond with the specific ack_id for concurrent ACK support
            if let Some(id) = ack_id {
                socket.ack_with_id(id, json!({"status": "received"})).unwrap();
            }
        },
        Payload::Binary(bytes, ack_id) => {
            println!("Received bytes: {:#?}", bytes);
            if let Some(id) = ack_id {
                socket.ack_with_id(id, vec![1, 2, 3]).unwrap();
            }
        },
        Payload::String(str, ack_id) => {
            println!("{}", str);
            if let Some(id) = ack_id {
                socket.ack_with_id(id, "response").unwrap();
            }
        },
    }
};

let mut socket = ClientBuilder::new("http://localhost:4200/")
    .on("messageWithAck", ack_callback)
    .connect()
    .expect("connection failed");
```

#### Async Example:
```rust
use futures_util::FutureExt;
use tf_rust_socketio::{asynchronous::{Client, ClientBuilder}, Payload};
use serde_json::json;

let callback = |payload: Payload, socket: Client| {
    async move {
        match payload {
            Payload::Text(values, ack_id) => {
                println!("{:#?}", values);
                // Respond with the specific ack_id for concurrent ACK support
                if let Some(id) = ack_id {
                    let _ = socket.ack_with_id(id, json!({"status": "received"})).await;
                }
            },
            Payload::Binary(bytes, ack_id) => {
                println!("Received bytes: {:#?}", bytes);
                if let Some(id) = ack_id {
                    let _ = socket.ack_with_id(id, vec![4, 5, 6]).await;
                }
            },
            Payload::String(str, ack_id) => {
                println!("{}", str);
                if let Some(id) = ack_id {
                    let _ = socket.ack_with_id(id, "response").await;
                }
            },
        }
    }.boxed()
};

let socket = ClientBuilder::new("http://localhost:4200")
    .namespace("/")
    .on("serverEvent", callback)
    .connect()
    .await
    .expect("Connection failed");
```

**Note**: The client maintains only the most recent ack ID. If multiple ack-eligible messages are received in quick succession, only the last one can be acknowledged.

## <a name="async"> Async version
This library provides an ability for being executed in an asynchronous context using `tokio` as
the execution runtime.
Please note that the current async implementation is still experimental, the interface can be object to
changes at any time.
The async `Client` and `ClientBuilder` support a similar interface to the sync version and live
in the `asynchronous` module. In order to enable the support, you need to enable the `async`
feature flag:
```toml
tf_rust_socketio = { version = "*", features = ["async"] }
```

The following code shows the example above in async fashion:
``` rust
use futures_util::FutureExt;
use tf_rust_socketio::{
    asynchronous::{Client, ClientBuilder},
    Payload,
};
use serde_json::json;
use std::time::Duration;

#[tokio::main]
async fn main() {
    // define a callback which is called when a payload is received
    // this callback gets the payload as well as an instance of the
    // socket to communicate with the server
    let callback = |payload: Payload, socket: Client| {
        async move {
            match payload {
                Payload::String(str) => println!("Received: {}", str),
                Payload::Binary(bin_data) => println!("Received bytes: {:#?}", bin_data),
            }
            socket
                .emit("test", json!({"got ack": true}))
                .await
                .expect("Server unreachable");
        }
        .boxed()
    };

    // get a socket that is connected to the admin namespace
    let socket = ClientBuilder::new("http://localhost:4200/")
        .namespace("/admin")
        .on("test", callback)
        .on("error", |err, _| {
            async move { eprintln!("Error: {:#?}", err) }.boxed()
        })
        .connect()
        .await
        .expect("Connection failed");

    // emit to the "foo" event
    let json_payload = json!({"token": 123});
    socket
        .emit("foo", json_payload)
        .await
        .expect("Server unreachable");

    // define a callback, that's executed when the ack got acked
    let ack_callback = |message: Payload, _: Client| {
        async move {
            println!("Yehaa! My ack got acked?");
            println!("Ack data: {:#?}", message);
        }
        .boxed()
    };

    let json_payload = json!({"myAckData": 123});
    // emit with an ack
    socket
        .emit_with_ack("test", json_payload, Duration::from_secs(2), ack_callback)
        .await
        .expect("Server unreachable");

    socket.disconnect().await.expect("Disconnect failed");
}
```

## Content of this repository

This repository contains a rust implementation of the socket.io protocol as well as the underlying engine.io protocol.

The details about the engine.io protocol can be found here:

* <https://github.com/socketio/engine.io-protocol>

The specification for the socket.io protocol here:

* <https://github.com/socketio/socket.io-protocol>

Looking at the component chart, the following parts are implemented (Source: https://socket.io/images/dependencies.jpg):

<img src="docs/res/dependencies.jpg" width="50%"/>

## Licence

MIT
