use serde::{Deserialize, Serialize};
use std::fs;
use std::net::{Shutdown, SocketAddr, TcpStream};
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, State, WindowEvent};
use tauri_plugin_notification::NotificationExt;
use tiny_http::{Header, Method, Response, Server};

const PORTS: &[u16] = &[7777, 7778, 7779, 7780];

const REPORTER_PY: &[u8] = include_bytes!("../../scripts/adhd-report.py");

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct TerminalRef {
    #[serde(default)]
    program: String,
    #[serde(default)]
    iterm_session: String,
    #[serde(default)]
    term_session: String,
    #[serde(default)]
    tmux_pane: String,
    #[serde(default)]
    tmux_socket: String,
    #[serde(default)]
    tty: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Task {
    id: String,
    project: String,
    summary: String,
    created_at: i64,
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default = "default_status")]
    status: String,
    #[serde(default)]
    viewed: bool,
    #[serde(default)]
    terminal: Option<TerminalRef>,
}

fn default_status() -> String {
    "done".into()
}

#[derive(Default, Deserialize)]
struct DoneReq {
    project: Option<String>,
    summary: Option<String>,
    session_id: Option<String>,
    status: Option<String>,
    terminal: Option<TerminalRef>,
}

#[derive(Default)]
struct AppState {
    tasks: Mutex<Vec<Task>>,
    port: Mutex<Option<u16>>,
}

fn data_dir() -> PathBuf {
    let mut p = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    p.push(".adhd-coder");
    let _ = fs::create_dir_all(&p);
    p
}

fn store_path() -> PathBuf {
    let mut p = data_dir();
    p.push("tasks.json");
    p
}

fn port_file() -> PathBuf {
    let mut p = data_dir();
    p.push("port");
    p
}

fn reporter_path() -> PathBuf {
    let mut p = data_dir();
    p.push("report.py");
    p
}

fn installed_flag_path() -> PathBuf {
    let mut p = data_dir();
    p.push(".installed");
    p
}

fn claude_settings_path() -> PathBuf {
    let mut p = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    p.push(".claude");
    p.push("settings.json");
    p
}

fn is_our_hook(cmd: &str) -> bool {
    cmd.contains("/.adhd-coder/report.py") || cmd.contains("127.0.0.1:7777/done")
}

fn upsert_event(
    hooks_obj: &mut serde_json::Map<String, serde_json::Value>,
    event: &str,
    cmd: &str,
) {
    use serde_json::{json, Value};

    let prev = match hooks_obj.get_mut(event) {
        Some(Value::Array(a)) => std::mem::take(a),
        _ => Vec::new(),
    };

    let mut cleaned: Vec<Value> = prev
        .into_iter()
        .filter_map(|mut entry| {
            let inner = entry.get_mut("hooks").and_then(|h| h.as_array_mut())?;
            inner.retain(|x| {
                let c = x.get("command").and_then(|v| v.as_str()).unwrap_or("");
                !is_our_hook(c)
            });
            if inner.is_empty() {
                None
            } else {
                Some(entry)
            }
        })
        .collect();

    cleaned.push(json!({
        "hooks": [{ "type": "command", "command": cmd }]
    }));

    hooks_obj.insert(event.to_string(), Value::Array(cleaned));
}

