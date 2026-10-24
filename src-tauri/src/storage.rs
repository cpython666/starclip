//! Data-directory switching is committed last. The old directory is never removed.
use crate::store::{Result, Store};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, DirBuilder, File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const DATABASE: &str = "starclip.sqlite3";
const SELECTION: &str = "handwriting-app.json";
const LOCATOR: &str = "data-location.json";

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DirectoryMode {
    Move,
    Use,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataLocation {
    pub current_path: String,
    pub default_path: String,
    pub config_path: String,
    pub pending_path: Option<String>,
    pub total_bytes: u64,
    pub database_bytes: u64,
    pub warnings: Vec<String>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataDirectoryPlan {
    pub mode: DirectoryMode,
    pub current_path: String,
    pub target_path: String,
    pub total_bytes: u64,
    pub warnings: Vec<String>,
}
#[derive(Deserialize, Serialize)]
struct Locator {
    version: u32,
    path: PathBuf,
}

// A single mutex around this object is also the write gate in desktop.rs.
// Both directories remain locked between a successful switch and process restart.
pub struct Storage {
    pub current: PathBuf,
    default: PathBuf,
    legacy_config: PathBuf,
    pending: Option<PathBuf>,
    _active_lock: File,
    pending_lock: Option<File>,
}

fn display(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}
fn io_error(error: std::io::Error) -> String {
    format!("数据目录操作失败：{error}")
}
fn nonce() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}
fn create_private_directory(path: &Path) -> Result<()> {
    let mut builder = DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path).map_err(io_error)
}
fn private_file(path: &Path) -> Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path).map_err(io_error)
}
fn lock_directory(path: &Path) -> Result<File> {
    let lock_path = path.join(".starclip.lock");
    if fs::symlink_metadata(&lock_path).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        return Err("数据目录锁文件不能是符号链接。".into());
    }
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options.open(lock_path).map_err(io_error)?;
    file.try_lock().map_err(|_| {
        "该数据目录正被另一个星笺实例使用，或所在磁盘不支持文件锁。请关闭其他实例后重试。"
    })?;
    Ok(file)
}
fn read_small(path: &Path) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(io_error)?
        .take(16 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(io_error)?;
    if bytes.len() > 16 * 1024 {
        return Err("数据目录配置过大，请检查配置文件。".into());
    }
    Ok(bytes)
}
fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().ok_or("无效的配置文件路径。")?;
    let temporary = parent.join(format!(
        ".starclip-location-{}-{}.tmp",
        std::process::id(),
        nonce()
    ));
    let result = (|| {
        let mut file = private_file(&temporary)?;
        file.write_all(bytes)
            .and_then(|_| file.sync_all())
            .map_err(io_error)?;
        drop(file);
        fs::rename(&temporary, path).map_err(io_error)?;
        // Once rename succeeds the switch is committed; a directory fsync failure must
        // not resume writing to the old database with a locator already pointing elsewhere.
        #[cfg(unix)]
        if let Ok(directory) = File::open(parent) {
            let _ = directory.sync_all();
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}
fn write_locator(default: &Path, path: &Path) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(&Locator {
        version: 1,
        path: path.to_owned(),
    })
    .map_err(|error| error.to_string())?;
    atomic_write(&default.join(LOCATOR), &bytes)
}
fn check_directory(path: &Path) -> Result<PathBuf> {
    let path = path.canonicalize().map_err(|_| {
        format!(
            "数据目录不存在或无法访问：{}。未创建新数据库。",
            display(path)
        )
    })?;
    if !path.is_dir() {
        return Err("请选择包含 starclip.sqlite3 的数据目录。".into());
    }
    let database = path.join(DATABASE);
    if fs::symlink_metadata(&database).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        return Err("数据库不能是指向其他位置的符号链接。".into());
    }
    Store::validate_database(&database)?;
    Ok(path)
}

