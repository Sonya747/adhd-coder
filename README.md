# ADHD Coder

并行跑多个 AI 编码对话时，屏幕角落的置顶小窗会列出当前所有会话的进度——谁还在响应、谁已完成、你看过哪些。点击一条直接跳到对应终端窗口/分屏。

技术栈：Tauri (Rust + Webview)，打包 ~5MB。

## 功能

### 功能概述

- **统一收件箱**：把分散在 Claude Code / Codex CLI / 自定义脚本里的「跑完了」事件汇总到一个置顶悬浮窗
- **三态可视化**：转圈（响应中）/ 亮绿（完成未看）/ 暗灰（已看），扫一眼就知道哪个该理
- **点击即跳**：iTerm2 / Terminal 精准到 split/tab，tmux 自动切 pane，VSCode / Cursor 按项目名抬窗
- **首启引导**：开盖即用,自动写 hook + 引导授权辅助功能,无需手敲脚本
- **端口冲突自动回退**：7777 被占时按序尝试 7778-7780,UI 与 reporter 自动跟进
- **会话去重**：同一 session 多轮活动复用同一行,不同 session 各占一行
- **持久化**：任务列表存 `~/.adhd-coder/tasks.json`，重启不丢
- **轻量**：Tauri 打包 ~5MB，纯 JS 前端无框架

### 任务三态

| 图标 | 状态 | 含义 |
| --- | --- | --- |
| 🟡 转圈 | responding | 用户刚提交、Claude 正在回复 |
| ✅ 亮绿 | done · 未查看 | 已完成，等你瞥一眼。同时弹系统通知 |
| ✓ 暗灰 | done · 已查看 | 你已点击查看过，整行变暗，不再抢注意力 |

同一个 session 后续新提交会复用同一行（spinner 重新转起来，已查看状态自动重置）；不同 session 各占一行。

### 点击行为

- **点击任务行** → 试图把对应终端窗口/分屏拉到前台，并标记为已查看
- **悬停行右侧 ×** → 删除该条任务
- **标题栏 ✕** → 清空整个列表

### 终端聚焦覆盖度

点击任务后能跳到哪一级，取决于宿主终端：

| 终端 | 能跳到 |
| --- | --- |
| iTerm2（含 splits、tabs） | 精确到具体 split/tab ✅ |
| Apple Terminal（tabs） | 精确到 tab ✅ |
| tmux pane（无论外层是哪个终端） | 自动 `select-window` + `select-pane` ✅ |
| iTerm2 + tmux 套用 | 先切 pane 再选中外层 session ✅ |
| Cursor / VSCode | 按窗口标题匹配项目名，抬对应项目窗口到前台 ⚠️（窗口内的具体 terminal pane 要自己 `Ctrl+\`` 切换） |
| Warp / Hyper / 其他终端 | 只标记已查看，不激活窗口 |

### 窗口控制

- 置顶悬浮、无边框、毛玻璃
- 拖标题栏左半移动；右下角斑纹手柄缩放
- 标题栏 `—` 隐藏；菜单栏托盘图标左键切换显隐、右键菜单 / dock 图标点击 也都能唤回
- 任务持久化在 `~/.adhd-coder/tasks.json`，重启不丢

## MVP 覆盖

| 场景 | 接入方式 | 状态 |
| --- | --- | --- |
| Claude Code CLI | Stop + UserPromptSubmit hook | ✅ |
| Claude Code IDE 插件 (VSCode/JetBrains/Cursor) | 复用同一个 hook | ✅ |
| Codex CLI / 任意 CLI | `adhd-wrap` 包裹脚本 | ✅ |
| 通用脚本/进程 | POST `http://127.0.0.1:7777/done` | ✅ |
| Codex Desktop / Web 端 | v2 再做 | ⏳ |

## 准备

1. 装 Rust：`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
2. 装前端依赖：`npm install`

## 开发运行

```bash
npm run start          # = tauri dev，会一起起 vite + 编译 rust + 开窗口
```

首次编译 Rust 大约 3-5 分钟。

## 打包

```bash
npm run tauri build
```

产物在 `src-tauri/target/release/bundle/`。

## 接入

### Claude Code（CLI / IDE 插件 / Cursor）

**首次启动 ADHD Coder 时会弹出引导窗口**,点「安装 hook」即可,会自动：
- 把上报脚本写到 `~/.adhd-coder/report.py`
- 往 `~/.claude/settings.json` 注册 `UserPromptSubmit` + `Stop` 两个 hook,分别对应「开始响应」和「响应完成」上报

需要重装或在新机器上手动操作时,可走脚本（开发者用途）：

```bash
bash scripts/install-claude-hook.sh
```

幂等：重复跑会覆盖旧版本。也可在托盘菜单选「重新运行引导」回到引导窗。

### Codex CLI / 任意命令

```bash
# 把 scripts/adhd-wrap 放到 PATH 里
cp scripts/adhd-wrap /usr/local/bin/

# 用法
adhd-wrap codex
adhd-wrap --summary "构建完成" -- npm run build
```

### 自定义脚本

```bash
curl -X POST http://127.0.0.1:7777/done \
  -H 'Content-Type: application/json' \
  -d '{"project":"my-proj","summary":"测试通过"}'
```

完整字段（皆可选）：

```json
{
  "project": "my-proj",
  "summary": "用户输入或完成描述（80 字内）",
  "session_id": "用于同一会话内更新同一行",
  "status": "responding | done",
  "terminal": {
    "program": "iTerm.app | Apple_Terminal | cursor | vscode | ...",
    "iterm_session": "$ITERM_SESSION_ID",
    "term_session": "$TERM_SESSION_ID",
    "tmux_pane": "$TMUX_PANE",
    "tmux_socket": "$TMUX 拆出的 socket 路径",
    "tty": "/dev/ttysXXX"
  }
}
```

## macOS 权限提示

点击任务激活终端会用到 AppleScript / System Events，首次会触发系统弹窗：

- **首次点 iTerm2 / Terminal.app 任务** → 「ADHD Coder 想控制 iTerm2/Terminal」→ 允许
- **首次点 Cursor / VSCode 任务** → ① 「想控制 Cursor/Code」允许 + ② 系统设置 → 隐私与安全性 → **辅助功能** → 勾上 ADHD Coder（System Events 需要）

## 端口

默认 `127.0.0.1:7777`。被占用时按序回退 7778 → 7779 → 7780，实际端口写到 `~/.adhd-coder/port`，`report.py` 和 `adhd-wrap` 会自动读取。四个都被占才放弃监听（窗口仍可用，只是不再接收上报）。

## 数据

任务持久化在 `~/.adhd-coder/tasks.json`，重启后仍在。

## 下一步（非 MVP）

- Codex Desktop 接入（macOS 通知拦截 / 窗口标题轮询）
- 浏览器扩展（claude.ai / chatgpt.com / aistudio）
- VSCode/Cursor 扩展：精确聚焦到具体 terminal pane（需要扩展进程响应 IPC）
