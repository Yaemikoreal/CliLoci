# CLAUDE.md — CliLoci 开发与使用指南

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目概述

**Loci** 是一个极简 CLI 启动器，用 Rust 编写。它扫描 `PATH` 环境变量，列出所有可执行文件，并允许通过模糊搜索启动。

设计哲学："只列出，只跳转" — 不管理版本、不安装包、不记忆别名。

## 双重身份

本仓库同时是：

1. **Rust 项目** — 构建 `loci` 二进制，供人类日常使用
2. **AI Agent Skill 包** — 提供工具发现能力，供 AI 加载使用

Agent 入口文件是 [`SKILL.md`](./SKILL.md)，非开发者用户应直接读取该文件。

## 构建与测试

```bash
# Debug 构建
cargo build

# Release 构建（优化体积）
cargo build --release

# 运行测试
cargo test

# 安装到本地
cargo install --path .

# 运行
cargo run -- [args]
# 或安装后：
loci [args]
```

## 架构

```
src/
├── main.rs      # 入口，参数解析，进程启动
├── scanner.rs   # PATH 扫描、缓存、黑名单管理
├── ui.rs        # skim 模糊查找器集成
└── platform.rs  # 平台相关的可执行文件检测
```

### 核心流程

1. **参数解析** (`main.rs`) — `loci -l [--json] [filter]`（列表模式）和 `loci [filter...] [-- args...]`（交互模式）
2. **可执行文件收集** (`scanner.rs`) — 扫描 PATH 目录，黑名单过滤，去重，缓存
3. **交互选择** (`ui.rs`) — 通过 skim crate 提供模糊查找 TUI
4. **进程启动** (`main.rs`) — Unix: `exec()` 替换当前进程；Windows: 创建子进程并等待

### `--json` 模式（v0.1.0+）

Agent 可用 `loci -l --json [filter]` 获取结构化输出：

```json
{
  "total": 3,
  "executables": ["cargo", "cargo-clippy", "cargo-fmt"],
  "filter": "git"
}
```

## 关键实现细节

- **缓存**：首次扫描持久化到 `~/.cache/loci/cache.json`，后续 PATH 不变时 <1ms
- **黑名单**：内置 `DEFAULT_BLACKLIST` 过滤 shell 内置命令；用户可添加 `~/.config/loci/blacklist`
- **跨平台可执行检测** (`platform.rs`)：Unix 检查 `0o111` 权限位；Windows 检查 `PATHEXT` 扩展名

## 依赖

| Crate | 用途 |
|-------|------|
| `skim` | 模糊查找 TUI（`frizbee` 功能） |
| `dirs` | 跨平台配置/缓存目录解析 |
| `serde` / `serde_json` | 缓存文件序列化 + JSON 输出 |
| `sha2` | PATH 指纹缓存失效检测 |

## 发布流程

推送 `v*` tag 触发 `.github/workflows/release.yml`：
- 在 Linux/macOS/Windows 上运行测试
- 为 5 个目标构建（linux-x64, linux-arm64, macos-x64, macos-arm64, windows-x64）
- 创建 GitHub Release 并附带二进制文件和 SHA-256 校验和

## npm 分发

`npm/` 目录包含 `@yaemikoreal/loci` 包：
1. postinstall 自动从 GitHub Releases 下载正确平台的二进制
2. `bin/loci.js` 封装脚本将参数传递给二进制

## 使用示例

```bash
loci                           # 交互式模糊查找器
loci git                       # 预过滤 "git"
loci -l                        # 列出所有可执行文件（文本）
loci -l --json                 # JSON 格式输出（Agent 首选）
loci -l --json git             # 搜索 git 相关工具
loci -- log --oneline          # 参数透传
loci git -- log --oneline      # 预过滤 + 参数透传
```
