use crate::storage::{DataDirectoryPlan, DataLocation, DirectoryMode, Storage};
use crate::store::{
    starpet_resize_position, CleanupFilter, CleanupPreview, CleanupResult, CleanupSnapshot, Clip,
    Group, Result, Settings, StarpetPreferencePatch, StarpetPreferences, Store, STALE_CLEANUP,
};
use arboard::Clipboard;
use serde::Serialize;
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

struct Reader {
    clipboard: Option<Clipboard>,
    previous: Option<String>,
    armed: bool,
}
struct Context {
    storage: Mutex<Storage>,
    store: Mutex<Store>,
    reader: Mutex<Reader>,
    clipboard_error: Mutex<Option<String>>,
    shortcut_error: Mutex<Option<String>>,
    window_error: Mutex<Option<String>>,
    cleanup: Mutex<Option<CleanupSnapshot>>,
    starpet_operation: Mutex<()>,
}
type AppContext<'a> = State<'a, Arc<Context>>;
fn lock<T>(mutex: &Mutex<T>) -> Result<MutexGuard<'_, T>> {
    mutex
        .lock()
        .map_err(|_| "应用内部状态异常，请重新启动星笺。".into())
}
fn write_guard(context: &Context) -> Result<MutexGuard<'_, Storage>> {
    let storage = lock(&context.storage)?;
    storage.writable()?;
    Ok(storage)
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

fn create_starpet(app: &AppHandle, preferences: &StarpetPreferences) -> Result<WebviewWindow> {
    let (width, height) = preferences.window_size();
    let mut builder = WebviewWindowBuilder::new(
        app,
        "starpet",
        WebviewUrl::App("index.html?view=starpet".into()),
    )
    .title("星伴 · Starclip")
    .inner_size(width, height)
    .resizable(false)
    .maximizable(false)
    .minimizable(false)
    .decorations(false)
    .transparent(true)
    .shadow(false)
    .always_on_top(true)
    .skip_taskbar(true)
    // Initial visibility with focused(false) does not activate the application.
    // Calling show() after a hidden build would steal focus on macOS.
    .visible(true)
    .focused(false);
    if let Ok(Some(monitor)) = app.primary_monitor() {
        let area = monitor.work_area();
        let scale = monitor.scale_factor();
        let left = f64::from(area.position.x) / scale;
        let top = f64::from(area.position.y) / scale;
        let x = left + (f64::from(area.size.width) / scale - width - 20.0).max(0.0);
        let y = top + (f64::from(area.size.height) / scale - height - 20.0).max(0.0);
        builder = builder.position(x, y);
    }
    builder
        .build()
        .map_err(|e| format!("无法创建星伴窗口，偏好未保存：{e}"))
}

struct StarpetGeometry {
    inner: tauri::PhysicalSize<u32>,
    outer: tauri::PhysicalSize<u32>,
    position: tauri::PhysicalPosition<i32>,
    scale: f64,
}
impl StarpetGeometry {
    fn capture(window: &WebviewWindow) -> Result<Self> {
        (|| -> tauri::Result<Self> {
            Ok(Self {
                inner: window.inner_size()?,
                outer: window.outer_size()?,
                position: window.outer_position()?,
                scale: window.scale_factor()?,
            })
        })()
        .map_err(|e| format!("无法读取星伴窗口大小，偏好未保存：{e}"))
    }
}

