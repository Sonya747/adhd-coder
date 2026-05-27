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
            get_port
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
