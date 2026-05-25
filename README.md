# ADHD Coder

并行跑多个 AI 编码对话时，屏幕角落的置顶小窗会列出当前所有会话的进度——谁还在响应、谁已完成、你看过哪些。点击一条直接跳到对应终端窗口/分屏。

技术栈：Tauri (Rust + Webview)，打包 ~5MB。

## 功能

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

```bash
bash scripts/install-claude-hook.sh
```

会做两件事：
- 把 `scripts/adhd-report.py` 复制到 `~/.adhd-coder/report.py`
- 往 `~/.claude/settings.json` 写入 `UserPromptSubmit` + `Stop` 两个 hook，分别对应「开始响应」和「响应完成」上报

幂等：重复跑会覆盖旧版本。

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

固定 `127.0.0.1:7777`。被占用时启动失败，终端会打日志，不阻断窗口。

## 数据

任务持久化在 `~/.adhd-coder/tasks.json`，重启后仍在。

## 下一步（非 MVP）

- Codex Desktop 接入（macOS 通知拦截 / 窗口标题轮询）
- 浏览器扩展（claude.ai / chatgpt.com / aistudio）
- VSCode/Cursor 扩展：精确聚焦到具体 terminal pane（需要扩展进程响应 IPC）
- 端口可配置 / 冲突自动回退
