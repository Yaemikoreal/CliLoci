# CLAUDE.md — CliLoci 开发与使用指南

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目概述

**Loci** 是一个极简 CLI 启动器，用 Rust 编写。它扫描 `PATH` 环境变量，列出所有可执行文件，并允许通过模糊搜索启动。设计哲学："只列出，只跳转" — 不管理版本、不安装包、不记忆别名。

本仓库同时是 Rust 项目和 AI Agent Skill 包。Agent 入口为 [`SKILL.md`](./SKILL.md)。

## 构建与测试

```bash
cargo build              # Debug 构建
cargo build --release    # Release（opt-level="z" + LTO + strip + panic=abort）
cargo test               # 运行所有单元测试 + 集成测试
cargo test -- --test-threads=1  # 单线程运行（usage 测试共享磁盘文件，需串行）
cargo test <module>::tests      # 只跑某个模块，如 cargo test scanner::tests
cargo test --test cli           # 只跑集成测试
cargo test <test_name>          # 按名称过滤，如 cargo test parse_args_default
cargo run -- [args]      # 直接运行（无需安装）
cargo install --path .   # 安装到本地 PATH
```

Release profile 在 `Cargo.toml:16-21` 中定义：`opt-level = "z"`, `lto = true`, `strip = true`, `codegen-units = 1`, `panic = "abort"`。

## 架构

```
src/
├── main.rs      # 入口、参数解析、进程启动（launch / fuzzy_match / parse_args）
├── scanner.rs   # PATH 扫描、SHA-256 缓存、黑白名单管理
├── metadata.rs  # 工具元数据收集（版本探测、类别推断、标签合并）
├── ui.rs        # skim 模糊查找 TUI 集成
├── usage.rs     # 使用频率追踪与排序（持久化到 ~/.local/share/loci/usage.json）
└── platform.rs  # 平台相关的可执行文件检测（Unix 权限位 / Windows PATHEXT）

tests/
└── cli.rs       # 端到端集成测试（26 个场景，运行编译后的二进制）

npm/             # npm 分发包（postinstall 自动下载二进制）
completions/     # Shell 补全（bash/zsh/fish）
docs/
└── usage.md     # 完整操作文档
scripts/
└── loci-list    # Python 封装脚本，供 AI Agent 便捷调用
```

## 数据流

```
PATH 环境变量 (+ LOCI_PATH_EXTRA)
  → scanner::collect()
    → SHA-256 指纹检查缓存 (~/.cache/loci/cache.json)
    → 缓存命中 → 直接返回
    → 缓存未命中 → 扫描目录 → 黑白名单过滤 → 按首次出现去重 → 字母序排序 → 写入缓存
  → 列表模式：过滤(可选) → 排序(可选) → 输出(文本/JSON)
  → 交互模式：skim TUI → 用户选择 → 记录使用 → 启动进程
```

## 测试架构

**共 121 个测试**：95 单元测试（内嵌 `#[cfg(test)] mod tests`）+ 26 集成测试（`tests/cli.rs`）。

### 单元测试模式

每个源文件有自己的 `#[cfg(test)] mod tests` 块，测试私有函数无需 `pub`。关键测试模式：

| 模式 | 说明 | 使用位置 |
|------|------|----------|
| `temp_dir(label)` | 创建 `$TMPDIR/loci-*-<label>-<pid>` 唯一临时目录 | scanner/metadata/platform 测试 |
| `touch_exec(path)` | 创建可执行文件（Unix 设 0o755，Windows 加 `.exe`） | scanner 测试 |
| `scan_name(name)` | 返回 scan_dirs 实际存储的名称（Windows 含 `.exe`） | scanner 测试断言 |
| `write_version_script(dir, name, ver)` | 创建版本探测用的脚本（Unix shell / Windows bat） | metadata 测试 |
| `setup_usage(json)` / `teardown_usage(dir)` | 写入/清理 `usage.json` 到真实 data 目录 | usage 测试 |
| `Scanner::test_new(blacklist, cache_dir)` | 测试用构造器，跳过真实配置加载 | scanner 测试 |

**重要**：`usage.rs` 的测试共享同一磁盘路径（`dirs::data_dir()/loci/usage.json`），必须串行运行：`cargo test -- --test-threads=1`。

### 集成测试（`tests/cli.rs`）

通过 `env!("CARGO_BIN_EXE_loci")` 获取编译后的二进制路径，用 `std::process::Command` 驱动。使用自定义 `PATH` 环境变量隔离系统工具：