/// Resolve renderer selections copied between machines without trusting `..` paths.
/// Managed renderer paths are relative so the whole directory remains portable.
pub fn renderer_path(directory: &Path, selected: &Path) -> Result<PathBuf> {
    // Never replace a user's absolute external selection with a similarly named
    // program inside this directory. A missing external program must be reselected.
    if !selected.is_absolute() {
        if !selected
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
            || !selected.starts_with("renderers")
        {
            return Err("相对渲染器路径必须位于数据目录的 renderers 文件夹。".into());
        }
        return Ok(directory.join(selected));
    }
    Ok(selected.to_owned())
}
fn selection_data(directory: &Path, source: &Path) -> Result<Option<(Vec<u8>, bool)>> {
    let path = source.join(SELECTION);
    if !path.exists() {
        return Ok(None);
    }
    if fs::symlink_metadata(&path)
        .map_err(io_error)?
        .file_type()
        .is_symlink()
    {
        return Err("手写应用配置不能是符号链接。".into());
    }
    let mut json: serde_json::Value = serde_json::from_slice(&read_small(&path)?)
        .map_err(|_| "手写应用配置损坏，请重新选择渲染器。")?;
    let selected = PathBuf::from(
        json.get("path")
            .and_then(|value| value.as_str())
            .ok_or("手写应用配置缺少 path。")?,
    );
    let resolved = renderer_path(directory, &selected)?;
    let relative = resolved
        .strip_prefix(directory)
        .ok()
        .filter(|relative| relative.starts_with("renderers"));
    let external = relative.is_none();
    if let Some(relative) = relative {
        json["path"] = serde_json::to_value(relative).map_err(|error| error.to_string())?;
    } else if resolved.starts_with(directory) {
        return Err("数据目录中的手写渲染器需放在 renderers 子目录后再迁移。原数据未改变。".into());
    }
    Ok(Some((
        serde_json::to_vec_pretty(&json).map_err(|error| error.to_string())?,
        external,
    )))
}
fn file_bytes(path: &Path) -> Result<u64> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(io_error(error)),
    };
    if metadata.file_type().is_symlink() {
        return Ok(0);
    }
    if metadata.is_file() {
        return Ok(metadata.len());
    }
    let mut bytes = 0u64;
    for entry in fs::read_dir(path).map_err(io_error)? {
        bytes = bytes.saturating_add(file_bytes(&entry.map_err(io_error)?.path())?);
    }
    Ok(bytes)
}
fn managed_bytes(directory: &Path, selection: &Path) -> Result<(u64, u64)> {
    let mut database = 0u64;
    for name in [DATABASE, "starclip.sqlite3-wal", "starclip.sqlite3-shm"] {
        database = database.saturating_add(file_bytes(&directory.join(name))?);
    }
    let total = database
        .saturating_add(file_bytes(&selection.join(SELECTION))?)
        .saturating_add(file_bytes(&directory.join("renderers"))?);
    Ok((total, database))
}
fn copy_tree(source: &Path, target: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(source).map_err(io_error)?;
    if metadata.file_type().is_symlink() {
        return Err("renderers 含符号链接，未迁移。请使用普通文件形式的完整渲染器目录。".into());
    }
    if metadata.is_dir() {
        create_private_directory(target)?;
        for entry in fs::read_dir(source).map_err(io_error)? {
            let entry = entry.map_err(io_error)?;
            copy_tree(&entry.path(), &target.join(entry.file_name()))?;
        }
    } else if metadata.is_file() {
        let mut destination = private_file(target)?;
        let mut input = File::open(source).map_err(io_error)?;
        std::io::copy(&mut input, &mut destination).map_err(io_error)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let executable = metadata.permissions().mode() & 0o111 != 0;
            destination
                .set_permissions(fs::Permissions::from_mode(if executable {
                    0o700
                } else {
                    0o600
                }))
                .map_err(io_error)?;
        }
        destination.sync_all().map_err(io_error)?;
    } else {
        return Err("renderers 只能包含普通文件与目录。".into());
    }
    Ok(())
}

impl Storage {
    pub fn open(default: &Path, legacy_config: &Path) -> Result<Self> {
        if !default.exists() {
            fs::create_dir_all(default).map_err(io_error)?;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(default, fs::Permissions::from_mode(0o700)).map_err(io_error)?;
        }
        let default = default.canonicalize().map_err(io_error)?;
        let locator_path = default.join(LOCATOR);
        let current = if fs::symlink_metadata(&locator_path).is_ok() {
            let locator: Locator =
                serde_json::from_slice(&read_small(&locator_path)?).map_err(|_| {
                    format!(
                        "数据位置配置无效：{}。请恢复该配置或选择已有目录。",
                        display(&locator_path)
                    )
                })?;
            if locator.version != 1 || !locator.path.is_absolute() {
                return Err(format!(
                    "不支持的数据位置配置：{}。",
                    display(&locator_path)
                ));
            }
            check_directory(&locator.path).map_err(|error| format!("{error}\n定位配置：{}\n旧数据目录未被删除；可重新连接磁盘或选择保留的已有数据目录。", display(&locator_path)))?
        } else {
            if default.join(DATABASE).exists() {
                check_directory(&default)?;
            }
            default.clone()
        };
        let active_lock = lock_directory(&current)?;
        Ok(Self {
            current,
            default,
            legacy_config: legacy_config.to_owned(),
            pending: None,
            _active_lock: active_lock,
            pending_lock: None,
        })
    }

