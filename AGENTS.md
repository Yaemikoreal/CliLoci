# Agent 使用指南 — loci Skill

> 本文件指导 AI Agent 如何加载并使用 `loci` 技能来发现和启动 CLI 工具。

## 技能加载

当用户的需求涉及 CLI 工具发现、查找、启动时，AI Agent 应加载本仓库作为技能包：

1. **加载入口**: 读取 [`SKILL.md`](./SKILL.md) 获取完整的技能协议
2. **权限约束**: Agent 必须遵守 SKILL.md 中的[Agent 能力约束表](./SKILL.md#agent-能力约束)，自动启动工具前必须先告知用户
3. **二进制依赖**: 确保 `loci` 已安装在用户系统上（`which loci` / `where loci`）
4. **首次使用**: 先运行 `loci -l --json` 预热缓存

## Agent 工作流

### 场景 1：用户问「有没有 XX 工具」

```
1. 运行 loci -l --json
2. 解析 JSON 获取 executables 数组
3. 检查目标工具是否在数组中
4. 告知用户：该工具已安装 / 未安装（可提供安装建议）
```

### 场景 2：用户问「有哪些工具可以做 YY 任务」

```
1. 运行 loci -l --json <keyword>   # 按关键词过滤
2. 从 JSON 输出读取总数量和工具列表
3. 按工具分类或用途向用户推荐
```

### 场景 3：用户说「帮我启动 XX 工具」

```
1. 运行 loci -l --json  确认该工具存在
2. 告知用户准备启动
3. 运行 loci <tool_name>    # 可选 -- 转发参数
```

### 场景 4：用户想浏览系统中可用的工具

```
1. 运行 loci -l --json 获取全部工具
2. 如果数量多（>50），按首字母或类别分组展示
3. 建议用户使用 loci 交互模式自行浏览
```

## 输出格式化建议

从 `loci -l --json` 获取数据后，Agent 应转换为友好格式：

```python
# 推荐：向用户展示时总结信息
total = data["total"]
tools = data["executables"]

if total > 20:
    print(f"系统中共有 {total} 个可用工具。以下是一些分类示例：")
    # 分组展示
else:
    print(f"找到以下相关工具（共 {total} 个）：")
    for t in tools:
        print(f"  • {t}")
```

## 跨平台注意事项

| 平台 | 注意事项 |
|------|----------|
| Linux/macOS | `loci` 自动检测可执行权限位，无需额外配置 |
| Windows | 识别 `PATHEXT` 中的扩展名（`.exe`、`.bat`、`.cmd`、`.ps1` 等） |

## 故障处理

| 现象 | Agent 响应 |
|------|-----------|
| `loci: command not found` | 引导用户安装（npm install / cargo install / 下载 release） |
| 输出为空 | PATH 可能为空，检查 `echo $PATH` |
| JSON 解析失败 | 降级到 `loci -l` 文本模式，逐行读取 |

---

**版本**: 0.2.1 | **相关文件**: [`SKILL.md`](./SKILL.md), [`README.md`](./README.md)
