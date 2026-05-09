# Loci

> A minimalist CLI launcher — list, filter, and jump to any executable on your PATH.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Crates.io](https://img.shields.io/crates/v/loci.svg)](https://crates.io/crates/loci)

**设计哲学**: loci 只做一件事 — 列出系统中所有可用的 CLI 工具，让你模糊查找并快速启动。不管理版本、不安装包、不记忆别名。只列出，只跳转。

```
$ loci
┌─ 42 executables │ type to filter, Enter to select, Esc to quit ─┐
│ loci > cargo▊                                                    │
│ cargo-clippy                                                      │
│ cargo-fmt                                                         │
│ cargo-miri                                                        │
│ ...                                                               │
└──────────────────────────────────────────────────────────────────┘
```

## Features

- **模糊搜索** — 交互式 TUI (基于 [skim](https://github.com/skim-rs/skim))
- **零配置** — 开箱即用，自动扫描 `PATH` 环境变量
- **极速启动** — 缓存扫描结果，首次运行后 <1ms 启动
- **跨平台** — 支持 Linux、macOS、Windows
- **轻量** — ~2.3 MB 单二进制文件，无运行时依赖
- **参数透传** — 所有额外参数原样传递给选中的工具

## Installation

### npm (推荐 — 一键安装)

```sh
npm install -g @yaemikoreal/loci
```

自动下载对应平台的预编译二进制文件，需要 Node.js ≥14。

### Cargo (从源码构建)

```sh
cargo install loci
```

### 预编译二进制

从 [GitHub Releases](https://github.com/Yaemikoreal/CliLoci/releases) 下载对应平台的二进制文件。

### Homebrew (macOS / Linux)

```sh
brew tap Yaemikoreal/tap
brew install loci
```

### Scoop (Windows)

```powershell
scoop bucket add Yaemikoreal https://github.com/Yaemikoreal/scoop-bucket
scoop install loci
```

## Usage

```sh
# 打开交互式模糊查找器，列出所有可执行文件
loci

# 预过滤关键词
loci git

# 透传参数给选中的工具
loci -- log --oneline

# 预过滤 + 参数透传
loci git -- log --oneline

# 列表模式：打印匹配的可执行文件（无 TUI）
loci -l
loci -l python
loci --list cargo
```

## Configuration

### 自定义黑名单

创建 `~/.config/loci/blacklist` (Linux/macOS) 或 `%APPDATA%/loci/blacklist` (Windows)，每行一个工具名：

```
# ~/.config/loci/blacklist
# 以 # 开头的行会被忽略
clang
clang++
x86_64-linux-gnu-gcc
```

内置默认黑名单过滤 shell 内置命令。

### 额外扫描路径

设置 `LOCI_PATH_EXTRA` 环境变量添加额外扫描目录：

```sh
export LOCI_PATH_EXTRA="$HOME/.local/bin:$HOME/scripts"
```

## How it works

1. 读取 `PATH` 环境变量
2. 扫描每个目录下的可执行文件
3. 去重（首次出现优先）
4. 使用 SHA-256 指纹（PATH + 目录 mtime）缓存结果
5. 启动 fuzzy-finder (skim) 交互选择
6. 选中后替换当前进程（Unix `exec`）或启动后退出（Windows）

## Development

```sh
# 克隆仓库
git clone https://github.com/Yaemikoreal/CliLoci.git
cd CliLoci

# 构建
cargo build --release

# 运行测试
cargo test

# 安装到本地
cargo install --path .
```

## License

MIT