```rust
fn create_executable(dir: &Path, name: &str) -> PathBuf  // 创建可执行测试文件
fn loci_with_path(args: &[&str], path: &str) -> Output    // 自定义 PATH 运行
fn tool_display_name(name: &str) -> String                // 平台感知的工具名
```

### Windows 兼容注意事项

- 所有测试工具文件通过 `touch_exec`/`create_executable` 创建，Windows 自动加 `.exe`
- 断言工具名时用 `scan_name("tool")` 或 `tool_display_name("tool")` 自动适配
- platform 测试仅 `#[cfg(windows)]` 时编译 Windows 分支
- usage 测试的 `record_usage`/`sort_by_frequency` 写 real data dir，避免干扰需串行

## 关键实现细节

### 参数解析（`main.rs`）

`parse_args()` 返回 `ParsedArgs` 结构体，包含 9 个字段：`list_mode`, `json_mode`, `meta_mode`, `project_mode`, `tag_filter`, `select_mode`, `sort_mode`, `filter`, `forwarded`。

- 所有标志级的标志（`--json`/`--meta`/`--project`/`--pick-first`/`--exact`）通过 `any()` 扫描全局位置（不限于 `-l` 后）
- `-l`/`--list` 必须在 `args[1]` 位置
- 值消费型标志（`--tag <name>`、`--index <N>`、`--sort <mode>`）通过 while 循环扫描，参数缺失时输出 `eprintln!` 警告
- `--` 之前的所有非标志 token 拼接为 filter 字符串，之后的为透传参数
- 示例：`loci -l --json git` → `list_mode=true, json_mode=true, filter=Some("git")`

### 缓存机制（`scanner.rs`）

- **路径**：`~/.cache/loci/cache.json`（通过 `dirs::cache_dir()` 解析）
- **指纹**：SHA-256 哈希所有 PATH 目录路径 + 各自 mtime
- **失效条件**：PATH 变化、目录 mtime 变化。注意：用户黑名单变化不会触发缓存失效（重启进程即重建）
- **写入**：先写 `cache.json.tmp`，再 `rename` 为 `cache.json`（原子写入，防损坏）
- **项目模式**（`--project`）不走磁盘缓存 — 项目目录小且短暂

### 黑名单（`scanner.rs:9-17`）

- **内置（DEFAULT_BLACKLIST）**：50+ shell builtins（`cd`, `echo`, `export`, `kill`, `test`, `true`/`false` 等）
- **用户黑名单**：`~/.config/loci/blacklist`，每行一个工具名，`#` 为注释，空行跳过
- 检查顺序：先 DEFAULT_BLACKLIST，再用户黑名单

### 平台检测（`platform.rs`）

- **Unix**：`metadata.permissions().mode() & 0o111 != 0` — 检查任一执行权限位
- **Windows**：读取 `PATHEXT`（默认 `.EXE;.BAT;.CMD;.COM;.PS1`），检查文件扩展名
- 两种平台均先验证 `metadata.is_file()`

### 进程启动（`main.rs:launch`）

- **Unix**：`Command::new(tool).args(args).exec()` — 替换当前进程，仅错误时返回
- **Windows**：`Command::new(tool).args(args).status()` — 创建子进程并等待

### LOCI_PATH_EXTRA

环境变量 `LOCI_PATH_EXTRA` 可追加扫描目录（`:` 分隔，Windows 用 `;`），不污染原始 `PATH`。

### 列表模式过滤（`main.rs:fuzzy_match`）

`loci -l <filter>` 使用简单的大小写不敏感子串匹配（`to_lowercase().contains()`），不是模糊匹配。真正的模糊匹配仅在交互式 skim TUI 中可用。

## 功能特性

### 元数据层（`--meta`，v0.2.0+）

`loci -l --json --meta` 在 JSON 输出中为每工具增加 `meta` 字段：

| 字段 | 来源 | 示例 |
|------|------|------|
| `version` | 运行 `tool --version`（3s 超时） | `"git version 2.52.0"` |
| `category` | `metadata.rs:infer_category()` 名称模式推断 | `"scm"`, `"python"`, `"container"` |
| `tags` | 内置 category + 用户 `tags.json` 合并 | `["scm", "devops"]` |
| `path` | 在 PATH 目录中解析完整路径 | `"/usr/bin/git"` |

### 版本探测细节（`metadata.rs`）
- 先试 `--version`，再试 `-V`，返回首条非空行（<120 字符）
- 每次探测超时 3 秒，每 50ms 轮询一次子进程状态
- **VERSION_PROBE_BLACKLIST**：`gitk`、`git-gui`、`gvim` — 这些 GUI 工具即使传 `--version` 也会打开窗口，因此跳过探测
- Windows 上使用 `CREATE_NO_WINDOW` (0x08000000) 阻止控制台弹窗

