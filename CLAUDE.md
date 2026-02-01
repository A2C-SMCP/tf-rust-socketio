# CLAUDE.md

本文件为 Claude Code (claude.ai/code) 在此代码库中工作时提供指导。

## 项目概述

这是一个 Rust 实现的 Socket.IO/Engine.IO 客户端库，采用双 crate 工作区结构：
- **tf-rust-engineio** (`engineio/`) - 底层 Engine.IO 协议实现
- **tf-rust-socketio** (`socketio/`) - 基于 engineio 构建的高层 Socket.IO 客户端

本项目是 rust-socketio 的分支，针对 A2C-SMCP 协议进行了增强，包括服务端到客户端的 ACK 支持和重连 header 更新功能。

## 构建命令

```bash
# 构建整个工作区
cargo build --all-features

# 快速测试（数据包解析，无需 Docker）
make test-fast

# 完整测试套件（需要 Docker）
make keys                    # 生成 TLS 证书（仅首次需要）
make run-test-servers        # 在 Docker 中启动测试服务器
make test-all                # 运行所有测试
docker stop socketio_test    # 完成后停止测试服务器

# 代码质量检查
make clippy                  # 代码检查
make format                  # 格式检查
make checks                  # build + test-fast + clippy + format
make pipeline                # 完整 CI（build + test-all + clippy + format）
```

## 运行单个测试

```bash
# 在 socketio crate 中运行特定测试
cargo test --package tf-rust-socketio test_name

# 在 engineio crate 中运行特定测试
cargo test --package tf-rust-engineio test_name

# 运行测试并显示输出
cargo test --package tf-rust-socketio -- --nocapture

# 运行异步测试（async 对 engineio 默认开启，对 socketio 需手动开启）
cargo test --package tf-rust-socketio --features async
```

## 架构

### 同步/异步双模式设计
每个 crate 都有并行的同步和异步模块。异步版本使用 Tokio 运行时。`async` 特性对 engineio 默认开启，对 socketio 需手动开启。

### 传输层 (engineio)
- 可自动从 HTTP 长轮询升级到 WebSocket
- 传输实现：`polling.rs`、`websocket.rs`、`websocket_secure.rs`
- 异步变体位于 `asynchronous/async_transports/`

### 客户端层 (socketio)
- **ClientBuilder** (`client/builder.rs`) - 链式配置，支持 `.on()`、`.namespace()`、`.auth()`、`.tls_config()`
- **RawClient** (`client/raw_client.rs`) - 核心 API：`emit()`、`emit_with_ack()`、`ack()`、`disconnect()`
- 事件回调通过闭包注册，存储在 `Arc<Mutex<HashMap<Event, Callback>>>` 中

### 数据包协议
- **Engine.IO** (`engineio/src/packet.rs`)：Open、Close、Ping、Pong、Message、MessageBinary、Upgrade、Noop
- **Socket.IO** (`socketio/src/packet.rs`)：Connect、Disconnect、Event、Ack、ConnectError、BinaryEvent、BinaryAck

### 分支特有增强
- **服务端到客户端 ACK**：`Payload` 包含可选的 `ack_id`，客户端提供 `ack()` 和 `ack_with_id()` 方法
- **重连 Header 更新**：`ReconnectSettings::opening_header()` 用于重连时刷新 token
- **CloseReason 枚举**：`IOServerDisconnect`、`IOClientDisconnect`、`TransportClose`

## 测试基础设施

集成测试需要 Docker 测试服务器（Node.js Socket.IO/Engine.IO）：
- 端口 4200：Socket.IO 默认命名空间
- 端口 4201：Engine.IO 轮询
- 端口 4202：Engine.IO 安全连接（HTTPS）
- 端口 4203：Engine.IO 仅轮询（不升级）
- 端口 4204：Socket.IO 带认证
- 端口 4205-4206：Socket.IO 带重启功能

测试服务器配置在 `ci/` 目录下，包含 `Dockerfile` 和 JavaScript 服务器实现。

## 关键文件

- `socketio/src/client/raw_client.rs` - 主客户端实现
- `socketio/src/packet.rs` - Socket.IO 协议解析
- `socketio/src/payload.rs` - 包含 ACK ID 支持的 Payload 类型
- `engineio/src/client/client.rs` - 传输层客户端
- `engineio/src/transports/` - 传输实现
