use crate::images::{self, ImageMetadata, PreparedImage};
use base64::{engine::general_purpose::STANDARD, Engine};
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
const MAX_BACKUP_IMAGE_PIXELS: u64 = 256_000_000;

fn check_backup_limits(bytes: usize, clips: usize, groups: usize) -> Result<()> {
    if bytes > MAX_BACKUP_BYTES || clips > MAX_BACKUP_CLIPS || groups > MAX_BACKUP_GROUPS {
        return Err("JSON 备份上限为 20 MiB、10000 条星笺和 1000 个分组；超限时可在数据设置中迁移完整数据目录，无需删除记录。".into());
    }
    Ok(())
}

fn check_backup_image_limits(clips: &[Clip]) -> Result<()> {
    let pixels = clips
        .iter()
        .filter_map(|clip| clip.image.as_ref())
        .fold(0u64, |sum, image| {
            sum.saturating_add(u64::from(image.width) * u64::from(image.height))
        });
    if pixels > MAX_BACKUP_IMAGE_PIXELS {
        return Err("JSON 备份中的图片累计不能超过 2.56 亿像素；请使用数据设置中的完整数据目录迁移，无需删除记录。".into());
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<ImageMetadata>,
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
        if !["dark", "light", "white", "forest", "rose", "system"].contains(&self.theme.as_str())
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

pub const STARPET_MOODS: &[&str] = &[
    "idle",
    "greet",
    "curious",
    "listening",
    "sleepy",
    "sleeping",
    "waking",
    "stretch",
    "happy",
    "delighted",
    "love",
    "shy",
    "surprised",
    "puzzled",
    "sad",
    "crying",
    "angry",
    "nervous",
    "proud",
    "relieved",
    "thinking",
    "searching",
    "reading",
    "writing",
    "drawing",
    "focused",
    "waiting",
    "loading",
    "success",
    "error",
    "celebrate",
    "resting",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct StarpetPreferences {
    pub enabled: bool,
    pub mood: String,
    pub color: String,
    pub size: u16,
}
impl Default for StarpetPreferences {
    fn default() -> Self {
        Self {
            enabled: false,
            mood: "idle".into(),
            color: "#FFD66E".into(),
            size: 100,
        }
    }
}
impl StarpetPreferences {
    pub fn validate(&self) -> Result<()> {
        if !STARPET_MOODS.contains(&self.mood.as_str()) {
            return Err("不支持的星伴表情。".into());
        }
        if self.color.len() != 7
            || !self.color.starts_with('#')
            || !self.color[1..].bytes().all(|c| c.is_ascii_hexdigit())
        {
            return Err("星伴颜色需为六位 HEX，例如 #FFD66E。".into());
        }
        if !(70..=160).contains(&self.size) {
            return Err("星伴大小需为 70%–160%。".into());
        }
        Ok(())
    }

    pub fn window_size(&self) -> (f64, f64) {
        let body = 180.0 * f64::from(self.size) / 100.0;
        (body + 50.0, body + 80.0)
    }
}

#[cfg(any(feature = "desktop", test))]
pub(crate) fn starpet_resize_position(
    position: (f64, f64),
    previous_size: (f64, f64),
    next_size: (f64, f64),
    work_area: (f64, f64, f64, f64),
) -> (f64, f64) {
    let (left, top, width, height) = work_area;
    let x = position.0 + (previous_size.0 - next_size.0) / 2.0;
    let y = position.1 + previous_size.1 - next_size.1;
    (
        x.clamp(left, (left + width - next_size.0).max(left)),
        y.clamp(top, (top + height - next_size.1).max(top)),
    )
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StarpetPreferencePatch {
    pub enabled: Option<bool>,
    pub mood: Option<String>,
    pub color: Option<String>,
    pub size: Option<u16>,
}
impl StarpetPreferencePatch {
    pub fn apply_to(self, preferences: &mut StarpetPreferences) {
        if let Some(enabled) = self.enabled {
            preferences.enabled = enabled;
        }
        if let Some(mood) = self.mood {
            preferences.mood = mood;
        }
        if let Some(color) = self.color {
            preferences.color = color.to_ascii_uppercase();
        }
        if let Some(size) = self.size {
            preferences.size = size;
        }
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
    clip.image
        .as_ref()
        .map_or(clip.content.len() as u64, |image| image.bytes)
        + clip.title.len() as u64
}
fn same_cleanup_record(before: &Clip, current: &Clip) -> bool {
    before.id == current.id
        && before.content == current.content
        && before.title == current.title
        && before.kind == current.kind
        && before.language == current.language
        && before.image == current.image
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    images: Vec<BackupImage>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BackupImage {
    clip_id: i64,
    png: String,
}

const CLIP_SELECT: &str = "SELECT c.id,c.content,c.title,c.kind,c.pinned,c.group_id,c.created_at,c.updated_at,c.copy_count,c.language,c.keep_forever,i.width,i.height,i.bytes FROM clips c LEFT JOIN clip_images i ON i.clip_id=c.id";

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
        image: match row.get::<_, Option<u32>>(11)? {
            Some(width) => Some(ImageMetadata {
                width,
                height: row.get(12)?,
                bytes: row.get(13)?,
            }),
            None => None,
        },
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
        let existing_database: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='clips')",
                [],
                |row| row.get(0),
            )
            .map_err(sql_error)?;
        conn.execute_batch("PRAGMA foreign_keys=ON;
            PRAGMA secure_delete=ON;
            CREATE TABLE IF NOT EXISTS groups (id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE, color TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS clips (
                id INTEGER PRIMARY KEY, content TEXT NOT NULL, title TEXT NOT NULL DEFAULT '',
                kind TEXT NOT NULL, pinned INTEGER NOT NULL DEFAULT 0, keep_forever INTEGER NOT NULL DEFAULT 0,
                group_id INTEGER REFERENCES groups(id) ON DELETE SET NULL,
                created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL, copy_count INTEGER NOT NULL DEFAULT 0, language TEXT
            );
            CREATE INDEX IF NOT EXISTS clips_updated ON clips(updated_at DESC);
            CREATE TABLE IF NOT EXISTS preferences (key TEXT PRIMARY KEY, value TEXT NOT NULL);").map_err(sql_error)?;
        if !existing_database {
            conn.execute("INSERT INTO preferences(key,value) VALUES('onboarding_v1','false') ON CONFLICT(key) DO NOTHING", []).map_err(sql_error)?;
        }
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
        // The original UNIQUE(content) mixed text and image identities. Rebuild once,
        // in one transaction, before any image table exists; preserve every original field.
        let legacy_unique: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM pragma_index_list('clips') WHERE origin='u')",
                [],
                |row| row.get(0),
            )
            .map_err(sql_error)?;
        let tx = conn.unchecked_transaction().map_err(sql_error)?;
        if legacy_unique {
            let has_images: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='clip_images')", [], |row| row.get(0)).map_err(sql_error)?;
            if has_images {
                return Err("图片数据库结构不兼容，原数据未更改。".into());
            }
            tx.execute_batch("CREATE TABLE clips_images_upgrade (
                id INTEGER PRIMARY KEY, content TEXT NOT NULL, title TEXT NOT NULL DEFAULT '',
                kind TEXT NOT NULL, pinned INTEGER NOT NULL DEFAULT 0, keep_forever INTEGER NOT NULL DEFAULT 0,
                group_id INTEGER REFERENCES groups(id) ON DELETE SET NULL,
                created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL, copy_count INTEGER NOT NULL DEFAULT 0, language TEXT);
                INSERT INTO clips_images_upgrade(id,content,title,kind,pinned,keep_forever,group_id,created_at,updated_at,copy_count,language)
                    SELECT id,content,title,kind,pinned,keep_forever,group_id,created_at,updated_at,copy_count,language FROM clips;
                DROP TABLE clips;
                ALTER TABLE clips_images_upgrade RENAME TO clips;").map_err(sql_error)?;
        }
        tx.execute_batch("CREATE INDEX IF NOT EXISTS clips_updated ON clips(updated_at DESC);
            CREATE UNIQUE INDEX IF NOT EXISTS clips_text_content ON clips(content) WHERE kind<>'image';
            CREATE UNIQUE INDEX IF NOT EXISTS clips_image_content ON clips(content) WHERE kind='image';
            CREATE TABLE IF NOT EXISTS clip_images (
                clip_id INTEGER PRIMARY KEY REFERENCES clips(id) ON DELETE CASCADE,
                width INTEGER NOT NULL CHECK(width>0), height INTEGER NOT NULL CHECK(height>0),
                bytes INTEGER NOT NULL CHECK(bytes>0), png BLOB NOT NULL, thumbnail BLOB NOT NULL
            );").map_err(sql_error)?;
        tx.commit().map_err(sql_error)?;
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

    pub fn onboarding_completed(&self) -> Result<bool> {
        self.conn
            .query_row(
                "SELECT COALESCE((SELECT value='true' FROM preferences WHERE key='onboarding_v1'), 1)",
                [],
                |row| row.get(0),
            )
            .map_err(sql_error)
    }

    pub fn complete_onboarding(&self) -> Result<()> {
        // This marker must not run save_settings, pruning, or clipboard transitions.
        self.conn
            .execute(
                "INSERT INTO preferences(key,value) VALUES('onboarding_v1','true')
            ON CONFLICT(key) DO UPDATE SET value='true'",
                [],
            )
            .map_err(sql_error)?;
        Ok(())
    }

    pub fn starpet_preferences(&self) -> Result<StarpetPreferences> {
        let json: Option<String> = self
            .conn
            .query_row(
                "SELECT value FROM preferences WHERE key='starpet_v1'",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(sql_error)?;
        match json {
            Some(json) => {
                let mut preferences: StarpetPreferences = serde_json::from_str(&json)
                    .map_err(|_| "星伴偏好损坏，未更改已保存的数据。")?;
                preferences.validate()?;
                preferences.color.make_ascii_uppercase();
                Ok(preferences)
            }
            None => Ok(StarpetPreferences::default()),
        }
    }

    pub fn save_starpet_preferences(&self, preferences: &StarpetPreferences) -> Result<()> {
        // Keep pet interactions independent of retention, clipboard capture and ordinary settings.
        preferences.validate()?;
        let mut preferences = preferences.clone();
        preferences.color.make_ascii_uppercase();
        let json = serde_json::to_string(&preferences).map_err(|e| e.to_string())?;
        self.conn
            .execute(
                "INSERT INTO preferences(key,value) VALUES('starpet_v1',?1)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                [json],
            )
            .map_err(sql_error)?;
        Ok(())
    }

    pub fn set_theme(&self, theme: &str) -> Result<Settings> {
        if !["dark", "light", "white", "forest", "rose", "system"].contains(&theme) {
            return Err("不支持的应用主题。".into());
        }
        let tx = self.conn.unchecked_transaction().map_err(sql_error)?;
        let stored: Option<String> = tx
            .query_row(
                "SELECT value FROM preferences WHERE key='settings'",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(sql_error)?;
        let mut json: serde_json::Value = match stored {
            Some(json) => serde_json::from_str(&json).map_err(|_| "偏好设置损坏，主题未保存。")?,
            None => serde_json::to_value(Settings::default()).map_err(|e| e.to_string())?,
        };
        json.as_object_mut()
            .ok_or("偏好设置损坏，主题未保存。")?
            .insert("theme".into(), theme.into());
        let settings: Settings =
            serde_json::from_value(json.clone()).map_err(|_| "偏好设置损坏，主题未保存。")?;
        settings.validate()?;
        // Preserve every other stored preference, including older missing fields and future keys.
        tx.execute("INSERT INTO preferences(key,value) VALUES('settings',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [json.to_string()]).map_err(sql_error)?;
        tx.commit().map_err(sql_error)?;
        Ok(settings)
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
        let mut stmt = self
            .conn
            .prepare(&format!(
                "{CLIP_SELECT} ORDER BY c.updated_at DESC,c.id DESC"
            ))
            .map_err(sql_error)?;
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
        self.conn
            .query_row(&format!("{CLIP_SELECT} WHERE c.id=?1"), [id], clip_from_row)
            .optional()
            .map_err(sql_error)?
            .ok_or_else(|| "这条星笺已被删除。".into())
    }

    pub fn add(&self, content: &str, title: &str, group_id: Option<i64>) -> Result<Clip> {
        self.add_protected(content, title, group_id, false)
    }

    fn put_image(conn: &Connection, id: i64, image: &PreparedImage) -> Result<()> {
        let metadata = image.metadata();
        conn.execute("INSERT INTO clip_images(clip_id,width,height,bytes,png,thumbnail) VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(clip_id) DO NOTHING",
            params![id, metadata.width, metadata.height, metadata.bytes, image.png, image.thumbnail]).map_err(sql_error)?;
        Ok(())
    }

    pub fn add_image(
        &self,
        image: &PreparedImage,
        title: &str,
        group_id: Option<i64>,
        keep_forever: bool,
    ) -> Result<Clip> {
        validate_content(image.digest(), title)?;
        let timestamp = now();
        let tx = self.conn.unchecked_transaction().map_err(sql_error)?;
        tx.execute("INSERT INTO clips(content,title,kind,group_id,created_at,updated_at,keep_forever) VALUES(?1,?2,'image',?3,?4,?4,?5)
            ON CONFLICT(content) WHERE kind='image' DO UPDATE SET updated_at=excluded.updated_at,
            title=CASE WHEN excluded.title<>'' THEN excluded.title ELSE clips.title END,
            group_id=COALESCE(excluded.group_id,clips.group_id),keep_forever=MAX(clips.keep_forever,excluded.keep_forever)",
            params![image.digest(), title.trim(), group_id, timestamp, keep_forever]).map_err(sql_error)?;
        let id: i64 = tx
            .query_row(
                "SELECT id FROM clips WHERE content=?1 AND kind='image'",
                [image.digest()],
                |row| row.get(0),
            )
            .map_err(sql_error)?;
        Self::put_image(&tx, id, image)?;
        Self::prune_with_settings(&tx, &self.settings()?, timestamp)?;
        let clip = self.clip(id)?;
        tx.commit().map_err(sql_error)?;
        Ok(clip)
    }

    pub fn image_png(&self, id: i64, thumbnail: bool) -> Result<Vec<u8>> {
        let column = if thumbnail { "thumbnail" } else { "png" };
        self.conn.query_row(&format!("SELECT i.{column} FROM clip_images i JOIN clips c ON c.id=i.clip_id WHERE c.id=?1 AND c.kind='image' AND length(i.{column})<=?2"),
            params![id, images::MAX_PNG_BYTES], |row| row.get(0)).optional().map_err(sql_error)?
            .ok_or_else(|| "图片不存在、已被删除或超过读取上限。".into())
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
            ON CONFLICT(content) WHERE kind<>'image' DO UPDATE SET updated_at=excluded.updated_at,
            title=CASE WHEN excluded.title<>'' THEN excluded.title ELSE clips.title END,
            group_id=COALESCE(excluded.group_id,clips.group_id),
            keep_forever=MAX(clips.keep_forever,excluded.keep_forever)", params![content, title.trim(), classify(content), group_id, timestamp, keep_forever]).map_err(sql_error)?;
        let id = tx
            .query_row(
                "SELECT id FROM clips WHERE content=?1 AND kind<>'image'",
                [content],
                |row| row.get(0),
            )
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
                if self.clip(id)?.kind == "image" {
                    return Err("图片不能转换为代码，请新建代码片段。".into());
                }
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
        if !["all", "text", "link", "code", "color", "image"].contains(&filter.kind.as_str()) {
            return Err("不支持的清理内容类型。".into());
        }
        let cutoff = now().saturating_sub(i64::from(filter.older_than_days) * 86_400_000);
        let mut total_count = 0;
        let mut protected_count = 0;
        let mut content_bytes = 0;
        let mut candidates = Vec::new();
        let mut stmt = self.conn.prepare(CLIP_SELECT).map_err(sql_error)?;
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
            let current = tx
                .query_row(
                    &format!("{CLIP_SELECT} WHERE c.id=?1"),
                    [expected.id],
                    clip_from_row,
                )
                .optional()
                .map_err(sql_error)?;
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
        // Bound allocations before loading histories or image BLOBs. Serialized JSON is
        // checked again because base64 and metadata make it larger than the stored content.
        let (count, bytes): (usize, u64) = self.conn.query_row(
            "SELECT count(*),COALESCE(sum(CASE WHEN c.kind='image' THEN COALESCE(i.bytes,0) ELSE length(CAST(c.content AS BLOB)) END + length(CAST(c.title AS BLOB))),0) FROM clips c LEFT JOIN clip_images i ON i.clip_id=c.id",
            [], |row| Ok((row.get(0)?, row.get(1)?))).map_err(sql_error)?;
        let group_count: usize = self
            .conn
            .query_row("SELECT count(*) FROM groups", [], |row| row.get(0))
            .map_err(sql_error)?;
        check_backup_limits(
            usize::try_from(bytes).unwrap_or(usize::MAX),
            count,
            group_count,
        )?;
        let clips = self.clips()?;
        check_backup_image_limits(&clips)?;
        let groups = self.groups()?;
        let mut images = Vec::new();
        let mut encoded_bytes = 0usize;
        for clip in &clips {
            if clip.kind == "image" {
                let png = self.image_png(clip.id, false)?;
                let metadata = clip
                    .image
                    .as_ref()
                    .ok_or("图片元数据缺失，无法导出完整备份。")?;
                let (width, height, rgba) = images::decode_png(&png)?;
                if metadata.width != width
                    || metadata.height != height
                    || metadata.bytes != png.len() as u64
                    || images::fingerprint_rgba(width, height, &rgba)? != clip.content
                {
                    return Err("图片数据与元数据不符，未导出损坏的备份。".into());
                }
                let png = STANDARD.encode(png);
                encoded_bytes = encoded_bytes.saturating_add(png.len());
                check_backup_limits(encoded_bytes, 0, 0)?;
                images.push(BackupImage {
                    clip_id: clip.id,
                    png,
                });
            }
        }
        let json = serde_json::to_string_pretty(&Backup {
            format: "starclip".into(),
            version: if images.is_empty() { 1 } else { 2 },
            exported_at: now(),
            clips,
            groups,
            images,
        })
        .map_err(|e| e.to_string())?;
        check_backup_limits(json.len(), 0, 0)?;
        Ok(json)
    }

    pub fn import(&mut self, json: &str) -> Result<usize> {
        check_backup_limits(json.len(), 0, 0)?;
        let backup: Backup = serde_json::from_str(json)
            .map_err(|_| "无法读取备份文件，请选择星笺导出的 JSON 文件。")?;
        if backup.format != "starclip"
            || ![1, 2].contains(&backup.version)
            || (backup.version == 1 && !backup.images.is_empty())
        {
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
        let mut clip_ids = HashSet::new();
        for clip in &backup.clips {
            validate_content(&clip.content, &clip.title)?;
            if !clip_ids.insert(clip.id) {
                return Err("备份存在重复星笺编号。".into());
            }
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
            if (clip.kind == "image"
                && (backup.version != 2 || clip.image.is_none() || clip.language.is_some()))
                || (clip.kind != "image" && clip.image.is_some())
            {
                return Err("备份图片类型或元数据无效。".into());
            }
        }
        let mut prepared = HashMap::new();
        let mut prepared_bytes = 0usize;
        check_backup_image_limits(&backup.clips)?;
        let image_clips: HashMap<_, _> = backup
            .clips
            .iter()
            .filter(|clip| clip.kind == "image")
            .map(|clip| (clip.id, clip))
            .collect();
        for image in backup.images {
            let clip = image_clips
                .get(&image.clip_id)
                .ok_or("备份图片没有对应的图片记录。")?;
            if prepared.contains_key(&image.clip_id) {
                return Err("备份存在重复图片。".into());
            }
            let png = STANDARD
                .decode(&image.png)
                .map_err(|_| "备份图片的 Base64 数据损坏。")?;
            let value = images::prepare_png(&png)?;
            let metadata = clip.image.as_ref().unwrap();
            if metadata.width != value.metadata().width
                || metadata.height != value.metadata().height
                || metadata.bytes != png.len() as u64
                || clip.content != value.digest()
            {
                return Err("备份图片的像素、尺寸或摘要不匹配。".into());
            }
            prepared_bytes = prepared_bytes.saturating_add(value.png.len());
            check_backup_limits(prepared_bytes, 0, 0)?;
            prepared.insert(image.clip_id, value);
        }
        if backup
            .clips
            .iter()
            .any(|clip| clip.kind == "image" && !prepared.contains_key(&clip.id))
        {
            return Err("备份缺少原始图片，未导入不完整记录。".into());
        }
        // All image bytes are validated before the transaction creates any group or clip.
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
            let is_image = clip.kind == "image";
            let predicate = if is_image {
                "kind='image'"
            } else {
                "kind<>'image'"
            };
            let existed: Option<i64> = tx
                .query_row(
                    &format!("SELECT id FROM clips WHERE content=?1 AND {predicate}"),
                    [&clip.content],
                    |row| row.get(0),
                )
                .optional()
                .map_err(sql_error)?;
            let kind = if is_image {
                "image"
            } else if clip.language.is_some() {
                "code"
            } else {
                classify(&clip.content)
            };
            tx.execute(&format!("INSERT INTO clips(content,title,kind,pinned,group_id,created_at,updated_at,copy_count,language,keep_forever) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)
                ON CONFLICT(content) WHERE {predicate} DO UPDATE SET pinned=MAX(clips.pinned,excluded.pinned),
                keep_forever=MAX(clips.keep_forever,excluded.keep_forever),
                group_id=COALESCE(clips.group_id,excluded.group_id),
                title=CASE WHEN clips.title='' THEN excluded.title ELSE clips.title END,
                language=COALESCE(clips.language,excluded.language),
                kind=CASE WHEN clips.language IS NOT NULL OR excluded.language IS NOT NULL THEN 'code' ELSE clips.kind END"),
                params![clip.content, clip.title, kind, clip.pinned, group_id, clip.created_at, clip.updated_at, clip.copy_count, clip.language, clip.keep_forever]).map_err(sql_error)?;
            if let Some(image) = prepared.remove(&clip.id) {
                let id = existed.unwrap_or_else(|| tx.last_insert_rowid());
                Self::put_image(&tx, id, &image)?;
            }
            if existed.is_none() {
                imported += 1;
            }
        }
        // Import keeps the entire backup; capture, startup and maintenance apply retention later.
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
    fn image_fixture(shade: u8) -> PreparedImage {
        images::prepare_rgba(2, 2, &[shade, 70, 180, 255].repeat(4)).unwrap()
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
    fn starpet_preferences_are_optional_persistent_and_independent_of_capture_and_retention() {
        let scratch = Scratch::new();
        let path = scratch.0.join("starpet.sqlite3");
        let fresh = Store::open(&path).unwrap();
        assert_eq!(
            fresh.starpet_preferences().unwrap(),
            StarpetPreferences::default()
        );
        assert_eq!(
            fresh
                .conn
                .query_row(
                    "SELECT count(*) FROM preferences WHERE key='starpet_v1'",
                    [],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            0
        );
        fresh
            .conn
            .execute(
                "INSERT INTO preferences(key,value) VALUES('starpet_v1','{}')",
                [],
            )
            .unwrap();
        assert_eq!(
            fresh.starpet_preferences().unwrap(),
            StarpetPreferences::default()
        );
        assert_eq!(
            fresh
                .conn
                .query_row(
                    "SELECT value FROM preferences WHERE key='starpet_v1'",
                    [],
                    |row| row.get::<_, String>(0)
                )
                .unwrap(),
            "{}",
            "reading defaults must not rewrite existing data"
        );
        let legacy = r##"{"enabled":true,"mood":"sleepy"}"##;
        fresh
            .conn
            .execute(
                "UPDATE preferences SET value=?1 WHERE key='starpet_v1'",
                [legacy],
            )
            .unwrap();
        assert_eq!(
            fresh.starpet_preferences().unwrap(),
            StarpetPreferences {
                enabled: true,
                mood: "sleepy".into(),
                color: "#FFD66E".into(),
                size: 100,
            }
        );
        assert_eq!(
            fresh
                .conn
                .query_row(
                    "SELECT value FROM preferences WHERE key='starpet_v1'",
                    [],
                    |row| row.get::<_, String>(0)
                )
                .unwrap(),
            legacy
        );
        let preferences = StarpetPreferences {
            enabled: true,
            mood: "curious".into(),
            color: "#8CB9BD".into(),
            size: 120,
        };
        fresh.save_starpet_preferences(&preferences).unwrap();
        drop(fresh);
        let db = Store::open(&path).unwrap();
        assert_eq!(db.starpet_preferences().unwrap(), preferences);

        let settings = Settings {
            paused: false,
            retention_days: 1,
            max_history: 50,
            ..Settings::default()
        };
        let json = serde_json::to_string(&settings).unwrap();
        db.conn
            .execute(
                "INSERT INTO preferences(key,value) VALUES('settings',?1)",
                [&json],
            )
            .unwrap();
        db.conn.execute("INSERT INTO clips(content,kind,created_at,updated_at) VALUES('星伴表情变化不能删除这条原创超期片段','text',0,0)", []).unwrap();
        db.conn.execute_batch("CREATE TRIGGER forbid_starpet_prune BEFORE DELETE ON clips BEGIN SELECT RAISE(ABORT, 'starpet must not prune'); END;").unwrap();
        assert_eq!(STARPET_MOODS.len(), 32);
        for mood in STARPET_MOODS {
            let value = StarpetPreferences {
                enabled: *mood != "sleeping",
                mood: (*mood).into(),
                color: preferences.color.clone(),
                size: preferences.size,
            };
            db.save_starpet_preferences(&value).unwrap();
            assert_eq!(db.starpet_preferences().unwrap(), value);
        }
        assert_eq!(db.settings().unwrap(), settings);
        assert_eq!(db.clips().unwrap().len(), 1);
        assert!(!db.onboarding_completed().unwrap());
        assert_eq!(
            db.conn
                .query_row(
                    "SELECT value FROM preferences WHERE key='settings'",
                    [],
                    |row| row.get::<_, String>(0)
                )
                .unwrap(),
            json
        );
        let old = db.starpet_preferences().unwrap();
        for mood in ["", "unknown", "IDLE", "idle "] {
            assert!(db
                .save_starpet_preferences(&StarpetPreferences {
                    enabled: true,
                    mood: mood.into(),
                    color: old.color.clone(),
                    size: old.size,
                })
                .is_err());
            assert_eq!(db.starpet_preferences().unwrap(), old);
        }
        for (patch, expected) in [
            (
                r#"{"enabled":false}"#,
                StarpetPreferences {
                    enabled: false,
                    ..old.clone()
                },
            ),
            (
                r#"{"mood":"greet"}"#,
                StarpetPreferences {
                    enabled: false,
                    mood: "greet".into(),
                    color: old.color.clone(),
                    size: old.size,
                },
            ),
            (
                r##"{"color":"#aabbcc"}"##,
                StarpetPreferences {
                    enabled: false,
                    mood: "greet".into(),
                    color: "#AABBCC".into(),
                    size: old.size,
                },
            ),
            (
                r#"{"size":160}"#,
                StarpetPreferences {
                    enabled: false,
                    mood: "greet".into(),
                    color: "#AABBCC".into(),
                    size: 160,
                },
            ),
        ] {
            let patch: StarpetPreferencePatch = serde_json::from_str(patch).unwrap();
            let mut value = db.starpet_preferences().unwrap();
            patch.apply_to(&mut value);
            db.save_starpet_preferences(&value).unwrap();
            assert_eq!(db.starpet_preferences().unwrap(), expected);
        }
        let old = db.starpet_preferences().unwrap();
        for size in [0, 69, 161, u16::MAX] {
            assert!(db
                .save_starpet_preferences(&StarpetPreferences {
                    size,
                    ..old.clone()
                })
                .is_err());
            assert_eq!(db.starpet_preferences().unwrap(), old);
        }
        for json in [r#"{"size":-1}"#, r#"{"size":100.5}"#, r#"{"size":65536}"#] {
            assert!(serde_json::from_str::<StarpetPreferencePatch>(json).is_err());
        }
        assert_eq!(db.settings().unwrap(), settings);
        assert_eq!(
            db.clips().unwrap().len(),
            1,
            "size changes must not run retention"
        );
        for color in [
            "",
            "#fff",
            "112233",
            "#1234567",
            "#12345g",
            "#１２３",
            " #123456",
            "#123456\n",
        ] {
            assert!(db
                .save_starpet_preferences(&StarpetPreferences {
                    color: color.into(),
                    ..old.clone()
                })
                .is_err());
            assert_eq!(db.starpet_preferences().unwrap(), old);
        }
        db.conn.execute_batch("CREATE TRIGGER fail_starpet_save BEFORE UPDATE ON preferences WHEN NEW.key='starpet_v1' BEGIN SELECT RAISE(ABORT, 'fixture disk error'); END;").unwrap();
        assert!(db.save_starpet_preferences(&preferences).is_err());
        assert_eq!(
            db.starpet_preferences().unwrap(),
            old,
            "failed persistence retains the complete previous preference"
        );
    }

    #[test]
    fn starpet_geometry_anchors_bottom_center_and_stays_in_work_area() {
        for (size, expected) in [
            (70, (176.0, 206.0)),
            (100, (230.0, 260.0)),
            (160, (338.0, 368.0)),
        ] {
            let preferences = StarpetPreferences {
                size,
                ..StarpetPreferences::default()
            };
            assert!(preferences.validate().is_ok());
            assert_eq!(preferences.window_size(), expected);
        }
        let area = (0.0, 0.0, 1920.0, 1080.0);
        let enlarged =
            starpet_resize_position((500.0, 500.0), (230.0, 260.0), (338.0, 368.0), area);
        assert_eq!(enlarged, (446.0, 392.0));
        assert_eq!(
            starpet_resize_position(enlarged, (338.0, 368.0), (230.0, 260.0), area),
            (500.0, 500.0)
        );
        assert_eq!(
            starpet_resize_position((1690.0, 820.0), (230.0, 260.0), (338.0, 368.0), area),
            (1582.0, 712.0)
        );
        assert_eq!(
            starpet_resize_position(
                (-1900.0, -180.0),
                (230.0, 260.0),
                (338.0, 368.0),
                (-1920.0, -200.0, 1920.0, 1080.0)
            ),
            (-1920.0, -200.0)
        );
        assert_eq!(
            starpet_resize_position(
                (1000.0, 1000.0),
                (460.0, 520.0),
                (676.0, 736.0),
                (0.0, 0.0, 3840.0, 2160.0)
            ),
            (892.0, 784.0),
            "geometry uses physical pixels consistently on HiDPI displays"
        );
        assert_eq!(
            starpet_resize_position(
                (20.0, 30.0),
                (10.0, 10.0),
                (200.0, 200.0),
                (10.0, 20.0, 100.0, 100.0)
            ),
            (10.0, 20.0)
        );
    }

    #[test]
    fn immediate_theme_change_preserves_legacy_preferences_and_never_prunes() {
        let scratch = Scratch::new();
        let path = scratch.0.join("theme.sqlite3");
        let fresh = Store::open(&path).unwrap();
        assert_eq!(fresh.settings().unwrap(), Settings::default());
        let initial = fresh.set_theme("white").unwrap();
        assert_eq!(
            initial,
            Settings {
                theme: "white".into(),
                ..Settings::default()
            }
        );
        assert!(!fresh.onboarding_completed().unwrap());
        drop(fresh);
        assert_eq!(Store::open(&path).unwrap().settings().unwrap(), initial);
        let db = store();
        let legacy = r#"{"paused":false,"theme":"dark","layout":"horizontal","maxHistory":50,"shortcut":"CommandOrControl+Shift+V","retentionDays":1,"futurePreference":"preserve"}"#;
        db.conn
            .execute(
                "INSERT INTO preferences(key,value) VALUES('settings',?1)",
                [legacy],
            )
            .unwrap();
        db.conn.execute("INSERT INTO clips(content,kind,created_at,updated_at) VALUES('只换主题不能清理这条原创超期记录','text',0,0)", []).unwrap();
        db.conn.execute_batch("CREATE TRIGGER forbid_theme_prune BEFORE DELETE ON clips BEGIN SELECT RAISE(ABORT, 'theme must not prune'); END;").unwrap();
        let mut expected: serde_json::Value = serde_json::from_str(legacy).unwrap();
        for theme in ["light", "white", "forest", "rose", "system", "dark"] {
            let settings = db.set_theme(theme).unwrap();
            assert_eq!(settings.theme, theme);
            assert!(!settings.paused);
            assert_eq!(settings.retention_days, 1);
            assert_eq!(db.clips().unwrap().len(), 1);
            expected["theme"] = theme.into();
            let saved: String = db
                .conn
                .query_row(
                    "SELECT value FROM preferences WHERE key='settings'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(&saved).unwrap(),
                expected
            );
        }
        let before = db.settings().unwrap();
        assert!(db.set_theme("unknown").is_err());
        assert_eq!(db.settings().unwrap(), before);
        db.conn.execute_batch("CREATE TRIGGER fail_theme_save BEFORE UPDATE ON preferences WHEN NEW.key='settings' BEGIN SELECT RAISE(ABORT, 'fixture write failure'); END;").unwrap();
        assert!(db.set_theme("rose").is_err());
        assert_eq!(db.settings().unwrap(), before);
        assert_eq!(db.clips().unwrap().len(), 1);
        assert_eq!(
            db.starpet_preferences().unwrap(),
            StarpetPreferences::default()
        );
    }

    #[test]
    fn onboarding_is_upgrade_safe_persistent_and_has_no_retention_or_settings_side_effects() {
        let scratch = Scratch::new();
        let path = scratch.0.join("onboarding.sqlite3");
        let fresh = Store::open(&path).unwrap();
        assert!(!fresh.onboarding_completed().unwrap());
        drop(fresh);
        let fresh = Store::open(&path).unwrap();
        assert!(
            !fresh.onboarding_completed().unwrap(),
            "unfinished first-run status survives restart"
        );
        fresh.complete_onboarding().unwrap();
        drop(fresh);
        let reopened = Store::open(&path).unwrap();
        assert!(reopened.onboarding_completed().unwrap());
        assert_eq!(reopened.settings().unwrap(), Settings::default());
        assert_eq!(
            reopened
                .conn
                .query_row(
                    "SELECT count(*) FROM preferences WHERE key='settings'",
                    [],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            0
        );

        let old_empty = store();
        old_empty
            .conn
            .execute("DELETE FROM preferences WHERE key='onboarding_v1'", [])
            .unwrap();
        let old_empty = Store::from_connection(old_empty.conn).unwrap();
        assert!(
            old_empty.onboarding_completed().unwrap(),
            "an existing empty database is still an upgrade"
        );

        let existing = store();
        existing
            .conn
            .execute("DELETE FROM preferences WHERE key='onboarding_v1'", [])
            .unwrap();
        let settings = Settings {
            paused: false,
            max_history: 50,
            retention_days: 1,
            ..Settings::default()
        };
        let json = serde_json::to_string(&settings).unwrap();
        existing
            .conn
            .execute(
                "INSERT INTO preferences(key,value) VALUES('settings',?1)",
                [&json],
            )
            .unwrap();
        existing.conn.execute("INSERT INTO clips(content,kind,created_at,updated_at) VALUES('引导操作不得清理这条超期片段','text',0,0)", []).unwrap();
        existing.conn.execute_batch("CREATE TRIGGER forbid_onboarding_prune BEFORE DELETE ON clips BEGIN SELECT RAISE(ABORT, 'onboarding must not prune'); END;").unwrap();
        assert!(existing.onboarding_completed().unwrap());
        existing.complete_onboarding().unwrap();
        assert!(existing.onboarding_completed().unwrap());
        assert_eq!(existing.clips().unwrap().len(), 1);
        assert_eq!(existing.settings().unwrap(), settings);
        assert_eq!(
            existing
                .conn
                .query_row(
                    "SELECT value FROM preferences WHERE key='settings'",
                    [],
                    |row| row.get::<_, String>(0)
                )
                .unwrap(),
            json
        );
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
        for theme in ["forest", "rose", "white"] {
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
    fn image_migration_preserves_legacy_fields_and_separates_text_identity() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;
            CREATE TABLE groups(id INTEGER PRIMARY KEY,name TEXT NOT NULL UNIQUE,color TEXT NOT NULL);
            INSERT INTO groups VALUES(7,'原创旧分组','#abcdef');
            CREATE TABLE clips(id INTEGER PRIMARY KEY,content TEXT NOT NULL UNIQUE,title TEXT NOT NULL DEFAULT '',
                kind TEXT NOT NULL,pinned INTEGER NOT NULL DEFAULT 0,keep_forever INTEGER NOT NULL DEFAULT 0,
                group_id INTEGER REFERENCES groups(id) ON DELETE SET NULL,created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,copy_count INTEGER NOT NULL DEFAULT 0,language TEXT);
            INSERT INTO clips VALUES(11,'fn original() {}','旧代码','code',1,1,7,12,34,9,'rust');").unwrap();
        let db = Store::from_connection(conn).unwrap();
        let legacy = db.clip(11).unwrap();
        assert_eq!(
            (legacy.created_at, legacy.updated_at, legacy.copy_count),
            (12, 34, 9)
        );
        assert_eq!(legacy.group_id, Some(7));
        assert!(legacy.pinned && legacy.keep_forever);
        assert_eq!(legacy.language.as_deref(), Some("rust"));
        assert_eq!(legacy.title, "旧代码");
        assert!(legacy.image.is_none());
        let old_backup: serde_json::Value = serde_json::from_str(&db.export().unwrap()).unwrap();
        assert_eq!(old_backup["version"], 1);
        assert!(old_backup.get("images").is_none());

        let mut budget_clip = legacy.clone();
        budget_clip.image = Some(ImageMetadata {
            width: 4000,
            height: 4000,
            bytes: 100,
        });
        assert!(check_backup_image_limits(&vec![budget_clip.clone(); 16]).is_ok());
        assert!(check_backup_image_limits(&vec![budget_clip; 17]).is_err());

        let pixels = image_fixture(50);
        let text = db.add(pixels.digest(), "恰好相同的普通文本", None).unwrap();
        let picture = db.add_image(&pixels, "图片", Some(7), true).unwrap();
        assert_ne!(text.id, picture.id);
        db.update(picture.id, "图片标题", Some(7), true).unwrap();
        db.mark_copied(picture.id).unwrap();
        db.conn
            .execute("UPDATE clips SET updated_at=0 WHERE id=?1", [picture.id])
            .unwrap();
        let repeated = db.add_image(&pixels, "", None, false).unwrap();
        assert_eq!(picture.id, repeated.id);
        assert!(repeated.updated_at > 0);
        assert!(repeated.pinned && repeated.keep_forever);
        assert_eq!(repeated.copy_count, 1);
        assert_eq!(repeated.group_id, Some(7));
        assert_eq!(repeated.title, "图片标题");
        assert_eq!(db.add(pixels.digest(), "", None).unwrap().id, text.id);
        assert_eq!(db.clip(text.id).unwrap().kind, "text");
        assert!(db
            .save_code(Some(picture.id), "不应转换", "hello", "plaintext", None)
            .is_err());
        let list_json = serde_json::to_value(db.clips().unwrap()).unwrap();
        let listed = list_json
            .as_array()
            .unwrap()
            .iter()
            .find(|clip| clip["id"] == picture.id)
            .unwrap();
        assert_eq!(listed["image"]["bytes"], pixels.png.len());
        assert!(listed.get("png").is_none());
        db.delete_group(7).unwrap();
        assert!(db.clip(11).unwrap().group_id.is_none());
        assert!(db.clip(picture.id).unwrap().group_id.is_none());
        let reopened = Store::from_connection(db.conn).unwrap();
        assert_eq!(reopened.image_png(picture.id, false).unwrap(), pixels.png);
    }

    #[test]
    fn image_backup_roundtrip_validation_and_failure_are_atomic() {
        let source = store();
        let group = source.create_group("原创像素", "#abcdef").unwrap();
        let pixels = image_fixture(91);
        let picture = source
            .add_image(&pixels, "透明像素测试", Some(group.id), true)
            .unwrap();
        source
            .update(picture.id, "透明像素测试", Some(group.id), true)
            .unwrap();
        source.add(pixels.digest(), "相同标识的文本", None).unwrap();
        let json = source.export().unwrap();
        let backup: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(backup["version"], 2);
        assert_eq!(
            STANDARD
                .decode(backup["images"][0]["png"].as_str().unwrap())
                .unwrap(),
            pixels.png
        );
        let mut destination = store();
        assert_eq!(destination.import(&json).unwrap(), 2);
        assert_eq!(destination.import(&json).unwrap(), 0);
        let imported = destination
            .clips()
            .unwrap()
            .into_iter()
            .find(|clip| clip.kind == "image")
            .unwrap();
        assert_eq!(
            destination.image_png(imported.id, false).unwrap(),
            pixels.png
        );
        assert_eq!(
            destination.image_png(imported.id, true).unwrap(),
            pixels.thumbnail
        );
        assert!(imported.pinned && imported.keep_forever && imported.group_id.is_some());
        assert!(destination.settings().unwrap().paused);
        let mut unprotected = backup.clone();
        for clip in unprotected["clips"].as_array_mut().unwrap() {
            clip["keepForever"] = false.into();
            clip["pinned"] = false.into();
        }
        destination.import(&unprotected.to_string()).unwrap();
        assert!(destination.clip(imported.id).unwrap().keep_forever);
        assert!(destination.clip(imported.id).unwrap().pinned);

        let picture_index = backup["clips"]
            .as_array()
            .unwrap()
            .iter()
            .position(|clip| clip["kind"] == "image")
            .unwrap();
        for fault in 0..9 {
            let mut broken = backup.clone();
            broken["groups"][0]["name"] = "不应留下的分组".into();
            match fault {
                0 => broken["images"] = serde_json::json!([]),
                1 => broken["images"][0]["png"] = "not-base64".into(),
                2 => broken["clips"][picture_index]["image"]["width"] = 99.into(),
                3 => broken["clips"][picture_index]["image"]["bytes"] = 1.into(),
                4 => broken["clips"][picture_index]["content"] = "sha256:forged".into(),
                5 => broken["version"] = 1.into(),
                6 => broken["images"][0]["clipId"] = 9999.into(),
                7 => {
                    let duplicate = broken["images"][0].clone();
                    broken["images"].as_array_mut().unwrap().push(duplicate);
                }
                _ => {
                    let mut png = pixels.png.clone();
                    png[29] ^= 1;
                    broken["images"][0]["png"] = STANDARD.encode(png).into();
                }
            }
            assert!(
                destination.import(&broken.to_string()).is_err(),
                "fault {fault}"
            );
            assert_eq!(destination.clips().unwrap().len(), 2);
            assert_eq!(destination.groups().unwrap().len(), 1);
        }
        let mut failing = store();
        failing.conn.execute_batch("CREATE TRIGGER refuse_image BEFORE INSERT ON clip_images BEGIN SELECT RAISE(ABORT, 'fixture image write failure'); END;").unwrap();
        assert!(failing.import(&json).is_err());
        assert!(failing.clips().unwrap().is_empty());
        assert!(failing.groups().unwrap().is_empty());
        assert!(failing.add_image(&pixels, "写入失败", None, false).is_err());
        assert!(failing.clips().unwrap().is_empty());
    }

    #[test]
    fn image_cleanup_bytes_protection_and_cascades_cover_all_delete_paths() {
        let mut db = store();
        let pixels = image_fixture(12);
        let picture = db.add_image(&pixels, "星图", None, false).unwrap();
        let expected = pixels.png.len() as u64 + "星图".len() as u64;
        let mut filter = all_cleanup();
        filter.kind = "image".into();
        filter.min_bytes = expected;
        assert_eq!(
            db.preview_cleanup(&filter).unwrap().preview.matched_count,
            0
        );
        filter.min_bytes -= 1;
        let preview = db.preview_cleanup(&filter).unwrap();
        assert_eq!(preview.preview.content_bytes, expected);
        assert_eq!(preview.preview.items[0].bytes, expected);
        db.set_keep_forever(picture.id, true).unwrap();
        assert!(db.execute_cleanup(preview, false).is_err());
        assert!(db.delete(picture.id).is_err());
        db.clear_history().unwrap();
        assert!(db.image_png(picture.id, false).is_ok());
        db.set_keep_forever(picture.id, false).unwrap();
        let preview = db.preview_cleanup(&filter).unwrap();
        let next = db
            .add_image(&image_fixture(13), "预览之后新图片", None, false)
            .unwrap();
        let result = db.execute_cleanup(preview, false).unwrap();
        assert_eq!(result.deleted_count, 1);
        assert_eq!(result.content_bytes, expected);
        assert!(db.image_png(picture.id, false).is_err());
        assert!(db.image_png(next.id, false).is_ok());
        db.delete(next.id).unwrap();
        assert_eq!(
            db.conn
                .query_row("SELECT count(*) FROM clip_images", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );

        let group = db.create_group("保护图片", "#123abc").unwrap();
        let protected = db
            .add_image(&image_fixture(20), "永久", None, true)
            .unwrap();
        let pinned = db
            .add_image(&image_fixture(21), "星标", None, false)
            .unwrap();
        db.update(pinned.id, "星标", None, true).unwrap();
        let grouped = db
            .add_image(&image_fixture(22), "分组", Some(group.id), false)
            .unwrap();
        let expired = db
            .add_image(&image_fixture(23), "超时", None, false)
            .unwrap();
        db.conn
            .execute("UPDATE clips SET updated_at=0", [])
            .unwrap();
        let settings = Settings {
            retention_days: 1,
            ..Settings::default()
        };
        db.save_settings(&settings).unwrap();
        assert!(db.image_png(expired.id, false).is_err());
        for id in [protected.id, pinned.id, grouped.id] {
            assert!(db.image_png(id, false).is_ok());
        }
        db.clear_history().unwrap();
        assert!(db.image_png(grouped.id, false).is_err());
        for id in [protected.id, pinned.id] {
            assert!(db.image_png(id, false).is_ok());
        }
        let mut settings = settings;
        settings.retention_days = 0;
        settings.max_history = 50;
        db.save_settings(&settings).unwrap();
        let old = db
            .add_image(&image_fixture(24), "容量淘汰", None, false)
            .unwrap();
        for index in 0..50 {
            db.add(&format!("原创容量片段 {index}"), "", None).unwrap();
        }
        assert!(db.image_png(old.id, false).is_err());
        assert_eq!(
            db.conn
                .query_row("SELECT count(*) FROM clip_images", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            2
        );
    }

    #[test]
    fn wal_snapshot_preserves_original_and_thumbnail_without_external_assets() {
        let scratch = Scratch::new();
        let path = scratch.0.join("images.sqlite3");
        let snapshot = scratch.0.join("snapshot.sqlite3");
        let db = Store::open(&path).unwrap();
        db.conn.pragma_update(None, "journal_mode", "WAL").unwrap();
        db.conn
            .pragma_update(None, "wal_autocheckpoint", 0)
            .unwrap();
        let pixels =
            images::prepare_rgba(640, 160, &[90, 120, 210, 120].repeat(640 * 160)).unwrap();
        let clip = db
            .add_image(&pixels, "原创半透明横幅", None, false)
            .unwrap();
        db.snapshot(&snapshot).unwrap();
        Store::validate_database(&snapshot).unwrap();
        db.delete(clip.id).unwrap();
        let copy = Store::open(&snapshot).unwrap();
        assert_eq!(copy.image_png(clip.id, false).unwrap(), pixels.png);
        assert_eq!(copy.image_png(clip.id, true).unwrap(), pixels.thumbnail);
        assert_eq!(
            copy.clip(clip.id).unwrap().image.as_ref(),
            Some(pixels.metadata())
        );
        assert_eq!(
            images::decode_png(&copy.image_png(clip.id, true).unwrap())
                .unwrap()
                .0,
            320
        );
        assert!(db.clips().unwrap().is_empty());
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