fn load_tasks() -> Vec<Task> {
    let p = store_path();
    fs::read_to_string(&p)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_tasks(tasks: &[Task]) {
    if let Ok(s) = serde_json::to_string_pretty(tasks) {
        let _ = fs::write(store_path(), s);
    }
}

#[tauri::command]
fn list_tasks(state: State<AppState>) -> Vec<Task> {
    state.tasks.lock().unwrap().clone()
}

#[tauri::command]
fn ack_task(id: String, state: State<AppState>) {
    let mut tasks = state.tasks.lock().unwrap();
    tasks.retain(|t| t.id != id);
    save_tasks(&tasks);
}

#[tauri::command]
fn get_port(state: State<AppState>) -> Option<u16> {
    *state.port.lock().unwrap()
}

#[derive(Serialize)]
struct InstallStatus {
    installed: bool,
    version: Option<String>,
    installed_at: Option<i64>,
}

#[tauri::command]
fn get_install_status() -> InstallStatus {
    let raw = match fs::read_to_string(installed_flag_path()) {
        Ok(s) => s,
        Err(_) => {
            return InstallStatus {
                installed: false,
                version: None,
                installed_at: None,
            }
        }
    };
    let parsed: serde_json::Value = serde_json::from_str(&raw).unwrap_or(serde_json::Value::Null);
    InstallStatus {
        installed: true,
        version: parsed
            .get("version")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        installed_at: parsed.get("installed_at").and_then(|v| v.as_i64()),
    }
}

#[tauri::command]
fn mark_installed() -> Result<PathBuf, String> {
    let p = installed_flag_path();
    let body = serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "installed_at": chrono::Utc::now().timestamp_millis(),
    });
    fs::write(&p, serde_json::to_string_pretty(&body).unwrap())
        .map_err(|e| format!("write {}: {}", p.display(), e))?;
    Ok(p)
}

#[tauri::command]
fn open_accessibility_settings() -> Result<(), String> {
    Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
        .status()
        .map_err(|e| format!("open accessibility settings: {}", e))?;
    Ok(())
}

#[tauri::command]
fn reset_install() -> Result<(), String> {
    let p = installed_flag_path();
    match fs::remove_file(&p) {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("remove {}: {}", p.display(), e)),
    }
}

#[tauri::command]
fn install_claude_hook() -> Result<PathBuf, String> {
    use serde_json::{json, Value};

    let settings = claude_settings_path();
    let dir = settings.parent().ok_or("settings has no parent")?;
    fs::create_dir_all(dir).map_err(|e| format!("mkdir {}: {}", dir.display(), e))?;

    let raw = fs::read_to_string(&settings).unwrap_or_else(|_| "{}".into());

    if settings.exists() {
        let ts = chrono::Utc::now().timestamp();
        let bak = dir.join(format!("settings.json.bak.{}", ts));
        fs::copy(&settings, &bak)
            .map_err(|e| format!("backup {}: {}", bak.display(), e))?;
    }

    let trimmed = raw.trim();
    let mut cfg: Value = if trimmed.is_empty() {
        json!({})
    } else {
        serde_json::from_str(trimmed).map_err(|e| format!("parse settings.json: {}", e))?
    };
    if !cfg.is_object() {
        cfg = json!({});
    }

    let obj = cfg.as_object_mut().unwrap();
    let hooks = obj.entry("hooks".to_string()).or_insert_with(|| json!({}));
    if !hooks.is_object() {
        *hooks = json!({});
    }
    let hooks_obj = hooks.as_object_mut().unwrap();

    let cmd = format!("python3 \"{}\"", reporter_path().display());
    upsert_event(hooks_obj, "UserPromptSubmit", &cmd);
    upsert_event(hooks_obj, "Stop", &cmd);

    let out = serde_json::to_string_pretty(&cfg)
        .map_err(|e| format!("serialize settings: {}", e))?;
    fs::write(&settings, out).map_err(|e| format!("write {}: {}", settings.display(), e))?;

    Ok(settings)
}

#[tauri::command]
fn install_reporter() -> Result<PathBuf, String> {
    let dst = reporter_path();
    fs::write(&dst, REPORTER_PY)
        .map_err(|e| format!("write {}: {}", dst.display(), e))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&dst)
            .map_err(|e| format!("stat {}: {}", dst.display(), e))?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&dst, perms)
            .map_err(|e| format!("chmod {}: {}", dst.display(), e))?;
    }

    Ok(dst)
}

#[tauri::command]
fn clear_tasks(state: State<AppState>) {
    let mut tasks = state.tasks.lock().unwrap();
    tasks.clear();
    save_tasks(&tasks);
}

