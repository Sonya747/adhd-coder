# M2 计划：团队分发

> 目标：把 M1 跑通的工具变成「队友下载一个 DMG，开盖即用」的形态。**不做代码签名 / 公证**。

## 范围边界

| 包含 | 不包含（留 M3+） |
| --- | --- |
| DMG 打包、GitHub Releases | 浏览器扩展（claude.ai / chatgpt.com） |
| 首启引导自动装 hook + 辅助功能授权 | Codex Desktop 通知拦截 |
| 端口冲突自动回退 | 按项目分组 / 等待时长徽章 / 历史归档 |
| 团队安装与使用文档 | VSCode / Cursor 扩展精确聚焦 pane |

## 里程碑

按依赖顺序，自上而下推进。每个里程碑独立可验收，可分别 commit。

### M2.1 · 端口冲突优雅处理

**问题**：当前 `127.0.0.1:7777` 被占用时只在终端打日志，UI 完全失语，hook 也照打 7777。

**改动**：
- Rust：依次尝试 7777 → 7778 → 7779 → 7780，记下实际端口
- 启动后把端口写到 `~/.adhd-coder/port`
- `report.py` 和 `adhd-wrap`：读取 `~/.adhd-coder/port`，回退默认 7777
- 标题栏 hover 显示监听端口（轻量，避免视觉污染）

**验收**：手动 `python3 -m http.server 7777` 占住端口，然后跑 ADHD Coder，仍能上报、UI 不报错。

### M2.2 · 首启引导

**问题**：现在新用户要手动跑 `bash scripts/install-claude-hook.sh`，DMG 用户根本看不到这个脚本。

**改动**：
- 内置一份 `report.py` 和 install 逻辑到 Rust 二进制里（资源文件嵌入）
- 首次启动检测 `~/.adhd-coder/.installed` 不存在 → 弹引导窗口：
  - 步骤 1：「安装 Claude Code hook」按钮 → 调用 Tauri 命令把内嵌的 `report.py` 写到 `~/.adhd-coder/`，并修改 `~/.claude/settings.json`
  - 步骤 2：「授权辅助功能」按钮 → `osascript` 打开「系统设置 → 隐私与安全性 → 辅助功能」
  - 步骤 3：「完成」→ 写入 `.installed` flag
- 引导窗口可重新打开：托盘菜单加「重新运行引导」

**验收**：删掉 `~/.adhd-coder/.installed` 和 `~/.claude/settings.json` 的 Stop hook，重启 app，引导窗口出现，走完一遍后 hook 生效。

### M2.3 · 图标 / Dock 名称收尾

**改动**：
- `npx @tauri-apps/cli icon assets/source.png` 生成全套 .icns / .ico / png
- 校对 `tauri.conf.json` 的 `productName`、`identifier`
- App 在 dock 显示 "ADHD Coder"，菜单栏托盘图标合宜（必要时单独做一张 16x16 模板图）

**验收**：DMG 装的 app 在 Launchpad、Spotlight、Dock 都显示对的名字和图标。

### M2.4 · 本地打包跑通

**改动**：
- 跑 `npm run tauri build`
- 测试：从生成的 `.dmg` 拖到 Applications，启动，端到端走一遍（开 Claude Code → 看到任务 → 点击聚焦）
- 修一切因为打包模式才暴露的问题（资源路径、entitlements 等）

**验收**：在一台没装 Rust 的 mac 上从 DMG 装能跑。

### M2.5 · GitHub Actions 自动 Release

**改动**：
- `.github/workflows/release.yml`
- 触发：push tag `v*`
- 双 runner：`macos-14`（Apple Silicon）+ `macos-13`（Intel） → 各产一份 DMG
- 步骤：checkout → setup-node → rustup → npm ci → tauri build → upload-artifact → release-action

**验收**：`git tag v0.2.0 && git push --tags` → 几分钟后 Release 页面出现两份 DMG 可下。

### M2.6 · 团队安装与使用文档

**交付物**：`docs/INSTALL.md`（骨架已先建好，M2.5 出第一个 Release 后填实际下载链接）

**章节**：
1. 系统要求（macOS 12+）
2. 下载（Releases 链接，区分 arm64 / x86_64）
3. 首次打开：**右键 → 打开**（因为没签名，Gatekeeper 会拦）
4. 首启向导（自动装 hook + 辅助功能授权）
5. 日常使用（三态、点击行为、托盘菜单、隐藏/显示）
6. 高级：用 `adhd-wrap` 给 Codex 等 CLI 接入；自定义脚本 POST `/done`
7. 常见问题（端口被占、辅助功能没生效、想升级、想卸载）

### M2.7 · README 更新

- 顶部加「**给队友：直接下载**」一行 + Releases 链接
- 现有「开发运行」收进「贡献者」小节
- 链到 `docs/INSTALL.md`

## 排期建议

并行/串行分组：

```
Phase 1 (基础体验)       :  M2.1 → M2.2
Phase 2 (能打包出 app)    :  M2.3 → M2.4
Phase 3 (能分发能让人会用) :  M2.5 → M2.6 → M2.7
```

每个 Phase 跑完 commit、自测一遍再进入下一 Phase，避免一气把锅都熬糊。

## 风险

| 风险 | 应对 |
| --- | --- |
| 未签名 DMG 队友打不开 / Gatekeeper 拦 | 文档明确「右键 → 打开」+ 第一次会有「来自未识别开发者」弹窗，点继续即可 |
| 辅助功能授权对小白不友好 | 引导步骤 2 直接 osascript 打开正确的设置页面，少一步操作 |
| GH Actions 编译 Tauri 慢 / 缓存命中差 | 加 `Swatinem/rust-cache`，首跑 15 分钟后续 5 分钟 |
| 两份 DMG 让队友不知道下哪个 | 文件名带 `-arm64` / `-x64`，README 标明「M 系列芯片选 arm64」 |

## 完成定义

- [ ] `v0.2.0` Release 在 GitHub 上可下
- [ ] 一位队友按 `docs/INSTALL.md` 从零装好，能跑通：开 Claude Code → 看到任务 → 点击跳转
- [ ] 队友反馈整理回 issue / 决定 M3 优先级