    pub fn writable(&self) -> Result<()> {
        if self.pending.is_some() {
            return Err("数据目录已切换，记录与修改已暂停。请立即重启星笺以使用新目录。".into());
        }
        Ok(())
    }
    pub fn restart_pending(&self) -> bool {
        self.pending.is_some()
    }
    pub fn pending_path(&self) -> Option<String> {
        self.pending.as_deref().map(display)
    }
    pub fn release_for_restart(&mut self) -> Result<()> {
        if !self.restart_pending() {
            return Err("当前没有等待重启的数据目录切换。".into());
        }
        // Tauri spawns the replacement process before exiting. Release its target
        // lock first, while pending continues to forbid writes in this process.
        self.pending_lock.take();
        Ok(())
    }
    pub fn handwriting_directory(&self) -> PathBuf {
        if self.current == self.default
            && !self.current.join(SELECTION).exists()
            && self.legacy_config.join(SELECTION).exists()
        {
            self.legacy_config.clone()
        } else {
            self.current.clone()
        }
    }
    pub fn info(&self) -> Result<DataLocation> {
        let selection = self.handwriting_directory();
        let (total_bytes, database_bytes) = managed_bytes(&self.current, &selection)?;
        let warnings = match selection_data(&self.current, &selection) {
            Ok(Some((_, true))) => vec!["当前手写渲染器位于数据目录外，仅迁移其路径设置，不复制外部程序；在其他电脑上可能需要重新选择渲染器。".into()],
            Err(error) => vec![error],
            _ => vec![],
        };
        Ok(DataLocation {
            current_path: display(&self.current),
            default_path: display(&self.default),
            config_path: display(&self.default.join(LOCATOR)),
            pending_path: self.pending.as_deref().map(display),
            total_bytes,
            database_bytes,
            warnings,
        })
    }
    pub fn plan(&self, mode: DirectoryMode, selected: &Path) -> Result<DataDirectoryPlan> {
        self.writable()?;
        let info = self.info()?;
        let target = match mode {
            DirectoryMode::Move => {
                let parent = selected.canonicalize().map_err(io_error)?;
                if !parent.is_dir() {
                    return Err("请选择用于存放数据的文件夹。".into());
                }
                parent.join(format!("Starclip-{}", nonce()))
            }
            DirectoryMode::Use => check_directory(selected)?,
        };
        if target == self.current {
            return Err("这已经是当前数据目录。".into());
        }
        if mode == DirectoryMode::Move && target.starts_with(&self.current) {
            return Err("目标不能位于当前数据目录内部，请选择其他位置。".into());
        }
        let mut warnings = if mode == DirectoryMode::Use {
            vec!["采用所选目录的历史和偏好设置，不合并当前数据。当前目录保留；在另一台电脑同时打开同一目录可能导致冲突，请先退出另一台电脑上的星笺。".into()]
        } else {
            info.warnings
        };
        let total_bytes = if mode == DirectoryMode::Use {
            match selection_data(&target, &target) {
                Ok(Some((_, true))) => warnings.push(
                    "目标目录记录了外部手写渲染器路径；在本机不可用时需要重新选择渲染器。".into(),
                ),
                Err(error) => warnings.push(error),
                _ => {}
            }
            managed_bytes(&target, &target)?.0
        } else {
            info.total_bytes
        };
        Ok(DataDirectoryPlan {
            mode,
            current_path: info.current_path,
            target_path: display(&target),
            total_bytes,
            warnings,
        })
    }

