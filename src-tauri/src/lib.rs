use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::thread;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, State, WindowEvent};
use tauri_plugin_notification::NotificationExt;
use tiny_http::{Header, Method, Response, Server};

const PORT: u16 = 7777;

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
}

#[derive(Default)]
struct AppState {
    tasks: Mutex<Vec<Task>>,
}

fn store_path() -> PathBuf {
    let mut p = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    p.push(".adhd-coder");
    let _ = fs::create_dir_all(&p);
    p.push("tasks.json");
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
fn clear_tasks(state: State<AppState>) {
    let mut tasks = state.tasks.lock().unwrap();
    tasks.clear();
    save_tasks(&tasks);
}

fn add_task(
    app: &AppHandle,
    project: String,
    summary: Option<String>,
    session_id: Option<String>,
    status: String,
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
                },
                false,
            )
        };

        tasks.insert(0, task.clone());
        save_tasks(&tasks);

        // 只在「响应完成」这次转换时通知，避免每次提交都打扰
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

fn start_http_server(app: AppHandle) {
    thread::spawn(move || {
        let server = match Server::http(format!("127.0.0.1:{}", PORT)) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[adhd] HTTP server failed to start on :{PORT}: {e}");
                return;
            }
        };
        eprintln!("[adhd] listening on http://127.0.0.1:{PORT}/done");

        for mut req in server.incoming_requests() {
            let path = req.url().to_string();
            let method = req.method().clone();

            if method == Method::Post && path.starts_with("/done") {
                let mut body = String::new();
                let _ = req.as_reader().read_to_string(&mut body);
                let parsed: DoneReq = serde_json::from_str(&body).unwrap_or_default();
                let project = parsed.project.unwrap_or_else(|| "task".into());
                let status = parsed.status.unwrap_or_else(|| "done".into());
                add_task(&app, project, parsed.summary, parsed.session_id, status);

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
        })
        .invoke_handler(tauri::generate_handler![list_tasks, ack_task, clear_tasks])
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