fn resize_starpet(
    window: &WebviewWindow,
    previous: &StarpetGeometry,
    preferences: &StarpetPreferences,
) -> Result<()> {
    let (width, height) = preferences.window_size();
    let inner = tauri::LogicalSize::new(width, height).to_physical::<u32>(previous.scale);
    let outer = (
        f64::from(inner.width + previous.outer.width.saturating_sub(previous.inner.width)),
        f64::from(inner.height + previous.outer.height.saturating_sub(previous.inner.height)),
    );
    let monitor = match window.current_monitor().map_err(|e| e.to_string())? {
        Some(monitor) => monitor,
        None => window
            .primary_monitor()
            .map_err(|e| e.to_string())?
            .ok_or("暂时无法确定星伴所在屏幕，大小未更改。")?,
    };
    let area = monitor.work_area();
    if outer.0 > f64::from(area.size.width) || outer.1 > f64::from(area.size.height) {
        return Err("当前屏幕可用空间不足，请选择更小的星伴大小。".into());
    }
    let (x, y) = starpet_resize_position(
        (
            f64::from(previous.position.x),
            f64::from(previous.position.y),
        ),
        (
            f64::from(previous.outer.width),
            f64::from(previous.outer.height),
        ),
        outer,
        (
            f64::from(area.position.x),
            f64::from(area.position.y),
            f64::from(area.size.width),
            f64::from(area.size.height),
        ),
    );
    window
        .set_size(inner)
        .map_err(|e| format!("无法调整星伴大小：{e}"))?;
    window
        .set_position(tauri::PhysicalPosition::new(
            x.round() as i32,
            y.round() as i32,
        ))
        .map_err(|e| format!("无法调整星伴位置：{e}"))
}

fn restore_starpet(
    window: &WebviewWindow,
    visible: bool,
    geometry: Option<&StarpetGeometry>,
) -> Result<()> {
    let mut errors = Vec::new();
    if let Some(geometry) = geometry {
        if let Err(error) = window.set_size(geometry.inner) {
            errors.push(error.to_string());
        }
        if let Err(error) = window.set_position(geometry.position) {
            errors.push(error.to_string());
        }
    }
    if window.is_visible().ok() != Some(visible) {
        let restore = if visible {
            window.show()
        } else {
            window.hide()
        };
        if let Err(error) = restore {
            errors.push(error.to_string());
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("；"))
    }
}

// Every caller runs off the UI thread: window creation may wait for the event loop.
fn update_starpet(
    app: &AppHandle,
    context: &Context,
    update: impl FnOnce(&mut StarpetPreferences),
) -> Result<StarpetPreferences> {
    let _operation = lock(&context.starpet_operation)?;
    let old = lock(&context.store)?.starpet_preferences()?;
    let mut preferences = old.clone();
    update(&mut preferences);
    preferences.validate()?;
    let previous_window = app.get_webview_window("starpet");
    let previously_visible = previous_window
        .as_ref()
        .map(|window| window.is_visible().map_err(|e| e.to_string()))
        .transpose()?
        .unwrap_or(false);
    let geometry = if preferences.size != old.size {
        previous_window
            .as_ref()
            .map(StarpetGeometry::capture)
            .transpose()?
    } else {
        None
    };
    let mut window = previous_window.clone();
    // Do not hold the data write gate while dispatching window operations: a
    // synchronous main-window command may need that gate on the UI thread.
    let save_result = (|| -> Result<()> {
        if let Some(window) = window.as_ref() {
            if let Some(geometry) = geometry.as_ref() {
                resize_starpet(window, geometry, &preferences)?;
            }
            if preferences.enabled != previously_visible {
                if preferences.enabled {
                    window.show()
                } else {
                    window.hide()
                }
                .map_err(|e| format!("无法更改星伴窗口可见性：{e}"))?;
            }
        } else if preferences.enabled {
            window = Some(create_starpet(app, &preferences)?);
        }
        if preferences != old {
            let _write = write_guard(context)?;
            lock(&context.store)?.save_starpet_preferences(&preferences)?;
        }
        Ok(())
    })();
    if let Err(error) = save_result {
        let rollback = if let Some(window) = window {
            if previous_window.is_none() {
                window.destroy().map_err(|e| e.to_string())
            } else {
                restore_starpet(&window, previously_visible, geometry.as_ref())
            }
        } else {
            Ok(())
        };
        return Err(match rollback {
            Ok(()) => format!("星伴偏好未保存，已恢复原窗口状态：{error}"),
            Err(rollback) => {
                format!("星伴偏好未保存：{error}；恢复窗口状态失败：{rollback}。请重启星笺。")
            }
        });
    }
    for label in ["main", "starpet"] {
        let _ = app.emit_to(label, "starpet-changed", &preferences);
    }
    Ok(preferences)
}

