---
name: version
description: 使用 cargo-workspaces 管理 Rust 工作区版本号。支持 patch/minor/major 版本升级，自动同步所有 crate 版本和内部依赖。当用户需要升级版本、发布新版本或查看版本状态时使用。
---

# Version Management

使用 cargo-workspaces 统一管理 Rust workspace 中多个 crate 的版本号。

## 为什么使用 cargo-workspaces

本项目是双 crate 工作区（engineio + socketio），手动管理版本面临以下问题：

1. **版本同步**：socketio 依赖 engineio，升级版本时需要同时修改 3 处（两个 `Cargo.toml` 的 version + 依赖声明）
2. **遗漏风险**：手动修改容易遗漏某个文件
3. **Git tag**：需要手动创建 tag 并推送

cargo-workspaces 自动处理这些问题，一条命令完成所有操作。

## 配置文件

项目配置位于 `Cargo.toml`：

```toml
[workspace.metadata.workspaces]
allow_branch = "main"
independent = false
version_message = "chore(release): bump version to %v"
```

完整示例见 [`Cargo.toml`](../../../Cargo.toml)。

## 版本升级步骤

### Step 1: 确认当前状态

```bash
# 查看当前版本
cargo ws list -l

# 查看自上次 tag 以来的变更
cargo ws changed
```

### Step 2: 执行版本升级

```bash
# 升级 patch 版本（0.6.1 -> 0.6.2）
cargo ws version patch

# 升级 minor 版本（0.6.1 -> 0.7.0）
cargo ws version minor

# 升级 major 版本（0.6.1 -> 1.0.0）
cargo ws version major

# 跳过确认提示
cargo ws version patch -y
```

执行后 cargo-workspaces 会自动：
- 更新所有 crate 的 `version` 字段
- 更新内部依赖版本（如 `tf-rust-engineio = { version = "0.6.2", ... }`）
- 创建 git commit
- 创建 git tag（如 `v0.6.2`）

### Step 3: 推送到远程

```bash
# 推送 commit 和 tag
git push && git push --tags
```

## 常用命令速查

| 命令 | 说明 |
|------|------|
| `cargo ws list` | 列出所有 crate |
| `cargo ws list -l` | 列出 crate 及其版本 |
| `cargo ws changed` | 显示有变更的 crate |
| `cargo ws version patch` | 升级 patch 版本 |
| `cargo ws version minor` | 升级 minor 版本 |
| `cargo ws version major` | 升级 major 版本 |
| `cargo ws version patch --no-git-push` | 升级但不自动推送 |

## 注意事项

1. **分支限制**：默认只允许在配置的 `allow_branch`（main）上操作
2. **工作区干净**：建议在干净的 git 状态下执行版本升级
3. **内部依赖**：socketio 对 engineio 的依赖版本会自动同步更新

## 相关文件

- [`Cargo.toml`](../../../Cargo.toml) - 工作区配置
- [`engineio/Cargo.toml`](../../../engineio/Cargo.toml) - engineio crate
- [`socketio/Cargo.toml`](../../../socketio/Cargo.toml) - socketio crate
