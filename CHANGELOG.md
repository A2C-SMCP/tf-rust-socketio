# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog], and this project adheres to
[Semantic Versioning]. The file is auto-generated using [Conventional Commits].

[keep a changelog]: https://keepachangelog.com/en/1.0.0/
[semantic versioning]: https://semver.org/spec/v2.0.0.html
[conventional commits]: https://www.conventionalcommits.org/en/v1.0.0-beta.4/

## Overview

* [`0.9.1`](#091) - _2026.08.26_
* [`0.9.0`](#090) - _2026.08.24_
* [`0.8.0`](#080) - _2026.04.27_
* [`0.7.0`](#070) - _2026.02.01_
* [`0.5.0`](#060) - _2024.04.16_
* [`0.5.0`](#050) - _2024.03.31_
* [`0.4.4`](#044) - _2023.11.18_
* [`0.4.3`](#043) - _2023.07.08_
* [`0.4.2`](#042) - _2023.06.25_
* [`0.4.1-alpha.2`](#041a2) - _2023.03.26_
* [`0.4.1-alpha.1`](#041a1) - _2023.01.15_
* [`0.4.0`](#041) - _2023.01.15_
* [`0.4.0`](#040) - _2022.10.20_
* [`0.3.1`](#031) - _2022.03.19_
* [`0.3.0`](#030) - _2021.12.16_
* [`0.3.0-alpha.2`](#030a3) - _2021.12.04_
* [`0.3.0-alpha.2`](#030a2) - _2021.10.14_
* [`0.3.0-alpha.1`](#030a1) - _2021.09.20_
* [`0.2.4`](#024) - _2021.05.25_
* [`0.2.3`](#023) - _2021.05.24_
* [`0.2.2`](#022) - _2021.05.13
* [`0.2.1`](#021) - _2021.04.27_
* [`0.2.0`](#020) – _2021.03.13_
* [`0.1.1`](#011) – _2021.01.10_
* [`0.1.0`](#010) – _2021.01.05_

## <a name="091">[0.9.1] - _Transport close no longer blocked by pending reconnect auth_ </a>

_2026.08.26_

### Fixed
- The asynchronous client (`async` feature) delivered the `Close` event only after a
  pending `on_reconnect` callback (e.g. auth refresh) finished: `reconnect()` held the
  builder lock while awaiting the user callback and the transport handshake, while the
  Close/event dispatch path needs the same lock in shared mode. A slow auth refresh could
  delay the disconnect notification — and a late Close could land on an already
  re-established session ([#14](https://github.com/A2C-SMCP/tf-rust-socketio/issues/14)).
- The user `on_reconnect` callback now runs outside any lock; the transport handshake runs
  under a shared lock — reconnect no longer blocks lifecycle notifications.

### Compatibility
- No public API changes; `on_reconnect` keeps its `FnMut` signature and semantics (a new
  token is still required before reconnecting). The synchronous client is unaffected.

## <a name="090">[0.9.0] - _Async callback concurrent dispatch — reader never blocks on user callbacks_ </a>

_2026.08.24_

### Breaking Changes
- The asynchronous client (`ClientBuilder::on` / `ClientBuilder::on_any` / `Client::emit_with_ack`,
  `async` feature) now requires callbacks to be `Fn` instead of `FnMut`. Callbacks sharing
  mutable state must use interior mutability (e.g. `Arc<Mutex<_>>`); plain captured state and
  non-capturing closures are unaffected.
- The asynchronous client dispatches socket.io packets as independent tasks instead of
  strictly serial dispatch: events within the same connection are no longer guaranteed to be
  delivered in completion order (only the socket.io wire order is preserved). User callbacks
  must be safe to invoke concurrently and must not rely on `FnMut`-style captured-state
  mutation. This aligns the client with python-socketio / JS socket.io / socketioxide.
  The synchronous client keeps the serial contract for now (see follow-up note in
  `raw_client.rs`).

### Changed
- Long-running event/ack callbacks no longer block the reader: Engine.IO pings (PONG replies)
  and subsequent packets keep flowing while a callback is pending — a callback cannot stall the
  heartbeat anymore ([#12](https://github.com/A2C-SMCP/tf-rust-socketio/issues/12)).
- Manual `disconnect()` / server `Disconnect` / transport close now abort in-flight dispatch
  tasks, so no user callback fires late against a torn-down connection.
- Ack handling no longer holds the outstanding-acks lock while invoking user callbacks
  (previously a slow ack callback stalled `emit_with_ack` and the reader). The buffered
  outstanding ack removal was also healed: index-based removal no longer leaks colliding ids.
- Teardown ordering: `disconnect()` aborts pending dispatch after the transport teardown, so a
  `disconnect()` issued from inside a callback still completes the protocol close.

### Compatibility
- Wire format, packet ordering on the wire and ack-id association semantics are unchanged.
- `reconnect_on_disconnect` / `DisconnectReason` semantics are unchanged; the transport-close
  callback is now dispatched concurrently instead of being awaited inline (a long Close handler
  no longer delays the reconnect decision).
- Senders that rely on the ordering between same-connection events (e.g. a `request` that should
  be processed before a later `cancel` of the same operation) must tolerate at the application
  layer that the `cancel` may be handled before the `request` handler is registered, in which
  case it is a no-op — the same semantics as python-socketio / JS socket.io. The 300ms/700ms
  fast-heartbeat boundary test covers this practice shape.

## <a name="080">[0.8.0] - _Preserve HTTP error body on Engine.IO handshake failure_ </a>

_2026.04.27_

### Added
- New error variant `tf_rust_engineio::Error::HttpErrorWithBody { status, body }`. The polling
  transport now preserves the response body when the server returns a non-200 status (e.g. an
  HTTP 400 + JSON payload from a Socket.IO middleware), so callers can match on structured
  server-side errors instead of only seeing a status code
  ([#7](https://github.com/A2C-SMCP/tf-rust-socketio/issues/7)).
- The new variant transparently propagates through `tf_rust_socketio::Error::IncompleteResponseFromEngineIo`,
  letting Socket.IO clients decode JSON bodies returned during handshake failures.

### Changed
- Sync and async `PollingTransport` (`emit` POST + GET) now read the response body on non-200
  responses and emit `HttpErrorWithBody`. If the body cannot be read (e.g. a truncated stream),
  the transports fall back to the existing `Error::IncompleteHttp(status)` so callers always
  observe one of the two variants — no information is silently dropped.

### Compatibility
- `Error::IncompleteHttp(u16)` is retained as the body-unavailable fallback. Existing downstream
  `match` arms keep compiling and stay reachable.
- Both `engineio::Error` and `socketio::Error` are `#[non_exhaustive]`, so adding the new
  variant is additive at the type level. Behavior change is opt-in: callers that do not pattern
  match on `HttpErrorWithBody` keep observing it via the existing `Display`/`Debug` formatting.

## <a name="070">[0.7.0] - _Concurrent ACK support_ </a>

_2026.02.01_

### Breaking Changes
- **Payload enum variants now include an optional `ack_id` field** to support concurrent acknowledgments
  - `Payload::Binary` now takes `(Bytes, Option<i32>)` instead of just `Bytes`
  - `Payload::Text` now takes `(Vec<Value>, Option<i32>)` instead of just `Vec<Value>`
  - `Payload::String` now takes `(String, Option<i32>)` instead of just `String`
  
### Added
- Concurrent ACK support allowing multiple acknowledgments to be processed simultaneously
- `ack_with_id()` method on both sync and async clients for explicit ack_id specification
- `with_ack_id()`, `ack_id()`, and `set_ack_id()` helper methods on Payload
- Unit tests verifying concurrent ACK functionality

### Changed
- Removed single `ack_id` field from Socket implementations to eliminate race conditions
- All callback functions now receive Payload with proper ack_id propagated from incoming packets
- `ack()` method now uses `None` for ack_id (use `ack_with_id()` for concurrent scenarios)

### Migration Guide
To update your code for the new Payload structure:

```rust
// Old code
match payload {
    Payload::Text(data) => handle_text(data),
    Payload::Binary(data) => handle_binary(data),
    Payload::String(data) => handle_string(data),
}

// New code
match payload {
    Payload::Text(data, ack_id) => {
        handle_text(data);
        if let Some(id) = ack_id {
            socket.ack_with_id(id, response)?;
        }
    },
    Payload::Binary(data, ack_id) => {
        handle_binary(data);
        if let Some(id) = ack_id {
            socket.ack_with_id(id, response)?;
        }
    },
    Payload::String(data, ack_id) => {
        handle_string(data);
        if let Some(id) = ack_id {
            socket.ack_with_id(id, response)?;
        }
    },
}
```

For creating payloads:
```rust
// Old code
let payload = Payload::Text(vec![json!("test")]);

// New code
let payload = Payload::Text(vec![json!("test")], None);
// Or use the helper
let payload = Payload::with_ack_id(vec![json!("test")], 42);
```>

## <a name="060">[0.6.0] - _Multi-payload fix and http 1.0_ </a>

_2024.04.16_

- Fix issues with processing multi-payload messages ([#392](https://github.com/1c3t3a/rust-socketio/pull/392)).
  Credits to shenjackyuanjie@.
- Bump http to 1.0 and all dependencies that use http to a version that also uses http 1.0 ([#418](https://github.com/1c3t3a/rust-socketio/pull/418)).
  Bumping those dependencies makes this a breaking change.

## <a name="050">[0.5.0] - _Packed with changes!_ </a>

_2024.03.31_

- Support multiple arguments to the payload through a new Payload variant called
  `Text` that holds a JSON value ([#384](https://github.com/1c3t3a/rust-socketio/pull/384)).
  Credits to ctrlaltf24@ and SalahaldinBilal@!
  Please note: This is a breaking change: `Payload::String` is deprecated and will be removed soon.
- Async reconnections: Support for automatic reconnection in the async version of the crate!
  ([#400](https://github.com/1c3t3a/rust-socketio/pull/400)). Credits to rageshkrishna@.
- Add an `on_reconnect` callback that allows to change the connection configuration
  ([#405](https://github.com/1c3t3a/rust-socketio/pull/405)). Credits to rageshkrishna@.
- Fix bug that ignored the ping interval ([#359](https://github.com/1c3t3a/rust-socketio/pull/359)).
  Credits to sirkrypt0@. This is a breaking change that removes the engine.io's stream impl.
  It is however replaced by a method called `as_stream` on the engine.io socket.
- Add macro `async_callback` and `async_any_callback` for async callbacks ([#399](https://github.com/1c3t3a/rust-socketio/pull/399).
  Credits to shenjackyuanjie@.

## <a name="044">[0.4.4] - _Bump dependencies_ </a>

_2023.11.18_

- Bump tungstenite version to v0.20.1 (avoiding security vulnerability) [#368](https://github.com/1c3t3a/rust-socketio/pull/368)
- Updating other dependencies


## <a name="043">[0.4.3] - _Bugfix!_ </a>

_2023.07.08_

- Fix of [#323](https://github.com/1c3t3a/rust-socketio/issues/323)
- Marking the async feature optional

## <a name="042">[0.4.2] - _Stabilizing the async interface!_ </a>

_2023.06.25_

- Fix "Error while parsing an incomplete packet socketio" on first heartbeat killing the connection async client
([#311](https://github.com/1c3t3a/rust-socketio/issues/311)). Credits to [@sirkrypt0](https://github.com/sirkrypt0)
- Fix allow awaiting async callbacks ([#313](https://github.com/1c3t3a/rust-socketio/issues/313)). Credits to [@felix-gohla](https://github.com/felix-gohla)
- Various performance improvements especially in packet parsing. Credits to [@MaxOhn](https://github.com/MaxOhn)
- API for setting the reconnect URL on a connected client ([#251](https://github.com/1c3t3a/rust-socketio/issues/251)).
Credits to [@tyilo](https://github.com/tyilo)

## <a name="041a2">[0.4.0-alpha.2] - _Async socket.io fixes_ </a>

_2023.03.26_

- Add `on_any` method for async `ClientBuilder`. This adds the capability to
  react to all incoming events (custom and otherwise).
- Add `auth` option to async `ClientBuilder`. This allows for specifying JSON
  data that is sent with the first open packet, which is commonly used for
  authentication.
- Bump dependencies and remove calls to deprecated library functions.

## <a name="041a1">[0.4.0-alpha.1] - _Async socket.io version_ </a>

_2023.01.05_

- Add an async socket.io interface under the `async` feature flag, relevant PR: [#180](https://github.com/1c3t3a/rust-socketio/pull/180).
- See example code under `socketio/examples/async.rs` and in the `async` section of the README.

## <a name="041">[0.4.1] - _Minor enhancements_ </a>

_2023.01.05_

- As of [#264](https://github.com/1c3t3a/rust-socketio/pull/264), the callbacks
  are now allowed to be `?Sync`.
- As of [#265](https://github.com/1c3t3a/rust-socketio/pull/265), the `Payload`
  type now implements `AsRef<u8>`.

## <a name="040">[0.4.0] - _Bugfixes and Reconnection feature_ </a>

_2022.10.20_

### Changes
- Fix [#214](https://github.com/1c3t3a/rust-socketio/issues/214).
- Fix [#215](https://github.com/1c3t3a/rust-socketio/issues/215).
- Fix [#219](https://github.com/1c3t3a/rust-socketio/issues/219).
- Fix [#221](https://github.com/1c3t3a/rust-socketio/issues/221).
- Fix [#222](https://github.com/1c3t3a/rust-socketio/issues/222).
- BREAKING: The default Client returned by the builder will automatically reconnect to the server unless stopped manually.
  The new `ReconnectClient` encapsulates this behaviour.

Special thanks to [@SSebo](https://github.com/SSebo) for his major contribution to this release.

## <a name="031">[0.3.1] - _Bugfix_ </a>

_2022.03.19_

### Changes
- Fixes regarding [#166](https://github.com/1c3t3a/rust-socketio/issues/166).

## <a name="030">[0.3.0] - _Stabilize alpha version_ </a>

_2021.12.16_

### Changes
- Stabilized alpha features.
- Fixes regarding [#133](https://github.com/1c3t3a/rust-socketio/issues/133).

## <a name="030a3">[0.3.0-alpha.3] - _Bugfixes_ </a>

_2021.12.04_

### Changes

- fix a bug that resulted in a blocking `emit` method (see [#133](https://github.com/1c3t3a/rust-socketio/issues/133)).
- Bump dependencies.

## <a name="030a2">[0.3.0-alpha.2] - _Further refactoring_ </a>

_2021.10.14_

### Changes

* Rename `Socket` to `Client` and `SocketBuilder` to `ClientBuilder`
* Removed headermap from pub use, internal type only

* Deprecations:
    * crate::payload (use crate::Payload instead)
    * crate::error (use crate::Error instead)
    * crate::event (use crate::Event instead)

## <a name="030a1">[0.3.0-alpha.1] - _Refactoring_ </a>

_2021.09.20_

### Changes

* Refactored Errors
    * Renamed EmptyPacket to EmptyPacket()
    * Renamed IncompletePacket to IncompletePacket()
    * Renamed InvalidPacket to InvalidPacket()
    * Renamed Utf8Error to InvalidUtf8()
    * Renamed Base64Error to InvalidBase64
    * Renamed InvalidUrl to InvalidUrlScheme
    * Renamed ReqwestError to IncompleteResponseFromReqwest
    * Renamed HttpError to IncompleteHttp
    * Renamed HandshakeError to InvalidHandshake
    * Renamed ~ActionBeforeOpen to IllegalActionBeforeOpen()~
    * Renamed DidNotReceiveProperAck to MissingAck
    * Renamed PoisonedLockError to InvalidPoisonedLock
    * Renamed FromWebsocketError to IncompleteResponseFromWebsocket
    * Renamed FromWebsocketParseError to InvalidWebsocketURL
    * Renamed FromIoError to IncompleteIo
    * New error type InvalidUrl(UrlParseError)
    * New error type InvalidInteger(ParseIntError)
    * New error type IncompleteResponseFromEngineIo(rust_engineio::Error)
    * New error type InvalidAttachmentPacketType(u8)
    * Removed EmptyPacket
* Refactored Packet
    * Renamed encode to From<&Packet>
    * Renamed decode to TryFrom<&Bytes>
    * Renamed attachments to attachments_count
    * New struct member attachments: Option<Vec<Bytes>>
* Refactor PacketId
    * Renamed u8_to_packet_id to TryFrom<u8> for PacketId
* Refactored SocketBuilder
    * Renamed set_namespace to namespace
    * Renamed set_tls_config to tls_config
    * Renamed set_opening_header to opening_header
    * namespace returns Self rather than Result<Self>
    * opening_header accepts a Into<HeaderValue> rather than HeaderValue
* Allows for pure websocket connections
* Refactor EngineIO module

## <a name="024">[0.2.4] - _Bugfixes_ </a>

_2021.05.25_

### Changes

* Fixed a bug that prevented the client from receiving data for a message event issued on the server.

## <a name="023">[0.2.3] - _Disconnect methods on the Socket struct_ </a>

_2021.05.24_

### Changes

* Added a `disconnect` method to the `Socket` struct as requested in [#43](https://github.com/1c3t3a/rust-socketio/issues/43).

## <a name="022">[0.2.2] - _Safe websockets and custom headers_ </a>

_2021.05.13_

### Changes

* Added websocket communication over TLS when either `wss`, or `https` are specified in the URL.
* Added the ability to configure the TLS connection by providing an own `TLSConnector`.
* Added the ability to set custom headers as requested in [#35](https://github.com/1c3t3a/rust-socketio/issues/35).

## <a name="021">[0.2.1] - _Bugfixes_ </a>

_2021.04.27_

### Changes

* Corrected memory ordering issues which might have become an issue on certain platforms.
* Added this CHANGELOG to keep track of all changes.
* Small stylistic changes to the codebase in general.

## <a name="020">[0.2.0] - _Fully implemented the socket.io protocol 🎉_ </a>

_2021.03.13_


### Changes

* Moved from async rust to sync rust.
* Implemented the missing protocol features.
    * Websocket as a transport layer.
    * Binary payload.
* Added a `SocketBuilder` class to easily configure a connected client.
* Added a new `Payload` type to manage the binary and string payload.


## <a name="011">[0.1.1] - _Update to tokio 1.0_</a>

_2021.01.10_

### Changes

* Bumped `tokio` to version `1.0.*`, and therefore reqwest to `0.11.*`.
* Removed all `unsafe` code.

## <a name="010">[0.1.0] - _First release of rust-socketio 🎉_</a>

_2021.01.05_

* First version of the library written in async rust. The features included:
    * connecting to a server.
    * register callbacks for the following event types:
        * open, close, error, message
    * custom events like "foo", "on_payment", etc.
    * send json-data to the server (recommended to use serde_json as it provides safe handling of json data).
    * send json-data to the server and receive an ack with a possible message.