fn starpet_error(app: &AppHandle, error: String) {
    app.dialog().message(error).title("星伴").show(|_| {});
}

#[tauri::command]
async fn get_starpet_preferences(context: AppContext<'_>) -> Result<StarpetPreferences> {
    let context = context.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        // A newly created webview must not read the old value before a save/rollback finishes.
        let _operation = lock(&context.starpet_operation)?;
        lock(&context.store)?.starpet_preferences()
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn set_starpet_preferences(
    app: AppHandle,
    context: AppContext<'_>,
    preferences: StarpetPreferencePatch,
) -> Result<StarpetPreferences> {
    let context = context.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        update_starpet(&app, &context, |value| preferences.apply_to(value))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn show_main_window(app: AppHandle) -> Result<()> {
    let window = app.get_webview_window("main").ok_or("主窗口不存在。")?;
    window
        .show()
        .and_then(|_| window.unminimize())
        .and_then(|_| window.set_focus())
        .map_err(|e| format!("无法打开主窗口：{e}"))
}

#[tauri::command]
fn starpet_start_dragging(window: WebviewWindow) -> Result<()> {
    if window.label() != "starpet" {
        return Err("此操作仅用于星伴窗口。".into());
    }
    window
        .start_dragging()
        .map_err(|e| format!("无法拖动星伴：{e}"))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppState {
    clips: Vec<Clip>,
    groups: Vec<Group>,
    settings: Settings,
    onboarding_completed: bool,
    clipboard_error: Option<String>,
    shortcut_error: Option<String>,
    window_error: Option<String>,
}
#[tauri::command]
fn get_state(context: AppContext<'_>) -> Result<AppState> {
    let store = lock(&context.store)?;
    Ok(AppState {
        clips: store.clips()?,
        groups: store.groups()?,
        settings: store.settings()?,
        onboarding_completed: store.onboarding_completed()?,
        clipboard_error: lock(&context.clipboard_error)?.clone(),
        shortcut_error: lock(&context.shortcut_error)?.clone(),
        window_error: lock(&context.window_error)?.clone(),
    })
}
#[tauri::command]
fn complete_onboarding(app: AppHandle, context: AppContext<'_>) -> Result<()> {
    let _write = write_guard(&context)?;
    lock(&context.store)?.complete_onboarding()?;
    changed(&app);
    Ok(())
}
#[tauri::command]
fn add_clip(
    app: AppHandle,
    context: AppContext<'_>,
    content: String,
    title: String,
    group_id: Option<i64>,
    keep_forever: Option<bool>,
) -> Result<Clip> {
    let _write = write_guard(&context)?;
    let clip = lock(&context.store)?.add_protected(
        &content,
        &title,
        group_id,
        keep_forever.unwrap_or(false),
    )?;
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
    let _write = write_guard(&context)?;
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
    let _write = write_guard(&context)?;
    let clip = lock(&context.store)?.save_code(id, &title, &content, &language, group_id)?;
    changed(&app);
    Ok(clip)
}
#[tauri::command]
fn delete_clip(app: AppHandle, context: AppContext<'_>, id: i64) -> Result<()> {
    let _write = write_guard(&context)?;
    lock(&context.store)?.delete(id)?;
    *lock(&context.cleanup)? = None;
    changed(&app);
    Ok(())
}
#[tauri::command]
fn copy_clip(app: AppHandle, context: AppContext<'_>, id: i64) -> Result<()> {
    let _write = write_guard(&context)?;
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
    let _write = write_guard(&context)?;
    let group = lock(&context.store)?.create_group(&name, &color)?;
    changed(&app);
    Ok(group)
}
#[tauri::command]
fn delete_group(app: AppHandle, context: AppContext<'_>, id: i64) -> Result<()> {
    let _write = write_guard(&context)?;
    lock(&context.store)?.delete_group(id)?;
    changed(&app);
    Ok(())
}
#[tauri::command]
fn set_theme(app: AppHandle, context: AppContext<'_>, theme: String) -> Result<Settings> {
    let _write = write_guard(&context)?;
    let settings = lock(&context.store)?.set_theme(&theme)?;
    changed(&app);
    Ok(settings)
}
#[tauri::command]
fn save_settings(app: AppHandle, context: AppContext<'_>, settings: Settings) -> Result<()> {
    let _write = write_guard(&context)?;
    settings.validate()?;
    let old = lock(&context.store)?.settings()?;
    let shortcut: Shortcut = settings
        .shortcut
        .parse()
        .map_err(|_| "快捷键格式无效，例如 CommandOrControl+Shift+V。")?;
    let old_shortcut = old.shortcut.parse::<Shortcut>().ok();
    let shortcut_changed = old_shortcut != Some(shortcut);
    let window = app
        .get_webview_window("main")
        .ok_or("主窗口不存在，未保存偏好设置。")?;
    let previous_top = window.is_always_on_top().unwrap_or(old.always_on_top);
    if shortcut_changed {
        app.global_shortcut()
            .register(shortcut)
            .map_err(|_| "该快捷键无法注册，可能被其他应用占用。")?;
    }
    if let Err(error) = window.set_always_on_top(settings.always_on_top) {
        if shortcut_changed {
            let _ = app.global_shortcut().unregister(shortcut);
        }
        let message = format!("无法应用窗口置顶，偏好设置未保存：{error}");
        *lock(&context.window_error)? = Some(message.clone());
        changed(&app);
        return Err(message);
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
        let rollback_error = window
            .set_always_on_top(previous_top)
            .err()
            .map(|error| format!("恢复原窗口置顶状态失败，请重试保存偏好：{error}"));
        *lock(&context.window_error)? = rollback_error.clone();
        changed(&app);
        return Err(match rollback_error {
            Some(rollback) => format!("{error}；{rollback}"),
            None => error,
        });
    }
    if shortcut_changed {
        if let Some(old_shortcut) = old_shortcut {
            let _ = app.global_shortcut().unregister(old_shortcut);
        }
        *lock(&context.shortcut_error)? = None;
    }
    *lock(&context.window_error)? = None;
    changed(&app);
    Ok(())
}
#[tauri::command]
fn clear_history(app: AppHandle, context: AppContext<'_>) -> Result<()> {
    let _write = write_guard(&context)?;
    lock(&context.store)?.clear_history()?;
    *lock(&context.cleanup)? = None;
    changed(&app);
    Ok(())
}
#[tauri::command]
fn set_keep_forever(
    app: AppHandle,
    context: AppContext<'_>,
    id: i64,
    keep_forever: bool,
) -> Result<()> {
    let _write = write_guard(&context)?;
    lock(&context.store)?.set_keep_forever(id, keep_forever)?;
    changed(&app);
    Ok(())
}
#[tauri::command]
async fn preview_cleanup(context: AppContext<'_>, filter: CleanupFilter) -> Result<CleanupPreview> {
    let context = context.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _write = write_guard(&context)?;
        let mut pending = lock(&context.cleanup)?;
        // A failed replacement preview also invalidates the old confirmation.
        *pending = None;
        let snapshot = lock(&context.store)?.preview_cleanup(&filter)?;
        let preview = snapshot.preview.clone();
        *pending = Some(snapshot);
        Ok(preview)
    })
    .await
    .map_err(|_| "清理预览任务失败。".to_string())?
}
#[tauri::command]
async fn execute_cleanup(
    app: AppHandle,
    context: AppContext<'_>,
    token: String,
    compact: bool,
) -> Result<CleanupResult> {
    let context = context.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _write = write_guard(&context)?;
        let mut pending = lock(&context.cleanup)?;
        if pending
            .as_ref()
            .is_none_or(|snapshot| snapshot.preview.token != token)
        {
            return Err(STALE_CLEANUP.into());
        }
        let snapshot = pending.take().ok_or(STALE_CLEANUP)?;
        let result = lock(&context.store)?.execute_cleanup(snapshot, compact)?;
        changed(&app);
        Ok(result)
    })
    .await
    .map_err(|_| "清理任务失败，请刷新预览确认当前状态。".to_string())?
}
#[tauri::command]
fn export_backup(context: AppContext<'_>) -> Result<String> {
    lock(&context.store)?.export()
}
#[tauri::command]
fn import_backup(app: AppHandle, context: AppContext<'_>, data: String) -> Result<usize> {
    let _write = write_guard(&context)?;
    let count = lock(&context.store)?.import(&data)?;
    *lock(&context.cleanup)? = None;
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
async fn save_export(
    app: AppHandle,
    data: String,
    format: String,
    filename: Option<String>,
) -> Result<bool> {
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
    let filename = filename
        .and_then(|name| {
            let name = name.rsplit(['/', '\\']).next()?.trim();
            if name.is_empty() || name.chars().count() > 200 || name.chars().any(char::is_control) {
                return None;
            }
            let stem = Path::new(name).file_stem()?.to_str()?;
            if stem.is_empty()
                || stem == "."
                || stem == ".."
                || stem.contains([':', '*', '?', '"', '<', '>', '|'])
            {
                return None;
            }
            let filename = format!("{stem}.{format}");
            (filename.chars().count() <= 200).then_some(filename)
        })
        .unwrap_or_else(|| format!("starclip-card.{format}"));
    tauri::async_runtime::spawn_blocking(move || {
        let Some(file) = app
            .dialog()
            .file()
            .add_filter("Starclip export", &[format.as_str()])
            .set_file_name(filename)
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

#[tauri::command]
async fn handwriting_assets(
    context: AppContext<'_>,
) -> Result<crate::handwriting::HandwritingAssets> {
    let directory = lock(&context.storage)?.handwriting_directory();
    tauri::async_runtime::spawn_blocking(move || crate::handwriting::assets(&directory))
        .await
        .map_err(|_| "无法读取手写应用素材。".to_string())
}
#[tauri::command]
async fn choose_handwriting_app(
    app: AppHandle,
    context: AppContext<'_>,
) -> Result<crate::handwriting::HandwritingAssets> {
    let context = context.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let picker = app
            .dialog()
            .file()
            .set_title("选择星笺.app 或 xingjian CLI");
        #[cfg(windows)]
        let picker = picker.add_filter("星笺 CLI", &["exe"]);
        let selected = picker.blocking_pick_file();
        let storage = write_guard(&context)?;
        let Some(selected) = selected else {
            return Ok(crate::handwriting::assets(&storage.handwriting_directory()));
        };
        let path = selected.into_path().map_err(|_| "请选择本机应用文件。")?;
        crate::handwriting::select_app(&storage.current, &path)
    })
    .await
    .map_err(|_| "选择手写应用失败。".to_string())?
}
#[tauri::command]
async fn render_handwriting(
    context: AppContext<'_>,
    options: crate::handwriting::HandwritingOptions,
) -> Result<String> {
    let directory = lock(&context.storage)?.handwriting_directory();
    tauri::async_runtime::spawn_blocking(move || crate::handwriting::render(&directory, options))
        .await
        .map_err(|_| "手写渲染任务失败。".to_string())?
}

#[tauri::command]
async fn get_data_location(context: AppContext<'_>) -> Result<DataLocation> {
    let context = context.inner().clone();
    tauri::async_runtime::spawn_blocking(move || lock(&context.storage)?.info())
        .await
        .map_err(|_| "读取数据目录失败。".to_string())?
}
#[tauri::command]
async fn choose_data_directory(
    app: AppHandle,
    context: AppContext<'_>,
    mode: DirectoryMode,
) -> Result<Option<DataDirectoryPlan>> {
    let context = context.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let title = if mode == DirectoryMode::Move {
            "选择父目录：将在其中新建星笺数据子目录"
        } else {
            "选择含 starclip.sqlite3 的已有星笺数据目录"
        };
        let Some(selected) = app.dialog().file().set_title(title).blocking_pick_folder() else {
            return Ok(None);
        };
        let path = selected.into_path().map_err(|_| "请选择本机目录。")?;
        lock(&context.storage)?.plan(mode, &path).map(Some)
    })
    .await
    .map_err(|_| "选择数据目录失败。".to_string())?
}
#[tauri::command]
async fn apply_data_directory(
    app: AppHandle,
    context: AppContext<'_>,
    mode: DirectoryMode,
    target_path: String,
) -> Result<DataLocation> {
    let context = context.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut storage = lock(&context.storage)?;
        let mut info = storage.info()?;
        let store = lock(&context.store)?;
        storage.apply(&store, mode, Path::new(&target_path))?;
        info.pending_path = storage.pending_path();
        changed(&app);
        Ok(info)
    })
    .await
    .map_err(|_| "切换数据目录失败。".to_string())?
}
#[tauri::command]
fn restart_app(app: AppHandle, context: AppContext<'_>) -> Result<()> {
    lock(&context.storage)?.release_for_restart()?;
    app.restart();
}
#[tauri::command]
async fn open_data_directory(context: AppContext<'_>) -> Result<()> {
    let directory = lock(&context.storage)?.current.clone();
    tauri::async_runtime::spawn_blocking(move || {
        if !directory.is_dir() {
            return Err("当前数据目录无法访问，请检查磁盘连接。".into());
        }
        #[cfg(target_os = "macos")]
        let mut command = std::process::Command::new("open");
        #[cfg(target_os = "windows")]
        let mut command = std::process::Command::new("explorer.exe");
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        let mut command = std::process::Command::new("xdg-open");
        command
            .arg(&directory)
            .spawn()
            .map_err(|error| format!("无法打开数据目录：{error}"))?;
        Ok(())
    })
    .await
    .map_err(|_| "打开数据目录失败。".to_string())?
}