### 元数据模式细分（`main.rs`）
- `explicit_meta`：用户显式传递 `--meta` → 触发版本探测（昂贵，每工具一个子进程）
- `meta_mode`：`explicit_meta || tag_filter.is_some()` → 启用类别推断和标签，但仅当 `explicit_meta` 为 true 时才探测版本
- `--tag` 自动启用元数据模式，但**不**触发版本探测 — 只有同时加 `--meta` 才探测版本
- `meta_cache` 在 name_filter 后一次计算，同时用于 tag 过滤和 JSON 输出，避免 `infer_category()` 重复执行

### 标签系统（`--tag`，v0.2.0+）

```bash
loci -l --json --tag scm       # 只列出版本控制工具
loci -l --json --tag python    # 只列出 Python 生态工具
```

- **内置标签**：来自 `metadata.rs:infer_category()` 的静态规则引擎，覆盖 scm/container/python/node/compress/network/editor/rust/go/database 共 10 个类别
- **用户扩展**：`~/.config/loci/tags.json` 格式 `{"tool-name": ["tag1", "tag2"]}`，用户标签合并到内置标签
- `--tag` 自动启用元数据模式（即使不加 `--meta`）
- 当 `--tag` 与 `--meta` 同时使用时，输出同时包含版本探测和标签数据

## 错误处理与鲁棒性模式

以下是代码库中贯彻的鲁棒性设计模式，修改时需保持一致：

### 原子持久化
所有磁盘写入（`cache.json`、`usage.json`）使用 tmp + rename 模式，防止写入中断导致文件损坏：
```rust
let tmp = path.with_extension("json.tmp");
let _ = std::fs::write(&tmp, &data);
let _ = std::fs::rename(&tmp, &path);
```

### 子进程生命周期管理
- **版本探测超时**：3s 超时 + 50ms 精细化轮询，超时后 `kill()` + `wait()` 确保回收
- **GUI 工具守卫**：`VERSION_PROBE_BLACKLIST` 阻止对 gitk/gvim 等 GUI 工具的 `--version` 探测
- **Windows 静默启动**：`CREATE_NO_WINDOW` (0x08000000) + `stdin(Stdio::null())` 阻止探测子进程的控制台弹窗
- **避免二次收割**：`try_wait()` 返回 `Ok(Some(_))` 后子进程已被收割，必须用 `child.stdout.take()` 读 pipe，不能调用 `wait_with_output()`
- **exit code 传播**：Windows 上 `status.code()` 返回 `None` 时（进程被 `TerminateProcess`）输出诊断信息而非静默 exit 1

### 扫描容错
- **不可读目录**：`read_dir()` 错误时 `continue`，不中断全盘扫描
- **非 UTF-8 文件名**：检测 `to_string_lossy()` 产生的 `U+FFFD` 替换字符，跳过并警告
- **不存在 PATH 目录**：`retain(|d| d.is_dir())` 静默过滤（PATH 通常包含很多不存在的目录）
- **LOCI_PATH_EXTRA 路径验证**：用户手动配置的路径逐一检查存在性，不存在的输出 `eprintln!` 警告

### 序列化回退
JSON 输出（`to_string_pretty`）失败时降级为紧凑格式（`to_string`），避免 panic 退出。

### 不变式保护
tag_filter 存在时 meta_cache 必须为 Some，通过 `debug_assert!` 在 debug 模式下验证。release 模式保留子串匹配降级分支作为双保险。

## 项目模式（`--project`，v0.2.0+）

`loci -l --project` 只列出当前项目的本地工具。自动检测（`scanner.rs:detect_project_dirs`）：

| 项目类型 | 检测标志 | 扫描目录 |
|----------|----------|----------|
| Node.js | `package.json` | `node_modules/.bin/` |
| Python venv | `pyvenv.cfg` | `.venv/bin/` 或 `venv/bin/` (Unix) / `Scripts` (Win) |
| Rust | `Cargo.toml` | `target/debug/` + `target/release/` |
| Conda | `$CONDA_PREFIX` | `$CONDA_PREFIX/bin/` |

不走磁盘缓存，可叠加 `--json`/`--meta`/`--tag`。

### 程序化选择（v0.2.0+）

用于 CI/CD 和 AI Agent 自动化场景的无交互选择：

| 参数 | 行为 | 匹配失败时 |
|------|------|-----------|
| `--index N` | 选择列表中第 N 个（0-based） | exit 1 + "index out of range" |
| `--pick-first` | 多个匹配时选第一个（字母序） | exit 1 + "no matching tool" |
| `--exact` | 精确匹配（区分大小写全等） | exit 1 + "exact match not found" |

