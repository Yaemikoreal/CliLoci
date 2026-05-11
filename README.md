<div align="center">

# Loci

> A minimalist CLI launcher — list, filter, and jump to any executable on your PATH.  
> **Designed for both humans and AI agents.**

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Crates.io](https://img.shields.io/crates/v/loci.svg)](https://crates.io/crates/loci)
[![npm](https://img.shields.io/npm/v/@yaemikoreal/loci)](https://www.npmjs.com/package/@yaemikoreal/loci)

[English](#english) · [中文](#中文)

</div>

---

<a name="english"></a>

# English

**loci** scans your `PATH`, lists everything executable, and lets you fuzzy-search + launch it.  
That's all it does. No version management, no package installation, no alias memory.  
**List only. Jump only.**

```
$ loci
┌─ 142 executables │ type to filter, Enter to select, Esc to quit ─┐
│ loci > cargo▊                                                    │
│ cargo-clippy                                                      │
│ cargo-fmt                                                         │
│ cargo-miri                                                        │
│ ...                                                               │
└──────────────────────────────────────────────────────────────────┘
```

## Features

- **Fuzzy search** — interactive TUI powered by [skim](https://github.com/skim-rs/skim)
- **Zero config** — automatically scans your `PATH` on first run
- **Blazing fast** — persistent cache: ~100ms first scan, <1ms subsequent
- **Cross-platform** — Linux, macOS, Windows
- **Lightweight** — ~2.3 MB single binary, no runtime dependencies
- **Argument passthrough** — all extra args forwarded to the selected tool
- **JSON output** — `loci -l --json` for AI / programmatic consumption

## Quick Install

```sh
# npm (recommended — one-liner)
npm install -g @yaemikoreal/loci

# Cargo
cargo install loci

# Homebrew (macOS / Linux)
brew tap Yaemikoreal/tap && brew install loci

# Scoop (Windows)
scoop bucket add Yaemikoreal https://github.com/Yaemikoreal/scoop-bucket
scoop install loci
```

Pre-built binaries on [GitHub Releases](https://github.com/Yaemikoreal/CliLoci/releases).

## Usage

```sh
loci                           # Interactive fuzzy-finder (TUI)
loci git                       # Pre-filter: show only git-related tools
loci -- log --oneline          # Forward args to selected tool
loci git -- log --oneline      # Pre-filter + forward args

loci -l                        # List mode (no TUI, plain text)
loci -l python                 # List mode with filter
loci -l --json                 # JSON output (programmatic / AI agent)
loci -l --json git             # JSON output with keyword filter
```

## For AI Agents

`loci -l --json` returns structured data that any AI agent can parse:

```json
$ loci -l --json
{
  "total": 142,
  "executables": ["7z", "7za", "cargo", "git", "python", ...],
  "filter": null
}

$ loci -l --json git
{
  "total": 4,
  "executables": ["cargo.exe", "cargo-clippy.exe", "cargo-fmt.exe", "cargo-miri.exe"],
  "filter": "git"
}
```

```python
import subprocess, json

result = subprocess.run(["loci", "-l", "--json", "git"],
    capture_output=True, text=True)
tools = json.loads(result.stdout)["executables"]
# → ["cargo.exe", "cargo-clippy.exe", "cargo-fmt.exe", "cargo-miri.exe"]
```

> **For full agent protocol, trigger rules, and integration patterns**, see [`SKILL.md`](./SKILL.md).

## Configuration

**Blacklist**: `~/.config/loci/blacklist` (Linux/macOS) or `%APPDATA%/loci/blacklist` (Windows)

```
# One name per line; # for comments
clang
clang++
```

Built-in blacklist already filters shell builtins (`cd`, `echo`, `export`, etc.).

**Extra scan paths**: `export LOCI_PATH_EXTRA="$HOME/.local/bin:$HOME/go/bin"`

## How It Works

1. Reads `PATH` environment variable  
2. Scans each directory for executables (permission bits on Unix, `PATHEXT` on Windows)  
3. Deduplicates by first-appearance priority  
4. Caches results with SHA-256 fingerprint (PATH + directory mtimes)  
5. Launches skim TUI for fuzzy selection, or prints list / JSON in list mode  
6. Selected tool replaces the current process (Unix `exec`) or spawns + waits (Windows)

## Development

```sh
git clone https://github.com/Yaemikoreal/CliLoci.git
cd CliLoci
cargo build --release
cargo test
cargo install --path .
```

## License

MIT

---

<a name="中文"></a>

# 中文

**loci** 扫描你的 `PATH`，列出所有可执行文件，支持模糊搜索和快速启动。  
就做一件事：**只列出，只跳转**。不管理版本、不安装包、不记忆别名。

```
$ loci
┌─ 142 个可执行文件 │ 输入过滤，回车选择，Esc 退出 ─┐
│ loci > cargo▊                                      │
│ cargo-clippy                                        │
│ cargo-fmt                                           │
│ cargo-miri                                          │
│ ...                                                 │
└─────────────────────────────────────────────────────┘
```

## 特性

- **模糊搜索** — 交互式 TUI（基于 [skim](https://github.com/skim-rs/skim)）
- **零配置** — 开箱即用，自动扫描 `PATH`
- **极速启动** — 缓存：首次 ~100ms，后续 <1ms
- **跨平台** — Linux、macOS、Windows
- **轻量** — ~2.3 MB 单二进制，无运行时依赖
- **参数透传** — 额外参数原样转给选中工具
- **JSON 输出** — `loci -l --json`，AI Agent / 程序化调用首选

## 一键安装

```sh
# npm（推荐）
npm install -g @yaemikoreal/loci

# Cargo
cargo install loci

# Homebrew（macOS / Linux）
brew tap Yaemikoreal/tap && brew install loci

# Scoop（Windows）
scoop bucket add Yaemikoreal https://github.com/Yaemikoreal/scoop-bucket
scoop install loci
```

预编译二进制见 [GitHub Releases](https://github.com/Yaemikoreal/CliLoci/releases)。

## 用法

```sh
loci                           # 交互式模糊查找器
loci git                       # 预过滤：只显示 git 相关工具
loci -- log --oneline          # 参数透传
loci git -- log --oneline      # 预过滤 + 参数透传

loci -l                        # 列表模式（文本，无 TUI）
loci -l python                 # 列表模式 + 过滤
loci -l --json                 # JSON 输出（AI Agent / 程序化调用）
loci -l --json git             # JSON 输出 + 关键词过滤
```

## AI Agent 集成

`loci -l --json` 输出结构化 JSON，Agent 可直接解析：

```json
$ loci -l --json
{
  "total": 142,
  "executables": ["7z", "7za", "cargo", "git", "python", ...],
  "filter": null
}
```

```python
import subprocess, json

result = subprocess.run(["loci", "-l", "--json", "git"],
    capture_output=True, text=True)
tools = json.loads(result.stdout)["executables"]
# → ["cargo.exe", "cargo-clippy.exe", "cargo-fmt.exe", "cargo-miri.exe"]
```

> **完整 Agent 协议、触发规则、集成模式**详见 [`SKILL.md`](./SKILL.md)。

## 配置

**黑名单**：`~/.config/loci/blacklist`（Linux/macOS）或 `%APPDATA%/loci/blacklist`（Windows）

```
# 每行一个工具名，# 开头为注释
clang
clang++
```

内置黑名单已过滤 shell 内置命令（`cd`、`echo`、`export` 等）。

**扩展扫描路径**：`export LOCI_PATH_EXTRA="$HOME/.local/bin:$HOME/go/bin"`

## 工作原理

1. 读取 `PATH` 环境变量
2. 扫描各目录下的可执行文件（Unix 检查权限位，Windows 检查 `PATHEXT`）
3. 按首次出现优先去重
4. SHA-256 指纹缓存结果
5. 启动 skim TUI 模糊选择，或列表模式直接输出 / JSON 输出
6. 选中后 Unix 用 `exec` 替换进程，Windows 创建子进程并等待

## 开发

```sh
git clone https://github.com/Yaemikoreal/CliLoci.git
cd CliLoci
cargo build --release
cargo test
cargo install --path .
```

## License

MIT