    /// Caller must hold the desktop write gate and Store lock until this returns.
    pub fn apply(&mut self, store: &Store, mode: DirectoryMode, target: &Path) -> Result<()> {
        self.writable()?;
        match mode {
            DirectoryMode::Use => {
                let target = check_directory(target)?;
                if target == self.current {
                    return Err("这已经是当前数据目录。".into());
                }
                let target_lock = lock_directory(&target)?;
                check_directory(&target)?;
                write_locator(&self.default, &target)?;
                self.pending = Some(target);
                self.pending_lock = Some(target_lock);
            }
            DirectoryMode::Move => {
                if !target.is_absolute() || target.file_name().is_none() {
                    return Err("目标必须是新的绝对目录路径。".into());
                }
                let parent = target
                    .parent()
                    .ok_or("目标目录无效。")?
                    .canonicalize()
                    .map_err(io_error)?;
                let target = parent.join(target.file_name().unwrap());
                if target.starts_with(&self.current) {
                    return Err("目标不能位于当前数据目录内部，请选择其他位置。".into());
                }
                create_private_directory(&target).map_err(|error| {
                    format!("目标必须是不存在的新子目录，不会覆盖任何已有目录：{error}")
                })?;
                let result = (|| -> Result<File> {
                    let target_lock = lock_directory(&target)?;
                    let db = target.join(DATABASE);
                    drop(private_file(&db)?);
                    store.snapshot(&db)?;
                    File::open(&db)
                        .and_then(|file| file.sync_all())
                        .map_err(io_error)?;
                    let renderers = self.current.join("renderers");
                    if fs::symlink_metadata(&renderers).is_ok() {
                        copy_tree(&renderers, &target.join("renderers"))?;
                    }
                    if let Some((bytes, _)) =
                        selection_data(&self.current, &self.handwriting_directory())?
                    {
                        let mut file = private_file(&target.join(SELECTION))?;
                        file.write_all(&bytes)
                            .and_then(|_| file.sync_all())
                            .map_err(io_error)?;
                    }
                    check_directory(&target)?;
                    write_locator(&self.default, &target)?;
                    Ok(target_lock)
                })();
                match result {
                    Ok(target_lock) => {
                        self.pending = Some(target);
                        self.pending_lock = Some(target_lock);
                    }
                    Err(error) => {
                        // Only remove the exclusively created destination owned by this attempt.
                        let _ = fs::remove_dir_all(&target);
                        return Err(error);
                    }
                }
            }
        }
        Ok(())
    }

