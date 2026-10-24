use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

pub type Result<T> = std::result::Result<T, String>;
pub const MAX_CONTENT_BYTES: usize = 256 * 1024;
const MAX_BACKUP_BYTES: usize = 20 * 1024 * 1024;
const MAX_BACKUP_CLIPS: usize = 10_000;
const MAX_BACKUP_GROUPS: usize = 1000;

fn check_backup_limits(bytes: usize, clips: usize, groups: usize) -> Result<()> {
    if bytes > MAX_BACKUP_BYTES || clips > MAX_BACKUP_CLIPS || groups > MAX_BACKUP_GROUPS {
        return Err("备份上限为 20 MB、10000 条星笺和 1000 个分组，请先整理数据。".into());
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Clip {
    pub id: i64,
    pub content: String,
    pub title: String,
    pub kind: String,
    #[serde(default)]
    pub language: Option<String>,
    pub pinned: bool,
    #[serde(default)]
    pub keep_forever: bool,
    pub group_id: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
    pub copy_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub id: i64,
    pub name: String,
    pub color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub paused: bool,
    pub theme: String,
    pub layout: String,
    pub max_history: usize,
    pub shortcut: String,
    #[serde(default)]
    pub always_on_top: bool,
    #[serde(default)]
    pub retention_days: u32,
    #[serde(default = "default_show_startup")]
    pub show_startup: bool,
    #[serde(default = "default_sticker_pack")]
    pub sticker_pack: String,
}

fn default_show_startup() -> bool {
    true
}
fn default_sticker_pack() -> String {
    "auto".into()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            paused: true,
            theme: "dark".into(),
            layout: "grid".into(),
            max_history: 0,
            shortcut: "CommandOrControl+Shift+V".into(),
            always_on_top: false,
            retention_days: 0,
            show_startup: true,
            sticker_pack: default_sticker_pack(),
        }
    }
}

