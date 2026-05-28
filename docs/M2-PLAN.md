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

**整体目标**：把 `install-claude-hook.sh` 的全部行为内化进 Rust 二进制，DMG 用户开盖即用。下面 8 个子项按依赖顺序排列，每个可独立 commit、独立验收。

依赖关系（→ 表示「依赖于」）：
```
M2.2.1 嵌入 reporter    ┐
M2.2.2 改 settings.json ┴→ M2.2.4 引导 UI ┐
M2.2.3 .installed flag                    ├→ M2.2.6 启动流程接线 → M2.2.7 托盘菜单 → M2.2.8 端到端验收
M2.2.5 辅助功能跳转                       ┘
```

---

#### M2.2.1 · 把 adhd-report.py 作为资源嵌入 + `install_reporter` 命令

**改动**：
- `tauri.conf.json` 的 `bundle.resources` 把 `../scripts/adhd-report.py` 列入
- Rust 新增 `#[tauri::command] install_reporter() -> Result<PathBuf, String>`：用 `path_resolver().resolve_resource(...)` 读出捆绑的 py，写到 `~/.adhd-coder/report.py` 并 `chmod 0o755`
- 同名文件存在时直接覆盖（hook 路径不变）

**验收**：`rm ~/.adhd-coder/report.py`，DevTools 里 `invoke('install_reporter')` 后文件出现、可执行、内容与 `scripts/adhd-report.py` 一致。

---

#### M2.2.2 · `install_claude_hook` 命令（替换掉 shell + node 那套）

**改动**：
- Rust 新增 `install_claude_hook() -> Result<(), String>`：
  - 读 `~/.claude/settings.json`（不存在则视为 `{}`）
  - 先 `cp settings.json settings.json.bak.{timestamp}`
  - 用 `serde_json::Value` 在 `hooks.UserPromptSubmit` 与 `hooks.Stop` 数组里 upsert 一条 `{ hooks: [{ type: "command", command: "python3 \"$HOME/.adhd-coder/report.py\"" }] }`
  - 去重判据沿用 shell 版：command 含 `/.adhd-coder/report.py` 或老的 `127.0.0.1:7777/done` 字符串
- 把判据和 upsert 抽成纯函数，方便后续加单测

**验收**：三种 settings.json 初态——空文件 / 有别人的 hook / 有旧版我们的 hook——执行后 Claude Code 真能跑通；并且没把别人的 hook 误删。

---

#### M2.2.3 · `.installed` flag + `get_install_status` 命令

**改动**：
- 定义 `~/.adhd-coder/.installed` 为 JSON：`{ version: "0.2.0", installed_at: <ms> }`
- Rust 命令：`get_install_status() -> { installed: bool, version: Option<String> }`、`mark_installed()`、`reset_install()`（给 M2.2.7 用）

**验收**：手动写一个 `.installed` 文件，前端 `invoke('get_install_status')` 拿到正确结果；`reset_install` 后文件消失。

---

#### M2.2.4 · 引导窗口前端（onboarding.html / onboarding.js）

**改动**：
- 新增 `src/onboarding.html` + `src/onboarding.js`，三步纯静态布局：
  - 步骤 1「安装 Claude Code hook」按钮 → `invoke('install_reporter')` + `invoke('install_claude_hook')`，成功打勾、失败展示 error
  - 步骤 2「授权辅助功能」按钮 → `invoke('open_accessibility_settings')`（在 M2.2.5 实现）
  - 步骤 3「完成」按钮 → `invoke('mark_installed')`，关闭引导窗、显示主窗
- 与主窗一致的视觉：透明背景、frosted、关闭按钮
- 不依赖 React/Vue，沿用 vanilla JS（与 CLAUDE.md 的约束一致）

**验收**：单独打开 `onboarding.html`（开发期通过 URL hash 或临时 menu 项触发）可点完三步，每步状态正确反映。

---

#### M2.2.5 · `open_accessibility_settings` 命令

**改动**：
- Rust 命令 `open_accessibility_settings()` 执行 `open "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"`
- 不等待用户实际授权（无法可靠检测，不阻塞流程）

**验收**：调用后系统设置自动打开到「辅助功能」面板。

---

#### M2.2.6 · 第二窗口配置 + 启动流程接线

**改动**：
- `tauri.conf.json` 增加 onboarding 窗口（`visible: false`、`width: 480`、`decorations: false`、`transparent: true`）
- Rust `setup` 阶段：先读 `get_install_status`，未安装则 `show()` onboarding 窗、`hide()` 主窗；已安装走原路径
- 引导窗 close 走 `hide` 而非 destroy（与主窗一致），便于 M2.2.7 重新打开

**验收**：清掉 `~/.adhd-coder/.installed`，重启 app，看到引导窗而不是主窗；走完三步后主窗出现、引导窗消失。

---

#### M2.2.7 · 托盘菜单「重新运行引导」

**改动**：
- `build_tray` 加菜单项 `rerun_onboarding`
- 点击：`reset_install()` + show 引导窗 + hide 主窗
- 顺手把现有 `显示窗口 / 隐藏窗口 / 退出` 与新项的分隔线加上

**验收**：托盘点「重新运行引导」，主窗消失、引导窗弹出。

---

#### M2.2.8 · 端到端验收 + 文档同步

**操作**：
- `rm ~/.adhd-coder/.installed`，并把 `~/.claude/settings.json` 里我们的 Stop / UserPromptSubmit hook 手动删掉
- `npm run start` → 引导窗弹出 → 点完三步 → 在终端开一次 Claude Code → 窗口收到任务
- 同步文档：README「下一步」里把首启引导划掉；`scripts/install-claude-hook.sh` 在头部加注释「DMG 用户无需运行，仅供开发者重置 hook 使用」

**验收**：以上流程全跑通；新装一次 + 托盘「重新运行引导」一次，两条路径都 OK。

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
