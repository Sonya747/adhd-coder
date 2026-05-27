#!/usr/bin/env python3
"""ADHD Coder · Claude Code hook reporter.

通过 stdin 接收 Claude Code 的 hook 事件 JSON：
  - UserPromptSubmit: 用户刚提交，上报 status="responding"，summary=用户输入
  - Stop:            响应完成，上报 status="done"
"""

import json
import os
import subprocess
import sys
import urllib.request

DEFAULT_PORT = 7777
MAX_LEN = 80


def resolve_endpoint():
    override = os.environ.get("ADHD_URL")
    if override:
        return override
    port = DEFAULT_PORT
    try:
        with open(os.path.expanduser("~/.adhd-coder/port"), "r") as f:
            port = int(f.read().strip()) or DEFAULT_PORT
    except Exception:
        pass
    return f"http://127.0.0.1:{port}/done"


ENDPOINT = resolve_endpoint()


def extract_user_text(event):
    if event.get("type") != "user":
        return None
    msg = event.get("message", {}) or {}
    content = msg.get("content")
    if isinstance(content, str):
        return content
    if isinstance(content, list):
        chunks = []
        for item in content:
            if not isinstance(item, dict):
                continue
            if item.get("type") == "tool_result":
                return None
            if item.get("type") == "text":
                chunks.append(item.get("text", ""))
        if chunks:
            return "\n".join(chunks)
    return None


def last_user_prompt(transcript_path):
    if not transcript_path or not os.path.exists(transcript_path):
        return None
    try:
        with open(transcript_path, "r", encoding="utf-8", errors="ignore") as f:
            lines = f.readlines()
    except OSError:
        return None
    for line in reversed(lines):
        line = line.strip()
        if not line:
            continue
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            continue
        text = extract_user_text(event)
        if text:
            return text
    return None


def truncate(text):
    text = " ".join(text.split())
    if len(text) <= MAX_LEN:
        return text
    return text[: MAX_LEN - 1] + "…"


def detect_vscode_family():
    """区分 Cursor / VSCode：两者 TERM_PROGRAM 都可能是 'vscode'，但环境变量里的安装路径不同。"""
    candidates = [
        os.environ.get("VSCODE_GIT_ASKPASS_NODE", ""),
        os.environ.get("VSCODE_GIT_ASKPASS_MAIN", ""),
        os.environ.get("VSCODE_IPC_HOOK_CLI", ""),
        os.environ.get("__CFBundleIdentifier", ""),
    ]
    blob = " ".join(candidates)
    if "Cursor" in blob or "cursor" in blob:
        return "cursor"
    if "Visual Studio Code" in blob or "/Code.app" in blob or "code" in blob.lower():
        return "vscode"
    return ""


def terminal_info():
    tmux_env = os.environ.get("TMUX") or ""
    tmux_socket = tmux_env.split(",", 1)[0] if tmux_env else ""
    program = os.environ.get("TERM_PROGRAM") or ""
    if program == "vscode":
        program = detect_vscode_family() or "vscode"
    info = {
        "program": program,
        "iterm_session": os.environ.get("ITERM_SESSION_ID") or "",
        "term_session": os.environ.get("TERM_SESSION_ID") or "",
        "tmux_pane": os.environ.get("TMUX_PANE") or "",
        "tmux_socket": tmux_socket,
        "tty": "",
    }
    try:
        out = subprocess.check_output(
            ["ps", "-o", "tty=", "-p", str(os.getpid())],
            stderr=subprocess.DEVNULL,
        ).decode().strip()
        if out and out != "??":
            info["tty"] = out if out.startswith("/dev/") else "/dev/" + out
    except Exception:
        pass
    return info


def post(body):
    data = json.dumps(body, ensure_ascii=False).encode("utf-8")
    req = urllib.request.Request(
        ENDPOINT,
        data=data,
        headers={"Content-Type": "application/json; charset=utf-8"},
        method="POST",
    )
    try:
        urllib.request.urlopen(req, timeout=2).read()
    except Exception:
        pass


def main():
    try:
        payload = json.loads(sys.stdin.read() or "{}")
    except json.JSONDecodeError:
        payload = {}

    event = payload.get("hook_event_name") or ""
    session_id = payload.get("session_id", "") or ""
    cwd = payload.get("cwd") or os.getcwd()
    project = os.path.basename(cwd.rstrip("/")) or "task"

    body = {"project": project, "session_id": session_id, "terminal": terminal_info()}

    if event == "UserPromptSubmit":
        prompt = payload.get("prompt") or ""
        body["status"] = "responding"
        body["summary"] = truncate(prompt) if prompt else "新对话"
    else:  # Stop or fallback
        prompt = last_user_prompt(payload.get("transcript_path"))
        body["status"] = "done"
        # 仅在拿到内容时覆盖 summary，否则保留 UserPromptSubmit 阶段写入的值
        if prompt:
            body["summary"] = truncate(prompt)

    post(body)


if __name__ == "__main__":
    main()
