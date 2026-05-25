#!/usr/bin/env bash
# 安装 Claude Code Stop hook，让对话完成时上报到 ADHD Coder
# 用法: bash scripts/install-claude-hook.sh

set -e

SETTINGS="$HOME/.claude/settings.json"
mkdir -p "$(dirname "$SETTINGS")"
[ -f "$SETTINGS" ] || echo '{}' > "$SETTINGS"

# 备份
cp "$SETTINGS" "$SETTINGS.bak.$(date +%s)"

# 用 node 改 JSON（跨平台稳）
node - "$SETTINGS" <<'NODE'
const fs = require("fs");
const path = process.argv[2];
const cfg = JSON.parse(fs.readFileSync(path, "utf8") || "{}");
cfg.hooks = cfg.hooks || {};
cfg.hooks.Stop = cfg.hooks.Stop || [];

const cmd = `PROJ=$(basename "$PWD"); curl -s -X POST http://127.0.0.1:7777/done -H 'Content-Type: application/json' -d "{\\"project\\":\\"$PROJ\\",\\"summary\\":\\"Claude Code 对话完成\\"}" >/dev/null || true`;

const exists = cfg.hooks.Stop.some(h =>
  (h.hooks || []).some(x => (x.command || "").includes("127.0.0.1:7777/done"))
);
if (!exists) {
  cfg.hooks.Stop.push({ hooks: [{ type: "command", command: cmd }] });
}
fs.writeFileSync(path, JSON.stringify(cfg, null, 2));
console.log("✓ 已写入 Stop hook 到", path);
NODE