fn offer_storage_recovery(app: &AppHandle, default: PathBuf, error: String) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
    let handle = app.clone();
    app.dialog().message(format!("{error}\n\n为防止写入分叉，星笺没有新建或切回数据库。可以重新连接磁盘后退出重试，或选择已有数据目录恢复。"))
        .title("星笺无法打开数据目录")
        .buttons(tauri_plugin_dialog::MessageDialogButtons::OkCancelCustom("选择已有目录".into(), "退出".into()))
        .show(move |choose| {
            if !choose { handle.exit(0); return; }
            let app = handle.clone();
            handle.dialog().file().set_title("选择要恢复使用的已有星笺数据目录").pick_folder(move |selected| {
                let Some(selected) = selected else { app.exit(0); return; };
                thread::spawn(move || {
                    let result = selected.into_path().map_err(|_| "请选择本机目录。".to_string()).and_then(|path| Storage::recover(&default, &path));
                    match result {
                        Ok(()) => app.restart(),
                        Err(error) => offer_storage_recovery(&app, default, error),
                    }
                });
            });
        });
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
    thread::spawn(move || {
        let mut maintenance_at = Instant::now() + Duration::from_secs(60);
        loop {
            thread::sleep(Duration::from_millis(700));
            // Every mutation takes this gate before reader/store locks. A completed
            // migration leaves it paused until restart, including scheduled cleanup.
            let Ok(storage) = lock(&context.storage) else {
                continue;
            };
            if storage.restart_pending() {
                continue;
            }
            if Instant::now() >= maintenance_at {
                maintenance_at = Instant::now() + Duration::from_secs(60);
                let removed = lock(&context.store).and_then(|store| {
                    if store.settings()?.retention_days == 0 {
                        return Ok(0);
                    }
                    store.prune_history()
                });
                match removed {
                    Ok(count) if count > 0 => changed(&app),
                    Err(error) => set_clipboard_error(&app, &context, Some(error)),
                    _ => {}
                }
            }
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
            let default = app.path().app_data_dir()?;
            let storage = match Storage::open(&default, &app.path().app_config_dir()?) {
                Ok(storage) => storage,
                Err(error) => {
                    offer_storage_recovery(app.handle(), default, error);
                    return Ok(());
                }
            };
            let db_path = storage.current.join("starclip.sqlite3");
            let store = match Store::open(&db_path) {
                Ok(store) => store,
                Err(error) => {
                    offer_storage_recovery(app.handle(), default, error);
                    return Ok(());
                }
            };
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
            let window_error = store
                .settings()
                .and_then(|settings| {
                    app.get_webview_window("main")
                        .ok_or("主窗口不存在。".to_string())?
                        .set_always_on_top(settings.always_on_top)
                        .map_err(|error| {
                            format!("无法应用已保存的窗口置顶设置，可在偏好设置中重试：{error}")
                        })
                })
                .err();
            let context = Arc::new(Context {
                storage: Mutex::new(storage),
                store: Mutex::new(store),
                reader: Mutex::new(Reader {
                    clipboard: None,
                    previous: None,
                    armed: false,
                }),
                clipboard_error: Mutex::new(None),
                shortcut_error: Mutex::new(shortcut_error),
                window_error: Mutex::new(window_error),
                cleanup: Mutex::new(None),
                starpet_operation: Mutex::new(()),
            });
            app.manage(context.clone());
            let show = MenuItem::with_id(app, "show", "打开星笺", true, None::<&str>)?;
            let pause = MenuItem::with_id(app, "pause", "暂停 / 继续记录", true, None::<&str>)?;
            let starpet = MenuItem::with_id(app, "starpet", "显示 / 隐藏星伴", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出星笺", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &pause, &starpet, &quit])?;
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
                    "starpet" => {
                        let app = app.clone();
                        let context = app.state::<Arc<Context>>().inner().clone();
                        tauri::async_runtime::spawn_blocking(move || {
                            if let Err(error) = update_starpet(&app, &context, |value| {
                                value.enabled = !value.enabled
                            }) {
                                starpet_error(&app, error);
                            }
                        });
                    }
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
            let pet_app = app.handle().clone();
            let pet_context = context.clone();
            tauri::async_runtime::spawn_blocking(move || {
                if let Err(error) = update_starpet(&pet_app, &pet_context, |_| {}) {
                    starpet_error(&pet_app, error);
                }
            });
            monitor(app.handle().clone(), context);
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                match window.label() {
                    "main" => {
                        api.prevent_close();
                        let _ = window.hide();
                    }
                    "starpet" => {
                        api.prevent_close();
                        let app = window.app_handle().clone();
                        let context = app.state::<Arc<Context>>().inner().clone();
                        tauri::async_runtime::spawn_blocking(move || {
                            if let Err(error) =
                                update_starpet(&app, &context, |value| value.enabled = false)
                            {
                                starpet_error(&app, error);
                            }
                        });
                    }
                    _ => {}
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_state,
            get_starpet_preferences,
            set_starpet_preferences,
            show_main_window,
            starpet_start_dragging,
            complete_onboarding,
            add_clip,
            update_clip,
            save_code,
            delete_clip,
            copy_clip,
            create_group,
            delete_group,
            save_settings,
            set_theme,
            clear_history,
            set_keep_forever,
            preview_cleanup,
            execute_cleanup,
            export_backup,
            import_backup,
            save_backup,
            save_export,
            hide_window,
            handwriting_assets,
            choose_handwriting_app,
            render_handwriting,
            get_data_location,
            choose_data_directory,
            apply_data_directory,
            open_data_directory,
            restart_app
        ])
        .run(tauri::generate_context!())
        .expect("星笺启动失败，请检查本地数据目录权限或磁盘空间。");
}
