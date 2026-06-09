# Loci 操作文档 v0.2.0

> 极简 CLI 启动器 —— 列出 PATH 中所有可执行文件，模糊搜索后一键跳转。  
> **设计哲学：只列出，只跳转。** 不管理版本、不安装包、不记忆别名。

---

## 目录

1. [快速安装](#1-快速安装)
2. [基本用法](#2-基本用法)
3. [列表模式](#3-列表模式)
4. [程序化选择](#4-程序化选择)
5. [元数据与标签](#5-元数据与标签)
6. [项目模式](#6-项目模式)
7. [排序](#7-排序)
8. [配置](#8-配置)
9. [JSON 输出格式](#9-json-输出格式)
10. [常见问题](#10-常见问题)
11. [开发指南](#11-开发指南)

---

## 1. 快速安装

### 方式一：从源码构建（推荐）

```bash
cargo install --path .
```

需要安装 [Rust](https://www.rust-lang.org/tools/install)。

### 方式二：下载预编译二进制

从 [GitHub Releases](https://github.com/Yaemikoreal/CliLoci/releases) 直接下载对应平台的二进制文件，放入 `PATH` 即可。

### 验证安装

```bash
loci -l --json
# 应返回非空 JSON，包含 executables 数组
```

---

## 2. 基本用法

### 交互式模式

最核心的使用方式——弹出 skim 模糊搜索界面（类似 fzf）：

```bash
loci
```

直接在 TUI 中搜索并选择工具，回车启动，Esc 退出。

### 预过滤后进入 TUI

```bash
loci git
# 打开 TUI 时已自动按 "git" 过滤
```

### 参数透传

将所有额外参数原样转发给选中的工具：

```bash
loci git -- log --oneline
# 选中 git 后执行: git log --oneline
```

```bash
loci -- npm install
# 先选择工具，再传递参数
```

---

## 3. 列表模式

不加 `-l`/`--list` 时的交互模式需要 TUI。如果只需要查看已安装的工具，使用列表模式：

### 文本列表

```bash
loci -l                          # 列出全部工具（字母序）
loci -l python                   # 按关键词过滤
loci -l --sort freq              # 按使用频率排序
```

输出格式：每行一个工具名，可直接用 `grep`/`head`/`wc` 等命令处理。

### JSON 列表（AI Agent 首选）

```bash
loci -l --json                   # 全量 JSON
loci -l --json git               # 按关键词过滤
loci -l --json --meta            # 含元数据（版本、类别、路径）
```

JSON 输出结构见[第 9 节](#9-json-输出格式)。

---

## 4. 程序化选择

用于脚本和自动化场景，无需 TUI 交互。

### 精确匹配启动

```bash
loci --exact python.exe           # 精确匹配后直接启动
loci --exact python.exe -- -V     # 匹配后带参数启动
```

失败时 exit code 1，stderr 输出 `exact match not found for 'xxx'`。

### 匹配第一个启动

```bash
loci --pick-first git             # 选中第一个匹配的工具
```

失败时 exit code 1，stderr 输出 `no matching tool for 'xxx'`。

### 按索引启动

```bash
loci --index 0                    # 选中列表第一个工具
loci --index 2 git                # 在 git 匹配结果中选第 3 个
```

支持预过滤后再索引，0-based。越界时 exit code 1 + `index out of range`。

### 使用频率记录

所有程序化选择（`--exact`、`--pick-first`、`--index`）以及交互模式选中工具后，都会自动记录使用频率到 `~/.local/share/loci/usage.json`。

---

## 5. 元数据与标签

### 工具元数据（`--meta`）

```bash
loci -l --json --meta             # 列出全部工具，附带元数据
loci -l --json --meta git         # 按关键词过滤 + 元数据
```

每个工具输出以下字段：

| 字段 | 说明 | 示例 |
|------|------|------|
| `version` | 版本号（通过 `tool --version` 探测） | `"git version 2.52.0"` |
| `category` | 自动推断的类别 | `"scm"` |
| `tags` | 类别标签 + 用户自定义标签 | `["scm", "devops"]` |
| `path` | 二进制文件的完整路径 | `"/usr/bin/git"` |

**注意**：`--meta` 会对每个工具运行 `--version`/`-V`（3 秒超时），首次扫描较慢。已知 GUI 工具（gitk、gvim 等）会被自动跳过以避免弹窗。

### 标签过滤（`--tag`）

```bash
loci -l --json --tag scm          # 只列出版本控制工具
loci -l --json --tag python       # 只列出 Python 生态工具
loci -l --json --tag python --meta  # 标签过滤 + 版本探测
```

**内置标签类别**：

| 标签 | 匹配规则示例 |
|------|------------|
| `scm` | git\*, hg, svn, jj |
| `container` | docker\*, kubectl, helm, podman |
| `python` | python\*, pip\*, poetry, conda, uv |
| `node` | node\*, npm, npx, yarn, bun, deno |
| `compress` | zip, tar, gzip, 7z, zstd, xz |
| `network` | curl, wget, ssh, rsync, ping, dig |
| `editor` | vim, nvim, code, emacs, nano |
| `rust` | cargo, rustc, rustup, cargo-\* |
| `go` | go, gopls, gofmt |
| `database` | mysql, psql, sqlite3, redis-cli |

`--tag` 自动启用元数据模式（即使不加 `--meta`），但**不会**触发版本探测——只有同时加 `--meta` 才会探测版本。

---

## 6. 项目模式

列出当前项目目录下的本地工具，不扫描全局 PATH。

```bash
loci -l --project                 # 文本输出
loci -l --json --project          # JSON 输出
loci -l --json --project --meta   # 含元数据
```

### 自动检测的项目类型

| 项目类型 | 检测标志 | 扫描目录 |
|----------|----------|----------|
| Node.js | `package.json` | `node_modules/.bin/` |
| Python venv | `pyvenv.cfg` | `.venv/bin/` 或 `venv/bin/`（Unix）/ `Scripts`（Win） |
| Rust | `Cargo.toml` | `target/debug/` + `target/release/` |
| Conda | `$CONDA_PREFIX` | `$CONDA_PREFIX/bin/` |

无项目标志时输出空列表（exit 0），stderr 提示 `no project-local tools found`。

---

## 7. 排序

```bash
loci -l --sort alpha              # 字母序排序（默认）
loci -l --sort freq               # 按使用频率排序
loci -l --sort freq --json        # 频率排序 + JSON 输出
```

### 频率排序规则

1. **使用次数降序**——用得越多的工具越靠前
2. **最近使用降序**——相同次数时最近使用的优先
3. **字母序**——以上均相同时按名称排序

数据持久化在 `~/.local/share/loci/usage.json`。

---

## 8. 配置

### 黑名单

过滤掉不想看到的工具。

**内置黑名单**：已过滤 50+ shell 内置命令（`cd`、`echo`、`export`、`kill`、`test` 等）。

**用户自定义黑名单**：

```bash
# Linux/macOS: ~/.config/loci/blacklist
# Windows:      %APPDATA%/loci/blacklist
```

格式：

```
# 每行一个工具名，# 为注释
clang
clang++
gcc-13
```

### 扩展扫描路径

设置环境变量 `LOCI_PATH_EXTRA` 追加额外扫描目录（不影响原始 `PATH`）：

```bash
# Unix (冒号分隔)
export LOCI_PATH_EXTRA="$HOME/.local/bin:$HOME/go/bin"

# Windows (分号分隔)
set LOCI_PATH_EXTRA=C:\tools\bin;D:\my-scripts
```

不存在的路径会输出警告（`eprintln!`）并跳过。

### 自定义标签

```bash
# Linux/macOS: ~/.config/loci/tags.json
# Windows:      %APPDATA%/loci/tags.json
```

格式：

```json
{
  "my-tool": ["devops", "internal"],
  "another-tool": ["data-science"]
}
```

用户标签与内置类别标签合并，自动去重。

---

## 9. JSON 输出格式

### 基础输出（`-l --json`）

```json
{
  "skill_version": "v0.2.0",
  "total": 142,
  "executables": ["7z", "7za", "cargo", "git", "python", "zip", ...],
  "filter": null
}
```

### 带过滤（`-l --json git`）

```json
{
  "skill_version": "v0.2.0",
  "total": 5,
  "executables": ["git", "git-lfs", "git-credential-manager", ...],
  "filter": "git"
}
```

### 带元数据（`-l --json --meta` 或 `--tag`）

```json
{
  "skill_version": "v0.2.0",
  "total": 3,
  "executables": ["cargo", "git", "python"],
  "filter": null,
  "tag_filter": "scm",
  "meta": {
    "git": {
      "version": "git version 2.52.0.windows.2",
      "category": "scm",
      "tags": ["scm"],
      "path": "C:/Program Files/Git/cmd/git.exe"
    }
  }
}
```

### 项目模式（`--project`）

```json
{
  "skill_version": "v0.2.0",
  "total": 2,
  "executables": ["my-app", "test-runner"],
  "filter": null,
  "project": true
}
```

### 字段说明

| 字段 | 类型 | 出现条件 | 说明 |
|------|------|----------|------|
| `skill_version` | string | 始终 | 编译时常量 `v0.2.0` |
| `total` | number | 始终 | `executables` 数组长度 |
| `executables` | string[] | 始终 | 工具名列表 |
| `filter` | string\|null | 始终 | 过滤关键词，无过滤时为 null |
| `project` | boolean | 仅 `--project` | 是否为项目模式 |
| `tag_filter` | string | 仅 `--tag` | 当前标签过滤器 |
| `meta` | object | 仅 `--meta` 或 `--tag` | 工具元数据映射 |
| `meta[x].version` | string\|null | 仅显式 `--meta` | 版本探测结果 |
| `meta[x].category` | string\|null | 有 `meta` 时 | 自动推断的类别 |
| `meta[x].tags` | string[] | 有 `meta` 时 | 内置标签 + 用户自定义 |
| `meta[x].path` | string | 有 `meta` 时 | 二进制完整路径 |

---

## 10. 常见问题

### 启动后显示 `loci: no executables found in PATH`

**原因**：PATH 环境变量为空，或所有 PATH 目录均不可读。

**排查**：
```bash
echo $PATH    # Unix
echo %PATH%   # Windows
```

### 某些工具不在列表中

**可能原因**：
- 该工具不在 `PATH` 中
- 文件名被内置或用户黑名单过滤了
- Windows 上扩展名不在 `PATHEXT` 中

**检查黑名单**：
```bash
cat ~/.config/loci/blacklist    # Unix
type %APPDATA%\loci\blacklist   # Windows
```

### 命令行卡住或超时

`--meta` 模式需要对每个工具运行 `--version`，有 3 秒超时。如果工具数量多（>200），可能耗时 10+ 秒。不带 `--meta` 则无此问题。

### 缓存刷新

loci 自动缓存扫描结果。当 PATH 变化或目录 mtime 更新时自动失效。如需强制刷新：

```bash
rm -rf ~/.cache/loci    # Unix
rmdir /s %LOCALAPPDATA%\loci\cache  # Windows
```

### Windows 上弹窗/控制台窗口

版本探测（`--meta`）会短暂启动子进程。loci 已使用 `CREATE_NO_WINDOW` 标志抑制控制台窗口，并断开 stdin 继承。如仍有弹窗，可能由目标工具自身行为（如打开 GUI 窗口）导致。可以在 `~/.config/loci/blacklist` 中添加对应工具名来跳过探测。

---

## 11. 开发指南

### 构建

```bash
cargo build                       # Debug
cargo build --release             # Release（压缩+LTO+strip+panic=abort）
```

### 测试

```bash
cargo test                        # 单元测试
cargo test -- --test-threads=1    # 单线程（避免文件系统冲突）
cargo test --test cli             # 仅集成测试
```

### 项目结构

```
src/
├── main.rs      # 入口、参数解析、进程启动
├── scanner.rs   # PATH 扫描、SHA-256 缓存、黑白名单管理
├── metadata.rs  # 工具元数据收集（版本探测、类别推断、标签合并）
├── ui.rs        # skim 模糊查找 TUI 集成
├── usage.rs     # 使用频率追踪与排序
└── platform.rs  # 平台相关的可执行文件检测
tests/
└── cli.rs       # 端到端集成测试
npm/             # npm 分发包
completions/     # Shell 补全脚本（bash/zsh/fish）
```

### 数据流

```
PATH 环境变量 + LOCI_PATH_EXTRA
  → scanner::collect() 或 collect_project()
    → SHA-256 指纹检查缓存（~/.cache/loci/cache.json）
    → 扫描目录 → 黑白名单过滤 → 去重 → 排序 → 写入缓存
  → 列表模式：过滤 → 排序 → 文本/JSON 输出
  → 交互模式：skim TUI → 选择 → 记录使用 → 启动进程
```

---

> **版本**: 0.2.0 | **许可**: MIT | **仓库**: https://github.com/Yaemikoreal/CliLoci
