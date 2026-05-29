import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow, Window } from "@tauri-apps/api/window";

const btn1 = document.getElementById("btn-1");
const btn2 = document.getElementById("btn-2");
const btn3 = document.getElementById("btn-3");
const num1 = document.getElementById("num-1");
const num2 = document.getElementById("num-2");
const num3 = document.getElementById("num-3");
const err1 = document.getElementById("err-1");
const skipBtn = document.getElementById("skip");

function markDone(numEl) {
  numEl.textContent = "✓";
  numEl.classList.add("done");
}

function setBusy(btn, busy, label) {
  btn.disabled = busy;
  if (label !== undefined) btn.textContent = label;
}

btn1.addEventListener("click", async () => {
  err1.hidden = true;
  err1.textContent = "";
  setBusy(btn1, true, "安装中…");
  try {
    await invoke("install_reporter");
    await invoke("install_claude_hook");
    markDone(num1);
    setBusy(btn1, true, "已安装");
    btn1.blur();
    btn3.disabled = false;
  } catch (e) {
    err1.textContent = String(e);
    err1.hidden = false;
    setBusy(btn1, false, "重试");
  }
});

btn2.addEventListener("click", async () => {
  try {
    await invoke("open_accessibility_settings");
    setBusy(btn2, false, "已打开系统设置");
    btn2.blur();
  } catch (e) {
    setBusy(btn2, false, "打开失败,重试");
  }
});

btn3.addEventListener("click", async () => {
  setBusy(btn3, true, "完成中…");
  try {
    await invoke("mark_installed");
    markDone(num3);

    const current = getCurrentWindow();
    const main = await Window.getByLabel("main");
    if (main && current.label !== "main") {
      await main.show();
      await main.setFocus();
      await current.close();
    }
  } catch (e) {
    setBusy(btn3, false, "完成");
    console.error(e);
  }
});

skipBtn.addEventListener("click", async () => {
  const current = getCurrentWindow();
  await current.hide();
});