    /// Startup recovery never initializes a missing selected database.
    pub fn recover(default: &Path, selected: &Path) -> Result<()> {
        let selected = check_directory(selected)?;
        let _lock = lock_directory(&selected)?;
        write_locator(default, &selected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    struct Scratch(PathBuf);
    impl Scratch {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "starclip-storage-test-{}-{}",
                std::process::id(),
                nonce()
            ));
            create_private_directory(&path).unwrap();
            Self(path.canonicalize().unwrap())
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn migration_snapshots_wal_preferences_and_portable_renderer_keeps_old_data() {
        let scratch = Scratch::new();
        let source = scratch.0.join("source");
        let mut storage = Storage::open(&source, &source).unwrap();
        let store = Store::open(&source.join(DATABASE)).unwrap();
        let wal = Connection::open(source.join(DATABASE)).unwrap();
        wal.pragma_update(None, "journal_mode", "WAL").unwrap();
        wal.pragma_update(None, "wal_autocheckpoint", 0).unwrap();
        let clip = store.add("迁移测试：星河入梦", "原创数据", None).unwrap();
        store.update(clip.id, "保留标题", None, true).unwrap();
        let mut settings = store.settings().unwrap();
        settings.theme = "forest".into();
        settings.show_startup = false;
        store.save_settings(&settings).unwrap();
        assert!(
            source
                .join("starclip.sqlite3-wal")
                .metadata()
                .unwrap()
                .len()
                > 0
        );
        create_private_directory(&source.join("renderers")).unwrap();
        fs::write(
            source.join("renderers/xingjian"),
            b"synthetic renderer, never executed",
        )
        .unwrap();
        fs::write(
            source.join(SELECTION),
            serde_json::json!({"path":source.join("renderers/xingjian")}).to_string(),
        )
        .unwrap();
        let target = scratch.0.join("migrated");
        storage.apply(&store, DirectoryMode::Move, &target).unwrap();
        assert!(storage.writable().is_err());
        assert_eq!(
            storage.info().unwrap().pending_path.as_deref(),
            Some(target.to_str().unwrap())
        );
        assert!(source.join(DATABASE).is_file());
        assert!(source.join(SELECTION).is_file());
        Store::validate_database(&target.join(DATABASE)).unwrap();
        let copied = Store::open(&target.join(DATABASE)).unwrap();
        assert_eq!(copied.clip(clip.id).unwrap().title, "保留标题");
        assert!(copied.clip(clip.id).unwrap().pinned);
        assert_eq!(copied.settings().unwrap(), settings);
        let json: serde_json::Value =
            serde_json::from_slice(&fs::read(target.join(SELECTION)).unwrap()).unwrap();
        assert_eq!(json["path"], "renderers/xingjian");
        assert_eq!(
            renderer_path(&target, Path::new(json["path"].as_str().unwrap())).unwrap(),
            target.join("renderers/xingjian")
        );
        assert!(
            Storage::open(&source, &source).is_err(),
            "pending target stays locked"
        );
        storage.release_for_restart().unwrap();
        assert!(
            storage.writable().is_err(),
            "restart handoff never resumes old writes"
        );
        let reopened = Storage::open(&source, &source).unwrap();
        assert_eq!(reopened.current, target);
    }

    #[test]
    fn existing_target_and_failed_commit_do_not_change_locator_or_old_database() {
        let scratch = Scratch::new();
        let source = scratch.0.join("source");
        let mut storage = Storage::open(&source, &source).unwrap();
        let store = Store::open(&source.join(DATABASE)).unwrap();
        store.add("失败恢复测试", "", None).unwrap();
        let target = scratch.0.join("existing");
        create_private_directory(&target).unwrap();
        fs::write(target.join("keep.txt"), "原文件").unwrap();
        assert!(storage.apply(&store, DirectoryMode::Move, &target).is_err());
        assert_eq!(
            fs::read_to_string(target.join("keep.txt")).unwrap(),
            "原文件"
        );
        assert!(storage.plan(DirectoryMode::Move, &source).is_err());
        assert!(storage
            .apply(&store, DirectoryMode::Move, &source.join("nested"))
            .is_err());
        assert!(!source.join("nested").exists());
        create_private_directory(&source.join(LOCATOR)).unwrap(); // force atomic rename failure
        let failed = scratch.0.join("failed");
        assert!(storage.apply(&store, DirectoryMode::Move, &failed).is_err());
        assert!(!failed.exists());
        assert!(source.join(LOCATOR).is_dir());
        assert!(storage.writable().is_ok());
        assert_eq!(store.clips().unwrap().len(), 1);
    }

    #[test]
    fn missing_or_corrupt_selected_database_is_never_initialized() {
        let scratch = Scratch::new();
        let source = scratch.0.join("source");
        let storage = Storage::open(&source, &source).unwrap();
        let _store = Store::open(&source.join(DATABASE)).unwrap();
        let missing = scratch.0.join("disconnected-drive");
        write_locator(&source, &missing).unwrap();
        drop(storage);
        assert!(Storage::open(&source, &source).is_err());
        assert!(!missing.exists());
        let invalid = scratch.0.join("invalid");
        create_private_directory(&invalid).unwrap();
        assert!(Storage::recover(&source, &invalid).is_err());
        assert!(!invalid.join(DATABASE).exists());
        fs::write(invalid.join(DATABASE), "不是数据库").unwrap();
        assert!(Storage::recover(&source, &invalid).is_err());
        assert_eq!(
            fs::read_to_string(invalid.join(DATABASE)).unwrap(),
            "不是数据库"
        );
        let locator: Locator =
            serde_json::from_slice(&fs::read(source.join(LOCATOR)).unwrap()).unwrap();
        assert_eq!(locator.path, missing);
    }

    #[test]
    fn adoption_is_read_only_until_confirmed_and_respects_directory_lock() {
        let scratch = Scratch::new();
        let source = scratch.0.join("source");
        let target = scratch.0.join("other");
        let mut storage = Storage::open(&source, &source).unwrap();
        let store = Store::open(&source.join(DATABASE)).unwrap();
        let other = Storage::open(&target, &target).unwrap();
        let existing = Store::open(&target.join(DATABASE)).unwrap();
        existing.add("另一目录原创片段", "", None).unwrap();
        let before = fs::read(target.join(DATABASE)).unwrap();
        storage.plan(DirectoryMode::Use, &target).unwrap();
        assert_eq!(before, fs::read(target.join(DATABASE)).unwrap());
        assert!(!source.join(LOCATOR).exists());
        assert!(storage.apply(&store, DirectoryMode::Use, &target).is_err());
        drop(other);
        storage.apply(&store, DirectoryMode::Use, &target).unwrap();
        assert!(source.join(DATABASE).exists());
        assert_eq!(before, fs::read(target.join(DATABASE)).unwrap());
    }

    #[test]
    fn validating_legacy_directory_does_not_upgrade_schema_or_prune_history() {
        let scratch = Scratch::new();
        let database = scratch.0.join(DATABASE);
        let connection = Connection::open(&database).unwrap();
        connection.execute_batch("CREATE TABLE groups(id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE, color TEXT NOT NULL);
            CREATE TABLE clips(id INTEGER PRIMARY KEY, content TEXT NOT NULL UNIQUE, title TEXT NOT NULL DEFAULT '',kind TEXT NOT NULL,pinned INTEGER NOT NULL DEFAULT 0,group_id INTEGER REFERENCES groups(id),created_at INTEGER NOT NULL,updated_at INTEGER NOT NULL,copy_count INTEGER NOT NULL DEFAULT 0);
            CREATE TABLE preferences(key TEXT PRIMARY KEY,value TEXT NOT NULL);
            INSERT INTO clips(content,kind,created_at,updated_at) VALUES('旧目录不可在预览时清理','text',0,0);").unwrap();
        let settings = r#"{"paused":true,"theme":"dark","layout":"grid","maxHistory":500,"shortcut":"CommandOrControl+Shift+V","retentionDays":1}"#;
        connection
            .execute("INSERT INTO preferences VALUES('settings',?1)", [settings])
            .unwrap();
        drop(connection);
        let before = fs::read(&database).unwrap();
        Store::validate_database(&database).unwrap();
        assert_eq!(fs::read(&database).unwrap(), before);
        let connection =
            Connection::open_with_flags(&database, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
                .unwrap();
        assert!(connection.prepare("SELECT language FROM clips").is_err());
        assert_eq!(
            connection
                .query_row("SELECT count(*) FROM clips", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            1
        );
    }

    #[test]
    fn legacy_external_selection_is_copied_but_external_program_is_not() {
        let scratch = Scratch::new();
        let source = scratch.0.join("source");
        let legacy = scratch.0.join("legacy-config");
        create_private_directory(&legacy).unwrap();
        let external_directory = scratch.0.join("external");
        create_private_directory(&external_directory).unwrap();
        create_private_directory(&external_directory.join("renderers")).unwrap();
        let external = external_directory.join("renderers/xingjian");
        fs::write(&external, "synthetic external program").unwrap();
        fs::write(
            legacy.join(SELECTION),
            serde_json::json!({"path":external}).to_string(),
        )
        .unwrap();
        let mut storage = Storage::open(&source, &legacy).unwrap();
        let store = Store::open(&source.join(DATABASE)).unwrap();
        create_private_directory(&source.join("renderers")).unwrap();
        fs::write(source.join("renderers/xingjian"), "different local program").unwrap();
        assert_eq!(renderer_path(&source, &external).unwrap(), external);
        assert_eq!(storage.info().unwrap().warnings.len(), 1);
        let target = scratch.0.join("migrated");
        storage.apply(&store, DirectoryMode::Move, &target).unwrap();
        assert!(target.join(SELECTION).exists());
        let selection: serde_json::Value =
            serde_json::from_slice(&fs::read(target.join(SELECTION)).unwrap()).unwrap();
        assert_eq!(selection["path"], external.to_str().unwrap());
        assert!(!target.join("external-renderer").exists());
        assert!(legacy.join(SELECTION).exists());
        assert!(renderer_path(&target, Path::new("../outside")).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn renderer_symlinks_abort_without_copying_external_files() {
        let scratch = Scratch::new();
        let source = scratch.0.join("source");
        let mut storage = Storage::open(&source, &source).unwrap();
        let store = Store::open(&source.join(DATABASE)).unwrap();
        create_private_directory(&source.join("renderers")).unwrap();
        std::os::unix::fs::symlink(&scratch.0, source.join("renderers/outside")).unwrap();
        let target = scratch.0.join("migrated");
        assert!(storage.apply(&store, DirectoryMode::Move, &target).is_err());
        assert!(!target.exists());
        assert!(!source.join(LOCATOR).exists());
        assert!(storage.writable().is_ok());
        let alias = scratch.0.join("source-alias");
        std::os::unix::fs::symlink(&source, &alias).unwrap();
        assert!(storage
            .apply(&store, DirectoryMode::Move, &alias.join("nested"))
            .is_err());
        assert!(!source.join("nested").exists());
    }
}
