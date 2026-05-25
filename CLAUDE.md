# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project purpose

A macOS floating always-on-top widget that lists "AI conversation tasks that finished and are waiting for you to glance at". Built for developers running several Claude Code / Codex / similar agents in parallel — the widget surfaces completions so you don't lose track of which window is done.

Status: MVP scaffold. Covers Claude Code (CLI + IDE plugins, via Stop hook), any CLI (via the `adhd-wrap` wrapper), and any process (via the generic HTTP endpoint). Codex Desktop and web-based chats are explicitly deferred to v2.

## Commands

```bash
npm install                   # install frontend deps (first time only)
npm run start                 # tauri dev — boots vite + compiles rust + opens window
npm run tauri build           # production bundle → src-tauri/target/release/bundle/
bash scripts/install-claude-hook.sh   # wire Claude Code's Stop hook to localhost:7777
```

Rust toolchain is required (`rustup`). First `npm run start` takes 3–5 min to compile Rust; subsequent runs are fast.

There are no tests or linters configured yet.

## Architecture

Two halves talking through Tauri's invoke/event bridge **plus** a third channel — a localhost HTTP server — that is the whole reason this project exists.

**Rust side (`src-tauri/src/lib.rs`):** owns the task list (in-memory `Mutex<Vec<Task>>`, mirrored to `~/.adhd-coder/tasks.json`). On startup it spawns a `tiny_http` server on `127.0.0.1:7777` in a background thread. `POST /done {project, summary}` is the public entry point — any external process can hit it. Each post: prepend task to list → persist → fire native notification → `emit("task-added")` so the webview refreshes. The port is hard-coded; if it's taken, the server logs and the app keeps running without ingest.

**Frontend (`src/`, vanilla JS, no framework):** calls three Tauri commands — `list_tasks`, `ack_task`, `clear_tasks` — and listens for the `task-added` event to re-render. Kept framework-free on purpose to stay lightweight; don't add React/Vue without a reason.

**Window config (`src-tauri/tauri.conf.json`):** `alwaysOnTop`, `decorations: false`, `transparent: true`, `macOSPrivateApi: true`. The frosted look depends on all of these together — flipping any one will break the visual.

**Integration scripts (`scripts/`):** `install-claude-hook.sh` edits `~/.claude/settings.json` to add a Stop hook that curls the local endpoint. `adhd-wrap` is a shell wrapper that runs any command and posts on exit — this is how Codex CLI and arbitrary commands get covered without per-tool integration code. Both treat the HTTP endpoint as the contract; they don't share code with the Rust side.

## Design constraints worth knowing before changing things

- **The HTTP endpoint is the public API.** New integrations (browser extension, Desktop app sniffer, etc.) should post to `/done`, not link against the Rust crate. Keep the request shape (`{project, summary}`) stable.
- **Lightweight is a feature.** The user picked Tauri over Electron specifically for size. Vanilla JS frontend, minimal Rust deps. Adding a heavy dep needs a reason.
- **macOS-first.** Window styling and `macOSPrivateApi` are Mac-specific; cross-platform work is out of scope for MVP.
- **No auth on `:7777`.** It's localhost-only and that's the intended threat model — don't add token checks without discussing; it would break the "any script can post" UX.

## Out of scope for MVP (do not build unprompted)

Codex Desktop integration, browser extensions for claude.ai/chatgpt.com, "click task → focus source terminal window", port configurability, cross-platform support. README's "下一步" section tracks these.
