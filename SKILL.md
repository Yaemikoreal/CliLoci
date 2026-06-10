---
name: loci
description: "A minimalist CLI launcher — list, filter, and jump to any executable on your PATH. Designed for AI agents to discover and launch CLI tools on the user's system."
trigger: |
  When the user asks to:
  - find, list, or discover CLI tools on their system
  - check if a specific tool is installed and available via PATH
  - launch or jump to another CLI tool
  - explore what executables are available
  - search for tools matching a keyword (e.g. "tools related to git", "find python tools")
  - enumerate all available commands
  - fast tool switching / command palette behavior

  Also triggered when:
  - The agent needs to verify a tool's existence before suggesting or using it
  - The agent needs to discover what tools are available for a task
  - The context requires understanding the user's CLI environment
type: tool
version: "0.2.1"
---

# loci — AI  CLI Tool Discovery Skill

## 概述

`loci` 扫描用户 `PATH` 下的所有可执行文件，提供**即时模糊搜索**和**快速跳转**能力。  
本技能将 `loci` 封装为 AI Agent 的"工具发现层"——Agent 可以通过 `loci` 快速了解当前环境中有哪些 CLI 工具可用，无需记忆或猜测。

**设计哲学**: 只列出，只跳转。不管理版本、不安装包、不记忆别名。

## 何时触发

| 场景 | 触发词（用户说） | 优先级 |
|------|------------------|--------|
| 工具发现 | "有哪些 cli 工具", "找一下 xx 工具", "what tools do I have" | ⭐ 高 |
| 存在性检查 | "有没有 xx", "check if xx is installed", "where is xx" | ⭐ 高 |
| 快速启动 | "帮我打开 xx", "launch xx", "jump to xx" | ⭐ 中 |
| 环境探查 | "我的 PATH 里有什么", "list available commands" | ⭐ 中 |
| 关键词搜索 | "找 git 相关的工具", "python 工具有哪些" | ⭐ 中 |

## AI 使用协议

### 通信格式

Agent 与 `loci` 之间的交互应遵循以下模式：

```
# 发现：获取所有工具列表（程序化）
loci -l [--json]

# 搜索：按关键词过滤（程序化）
loci -l <keyword>

# 启动：交互式选择器（需人类参与）
loci [filter]

# 检查：验证工具是否存在
loci -l <tool-name> | grep <tool-name>
```

### 输出规范

**文本模式** (`loci -l`)：每行一个工具名，按字母序排列，可直接 `grep`/`head`/`tail`/`wc -l` 处理。

```
$ loci -l
7z
7za
7zr
...
```

**JSON 模式** (`loci -l --json`)：结构化输出，适合 Agent 程序化解析：

```json
{
  "skill_version": "v0.2.1",
  "total": 142,
  "executables": ["7z", "7za", "7zr", ...],
  "filter": null
}
```

搜索时：

```json
{
  "skill_version": "v0.2.1",
  "total": 3,
  "executables": ["cargo", "cargo-clippy", "cargo-fmt"],
  "filter": "cargo"
}
```

**高级 JSON 模式**（`--meta` 或 `--tag`）：

```json
{
  "skill_version": "v0.2.1",
  "total": 3,
  "executables": ["cargo", "git", "python"],
  "filter": null,
  "tag_filter": "scm",
  "meta": {
    "git": {
      "version": "git version 2.52.0",
      "category": "scm",
      "tags": ["scm"],
      "path": "/usr/bin/git"
    }
  }
}
```

### Agent 能力约束

| 操作 | 自主执行 | 需确认 |
|------|----------|--------|
| `loci -l --json` 列出工具 | ✅ 无需询问 | — |
| `loci --exact <name>` 自动启动 | ❌ | 必须先告知用户要启动哪个工具 |
| `loci --pick-first <name>` 自动启动 | ❌ | 必须先告知用户 |
| `loci --index N <name>` 自动启动 | ❌ | 必须先告知用户 |
| `loci` 交互模式 TUI | ✅ 需说明"正在打开工具选择器" | — |
| `loci --meta` 版本探测 | ✅ 只读操作，不会修改系统 | — |
| `loci --project` 项目模式 | ✅ 无需询问 | — |
| `loci -l` 列表模式 | ✅ 无需询问 | — |

Agent **不得**在未告知用户的情况下自动执行选中的工具。所有自动启动方式（`--exact`、`--pick-first`、`--index`）必须让用户知情并确认。

### 缓存策略

`loci` 自动缓存扫描结果，以 SHA-256（PATH 目录 + mtime）为指纹。  
**Agent 应知道**：首次调用会扫描全部 PATH（~100ms-2s，取决于工具数量），后续同 PATH->sub-ms。  
Agent 不需要手动管理缓存。如需强制刷新，告诉用户先运行 `rm -rf ~/.cache/loci` 或等 PATH 变化自动触发。

