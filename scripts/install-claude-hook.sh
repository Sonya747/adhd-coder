#!/usr/bin/env bash
# 安装 / 升级 Claude Code Stop hook，让对话完成时上报到 ADHD Coder
# 用法: bash scripts/install-claude-hook.sh

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPORTER_SRC="$SCRIPT_DIR/adhd-report.py"
REPORTER_DST="$HOME/.adhd-coder/report.py"
SETTINGS="$HOME/.claude/settings.json"

# 1) 安装 python 上报脚本
mkdir -p "$(dirname "$REPORTER_DST")"
cp "$REPORTER_SRC" "$REPORTER_DST"
chmod +x "$REPORTER_DST"
echo "✓ 已部署上报脚本 → $REPORTER_DST"

# 2) 写入 hook
mkdir -p "$(dirname "$SETTINGS")"
[ -f "$SETTINGS" ] || echo '{}' > "$SETTINGS"
cp "$SETTINGS" "$SETTINGS.bak.$(date +%s)"

REPORTER_DST="$REPORTER_DST" node - "$SETTINGS" <<'NODE'
const fs = require("fs");
const path = process.argv[2];
const reporter = process.env.REPORTER_DST;
const cfg = JSON.parse(fs.readFileSync(path, "utf8") || "{}");
cfg.hooks = cfg.hooks || {};

const cmd = `python3 "${reporter}"`;
const marker = "/.adhd-coder/report.py";
const isOurs = (c) => c.includes(marker) || c.includes("127.0.0.1:7777/done");

function upsertEvent(eventName) {
  const list = cfg.hooks[eventName] || [];
  const cleaned = list
    .map(h => ({
      ...h,
      hooks: (h.hooks || []).filter(x => !isOurs(x.command || "")),
    }))
    .filter(h => (h.hooks || []).length > 0);
  cleaned.push({ hooks: [{ type: "command", command: cmd }] });
  cfg.hooks[eventName] = cleaned;
}

upsertEvent("UserPromptSubmit");
upsertEvent("Stop");

fs.writeFileSync(path, JSON.stringify(cfg, null, 2));
console.log("✓ 已写入/升级 UserPromptSubmit + Stop hook →", path);
NODE