支持参数透传：`loci --index 0 python -- --version`

### 使用频率排序（`--sort`，v0.2.0+）

| 参数 | 行为 |
|------|------|
| `--sort alpha` | 字母序（默认） |
| `--sort freq` | 按使用频率降序 → 最近使用 → 字母序 |

- 数据持久化到 `~/.local/share/loci/usage.json`（原子写入，与缓存相同的 tmp+rename 模式）
- `record_usage()` 在交互模式和程序化选择模式中，`launch()` 之前调用
- `usage.rs` 自行实现 UTC 时间戳和公历计算（无 chrono 依赖）：`from_unix_secs()` 含闰年判断

### JSON 输出格式

```json
{
  "skill_version": "v0.2.0",
  "total": 3,
  "executables": ["cargo", "git", "python"],
  "filter": "git",
  "project": true,          // 仅 --project 时出现
  "tag_filter": "scm",      // 仅 --tag 时出现
  "meta": {                  // 仅 --meta 或 --tag 时出现
    "git": {
      "version": "git version 2.52.0",
      "category": "scm",
      "tags": ["scm"],
      "path": "/usr/bin/git"
    }
  }
}
```

- `skill_version` 始终存在（编译时常量 `env!("CARGO_PKG_VERSION")`）
- `meta` 仅在 `--meta`/`--tag` 标志时出现；`meta` 内的 `version` 仅当显式 `--meta` 时出现
- 向后兼容：不加 `--meta` 的旧解析代码不受影响

## 依赖

| Crate | 用途 |
|-------|------|
| `skim` (features: `frizbee`) | 模糊查找 TUI（Rust 原生 fzf） |
| `dirs` | 跨平台配置/缓存/数据目录解析 |
| `serde` / `serde_json` | 缓存序列化 + JSON 输出 |
| `sha2` | PATH 指纹缓存失效检测 |

其余全部使用 Rust 标准库（进程启动、文件系统扫描、参数解析、UTC 时间戳计算）。

## 发布流程

推送 `v*` tag 触发 `.github/workflows/release.yml`：
1. **test** — ubuntu/macos/windows 上 `cargo test --release`
2. **build** — 为 5 个目标交叉编译（linux-x64, linux-arm64, macos-x64, macos-arm64, windows-x64），上传 artifact
3. **release** — 下载所有 artifact，生成 checksums.txt，创建 GitHub Release

npm 包 `@yaemikoreal/loci` 的 `postinstall`（`npm/install.js`）自动从 GitHub Releases 下载对应平台二进制。`npm/bin/loci.js` Node 封装脚本通过 `spawnSync` 透传参数，`stdio: 'inherit'`。版本号从 `package.json` 动态读取（`process.env.npm_package_version`），无需手动同步。

## 用户文档

完整操作手册（安装、配置、JS 输出格式、故障排除）见 [`docs/usage.md`](./docs/usage.md)。  
AI Agent 集成协议见 [`SKILL.md`](./SKILL.md) 和 [`AGENTS.md`](./AGENTS.md)。

## 常用开发命令

```bash
cargo build --release              # 发布构建（LTO + 压缩 + strip）
cargo test                         # 运行全部测试
cargo test -- --test-threads=1     # 串行（避免 usage 测试冲突）
cargo test metadata::tests         # 只跑 metadata 模块测试
cargo test --test cli              # 只跑集成测试
cargo test parse_args_default      # 按名称过滤单个测试
cargo clippy                       # 代码风格检查（需要 nightly 或 clippy）
cargo install --path .             # 安装到 ~/.cargo/bin/
cargo run -- -l --json             # 直接运行（不安装）
```

## 使用场景速查

```bash
loci                              # 交互式模糊查找器
loci git                          # 预过滤后进入 TUI
loci -l                           # 列出所有可执行文件（文本）
loci -l --json                    # JSON 输出（Agent 首选）
loci -l --json git                # JSON + 关键词过滤
loci -l --json --meta             # JSON + 元数据（含版本探测）
loci -l --json --tag scm          # JSON + 标签过滤（无版本探测）
loci -l --json --tag scm --meta   # JSON + 标签过滤 + 版本探测
loci -l --project --json          # 项目本地工具（JSON）
loci -l --sort freq               # 按使用频率排序
loci --exact python.exe           # 精确匹配直接启动
loci --pick-first git             # 选第一个匹配启动
loci --index 0 python -- --version # 选第 N 个 + 参数透传
loci git -- log --oneline         # 预过滤 + 参数透传
```
