# ADHD Coder · 安装与使用指南

> 给团队队友。如果你只想用、不想读源码，本文足够。
> 状态：**M2 完成前为骨架**，下载链接和截图等 v0.2.0 发布后回填。

## 系统要求

- macOS 12 (Monterey) 或更高
- 双架构都有：Apple Silicon (M1/M2/M3/M4) 选 `arm64`，Intel Mac 选 `x64`

## 1. 下载

去 [Releases 页面](https://github.com/Sonya747/adhd-coder/releases/latest) 下最新版：

- `ADHD-Coder_<version>_aarch64.dmg` — M 系列芯片
- `ADHD-Coder_<version>_x64.dmg` — Intel

## 2. 第一次打开（重要！）

> ⚠️ 本 app **未做苹果代码签名**（公司没买 Apple Developer 账号）。直接双击会被 Gatekeeper 拦。

1. 拖 `.dmg` 里的 ADHD Coder.app 到 `/Applications`
2. **在 Applications 文件夹里右键 ADHD Coder.app → 选「打开」**（不能直接双击，至少第一次必须右键）
3. 弹窗「无法验证开发者」→ 点「**打开**」
4. 之后双击就能直接开了

如果右键也没「打开」选项，去 系统设置 → 隐私与安全性 → 拉到最下面有「**仍要打开**」按钮。

## 3. 首启引导

第一次启动会弹出引导窗口，走两步：

### 步骤 1：装 Claude Code hook

点「安装」即可。背后做的事：

- 把上报脚本部署到 `~/.adhd-coder/report.py`
- 在 `~/.claude/settings.json` 里加两条 hook：`UserPromptSubmit`（用户提交时）和 `Stop`（响应完成时）
- 如果你之前手动装过旧版本，会被无缝覆盖

⚠️ 已经开着的 Claude Code 会话需要 **重启** 才能加载新 hook（`/exit` 再 `claude`）。

### 步骤 2：授权辅助功能（macOS 系统权限）

点「打开设置」会跳到 系统设置 → 隐私与安全性 → **辅助功能**。把列表里的 **ADHD Coder** 勾上。

这一步是为了点击任务时能跳到 Cursor / VSCode 对应窗口。**只授权 iTerm2 / Terminal.app 用户也可以跳过**，但点 VSCode/Cursor 任务时只会停留在当前 app，不会切窗口。

## 4. 日常使用

### 窗口

- 拖标题栏左半移动；拖右下角斑纹手柄缩放
- 标题栏 `—` 隐藏窗口；菜单栏右上角的 ADHD 托盘图标点一下能重新呼出
- 也可以点 Dock 图标呼出

### 任务三态

| 视觉 | 含义 |
| --- | --- |
| 🟡 转圈 | Claude 正在响应你的提问 |
| ✅ 亮绿 ✓ | 已完成，等你看一眼。系统通知会响一下 |
| ✓ 暗灰 | 你已点过查看，不再抢注意力 |

同一会话连续问问题 → 同一行会刷新；不同项目 / 不同会话各占一行。

### 点击行为

- **单击任务行** → 跳到对应的终端窗口/分屏，并把这行标记为已查看
- **悬停时行右侧 ×** → 删除该行
- **标题栏 ✕** → 清空整个列表

### 终端聚焦支持度

点击任务能跳到哪一层，取决于你用什么终端：

| 终端 | 跳转精度 |
| --- | --- |
| iTerm2（含 splits、tabs） | 精确到 split / tab ✅ |
| Apple Terminal（tabs） | 精确到 tab ✅ |
| 任何终端里跑 tmux | 自动切到对应 tmux pane ✅ |
| Cursor / VSCode | 跳到对应项目窗口；具体哪个 terminal 要自己 `Ctrl+\`` ⚠️ |
| Warp / Hyper 等 | 只标记已查看，不切窗口 |

## 5. 高级：给别的工具加入上报

ADHD Coder 内置一个 HTTP 接口（`POST http://127.0.0.1:7777/done`），任何工具/脚本都能上报。

### Codex CLI / 任意 CLI

下载源码后用 `scripts/adhd-wrap` 包裹命令：

```bash
adhd-wrap codex
adhd-wrap --summary "build done" -- npm run build
```

进程退出时自动上报，summary 自动带上是否成功（`exit 0` / `exit N`）。

### 完全自定义

```bash
curl -X POST http://127.0.0.1:7777/done \
  -H 'Content-Type: application/json' \
  -d '{"project":"my-task","summary":"测试通过"}'
```

完整字段见仓库根 [README.md](../README.md#自定义脚本)。

## 6. 常见问题

### Q：app 装好了但小窗里啥也没有

A：检查三件事：

1. Claude Code 会话是 hook **安装之后**重启的吗？老会话不会加载新 hook。
2. ADHD Coder 在运行吗？（菜单栏看托盘图标）
3. 端口 7777 被占用了？打开 ADHD Coder 后看下托盘 tooltip 上的端口号（M2.1 之后会显示）。

### Q：点击 Cursor / VSCode 任务没反应

A：去 系统设置 → 隐私与安全性 → 辅助功能，确认 ADHD Coder 是**打勾**的状态。重新启动 ADHD Coder。

### Q：点击 iTerm2 任务时弹「想控制 iTerm2」

A：第一次会弹，点允许。永久生效。

### Q：升级版本

A：直接下新 DMG 覆盖 `/Applications/ADHD Coder.app`。任务列表（`~/.adhd-coder/tasks.json`）和 hook 配置都保留。

### Q：怎么彻底卸载

```bash
# 删 app
rm -rf "/Applications/ADHD Coder.app"

# 删数据
rm -rf ~/.adhd-coder

# 删 Claude Code hook（如果你不再需要任何上报）
# 编辑 ~/.claude/settings.json，移除 hooks.Stop 和 hooks.UserPromptSubmit 里包含
# ".adhd-coder/report.py" 的条目
```

### Q：怎么临时停掉上报但保留 app

把 ADHD Coder 退出（托盘菜单 → 退出）。Claude Code 的 hook 还会 POST，但服务器没人接，hook 静默失败、不会影响 Claude Code 本身。

## 反馈

bug / 建议 → [GitHub Issues](https://github.com/Sonya747/adhd-coder/issues)