#[tauri::command]
fn focus_task(id: String, state: State<AppState>) -> bool {
    let (terminal, project) = {
        let mut tasks = state.tasks.lock().unwrap();
        let found = tasks
            .iter()
            .find(|t| t.id == id)
            .map(|t| (t.terminal.clone(), t.project.clone()));
        for t in tasks.iter_mut() {
            if t.id == id {
                t.viewed = true;
            }
        }
        save_tasks(&tasks);
        match found {
            Some((term, proj)) => (term, proj),
            None => return false,
        }
    };

    if let Some(t) = terminal {
        return run_focus_script(&t, &project);
    }
    false
}

fn applescript_str(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn focus_vscode_like(app_name: &str, project: &str) -> bool {
    let app = applescript_str(app_name);
    let proj = applescript_str(project);
    let script = format!(
        r#"tell application "{app}" to activate
delay 0.1
tell application "System Events"
  tell process "{app}"
    try
      set theWin to first window whose title contains "{proj}"
      perform action "AXRaise" of theWin
      set frontmost to true
    end try
  end tell
end tell"#,
        app = app,
        proj = proj
    );
    Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn run_focus_script(t: &TerminalRef, project: &str) -> bool {
    // 1) 如果在 tmux 内：先切到目标 pane（同时会切到所在 window）
    if !t.tmux_pane.is_empty() {
        let mut cmd = Command::new("tmux");
        if !t.tmux_socket.is_empty() {
            cmd.arg("-S").arg(&t.tmux_socket);
        }
        let _ = cmd
            .args(["select-window", "-t", &t.tmux_pane])
            .status();
        let mut cmd = Command::new("tmux");
        if !t.tmux_socket.is_empty() {
            cmd.arg("-S").arg(&t.tmux_socket);
        }
        let _ = cmd
            .args(["select-pane", "-t", &t.tmux_pane])
            .status();
    }

    // 2) 再激活承载终端的 macOS app（iTerm 的话会选中具体 session/split）
    let script = match t.program.as_str() {
        "iTerm.app" => {
            let uuid = t
                .iterm_session
                .rsplit(':')
                .next()
                .unwrap_or("")
                .to_string();
            if uuid.is_empty() {
                return false;
            }
            format!(
                r#"tell application "iTerm2"
  activate
  set target to "{uuid}"
  repeat with w in windows
    repeat with tb in tabs of w
      repeat with s in sessions of tb
        if unique id of s is target then
          select w
          select tb
          select s
          return
        end if
      end repeat
    end repeat
  end repeat
end tell"#,
                uuid = uuid
            )
        }
        "Apple_Terminal" => {
            if t.tty.is_empty() {
                return false;
            }
            format!(
                r#"tell application "Terminal"
  activate
  set target to "{tty}"
  repeat with w in windows
    repeat with tb in tabs of w
      if tty of tb is target then
        set selected tab of w to tb
        set frontmost of w to true
        return
      end if
    end repeat
  end repeat
end tell"#,
                tty = t.tty
            )
        }
        "vscode" => return focus_vscode_like("Code", project),
        "cursor" => return focus_vscode_like("Cursor", project),
        _ => return !t.tmux_pane.is_empty(),
    };

    Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn add_task(
    app: &AppHandle,
    project: String,
    summary: Option<String>,
    session_id: Option<String>,
    status: String,
    terminal: Option<TerminalRef>,
) {
    let now = chrono::Utc::now().timestamp_millis();
    let session_key = session_id.as_ref().filter(|s| !s.is_empty()).cloned();

    let (task, notify) = {
        let state = app.state::<AppState>();
        let mut tasks = state.tasks.lock().unwrap();

        let existing_idx = session_key.as_ref().and_then(|sid| {
            tasks
                .iter()
                .position(|t| t.session_id.as_deref() == Some(sid))
        });

        let (task, was_responding) = if let Some(idx) = existing_idx {
            let mut t = tasks.remove(idx);
            let prev_status = t.status.clone();
            t.project = project;
            if let Some(s) = summary {
                t.summary = s;
            }
            t.status = status.clone();
            t.created_at = now;
            t.session_id = session_key.clone();
            if terminal.is_some() {
                t.terminal = terminal;
            }
            // 新一轮活动 → 重置已查看标记，重新需要关注
            t.viewed = false;
            (t, prev_status == "responding")
        } else {
            (
                Task {
                    id: uuid::Uuid::new_v4().to_string(),
                    project,
                    summary: summary.unwrap_or_else(|| "完成".into()),
                    created_at: now,
                    session_id: session_key.clone(),
                    status: status.clone(),
                    viewed: false,
                    terminal,
                },
                false,
            )
        };

        tasks.insert(0, task.clone());
        save_tasks(&tasks);

        let notify = status == "done" && was_responding;
        (task, notify)
    };

    if notify {
        let _ = app
            .notification()
            .builder()
            .title(format!("✓ {}", task.project))
            .body(&task.summary)
            .show();
    }

    let _ = app.emit("task-added", &task);
}

fn port_in_use(port: u16) -> bool {
    // macOS 上 IPv6 dual-stack(*:port) 与 IPv4(127.0.0.1:port) 可共存绑定,
    // 仅靠 bind 失败检测不到外部冲突,因此先 connect 探测。
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    match TcpStream::connect_timeout(&addr, Duration::from_millis(150)) {
        Ok(s) => {
            let _ = s.shutdown(Shutdown::Both);
            true
        }
        Err(_) => false,
    }
}

fn start_http_server(app: AppHandle) {
    thread::spawn(move || {
        let mut bound: Option<(u16, Server)> = None;
        for &p in PORTS {
            if port_in_use(p) {
                eprintln!("[adhd] :{p} already serving, trying next");
                continue;
            }
            match Server::http(format!("127.0.0.1:{}", p)) {
                Ok(s) => {
                    bound = Some((p, s));
                    break;
                }
                Err(e) => eprintln!("[adhd] :{p} bind failed ({e}), trying next"),
            }
        }
        let (port, server) = match bound {
            Some(v) => v,
            None => {
                eprintln!("[adhd] all candidate ports busy: {:?}", PORTS);
                return;
            }
        };

        *app.state::<AppState>().port.lock().unwrap() = Some(port);
        let _ = fs::write(port_file(), port.to_string());
        let _ = app.emit("port-changed", port);
        eprintln!("[adhd] listening on http://127.0.0.1:{port}/done");

        for mut req in server.incoming_requests() {
            let path = req.url().to_string();
            let method = req.method().clone();

            if method == Method::Post && path.starts_with("/done") {
                let mut body = String::new();
                let _ = req.as_reader().read_to_string(&mut body);
                let parsed: DoneReq = serde_json::from_str(&body).unwrap_or_default();
                let project = parsed.project.unwrap_or_else(|| "task".into());
                let status = parsed.status.unwrap_or_else(|| "done".into());
                add_task(
                    &app,
                    project,
                    parsed.summary,
                    parsed.session_id,
                    status,
                    parsed.terminal,
                );

                let resp = Response::from_string(r#"{"ok":true}"#)
                    .with_header(json_header());
                let _ = req.respond(resp);
            } else if method == Method::Get && path == "/health" {
                let _ = req.respond(Response::from_string("ok"));
            } else {
                let _ = req.respond(Response::from_string("not found").with_status_code(404));
            }
        }
    });
}

fn json_header() -> Header {
    Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap()
}

fn toggle_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        if w.is_visible().unwrap_or(false) {
            let _ = w.hide();
        } else {
            let _ = w.show();
            let _ = w.set_focus();
        }
    }
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "显示窗口", true, None::<&str>)?;
    let hide = MenuItem::with_id(app, "hide", "隐藏窗口", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &hide, &quit])?;

    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or_else(|| tauri::Error::AssetNotFound("default window icon".into()))?;

    let _tray = TrayIconBuilder::with_id("main-tray")
        .icon(icon)
        .tooltip("ADHD Coder")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.show();
                    let _ = w.set_focus();
                }
            }
            "hide" => {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.hide();
                }
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let tauri::tray::TrayIconEvent::Click {
                button: tauri::tray::MouseButton::Left,
                button_state: tauri::tray::MouseButtonState::Up,
                ..
            } = event
            {
                toggle_window(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

pub fn run() {
    let initial = load_tasks();

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .manage(AppState {
            tasks: Mutex::new(initial),
            port: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            list_tasks,
            ack_task,
            clear_tasks,
            focus_task,
            get_port,
            install_reporter,
            install_claude_hook,
            get_install_status,
            mark_installed,
            reset_install,
            open_accessibility_settings
        ])
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .setup(|app| {
            start_http_server(app.handle().clone());
            build_tray(app.handle())?;
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let tauri::RunEvent::Reopen { has_visible_windows, .. } = event {
                if !has_visible_windows {
                    if let Some(w) = app.get_webview_window("main") {
                        let _ = w.show();
                        let _ = w.set_focus();
                    }
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    fn run_upsert(mut cfg: Value, cmd: &str) -> Value {
        let obj = cfg.as_object_mut().unwrap();
        let hooks = obj.entry("hooks".to_string()).or_insert_with(|| json!({}));
        let hooks_obj = hooks.as_object_mut().unwrap();
        upsert_event(hooks_obj, "Stop", cmd);
        cfg
    }

    #[test]
    fn upsert_into_empty_config() {
        let out = run_upsert(json!({}), "python3 \"/x/.adhd-coder/report.py\"");
        assert_eq!(
            out["hooks"]["Stop"],
            json!([{
                "hooks": [{ "type": "command", "command": "python3 \"/x/.adhd-coder/report.py\"" }]
            }])
        );
    }

    #[test]
    fn upsert_preserves_other_hooks() {
        let input = json!({
            "hooks": {
                "Stop": [{ "hooks": [{ "type": "command", "command": "echo hi" }] }]
            }
        });
        let out = run_upsert(input, "python3 \"/x/.adhd-coder/report.py\"");
        let stop = out["hooks"]["Stop"].as_array().unwrap();
        assert_eq!(stop.len(), 2);
        assert_eq!(stop[0]["hooks"][0]["command"], "echo hi");
        assert_eq!(
            stop[1]["hooks"][0]["command"],
            "python3 \"/x/.adhd-coder/report.py\""
        );
    }

    #[test]
    fn upsert_replaces_old_version_in_place() {
        let input = json!({
            "hooks": {
                "Stop": [
                    { "hooks": [{ "type": "command", "command": "echo hi" }] },
                    { "hooks": [{ "type": "command", "command": "curl 127.0.0.1:7777/done" }] },
                    { "hooks": [
                        { "type": "command", "command": "python3 \"/Users/old/.adhd-coder/report.py\"" },
                        { "type": "command", "command": "echo also" }
                    ]}
                ]
            }
        });
        let out = run_upsert(input, "python3 \"/x/.adhd-coder/report.py\"");
        let stop = out["hooks"]["Stop"].as_array().unwrap();
        assert_eq!(stop.len(), 3, "old curl entry should drop, mixed entry kept with one cmd, new appended");
        assert_eq!(stop[0]["hooks"][0]["command"], "echo hi");
        assert_eq!(stop[1]["hooks"].as_array().unwrap().len(), 1);
        assert_eq!(stop[1]["hooks"][0]["command"], "echo also");
        assert_eq!(
            stop[2]["hooks"][0]["command"],
            "python3 \"/x/.adhd-coder/report.py\""
        );
    }
}
