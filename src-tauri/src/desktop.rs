use crate::store::{Clip, Group, Result, Settings, Store};
use arboard::Clipboard;
use serde::Serialize;
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
    sync::{Arc, Mutex, MutexGuard},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, State,
};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

struct Reader {
    clipboard: Option<Clipboard>,
    previous: Option<String>,
    armed: bool,
}
struct Context {
    store: Mutex<Store>,
    reader: Mutex<Reader>,
    clipboard_error: Mutex<Option<String>>,
    shortcut_error: Mutex<Option<String>>,
}
type AppContext<'a> = State<'a, Arc<Context>>;
fn lock<T>(mutex: &Mutex<T>) -> Result<MutexGuard<'_, T>> {
    mutex
        .lock()
        .map_err(|_| "应用内部状态异常，请重新启动星笺。".into())
}
fn changed(app: &AppHandle) {
    let _ = app.emit("clips-changed", ());
}
fn show_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}
fn toggle_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) && window.is_focused().unwrap_or(false) {
            let _ = window.hide();
        } else {
            show_window(app);
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppState {
    clips: Vec<Clip>,
    groups: Vec<Group>,
    settings: Settings,
    clipboard_error: Option<String>,
    shortcut_error: Option<String>,
}
#[tauri::command]
fn get_state(context: AppContext<'_>) -> Result<AppState> {
    let store = lock(&context.store)?;
    Ok(AppState {
        clips: store.clips()?,
        groups: store.groups()?,
        settings: store.settings()?,
        clipboard_error: lock(&context.clipboard_error)?.clone(),
        shortcut_error: lock(&context.shortcut_error)?.clone(),
    })
}
#[tauri::command]
fn add_clip(
    app: AppHandle,
    context: AppContext<'_>,
    content: String,
    title: String,
    group_id: Option<i64>,
) -> Result<Clip> {
    let clip = lock(&context.store)?.add(&content, &title, group_id)?;
    changed(&app);
    Ok(clip)
}
#[tauri::command]
fn update_clip(
    app: AppHandle,
    context: AppContext<'_>,
    id: i64,
    title: String,
    group_id: Option<i64>,
    pinned: bool,
) -> Result<()> {
    lock(&context.store)?.update(id, &title, group_id, pinned)?;
    changed(&app);
    Ok(())
}
#[tauri::command]
fn save_code(
    app: AppHandle,
    context: AppContext<'_>,
    id: Option<i64>,
    title: String,
    content: String,
    language: String,
    group_id: Option<i64>,
) -> Result<Clip> {
    let clip = lock(&context.store)?.save_code(id, &title, &content, &language, group_id)?;
    changed(&app);
    Ok(clip)
}
#[tauri::command]
fn delete_clip(app: AppHandle, context: AppContext<'_>, id: i64) -> Result<()> {
    lock(&context.store)?.delete(id)?;
    changed(&app);
    Ok(())
}
#[tauri::command]
fn copy_clip(app: AppHandle, context: AppContext<'_>, id: i64) -> Result<()> {
    let content = lock(&context.store)?.clip(id)?.content;
    {
        let mut reader = lock(&context.reader)?;
        if reader.clipboard.is_none() {
            reader.clipboard = Some(Clipboard::new().map_err(|e| format!("无法访问剪贴板：{e}"))?);
        }
        reader
            .clipboard
            .as_mut()
            .unwrap()
            .set_text(content.clone())
            .map_err(|e| format!("复制失败：{e}"))?;
        reader.previous = Some(content);
    }
    lock(&context.store)?.mark_copied(id)?;
    *lock(&context.clipboard_error)? = None;
    changed(&app);
    Ok(())
}
#[tauri::command]
fn create_group(
    app: AppHandle,
    context: AppContext<'_>,
    name: String,
    color: String,
) -> Result<Group> {
    let group = lock(&context.store)?.create_group(&name, &color)?;
    changed(&app);
    Ok(group)
}
#[tauri::command]
fn delete_group(app: AppHandle, context: AppContext<'_>, id: i64) -> Result<()> {
    lock(&context.store)?.delete_group(id)?;
    changed(&app);
    Ok(())
}
#[tauri::command]
fn save_settings(app: AppHandle, context: AppContext<'_>, settings: Settings) -> Result<()> {
    settings.validate()?;
    let old = lock(&context.store)?.settings()?;
    let shortcut: Shortcut = settings
        .shortcut
        .parse()
        .map_err(|_| "快捷键格式无效，例如 CommandOrControl+Shift+V。")?;
    let old_shortcut = old.shortcut.parse::<Shortcut>().ok();
    let shortcut_changed = old_shortcut != Some(shortcut);
    if shortcut_changed {
        app.global_shortcut()
            .register(shortcut)
            .map_err(|_| "该快捷键无法注册，可能被其他应用占用。")?;
    }
    let save_result = (|| -> Result<()> {
        // Lock order is reader -> store, also used by the monitor. Pause cannot race with capture.
        let mut reader = lock(&context.reader)?;
        lock(&context.store)?.save_settings(&settings)?;
        if old.paused != settings.paused {
            reader.armed = false;
            reader.previous = None;
        }
        Ok(())
    })();
    if let Err(error) = save_result {
        if shortcut_changed {
            let _ = app.global_shortcut().unregister(shortcut);
        }
        return Err(error);
    }
    if shortcut_changed {
        if let Some(old_shortcut) = old_shortcut {
            let _ = app.global_shortcut().unregister(old_shortcut);
        }
        *lock(&context.shortcut_error)? = None;
    }
    changed(&app);
    Ok(())
}
#[tauri::command]
fn clear_history(app: AppHandle, context: AppContext<'_>) -> Result<()> {
    lock(&context.store)?.clear_history()?;
    changed(&app);
    Ok(())
}
#[tauri::command]
fn export_backup(context: AppContext<'_>) -> Result<String> {
    lock(&context.store)?.export()
}
#[tauri::command]
fn import_backup(app: AppHandle, context: AppContext<'_>, data: String) -> Result<usize> {
    let count = lock(&context.store)?.import(&data)?;
    changed(&app);
    Ok(count)
}

fn write_private(path: &Path, data: &[u8]) -> Result<()> {
    let parent = path.parent().ok_or("无效的文件路径。")?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temporary = parent.join(format!(
        ".starclip-export-{}-{nonce}.tmp",
        std::process::id()
    ));
    let result = (|| -> Result<()> {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&temporary)
            .map_err(|_| "无法写入所选目录，请检查权限。")?;
        file.write_all(data)
            .and_then(|_| file.sync_all())
            .map_err(|_| "文件写入失败，请检查磁盘空间。")?;
        drop(file);
        fs::rename(&temporary, path).map_err(|_| "无法保存文件，请检查文件是否被占用。")?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}
#[tauri::command]
async fn save_backup(app: AppHandle, context: AppContext<'_>) -> Result<bool> {
    let data = lock(&context.store)?.export()?;
    tauri::async_runtime::spawn_blocking(move || {
        let Some(file) = app
            .dialog()
            .file()
            .add_filter("Starclip JSON", &["json"])
            .set_file_name("starclip-backup.json")
            .blocking_save_file()
        else {
            return Ok(false);
        };
        let path = file.into_path().map_err(|_| "请选择本地文件路径。")?;
        write_private(&path, data.as_bytes())?;
        Ok(true)
    })
    .await
    .map_err(|_| "导出任务失败。".to_string())?
}
#[tauri::command]
async fn save_export(app: AppHandle, data: String, format: String) -> Result<bool> {
    use base64::Engine;
    let bytes = match format.as_str() {
        "md" => {
            if data.len() > 1024 * 1024 {
                return Err("导出内容过大。".into());
            }
            data.into_bytes()
        }
        "png" => {
            if data.len() > 14 * 1024 * 1024 {
                return Err("图片不能超过 10 MB。".into());
            }
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(data)
                .map_err(|_| "PNG 编码无效。")?;
            if bytes.len() > 10 * 1024 * 1024 || !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
                return Err("PNG 文件无效或过大。".into());
            }
            bytes
        }
        _ => return Err("仅支持 Markdown 与 PNG 导出。".into()),
    };
    tauri::async_runtime::spawn_blocking(move || {
        let Some(file) = app
            .dialog()
            .file()
            .add_filter("Starclip export", &[format.as_str()])
            .set_file_name(format!("starclip-code.{format}"))
            .blocking_save_file()
        else {
            return Ok(false);
        };
        let path = file.into_path().map_err(|_| "请选择本地文件路径。")?;
        write_private(&path, &bytes)?;
        Ok(true)
    })
    .await
    .map_err(|_| "导出任务失败。".to_string())?
}
#[tauri::command]
fn hide_window(app: AppHandle) -> Result<()> {
    app.get_webview_window("main")
        .ok_or("窗口不存在。")?
        .hide()
        .map_err(|e| e.to_string())
}

fn set_clipboard_error(app: &AppHandle, context: &Context, error: Option<String>) {
    if let Ok(mut current) = context.clipboard_error.lock() {
        if *current != error {
            *current = error;
            changed(app);
        }
    }
}
fn monitor(app: AppHandle, context: Arc<Context>) {
    thread::spawn(move || loop {
        thread::sleep(Duration::from_millis(700));
        let result: Result<bool> = (|| {
            // ponytail: one lock order serializes this personal clipboard; no worker queue is needed.
            let mut reader = lock(&context.reader)?;
            let store = lock(&context.store)?;
            if store.settings()?.paused {
                return Ok(false);
            }
            if reader.clipboard.is_none() {
                reader.clipboard =
                    Some(Clipboard::new().map_err(|e| format!("无法访问剪贴板：{e}"))?);
            }
            let content = match reader.clipboard.as_mut().unwrap().get_text() {
                Ok(text) => Some(text),
                Err(arboard::Error::ContentNotAvailable) => None,
                Err(error) => return Err(format!("剪贴板读取失败：{error}")),
            };
            if !reader.armed {
                reader.armed = true;
                reader.previous = content;
                return Ok(false);
            }
            if reader.previous == content {
                return Ok(false);
            }
            if let Some(text) = content.as_ref().filter(|text| !text.trim().is_empty()) {
                store.add(text, "", None)?;
                reader.previous = content;
                Ok(true)
            } else {
                reader.previous = content;
                Ok(false)
            }
        })();
        match result {
            Ok(true) => {
                set_clipboard_error(&app, &context, None);
                changed(&app);
            }
            Ok(false) => {}
            Err(error) => set_clipboard_error(&app, &context, Some(error)),
        }
    });
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            show_window(app)
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _, event| {
                    if event.state() == ShortcutState::Pressed {
                        toggle_window(app);
                    }
                })
                .build(),
        )
        .setup(|app| {
            let directory = app.path().app_data_dir()?;
            fs::create_dir_all(&directory)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))?;
            }
            let db_path = directory.join("starclip.sqlite3");
            let store = Store::open(&db_path).map_err(std::io::Error::other)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&db_path, fs::Permissions::from_mode(0o600))?;
            }
            let shortcut_error = store
                .settings()
                .and_then(|settings| {
                    let shortcut: Shortcut = settings
                        .shortcut
                        .parse()
                        .map_err(|_| "已保存的快捷键无效。".to_string())?;
                    app.global_shortcut()
                        .register(shortcut)
                        .map_err(|_| "快捷键注册失败，可在偏好设置中更换。".to_string())
                })
                .err();
            let context = Arc::new(Context {
                store: Mutex::new(store),
                reader: Mutex::new(Reader {
                    clipboard: None,
                    previous: None,
                    armed: false,
                }),
                clipboard_error: Mutex::new(None),
                shortcut_error: Mutex::new(shortcut_error),
            });
            app.manage(context.clone());
            let show = MenuItem::with_id(app, "show", "打开星笺", true, None::<&str>)?;
            let pause = MenuItem::with_id(app, "pause", "暂停 / 继续记录", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出星笺", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &pause, &quit])?;
            let mut tray = TrayIconBuilder::new()
                .tooltip("星笺 Starclip")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => show_window(app),
                    "pause" => {
                        let context = app.state::<Arc<Context>>();
                        let result = (|| -> Result<()> {
                            let mut settings = lock(&context.store)?.settings()?;
                            settings.paused = !settings.paused;
                            save_settings(app.clone(), context, settings)
                        })();
                        if let Err(error) = result {
                            let context = app.state::<Arc<Context>>();
                            set_clipboard_error(app, &context, Some(error));
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if matches!(
                        event,
                        TrayIconEvent::Click {
                            button: tauri::tray::MouseButton::Left,
                            button_state: tauri::tray::MouseButtonState::Up,
                            ..
                        }
                    ) {
                        toggle_window(tray.app_handle());
                    }
                });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;
            monitor(app.handle().clone(), context);
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_state,
            add_clip,
            update_clip,
            save_code,
            delete_clip,
            copy_clip,
            create_group,
            delete_group,
            save_settings,
            clear_history,
            export_backup,
            import_backup,
            save_backup,
            save_export,
            hide_window
        ])
        .run(tauri::generate_context!())
        .expect("星笺启动失败，请检查本地数据目录权限或磁盘空间。");
}