impl Settings {
    pub fn validate(&self) -> Result<()> {
        if !["dark", "light", "forest", "rose", "system"].contains(&self.theme.as_str())
            || !["grid", "list", "horizontal"].contains(&self.layout.as_str())
            || (self.max_history != 0 && !(50..=10_000).contains(&self.max_history))
            || self.shortcut.trim().is_empty()
            || self.shortcut.len() > 80
        {
            return Err("设置无效：历史上限可选不限条数（0）或 50–10000 条。".into());
        }
        if !["auto", "none", "cosmos", "cats", "puppies", "garden"]
            .contains(&self.sticker_pack.as_str())
        {
            return Err("不支持的贴纸组合。".into());
        }
        if ![0, 1, 7, 30, 90, 180, 365].contains(&self.retention_days) {
            return Err("保留天数仅支持不限时间、1、7、30、90、180 或 365 天。".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CleanupFilter {
    pub min_bytes: u64,
    pub older_than_days: u32,
    pub kind: String,
    pub include_grouped: bool,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupItem {
    pub id: i64,
    pub title: String,
    pub kind: String,
    pub bytes: u64,
    pub updated_at: i64,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupPreview {
    pub token: String,
    pub total_count: usize,
    pub protected_count: usize,
    pub matched_count: usize,
    pub content_bytes: u64,
    pub database_bytes: u64,
    pub reusable_bytes: u64,
    pub items: Vec<CleanupItem>,
    pub truncated: bool,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupResult {
    pub deleted_count: usize,
    pub content_bytes: u64,
    pub database_bytes_before: u64,
    pub database_bytes_after: u64,
    pub released_bytes: u64,
    pub warning: Option<String>,
}
pub struct CleanupSnapshot {
    pub preview: CleanupPreview,
    candidates: Vec<Clip>,
}
pub const STALE_CLEANUP: &str = "清理预览已过期，请重新预览后确认。";
static CLEANUP_SEQUENCE: AtomicU64 = AtomicU64::new(0);
fn clip_bytes(clip: &Clip) -> u64 {
    (clip.content.len() + clip.title.len()) as u64
}
fn same_cleanup_record(before: &Clip, current: &Clip) -> bool {
    before.id == current.id
        && before.content == current.content
        && before.title == current.title
        && before.kind == current.kind
        && before.language == current.language
        && before.updated_at == current.updated_at
        && before.created_at == current.created_at
        && before.group_id == current.group_id
        && before.pinned == current.pinned
        && before.keep_forever == current.keep_forever
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Backup {
    format: String,
    version: u32,
    exported_at: i64,
    clips: Vec<Clip>,
    groups: Vec<Group>,
}

pub struct Store {
    conn: Connection,
}

fn sql_error(error: rusqlite::Error) -> String {
    format!("本地数据库操作失败：{error}")
}
fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
fn clip_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Clip> {
    Ok(Clip {
        id: row.get(0)?,
        content: row.get(1)?,
        title: row.get(2)?,
        kind: row.get(3)?,
        pinned: row.get(4)?,
        group_id: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
        copy_count: row.get(8)?,
        language: row.get(9)?,
        keep_forever: row.get(10)?,
    })
}

pub fn validate_content(content: &str, title: &str) -> Result<()> {
    if content.trim().is_empty() {
        return Err("内容不能为空。".into());
    }
    if content.len() > MAX_CONTENT_BYTES {
        return Err("单条内容不能超过 256 KB。".into());
    }
    if title.chars().count() > 200 {
        return Err("标题不能超过 200 个字符。".into());
    }
    Ok(())
}

fn validate_group(name: &str, color: &str) -> Result<()> {
    if name.trim().is_empty() || name.chars().count() > 40 {
        return Err("分组名需为 1–40 个字符。".into());
    }
    if color.len() != 7
        || !color.starts_with('#')
        || !color[1..].bytes().all(|c| c.is_ascii_hexdigit())
    {
        return Err("分组颜色需为 #RRGGBB。".into());
    }
    Ok(())
}

const LANGUAGES: &[&str] = &[
    "plaintext",
    "javascript",
    "typescript",
    "python",
    "rust",
    "json",
    "html",
    "css",
    "bash",
    "sql",
    "markdown",
    "go",
    "java",
];
fn validate_language(language: &str) -> Result<()> {
    if !LANGUAGES.contains(&language) {
        return Err("不支持的代码语言。".into());
    }
    Ok(())
}

pub fn classify(content: &str) -> &'static str {
    let text = content.trim();
    if [4, 7, 9].contains(&text.len())
        && text.starts_with('#')
        && text[1..].bytes().all(|c| c.is_ascii_hexdigit())
    {
        "color"
    } else if (text.starts_with("https://") || text.starts_with("http://"))
        && !text.chars().any(char::is_whitespace)
    {
        "link"
    } else if text.starts_with("```")
        || (text.contains('\n')
            && [
                "const ",
                "function ",
                "fn ",
                "def ",
                "import ",
                "SELECT ",
                "<div",
                "{\n",
            ]
            .iter()
            .any(|s| text.contains(s)))
    {
        "code"
    } else {
        "text"
    }
}

impl Store {
    /// Read-only validation must never initialize, migrate, or prune a selected database.
    pub fn validate_database(path: &Path) -> Result<()> {
        if !path.is_file() {
            return Err("所选目录缺少 starclip.sqlite3，未创建新数据库。".into());
        }
        let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(sql_error)?;
        conn.busy_timeout(std::time::Duration::from_secs(3))
            .map_err(sql_error)?;
        let tx = conn.unchecked_transaction().map_err(sql_error)?;
        let integrity: String = tx
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))
            .map_err(sql_error)?;
        if integrity != "ok" {
            return Err("数据库完整性检查失败，未切换数据目录。".into());
        }
        // The original schema had no language column; normal startup upgrades it later.
        for query in [
            "SELECT id,content,title,kind,pinned,group_id,created_at,updated_at,copy_count FROM clips LIMIT 0",
            "SELECT id,name,color FROM groups LIMIT 0",
            "SELECT key,value FROM preferences LIMIT 0",
        ] {
            tx.prepare(query).map_err(|_| "所选文件不是兼容的星笺数据库。".to_string())?;
        }
        if tx
            .prepare("PRAGMA foreign_key_check")
            .map_err(sql_error)?
            .exists([])
            .map_err(sql_error)?
        {
            return Err("数据库分组引用损坏，未切换数据目录。".into());
        }
        let json: Option<String> = tx
            .query_row(
                "SELECT value FROM preferences WHERE key='settings'",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(sql_error)?;
        if let Some(json) = json {
            let settings: Settings =
                serde_json::from_str(&json).map_err(|_| "目标数据库的偏好设置损坏。")?;
            settings.validate()?;
        }
        Ok(())
    }

    /// SQLite includes committed WAL pages in this consistent standalone snapshot.
    pub fn snapshot(&self, target: &Path) -> Result<()> {
        let target = target.to_str().ok_or("目标路径不是有效的 Unicode 路径。")?;
        self.conn
            .execute("VACUUM main INTO ?1", [target])
            .map_err(sql_error)?;
        Ok(())
    }

    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path).map_err(sql_error)?;
        Self::from_connection(conn)
    }

    fn from_connection(conn: Connection) -> Result<Self> {
        conn.busy_timeout(std::time::Duration::from_secs(3))
            .map_err(sql_error)?;
        conn.execute_batch("PRAGMA foreign_keys=ON;
            PRAGMA secure_delete=ON;
            CREATE TABLE IF NOT EXISTS groups (id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE, color TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS clips (
                id INTEGER PRIMARY KEY, content TEXT NOT NULL UNIQUE, title TEXT NOT NULL DEFAULT '',
                kind TEXT NOT NULL, pinned INTEGER NOT NULL DEFAULT 0, keep_forever INTEGER NOT NULL DEFAULT 0,
                group_id INTEGER REFERENCES groups(id) ON DELETE SET NULL,
                created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL, copy_count INTEGER NOT NULL DEFAULT 0, language TEXT
            );
            CREATE INDEX IF NOT EXISTS clips_updated ON clips(updated_at DESC);
            CREATE TABLE IF NOT EXISTS preferences (key TEXT PRIMARY KEY, value TEXT NOT NULL);").map_err(sql_error)?;
        let columns = {
            let mut stmt = conn
                .prepare("PRAGMA table_info(clips)")
                .map_err(sql_error)?;
            let columns = stmt
                .query_map([], |row| row.get::<_, String>(1))
                .map_err(sql_error)?;
            columns
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(sql_error)?
        };
        if !columns.iter().any(|name| name == "language") {
            conn.execute("ALTER TABLE clips ADD COLUMN language TEXT", [])
                .map_err(sql_error)?;
        }
        if !columns.iter().any(|name| name == "keep_forever") {
            conn.execute(
                "ALTER TABLE clips ADD COLUMN keep_forever INTEGER NOT NULL DEFAULT 0",
                [],
            )
            .map_err(sql_error)?;
        }
        let store = Self { conn };
        store.prune_history()?;
        Ok(store)
    }

    pub fn settings(&self) -> Result<Settings> {
        let json: Option<String> = self
            .conn
            .query_row(
                "SELECT value FROM preferences WHERE key='settings'",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(sql_error)?;
        match json {
            Some(json) => {
                let settings: Settings =
                    serde_json::from_str(&json).map_err(|_| "偏好设置损坏，请从备份恢复。")?;
                settings.validate()?;
                Ok(settings)
            }
            None => Ok(Settings::default()),
        }
    }

    pub fn save_settings(&self, settings: &Settings) -> Result<()> {
        settings.validate()?;
        let json = serde_json::to_string(settings).map_err(|e| e.to_string())?;
        let tx = self.conn.unchecked_transaction().map_err(sql_error)?;
        tx.execute("INSERT INTO preferences(key,value) VALUES('settings',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [json]).map_err(sql_error)?;
        Self::prune_with_settings(&tx, settings, now())?;
        tx.commit().map_err(sql_error)
    }

    pub fn clips(&self) -> Result<Vec<Clip>> {
        let mut stmt = self.conn.prepare("SELECT id,content,title,kind,pinned,group_id,created_at,updated_at,copy_count,language,keep_forever FROM clips ORDER BY updated_at DESC,id DESC").map_err(sql_error)?;
        let rows = stmt.query_map([], clip_from_row).map_err(sql_error)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(sql_error)
    }

    pub fn groups(&self) -> Result<Vec<Group>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id,name,color FROM groups ORDER BY id")
            .map_err(sql_error)?;
        let rows = stmt
            .query_map([], |row| {
                Ok(Group {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    color: row.get(2)?,
                })
            })
            .map_err(sql_error)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(sql_error)
    }

    pub fn clip(&self, id: i64) -> Result<Clip> {
        self.conn.query_row("SELECT id,content,title,kind,pinned,group_id,created_at,updated_at,copy_count,language,keep_forever FROM clips WHERE id=?1", [id], clip_from_row).optional().map_err(sql_error)?.ok_or_else(|| "这条星笺已被删除。".into())
    }

    pub fn add(&self, content: &str, title: &str, group_id: Option<i64>) -> Result<Clip> {
        self.add_protected(content, title, group_id, false)
    }

    pub fn add_protected(
        &self,
        content: &str,
        title: &str,
        group_id: Option<i64>,
        keep_forever: bool,
    ) -> Result<Clip> {
        validate_content(content, title)?;
        let timestamp = now();
        let tx = self.conn.unchecked_transaction().map_err(sql_error)?;
        tx.execute("INSERT INTO clips(content,title,kind,group_id,created_at,updated_at,keep_forever) VALUES(?1,?2,?3,?4,?5,?5,?6)
            ON CONFLICT(content) DO UPDATE SET updated_at=excluded.updated_at,
            title=CASE WHEN excluded.title<>'' THEN excluded.title ELSE clips.title END,
            group_id=COALESCE(excluded.group_id,clips.group_id),
            keep_forever=MAX(clips.keep_forever,excluded.keep_forever)", params![content, title.trim(), classify(content), group_id, timestamp, keep_forever]).map_err(sql_error)?;
        let id = tx
            .query_row("SELECT id FROM clips WHERE content=?1", [content], |row| {
                row.get(0)
            })
            .map_err(sql_error)?;
        Self::prune_with_settings(&tx, &self.settings()?, timestamp)?;
        let clip = self.clip(id)?;
        tx.commit().map_err(sql_error)?;
        Ok(clip)
    }

    pub fn save_code(
        &self,
        id: Option<i64>,
        title: &str,
        content: &str,
        language: &str,
        group_id: Option<i64>,
    ) -> Result<Clip> {
        validate_content(content, title)?;
        validate_language(language)?;
        let timestamp = now();
        let clip = match id {
            Some(id) => {
                self.clip(id)?;
                self.conn.execute("UPDATE clips SET content=?1,title=?2,kind='code',language=?3,group_id=?4,updated_at=?5,pinned=1 WHERE id=?6", params![content,title.trim(),language,group_id,timestamp,id])
                    .map_err(|error| match error {
                        rusqlite::Error::SqliteFailure(ref details, _) if details.extended_code == 2067 => "已存在相同内容，请编辑已有星笺。".into(),
                        other => sql_error(other),
                    })?;
                self.clip(id)
            }
            None => {
                self.conn.execute("INSERT INTO clips(content,title,kind,language,group_id,pinned,created_at,updated_at) VALUES(?1,?2,'code',?3,?4,1,?5,?5)", params![content,title.trim(),language,group_id,timestamp])
                    .map_err(|error| match error {
                        rusqlite::Error::SqliteFailure(ref details, _) if details.extended_code == 2067 => "已存在相同内容，请编辑已有星笺。".into(),
                        other => sql_error(other),
                    })?;
                self.clip(self.conn.last_insert_rowid())
            }
        }?;
        self.prune_history()?;
        Ok(clip)
    }

    pub fn update(&self, id: i64, title: &str, group_id: Option<i64>, pinned: bool) -> Result<()> {
        if title.chars().count() > 200 {
            return Err("标题不能超过 200 个字符。".into());
        }
        let changed = self
            .conn
            .execute(
                "UPDATE clips SET title=?1,group_id=?2,pinned=?3 WHERE id=?4",
                params![title.trim(), group_id, pinned, id],
            )
            .map_err(sql_error)?;
        if changed == 0 {
            return Err("这条星笺已被删除。".into());
        }
        Ok(())
    }

    pub fn mark_copied(&self, id: i64) -> Result<()> {
        self.conn
            .execute("UPDATE clips SET copy_count=copy_count+1 WHERE id=?1", [id])
            .map_err(sql_error)?;
        Ok(())
    }

    pub fn delete(&self, id: i64) -> Result<()> {
        if self.clip(id)?.keep_forever {
            return Err("这条星笺已设为永久保留，请先取消保留再删除。".into());
        }
        self.conn
            .execute("DELETE FROM clips WHERE id=?1 AND keep_forever=0", [id])
            .map_err(sql_error)?;
        Ok(())
    }

    pub fn clear_history(&self) -> Result<()> {
        self.conn
            .execute("DELETE FROM clips WHERE pinned=0 AND keep_forever=0", [])
            .map_err(sql_error)?;
        Ok(())
    }

    pub fn set_keep_forever(&self, id: i64, keep_forever: bool) -> Result<()> {
        let changed = self
            .conn
            .execute(
                "UPDATE clips SET keep_forever=?1 WHERE id=?2",
                params![keep_forever, id],
            )
            .map_err(sql_error)?;
        if changed == 0 {
            return Err("这条星笺已被删除。".into());
        }
        Ok(())
    }

    fn database_bytes(&self) -> Result<u64> {
        let Some(path) = self.conn.path().filter(|path| !path.is_empty()) else {
            return Ok(0);
        };
        let database = fs::metadata(path)
            .map_err(|error| format!("无法读取数据库占用：{error}"))?
            .len();
        let wal = match fs::metadata(format!("{path}-wal")) {
            Ok(metadata) => metadata.len(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
            Err(error) => return Err(format!("无法读取数据库日志占用：{error}")),
        };
        Ok(database.saturating_add(wal))
    }

    /// This only reads rows, file sizes and SQLite page statistics; it never prunes.
    pub fn preview_cleanup(&self, filter: &CleanupFilter) -> Result<CleanupSnapshot> {
        if !["all", "text", "link", "code", "color"].contains(&filter.kind.as_str()) {
            return Err("不支持的清理内容类型。".into());
        }
        let cutoff = now().saturating_sub(i64::from(filter.older_than_days) * 86_400_000);
        let mut total_count = 0;
        let mut protected_count = 0;
        let mut content_bytes = 0;
        let mut candidates = Vec::new();
        let mut stmt = self.conn.prepare("SELECT id,content,title,kind,pinned,group_id,created_at,updated_at,copy_count,language,keep_forever FROM clips").map_err(sql_error)?;
        for row in stmt.query_map([], clip_from_row).map_err(sql_error)? {
            let clip = row.map_err(sql_error)?;
            total_count += 1;
            if clip.pinned || clip.keep_forever {
                protected_count += 1;
                continue;
            }
            let bytes = clip_bytes(&clip);
            if (filter.min_bytes == 0 || bytes > filter.min_bytes)
                && (filter.older_than_days == 0 || clip.updated_at < cutoff)
                && (filter.kind == "all" || clip.kind == filter.kind)
                && (filter.include_grouped || clip.group_id.is_none())
            {
                content_bytes += bytes;
                candidates.push(clip);
            }
        }
        // One exact in-memory snapshot avoids persistent preview tables and trusts no client IDs.
        candidates.sort_by(|a, b| {
            clip_bytes(b)
                .cmp(&clip_bytes(a))
                .then_with(|| b.id.cmp(&a.id))
        });
        let items = candidates
            .iter()
            .take(100)
            .map(|clip| CleanupItem {
                id: clip.id,
                title: clip.title.clone(),
                kind: clip.kind.clone(),
                bytes: clip_bytes(clip),
                updated_at: clip.updated_at,
            })
            .collect();
        let page_size: u64 = self
            .conn
            .query_row("PRAGMA page_size", [], |row| row.get(0))
            .map_err(sql_error)?;
        let free_pages: u64 = self
            .conn
            .query_row("PRAGMA freelist_count", [], |row| row.get(0))
            .map_err(sql_error)?;
        let preview = CleanupPreview {
            token: format!(
                "{:x}-{:x}-{:x}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos(),
                CLEANUP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ),
            total_count,
            protected_count,
            matched_count: candidates.len(),
            content_bytes,
            database_bytes: self.database_bytes()?,
            reusable_bytes: free_pages.saturating_mul(page_size),
            items,
            truncated: candidates.len() > 100,
        };
        Ok(CleanupSnapshot {
            preview,
            candidates,
        })
    }

    fn compact_database(&self) -> Result<()> {
        self.conn.execute_batch("VACUUM").map_err(sql_error)?;
        let busy: i64 = self
            .conn
            .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| row.get(0))
            .map_err(sql_error)?;
        if busy != 0 {
            return Err("数据库日志仍被其他读取占用，尚未全部释放磁盘空间。".into());
        }
        Ok(())
    }

    pub fn execute_cleanup(
        &mut self,
        snapshot: CleanupSnapshot,
        compact: bool,
    ) -> Result<CleanupResult> {
        let before = self.database_bytes()?;
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(sql_error)?;
        for expected in &snapshot.candidates {
            let current = tx.query_row("SELECT id,content,title,kind,pinned,group_id,created_at,updated_at,copy_count,language,keep_forever FROM clips WHERE id=?1", [expected.id], clip_from_row).optional().map_err(sql_error)?;
            if current.as_ref().is_none_or(|current| {
                !same_cleanup_record(expected, current) || current.keep_forever || current.pinned
            }) {
                return Err(STALE_CLEANUP.into());
            }
        }
        let mut deleted_count = 0;
        for clip in &snapshot.candidates {
            let deleted = tx
                .execute(
                    "DELETE FROM clips WHERE id=?1 AND pinned=0 AND keep_forever=0",
                    [clip.id],
                )
                .map_err(sql_error)?;
            if deleted != 1 {
                return Err(STALE_CLEANUP.into());
            }
            deleted_count += deleted;
        }
        tx.commit().map_err(sql_error)?;
        // After this point deletion succeeded. Compaction/stat failures are warnings,
        // never a false failure that invites the user to repeat destructive work.
        let mut warning = if compact {
            self.compact_database()
                .err()
                .map(|error| format!("记录已清理，但磁盘整理未完成：{error}"))
        } else {
            None
        };
        let after = match self.database_bytes() {
            Ok(bytes) => bytes,
            Err(error) => {
                let message = format!("{error}；最终占用暂沿用清理前数值，释放量尚无法测量。");
                warning = Some(match warning {
                    Some(warning) => format!("{warning}；{message}"),
                    None => message,
                });
                before
            }
        };
        Ok(CleanupResult {
            deleted_count,
            content_bytes: snapshot.preview.content_bytes,
            database_bytes_before: before,
            database_bytes_after: after,
            released_bytes: before.saturating_sub(after),
            warning,
        })
    }

    pub fn prune_history(&self) -> Result<usize> {
        Self::prune_with_settings(&self.conn, &self.settings()?, now())
    }

    fn prune_with_settings(
        conn: &Connection,
        settings: &Settings,
        timestamp: i64,
    ) -> Result<usize> {
        if settings.max_history == 0 && settings.retention_days == 0 {
            return Ok(0);
        }
        let cutoff = timestamp.saturating_sub(i64::from(settings.retention_days) * 86_400_000);
        // Protected and grouped snippets never consume the ordinary history quota.
        conn.execute("DELETE FROM clips WHERE pinned=0 AND keep_forever=0 AND group_id IS NULL AND (
            (?2 > 0 AND updated_at < ?3) OR (?1 > 0 AND id NOT IN
            (SELECT id FROM clips WHERE pinned=0 AND keep_forever=0 AND group_id IS NULL ORDER BY updated_at DESC,id DESC LIMIT ?1)))",
            params![settings.max_history as i64, settings.retention_days, cutoff]).map_err(sql_error)
    }

    pub fn create_group(&self, name: &str, color: &str) -> Result<Group> {
        let name = name.trim();
        validate_group(name, color)?;
        self.conn
            .execute(
                "INSERT INTO groups(name,color) VALUES(?1,?2)",
                params![name, color],
            )
            .map_err(|_| "分组创建失败，名称可能已存在。".to_string())?;
        Ok(Group {
            id: self.conn.last_insert_rowid(),
            name: name.into(),
            color: color.into(),
        })
    }

    pub fn delete_group(&self, id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM groups WHERE id=?1", [id])
            .map_err(sql_error)?;
        Ok(())
    }

    pub fn export(&self) -> Result<String> {
        let clips = self.clips()?;
        let groups = self.groups()?;
        check_backup_limits(0, clips.len(), groups.len())?;
        let json = serde_json::to_string_pretty(&Backup {
            format: "starclip".into(),
            version: 1,
            exported_at: now(),
            clips,
            groups,
        })
        .map_err(|e| e.to_string())?;
        check_backup_limits(json.len(), 0, 0)?;
        Ok(json)
    }

    pub fn import(&mut self, json: &str) -> Result<usize> {
        check_backup_limits(json.len(), 0, 0)?;
        let backup: Backup = serde_json::from_str(json)
            .map_err(|_| "无法读取备份文件，请选择星笺导出的 JSON 文件。")?;
        if backup.format != "starclip" || backup.version != 1 {
            return Err("不支持的备份版本。".into());
        }
        check_backup_limits(json.len(), backup.clips.len(), backup.groups.len())?;
        let mut group_ids = HashSet::new();
        for group in &backup.groups {
            validate_group(&group.name, &group.color)?;
            if !group_ids.insert(group.id) {
                return Err("备份存在重复分组编号。".into());
            }
        }
        for clip in &backup.clips {
            validate_content(&clip.content, &clip.title)?;
            if let Some(language) = &clip.language {
                validate_language(language)?;
            }
            if clip.group_id.is_some_and(|id| !group_ids.contains(&id))
                || clip.created_at < 0
                || clip.updated_at < 0
                || clip.copy_count < 0
            {
                return Err("备份包含无效分组或记录。".into());
            }
        }
        let tx = self.conn.transaction().map_err(sql_error)?;
        let mut group_mapping = HashMap::new();
        for group in backup.groups {
            tx.execute(
                "INSERT INTO groups(name,color) VALUES(?1,?2) ON CONFLICT(name) DO NOTHING",
                params![group.name.trim(), group.color],
            )
            .map_err(sql_error)?;
            let id: i64 = tx
                .query_row(
                    "SELECT id FROM groups WHERE name=?1",
                    [group.name.trim()],
                    |row| row.get(0),
                )
                .map_err(sql_error)?;
            group_mapping.insert(group.id, id);
        }
        let mut imported = 0;
        for clip in backup.clips {
            let group_id = clip.group_id.and_then(|id| group_mapping.get(&id).copied());
            let existed: bool = tx
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM clips WHERE content=?1)",
                    [&clip.content],
                    |row| row.get(0),
                )
                .map_err(sql_error)?;
            tx.execute("INSERT INTO clips(content,title,kind,pinned,group_id,created_at,updated_at,copy_count,language,keep_forever) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)
                ON CONFLICT(content) DO UPDATE SET pinned=MAX(clips.pinned,excluded.pinned),
                keep_forever=MAX(clips.keep_forever,excluded.keep_forever),
                group_id=COALESCE(clips.group_id,excluded.group_id),
                title=CASE WHEN clips.title='' THEN excluded.title ELSE clips.title END,
                language=COALESCE(clips.language,excluded.language),
                kind=CASE WHEN clips.language IS NOT NULL OR excluded.language IS NOT NULL THEN 'code' ELSE clips.kind END",
                params![clip.content, clip.title, if clip.language.is_some() { "code" } else { classify(&clip.content) }, clip.pinned, group_id, clip.created_at, clip.updated_at, clip.copy_count, clip.language, clip.keep_forever]).map_err(sql_error)?;
            if !existed {
                imported += 1;
            }
        }
        // Import keeps the entire backup; normal capture, startup and periodic maintenance apply the selected retention policy.
        tx.commit().map_err(sql_error)?;
        Ok(imported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn store() -> Store {
        Store::from_connection(Connection::open_in_memory().unwrap()).unwrap()
    }
    fn all_cleanup() -> CleanupFilter {
        CleanupFilter {
            min_bytes: 0,
            older_than_days: 0,
            kind: "all".into(),
            include_grouped: false,
        }
    }
    struct Scratch(std::path::PathBuf);
    impl Scratch {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "starclip-cleanup-test-{}-{}",
                std::process::id(),
                CLEANUP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn unlimited_history_and_new_protected_records_do_not_consume_capacity() {
        let db = store();
        assert_eq!(db.settings().unwrap().max_history, 0);
        let mut bytes = 0;
        for index in 0..550 {
            let content = format!("不限历史：{index}");
            bytes += content.len() as u64;
            db.add(&content, "", None).unwrap();
        }
        assert_eq!(db.clips().unwrap().len(), 550);
        let preview = db.preview_cleanup(&all_cleanup()).unwrap().preview;
        assert_eq!(preview.matched_count, 550);
        assert_eq!(preview.content_bytes, bytes);
        assert_eq!(preview.items.len(), 100);
        assert!(preview.truncated);
        assert!(preview
            .items
            .windows(2)
            .all(|items| items[0].bytes >= items[1].bytes));
        let mut settings = db.settings().unwrap();
        settings.max_history = 50;
        db.save_settings(&settings).unwrap();
        let ordinary: Vec<_> = db
            .clips()
            .unwrap()
            .into_iter()
            .map(|clip| clip.id)
            .collect();
        let keep = db
            .add_protected("新增时即永久保留", "", None, true)
            .unwrap();
        assert!(keep.keep_forever);
        assert_eq!(db.clips().unwrap().len(), 51);
        for id in ordinary {
            assert!(db.clip(id).is_ok());
        }
        settings.max_history = 0;
        settings.retention_days = 1;
        db.conn
            .execute("UPDATE clips SET updated_at=0", [])
            .unwrap();
        db.save_settings(&settings).unwrap();
        assert_eq!(
            db.clips().unwrap().len(),
            1,
            "unlimited count does not disable the chosen age policy"
        );
        assert_eq!(db.clips().unwrap()[0].id, keep.id);
    }

    #[test]
    fn permanent_protection_survives_every_mutation_and_backup_merge() {
        let mut db = store();
        let keep = db
            .add_protected("保留原创片段", "初稿", None, true)
            .unwrap();
        let pinned = db.add("星标片段", "", None).unwrap();
        db.update(pinned.id, "", None, true).unwrap();
        assert!(db.add("保留原创片段", "备注", None).unwrap().keep_forever);
        db.save_code(Some(keep.id), "代码", "println!(\"星光\");", "rust", None)
            .unwrap();
        db.update(keep.id, "取消星标仍保留", None, false).unwrap();
        assert!(db.clip(keep.id).unwrap().keep_forever);
        assert!(db.delete(keep.id).unwrap_err().contains("先取消保留"));
        db.add("可以清理的片段", "", None).unwrap();
        let mut backup: serde_json::Value = serde_json::from_str(&db.export().unwrap()).unwrap();
        for clip in backup["clips"].as_array_mut().unwrap() {
            clip["keepForever"] = false.into();
        }
        db.import(&backup.to_string()).unwrap();
        assert!(
            db.clip(keep.id).unwrap().keep_forever,
            "import cannot downgrade existing protection"
        );
        let mut restored = store();
        restored.import(&db.export().unwrap()).unwrap();
        assert!(restored
            .clips()
            .unwrap()
            .iter()
            .any(|clip| clip.keep_forever));
        db.clear_history().unwrap();
        assert_eq!(db.clips().unwrap().len(), 2);
        let preview = db.preview_cleanup(&all_cleanup()).unwrap();
        assert_eq!(preview.preview.protected_count, 2);
        assert_eq!(preview.preview.matched_count, 0);
        assert_eq!(db.execute_cleanup(preview, false).unwrap().deleted_count, 0);
        db.set_keep_forever(keep.id, false).unwrap();
        db.delete(keep.id).unwrap();
        assert!(db.clip(keep.id).is_err());
    }

    #[test]
    fn cleanup_filters_are_utf8_strict_and_combined_and_preview_is_read_only() {
        let db = store();
        db.add("星", "月", None).unwrap(); // 6 UTF-8 bytes
        db.add("abcd", "ef", None).unwrap(); // exactly the same byte threshold
        let large = db.add("银河", "夜", None).unwrap(); // 9 bytes
        let link = db.add("https://example.com", "", None).unwrap();
        let group = db.create_group("清理测试分组", "#aabbcc").unwrap();
        db.add("分组内容", "", Some(group.id)).unwrap();
        let keep = db.add_protected("保留内容", "", None, true).unwrap();
        db.update(keep.id, "", None, true).unwrap(); // overlapping protection counts once
        let pin = db.add("星标内容", "", None).unwrap();
        db.update(pin.id, "", None, true).unwrap();
        db.conn
            .execute("UPDATE clips SET updated_at=0", [])
            .unwrap();
        db.add("fresh text", "", None).unwrap();
        let changes = db.conn.total_changes();
        let mut filter = CleanupFilter {
            min_bytes: 6,
            older_than_days: 1,
            kind: "text".into(),
            include_grouped: false,
        };
        let preview = db.preview_cleanup(&filter).unwrap().preview;
        assert_eq!(db.conn.total_changes(), changes);
        assert_eq!(preview.total_count, 8);
        assert_eq!(preview.protected_count, 2);
        assert_eq!(preview.matched_count, 1);
        assert_eq!(preview.content_bytes, 9);
        assert_eq!(preview.items[0].id, large.id);
        filter.include_grouped = true;
        let grouped = db.preview_cleanup(&filter).unwrap().preview;
        assert_eq!((grouped.matched_count, grouped.content_bytes), (2, 21));
        filter.include_grouped = false;
        filter.kind = "all".into();
        let both = db.preview_cleanup(&filter).unwrap().preview;
        assert_eq!(both.matched_count, 2);
        assert!(both.items.iter().any(|item| item.id == link.id));
        assert_eq!(
            db.preview_cleanup(&all_cleanup())
                .unwrap()
                .preview
                .matched_count,
            5
        );
        filter.kind = "invalid".into();
        assert!(db.preview_cleanup(&filter).is_err());
    }

    #[test]
    fn stale_cleanup_checks_all_sensitive_fields_and_rolls_back() {
        for change in [
            "content='变更正文'",
            "title='变更备注'",
            "updated_at=updated_at+1",
            "group_id=1",
            "pinned=1",
            "keep_forever=1",
            "kind='code'",
        ] {
            let mut db = store();
            db.create_group("备选分组", "#ffffff").unwrap();
            db.add("未变化的原创片段", "", None).unwrap();
            let clip = db.add("预览中的片段", "", None).unwrap();
            let snapshot = db.preview_cleanup(&all_cleanup()).unwrap();
            db.conn
                .execute(&format!("UPDATE clips SET {change} WHERE id=?1"), [clip.id])
                .unwrap();
            assert_eq!(
                db.execute_cleanup(snapshot, false).unwrap_err(),
                STALE_CLEANUP
            );
            assert_eq!(
                db.clips().unwrap().len(),
                2,
                "{change} must not partially delete other candidates"
            );
        }
        let mut db = store();
        let old = db.add("被删除的预览项", "", None).unwrap();
        let snapshot = db.preview_cleanup(&all_cleanup()).unwrap();
        db.delete(old.id).unwrap();
        let replacement = db.add("新记录复用了最大编号", "", None).unwrap();
        assert_eq!(replacement.id, old.id);
        assert_eq!(
            db.execute_cleanup(snapshot, false).unwrap_err(),
            STALE_CLEANUP
        );
        assert!(db.clip(replacement.id).is_ok());
    }

    #[test]
    fn cleanup_does_not_delete_new_records_and_failed_delete_is_atomic() {
        let mut db = store();
        let first = db.add("预览片段一", "", None).unwrap();
        db.add("预览片段二", "", None).unwrap();
        let preview = db.preview_cleanup(&all_cleanup()).unwrap();
        let new = db.add("确认之前新收录", "", None).unwrap();
        db.mark_copied(first.id).unwrap(); // copy statistics are not a content edit
        let result = db.execute_cleanup(preview, false).unwrap();
        assert_eq!(result.deleted_count, 2);
        assert_eq!(db.clips().unwrap().len(), 1);
        assert_eq!(db.clips().unwrap()[0].id, new.id);
        db.add("另一条不能丢失的记录", "", None).unwrap();
        let preview = db.preview_cleanup(&all_cleanup()).unwrap();
        db.conn.execute_batch("CREATE TRIGGER fail_delete BEFORE DELETE ON clips WHEN old.content='确认之前新收录' BEGIN SELECT RAISE(ABORT, 'synthetic failure'); END;").unwrap();
        assert!(db.execute_cleanup(preview, false).is_err());
        assert_eq!(db.clips().unwrap().len(), 2);
    }

    #[test]
    fn compaction_reports_actual_wal_file_reduction_and_handles_empty_match() {
        let scratch = Scratch::new();
        let path = scratch.0.join("cleanup.sqlite3");
        let mut db = Store::open(&path).unwrap();
        db.conn.pragma_update(None, "journal_mode", "WAL").unwrap();
        db.conn
            .pragma_update(None, "wal_autocheckpoint", 0)
            .unwrap();
        let tx = db.conn.unchecked_transaction().unwrap();
        for index in 0..128 {
            tx.execute(
                "INSERT INTO clips(content,kind,created_at,updated_at) VALUES(?1,'text',0,0)",
                [format!("原创建库 {index} {}", "星".repeat(5500))],
            )
            .unwrap();
        }
        tx.commit().unwrap();
        let before_preview = db.preview_cleanup(&all_cleanup()).unwrap();
        assert_eq!(
            before_preview.preview.database_bytes,
            fs::metadata(&path).unwrap().len()
                + fs::metadata(format!("{}-wal", path.display()))
                    .unwrap()
                    .len()
        );
        let expected_content_bytes = before_preview.preview.content_bytes;
        let result = db.execute_cleanup(before_preview, false).unwrap();
        assert_eq!(result.deleted_count, 128);
        assert_eq!(result.content_bytes, expected_content_bytes);
        assert_eq!(
            result.released_bytes, 0,
            "deletion alone does not claim free disk space"
        );
        let empty = db.preview_cleanup(&all_cleanup()).unwrap();
        assert_eq!(empty.preview.matched_count, 0);
        assert!(empty.preview.reusable_bytes > 0);
        let result = db.execute_cleanup(empty, true).unwrap();
        assert_eq!(result.deleted_count, 0);
        assert!(result.warning.is_none());
        assert!(result.database_bytes_after < result.database_bytes_before);
        assert_eq!(
            result.released_bytes,
            result.database_bytes_before - result.database_bytes_after
        );
        assert_eq!(result.database_bytes_after, db.database_bytes().unwrap());
        assert!(db.clips().unwrap().is_empty());
    }

    #[test]
    fn checkpoint_failure_returns_committed_deletion_with_warning() {
        let scratch = Scratch::new();
        let path = scratch.0.join("cleanup.sqlite3");
        let mut db = Store::open(&path).unwrap();
        db.conn.pragma_update(None, "journal_mode", "WAL").unwrap();
        db.conn
            .busy_timeout(std::time::Duration::from_millis(20))
            .unwrap();
        db.add("整理失败但删除事务成功", "", None).unwrap();
        let reader = Connection::open(&path).unwrap();
        reader
            .execute_batch("BEGIN; SELECT count(*) FROM clips;")
            .unwrap();
        let preview = db.preview_cleanup(&all_cleanup()).unwrap();
        let result = db.execute_cleanup(preview, true).unwrap();
        assert_eq!(result.deleted_count, 1);
        assert!(result.warning.as_deref().unwrap().contains("整理未完成"));
        assert!(db.clips().unwrap().is_empty());
        assert_eq!(result.database_bytes_after, db.database_bytes().unwrap());
        reader.execute_batch("ROLLBACK").unwrap();
    }

    #[test]
    fn dedup_keeps_metadata_and_group_delete_preserves_content() {
        let db = store();
        let group = db.create_group("工作", "#9977ff").unwrap();
        let first = db.add("你好，星空", "问候", Some(group.id)).unwrap();
        db.update(first.id, "问候", Some(group.id), true).unwrap();
        let again = db.add("你好，星空", "", None).unwrap();
        assert_eq!(first.id, again.id);
        assert_eq!(db.clips().unwrap().len(), 1);
        assert!(again.pinned);
        assert_eq!(again.title, "问候");
        db.delete_group(group.id).unwrap();
        assert_eq!(db.clip(first.id).unwrap().group_id, None);
        db.clear_history().unwrap();
        assert_eq!(db.clips().unwrap().len(), 1);
        db.delete(first.id).unwrap();
        assert!(db.clips().unwrap().is_empty());
    }

    #[test]
    fn retention_preserves_pins_and_groups() {
        let db = store();
        let mut settings = db.settings().unwrap();
        assert!(settings.paused);
        settings.max_history = 50;
        db.save_settings(&settings).unwrap();
        let pinned = db.add("收藏", "", None).unwrap();
        db.update(pinned.id, "", None, true).unwrap();
        let group = db.create_group("常用", "#ffffff").unwrap();
        db.add("分组常用语", "", Some(group.id)).unwrap();
        for i in 0..55 {
            db.add(&format!("第{i}项"), "", None).unwrap();
        }
        assert_eq!(db.clips().unwrap().len(), 52);
        assert!(db.clip(pinned.id).unwrap().pinned);
    }

    #[test]
    fn backup_roundtrip_merge_and_invalid_import_is_atomic() {
        let source = store();
        let group = source.create_group("链接", "#12ab34").unwrap();
        let clip = source
            .add("https://example.com", "主页", Some(group.id))
            .unwrap();
        source
            .update(clip.id, "主页", Some(group.id), true)
            .unwrap();
        let json = source.export().unwrap();
        let mut dest = store();
        assert_eq!(dest.import(&json).unwrap(), 1);
        assert_eq!(dest.import(&json).unwrap(), 0);
        assert_eq!(dest.clips().unwrap()[0].kind, "link");
        assert!(dest.clips().unwrap()[0].pinned);
        assert!(dest.settings().unwrap().paused);
        let mut invalid: serde_json::Value = serde_json::from_str(&json).unwrap();
        invalid["groups"][0]["name"] = "不应导入的分组".into();
        invalid["clips"][0]["content"] = "".into();
        assert!(dest.import(&invalid.to_string()).is_err());
        assert_eq!(dest.groups().unwrap().len(), 1);
        assert_eq!(dest.clips().unwrap().len(), 1);
    }

    #[test]
    fn code_edit_retains_identity_and_conflicts_do_not_overwrite() {
        let mut db = store();
        let code = db
            .save_code(None, "Rust 问候", "fn main() {}", "rust", None)
            .unwrap();
        assert!(code.pinned);
        assert_eq!(code.kind, "code");
        assert_eq!(code.language.as_deref(), Some("rust"));
        db.mark_copied(code.id).unwrap();
        let edited = db
            .save_code(
                Some(code.id),
                "入口",
                "fn main() { println!(\"hi\"); }",
                "rust",
                None,
            )
            .unwrap();
        assert_eq!(edited.id, code.id);
        assert_eq!(edited.copy_count, 1);
        let other = db.add("other", "不能覆盖", None).unwrap();
        assert!(db
            .save_code(Some(code.id), "错误", "other", "rust", None)
            .is_err());
        assert_eq!(db.clip(other.id).unwrap().title, "不能覆盖");
        assert_eq!(db.clip(code.id).unwrap().content, edited.content);
        let backup = db.export().unwrap();
        db.clear_history().unwrap();
        assert_eq!(db.import(&backup).unwrap(), 1);
        assert_eq!(db.clip(code.id).unwrap().language.as_deref(), Some("rust"));
    }

    #[test]
    fn backup_limits_roundtrip_and_reject_overflow() {
        let source = store();
        source.conn.execute_batch("WITH RECURSIVE n(i) AS (VALUES(1) UNION ALL SELECT i+1 FROM n WHERE i<1000)
            INSERT INTO groups(name,color) SELECT 'group-'||i,'#ffffff' FROM n;
            WITH RECURSIVE n(i) AS (VALUES(1) UNION ALL SELECT i+1 FROM n WHERE i<10000)
            INSERT INTO clips(content,title,kind,pinned,created_at,updated_at) SELECT 'clip-'||i,'','text',1,0,0 FROM n;").unwrap();
        let json = source.export().unwrap();
        let mut destination = store();
        assert_eq!(destination.import(&json).unwrap(), MAX_BACKUP_CLIPS);
        assert_eq!(destination.groups().unwrap().len(), MAX_BACKUP_GROUPS);
        assert!(check_backup_limits(MAX_BACKUP_BYTES, MAX_BACKUP_CLIPS, MAX_BACKUP_GROUPS).is_ok());
        assert!(check_backup_limits(MAX_BACKUP_BYTES + 1, 0, 0).is_err());
        let extra = source.add("overflow", "", None).unwrap();
        assert!(source.export().is_err());
        source.delete(extra.id).unwrap();
        source.create_group("overflow", "#ffffff").unwrap();
        assert!(source.export().is_err());
    }

    #[test]
    fn legacy_settings_and_backup_keep_their_meaning() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE clips (
            id INTEGER PRIMARY KEY, content TEXT NOT NULL UNIQUE, title TEXT NOT NULL DEFAULT '',
            kind TEXT NOT NULL, pinned INTEGER NOT NULL DEFAULT 0, group_id INTEGER,
            created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL, copy_count INTEGER NOT NULL DEFAULT 0);
            CREATE TABLE preferences (key TEXT PRIMARY KEY, value TEXT NOT NULL);
            INSERT INTO clips(content,kind,pinned,created_at,updated_at) VALUES('旧版原创片段','text',1,0,0);").unwrap();
        let legacy = r#"{"paused":true,"theme":"light","layout":"list","maxHistory":500,"shortcut":"CommandOrControl+Shift+V"}"#;
        conn.execute(
            "INSERT INTO preferences(key,value) VALUES('settings',?1)",
            [legacy],
        )
        .unwrap();
        let db = Store::from_connection(conn).unwrap();
        let settings = db.settings().unwrap();
        assert!(!settings.always_on_top);
        assert!(settings.show_startup);
        assert_eq!(settings.sticker_pack, "auto");
        assert_eq!(
            settings.max_history, 500,
            "legacy configured limits remain unchanged"
        );
        assert_eq!(settings.retention_days, 0);
        assert_eq!(settings.layout, "list");
        assert_eq!(db.clips().unwrap()[0].language, None);
        assert!(!db.clips().unwrap()[0].keep_forever);
        assert_eq!(
            db.conn
                .query_row(
                    "SELECT value FROM preferences WHERE key='settings'",
                    [],
                    |row| row.get::<_, String>(0)
                )
                .unwrap(),
            legacy
        );
        let mut old_backup: serde_json::Value =
            serde_json::from_str(&db.export().unwrap()).unwrap();
        old_backup["clips"][0]
            .as_object_mut()
            .unwrap()
            .remove("language");
        old_backup["clips"][0]
            .as_object_mut()
            .unwrap()
            .remove("keepForever");
        let mut destination = store();
        let mut updated = settings.clone();
        updated.layout = "horizontal".into();
        updated.always_on_top = true;
        updated.retention_days = 7;
        updated.show_startup = false;
        updated.sticker_pack = "cats".into();
        for theme in ["forest", "rose"] {
            updated.theme = theme.into();
            destination.save_settings(&updated).unwrap();
            assert_eq!(destination.settings().unwrap(), updated);
        }
        destination.save_settings(&updated).unwrap();
        assert_eq!(destination.import(&old_backup.to_string()).unwrap(), 1);
        assert_eq!(destination.settings().unwrap(), updated);
        assert!(destination.clips().unwrap()[0].pinned);
        assert!(!destination.clips().unwrap()[0].keep_forever);
        updated.retention_days = 2;
        assert!(destination.save_settings(&updated).is_err());
        assert_eq!(destination.settings().unwrap().retention_days, 7);
    }

    #[test]
    fn age_cleanup_preserves_protected_items_and_recapture_renews_history() {
        let db = store();
        let mut settings = db.settings().unwrap();
        settings.retention_days = 7;
        db.save_settings(&settings).unwrap();
        let expired = db.add("过期片段", "", None).unwrap();
        let renewed = db.add("再次收录", "", None).unwrap();
        let pinned = db.add("重要星标", "", None).unwrap();
        db.update(pinned.id, "", None, true).unwrap();
        let group = db.create_group("常用片段", "#aabbcc").unwrap();
        let grouped = db.add("分组保护", "", Some(group.id)).unwrap();
        db.conn
            .execute("UPDATE clips SET created_at=0,updated_at=0", [])
            .unwrap();
        let fresh = db.add("再次收录", "", None).unwrap();
        assert_eq!(fresh.id, renewed.id);
        assert_eq!(fresh.created_at, 0);
        assert!(fresh.updated_at > 0);
        assert!(db.clip(expired.id).is_err());
        assert!(db.clip(pinned.id).is_ok());
        assert!(db.clip(grouped.id).is_ok());
        let boundary = fresh.updated_at - 7 * 86_400_000;
        db.conn
            .execute(
                "UPDATE clips SET updated_at=?1 WHERE id=?2",
                params![boundary, fresh.id],
            )
            .unwrap();
        assert_eq!(
            Store::prune_with_settings(&db.conn, &settings, fresh.updated_at).unwrap(),
            0
        );
        assert_eq!(
            Store::prune_with_settings(&db.conn, &settings, fresh.updated_at + 1).unwrap(),
            1
        );
        assert_eq!(db.clips().unwrap().len(), 2);
    }

    #[test]
    fn age_and_capacity_apply_together_at_startup_and_settings_are_atomic() {
        let db = store();
        let mut settings = db.settings().unwrap();
        settings.retention_days = 7;
        settings.max_history = 50;
        db.save_settings(&settings).unwrap();
        let timestamp = now();
        // Seed history directly so one maintenance pass exercises both limits together.
        let tx = db.conn.unchecked_transaction().unwrap();
        for index in 0..60 {
            let updated = if index < 5 { 0 } else { timestamp + index };
            tx.execute(
                "INSERT INTO clips(content,kind,created_at,updated_at) VALUES(?1,'text',0,?2)",
                params![format!("容量与时间测试 {index}"), updated],
            )
            .unwrap();
        }
        tx.execute("INSERT INTO clips(content,kind,pinned,created_at,updated_at) VALUES('受保护星标','text',1,0,0)", []).unwrap();
        tx.commit().unwrap();
        let reopened = Store::from_connection(db.conn).unwrap();
        assert_eq!(reopened.clips().unwrap().len(), 51);
        assert_eq!(reopened.prune_history().unwrap(), 0);
        assert!(reopened
            .clips()
            .unwrap()
            .iter()
            .any(|clip| clip.content == "容量与时间测试 10"));
        assert!(!reopened
            .clips()
            .unwrap()
            .iter()
            .any(|clip| clip.content == "容量与时间测试 9"));
        reopened
            .conn
            .execute("UPDATE clips SET updated_at=0 WHERE pinned=0", [])
            .unwrap();
        reopened.conn.execute_batch("CREATE TRIGGER reject_cleanup BEFORE DELETE ON clips BEGIN SELECT RAISE(ABORT, 'test cleanup failure'); END;").unwrap();
        let mut changed = settings.clone();
        changed.retention_days = 1;
        changed.always_on_top = true;
        assert!(reopened.save_settings(&changed).is_err());
        assert_eq!(reopened.settings().unwrap(), settings);
        assert_eq!(reopened.clips().unwrap().len(), 51);
    }

    #[test]
    fn unicode_and_limits_are_checked_without_mutation() {
        let db = store();
        assert!(db.add("  ", "", None).is_err());
        assert!(db
            .add(&"x".repeat(MAX_CONTENT_BYTES + 1), "", None)
            .is_err());
        assert!(db.create_group("星", "#星星").is_err());
        assert_eq!(classify("#a1b2c3"), "color");
        assert_eq!(classify("https://example.com\nhello"), "text");
        assert!(db.clips().unwrap().is_empty());
    }
}
