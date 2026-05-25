# ADHD Coder

并行跑多个 AI 编码对话时，屏幕角落的置顶小窗会列出所有"已完成、待你确认"的任务。点一下勾掉。

技术栈：Tauri (Rust + Webview)，打包 ~5MB。

## MVP 覆盖

| 场景 | 接入方式 | 状态 |
| --- | --- | --- |
| Claude Code CLI | Stop hook → curl localhost | ✅ |
| Claude Code IDE 插件 (VSCode/JetBrains) | 复用同一个 hook | ✅ |
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

### Claude Code（CLI 或 IDE 插件）

```bash
bash scripts/install-claude-hook.sh
```

会往 `~/.claude/settings.json` 写一个 Stop hook，每次对话结束自动上报。

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

## 数据

任务持久化在 `~/.adhd-coder/tasks.json`，重启后仍在。

## 端口

固定 `127.0.0.1:7777`。被占用时启动会失败 — 终端会打印日志。

## 下一步（非 MVP）

- Codex Desktop 接入（macOS 通知拦截）
- 浏览器扩展（claude.ai / chatgpt.com）
- 点击任务项 → 聚焦上报来源的终端窗口
