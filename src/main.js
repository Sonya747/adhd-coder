import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

const listEl = document.getElementById("list");
const countEl = document.getElementById("count");
const clearBtn = document.getElementById("clear");
const hideBtn = document.getElementById("hide");
const resizeEl = document.getElementById("resize");
const brandEl = document.getElementById("brand");
const portBadge = document.getElementById("port-badge");

const DEFAULT_PORT = 7777;

function fmtTime(ts) {
  const d = new Date(ts);
  const now = new Date();
  const sameDay = d.toDateString() === now.toDateString();
  const hh = String(d.getHours()).padStart(2, "0");
  const mm = String(d.getMinutes()).padStart(2, "0");
  return sameDay ? `${hh}:${mm}` : `${d.getMonth() + 1}/${d.getDate()} ${hh}:${mm}`;
}

function render(tasks) {
  countEl.textContent = tasks.length;
  countEl.classList.toggle("zero", tasks.length === 0);

  if (tasks.length === 0) {
    listEl.innerHTML = '<li class="empty">等待任务完成…</li>';
    return;
  }

  listEl.innerHTML = "";
  for (const t of tasks) {
    const li = document.createElement("li");
    li.className = "item";
    li.dataset.id = t.id;
    const status = t.status === "responding" ? "responding" : "done";
    const viewed = !!t.viewed && status === "done";
    li.classList.add(`status-${status}`);
    if (viewed) li.classList.add("viewed");
    li.innerHTML = `
      <div class="status-icon ${status}">${status === "done" ? "✓" : ""}</div>
      <div class="body">
        <div class="proj">${escapeHtml(t.project || "task")}</div>
        <div class="summary">${escapeHtml(t.summary || "完成")}</div>
        <div class="time">${fmtTime(t.created_at)}</div>
      </div>
      <button class="row-x" title="删除">×</button>
    `;
    li.addEventListener("click", () => focusTask(t.id));
    li.querySelector(".row-x").addEventListener("click", (e) => {
      e.stopPropagation();
      acknowledge(t.id, li);
    });
    listEl.appendChild(li);
  }
}

function escapeHtml(s) {
  return String(s).replace(/[&<>"']/g, (c) => ({
    "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;",
  })[c]);
}

async function acknowledge(id, li) {
  li.classList.add("removing");
  setTimeout(async () => {
    await invoke("ack_task", { id });
    refresh();
  }, 160);
}

async function focusTask(id) {
  await invoke("focus_task", { id });
  refresh();
}

async function refresh() {
  const tasks = await invoke("list_tasks");
  render(tasks);
}

clearBtn.addEventListener("click", async () => {
  await invoke("clear_tasks");
  refresh();
});

hideBtn.addEventListener("click", () => {
  getCurrentWindow().hide();
});

resizeEl.addEventListener("mousedown", (e) => {
  e.preventDefault();
  getCurrentWindow().startResizeDragging("SouthEast");
});

async function refreshPort() {
  const port = await invoke("get_port");
  if (!port) {
    brandEl.title = "未监听（端口全部被占）";
    portBadge.hidden = false;
    portBadge.textContent = "× port";
    portBadge.classList.add("err");
    return;
  }
  brandEl.title = `监听 127.0.0.1:${port}`;
  if (port === DEFAULT_PORT) {
    portBadge.hidden = true;
    portBadge.classList.remove("err");
  } else {
    portBadge.hidden = false;
    portBadge.classList.remove("err");
    portBadge.textContent = `:${port}`;
  }
}

listen("task-added", () => refresh());
listen("port-changed", () => refreshPort());

refresh();
refreshPort();