## 使用模式

### 模式 1：程序化发现（Agent 首选）

```python
# Agent 内部逻辑示例
import subprocess, json

# 获取所有工具
result = subprocess.run(["loci", "-l", "--json"], capture_output=True, text=True)
data = json.loads(result.stdout)
tools = data["executables"]

# 搜索 git 相关工具
result = subprocess.run(["loci", "-l", "--json", "git"], capture_output=True, text=True)
git_tools = json.loads(result.stdout)["executables"]
```

### 模式 2：交互式选择（人类参与）

当 Agent 启动 `loci` 时不加 `-l`/`--list`，会弹出 skim TUI 预览界面，用户可：

1. 输入关键词模糊过滤
2. 用 `↑`/`↓` 选择
3. `Enter` 确认启动
4. `Esc` 取消

### 模式 3：参数透传

```
# 预过滤 + 参数转发
loci git -- log --oneline

# 仅转发参数（无预过滤）
loci -- log --oneline

# 预过滤后进入 TUI
loci git
```

详见 [loci CLI 参考](#cli-参考)。

## 为 Agent 扩展配置

### 黑名单

Agent 可以建议用户创建 `~/.config/loci/blacklist`，每行一个工具名，`#` 为注释：

```
# 对 Agent 无用的系统命令
aa-enabled
sensors
```

内置默认黑名单已过滤 shell builtins（`cd`、`echo`、`export` 等）。

### 扩展扫描路径

```sh
export LOCI_PATH_EXTRA="$HOME/.local/bin:$HOME/go/bin"
```

### 虚拟环境

虚拟环境（Python venv、conda env、npm global）在**激活后**会自动加入 `PATH`，loci 可直接扫到。
如需扫描未激活的虚拟环境，将其 `bin/`（Unix）或 `Scripts/`（Windows）目录加入 `LOCI_PATH_EXTRA`：

```sh
export LOCI_PATH_EXTRA="$HOME/projects/myapp/.venv/bin:$LOCI_PATH_EXTRA"
```

Agent 可以在检测到常见工具目录（如 `~/.cargo/bin`、`~/.local/bin`、`~/go/bin`）不在 `PATH` 中时，提示用户添加。

## CLI 参考

```
loci                           → 交互式模糊查找器
loci <filter>                  → 预过滤后进入交互式查找器
loci -l                        → 打印所有可执行文件（无 TUI）
loci -l <filter>               → 打印匹配的可执行文件
loci -l --json                 → JSON 格式输出（含 skill_version）
loci -l --json <filter>        → JSON 格式输出（带过滤）
loci -l --json --meta          → JSON + 元数据（版本/类别/路径）
loci -l --json --meta <filter> → JSON + 元数据（带过滤）
loci -l --json --tag scm       → JSON + 按标签 scm 过滤（自动含元数据）
loci -l --json --tag scm git   → JSON + 标签 + 关键词过滤
loci -l --project              → 只列出当前项目本地工具
loci -l --project --json       → 项目工具（JSON 格式）
loci --exact <filter>          → 精确匹配后直接启动
loci --pick-first <filter>     → 选第一个匹配启动
loci --index N <filter>        → 选第 N 个（0-based）匹配启动
loci <filter> -- <args...>     → 预过滤 + 参数透传给选中工具
loci -- <args...>              → 参数透传
```

## 安装（Agent 应如何引导用户）

AI Agent 应优先推荐最简洁的安装方式：

```
# 从源码构建（需 Rust，推荐）
cargo install --path .

# 或从 release 下载二进制
# https://github.com/Yaemikoreal/CliLoci/releases
```

**Agent 应检查**：安装后运行 `loci -l --json` 验证是否成功，返回非空 JSON 即为成功。

## 故障处理

| 问题 | Agent 应如何响应 |
|------|------------------|
| `loci: no executables found in PATH` | PATH 可能为空或所有目录不可访问。检查 `echo $PATH`。 |
| `loci: command not found` | 未安装。引导用户安装（见上方安装章节）。 |
| JSON 输出为空数组 | 检查 `loci -l` 文本模式是否正常输出。IO 重定向可能有问题。 |
| 工具不在列表中 | 该工具不在 PATH 中，或扩展名不被识别（Windows 需在 PATHEXT 中）。 |

## 与人类协作说明

当 Agent 使用 `loci` 时：

- **Agent 自主决定**：用 `loci -l --json` 安静地获取工具列表并解析，不需要询问用户
- **需要告知用户**：发现了哪些相关工具、推荐使用哪个
- **交互启动**：`loci`（无参数）会弹出 TUI，Agent 应说明"正在打开工具选择器，请选择要启动的工具"

---

> **Skill 版本**: 0.2.1 | **项目主页**: https://github.com/Yaemikoreal/CliLoci
