use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    path::Path,
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
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            paused: true,
            theme: "dark".into(),
            layout: "grid".into(),
            max_history: 500,
            shortcut: "CommandOrControl+Shift+V".into(),
            always_on_top: false,
            retention_days: 0,
        }
    }
}

impl Settings {
    pub fn validate(&self) -> Result<()> {
        if !["dark", "light", "system"].contains(&self.theme.as_str())
            || !["grid", "list", "horizontal"].contains(&self.layout.as_str())
            || !(50..=10_000).contains(&self.max_history)
            || self.shortcut.trim().is_empty()
            || self.shortcut.len() > 80
        {
            return Err("设置无效：历史上限需在 50–10000 之间。".into());
        }
        if ![0, 1, 7, 30, 90, 180, 365].contains(&self.retention_days) {
            return Err("保留天数仅支持不限时间、1、7、30、90、180 或 365 天。".into());
        }
        Ok(())
    }
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
                kind TEXT NOT NULL, pinned INTEGER NOT NULL DEFAULT 0,
                group_id INTEGER REFERENCES groups(id) ON DELETE SET NULL,
                created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL, copy_count INTEGER NOT NULL DEFAULT 0, language TEXT
            );
            CREATE INDEX IF NOT EXISTS clips_updated ON clips(updated_at DESC);
            CREATE TABLE IF NOT EXISTS preferences (key TEXT PRIMARY KEY, value TEXT NOT NULL);").map_err(sql_error)?;
        let has_language = {
            let mut stmt = conn
                .prepare("PRAGMA table_info(clips)")
                .map_err(sql_error)?;
            let columns = stmt
                .query_map([], |row| row.get::<_, String>(1))
                .map_err(sql_error)?;
            columns
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(sql_error)?
                .iter()
                .any(|name| name == "language")
        };
        if !has_language {
            conn.execute("ALTER TABLE clips ADD COLUMN language TEXT", [])
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
        let mut stmt = self.conn.prepare("SELECT id,content,title,kind,pinned,group_id,created_at,updated_at,copy_count,language FROM clips ORDER BY updated_at DESC,id DESC").map_err(sql_error)?;
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
        self.conn.query_row("SELECT id,content,title,kind,pinned,group_id,created_at,updated_at,copy_count,language FROM clips WHERE id=?1", [id], clip_from_row).optional().map_err(sql_error)?.ok_or_else(|| "这条星笺已被删除。".into())
    }

    pub fn add(&self, content: &str, title: &str, group_id: Option<i64>) -> Result<Clip> {
        validate_content(content, title)?;
        let timestamp = now();
        self.conn.execute("INSERT INTO clips(content,title,kind,group_id,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?5)
            ON CONFLICT(content) DO UPDATE SET updated_at=excluded.updated_at,
            title=CASE WHEN excluded.title<>'' THEN excluded.title ELSE clips.title END,
            group_id=COALESCE(excluded.group_id,clips.group_id)", params![content, title.trim(), classify(content), group_id, timestamp]).map_err(sql_error)?;
        let id = self
            .conn
            .query_row("SELECT id FROM clips WHERE content=?1", [content], |row| {
                row.get(0)
            })
            .map_err(sql_error)?;
        self.prune_history()?;
        self.clip(id)
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
        self.conn
            .execute("DELETE FROM clips WHERE id=?1", [id])
            .map_err(sql_error)?;
        Ok(())
    }

    pub fn clear_history(&self) -> Result<()> {
        self.conn
            .execute("DELETE FROM clips WHERE pinned=0", [])
            .map_err(sql_error)?;
        Ok(())
    }

    pub fn prune_history(&self) -> Result<usize> {
        Self::prune_with_settings(&self.conn, &self.settings()?, now())
    }

    fn prune_with_settings(
        conn: &Connection,
        settings: &Settings,
        timestamp: i64,
    ) -> Result<usize> {
        let cutoff = timestamp.saturating_sub(i64::from(settings.retention_days) * 86_400_000);
        // Keep both limits in one atomic delete. Pins and grouped snippets are never automatic cleanup targets.
        conn.execute("DELETE FROM clips WHERE pinned=0 AND group_id IS NULL AND (
            (?2 > 0 AND updated_at < ?3) OR id NOT IN
            (SELECT id FROM clips WHERE pinned=0 AND group_id IS NULL ORDER BY updated_at DESC,id DESC LIMIT ?1))",
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
            tx.execute("INSERT INTO clips(content,title,kind,pinned,group_id,created_at,updated_at,copy_count,language) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)
                ON CONFLICT(content) DO UPDATE SET pinned=MAX(clips.pinned,excluded.pinned),
                group_id=COALESCE(clips.group_id,excluded.group_id),
                title=CASE WHEN clips.title='' THEN excluded.title ELSE clips.title END,
                language=COALESCE(clips.language,excluded.language),
                kind=CASE WHEN clips.language IS NOT NULL OR excluded.language IS NOT NULL THEN 'code' ELSE clips.kind END",
                params![clip.content, clip.title, if clip.language.is_some() { "code" } else { classify(&clip.content) }, clip.pinned, group_id, clip.created_at, clip.updated_at, clip.copy_count, clip.language]).map_err(sql_error)?;
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
        assert_eq!(settings.retention_days, 0);
        assert_eq!(settings.layout, "list");
        assert_eq!(db.clips().unwrap()[0].language, None);
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
        let mut destination = store();
        let mut updated = settings.clone();
        updated.layout = "horizontal".into();
        updated.always_on_top = true;
        updated.retention_days = 7;
        destination.save_settings(&updated).unwrap();
        assert_eq!(destination.import(&old_backup.to_string()).unwrap(), 1);
        assert_eq!(destination.settings().unwrap(), updated);
        assert!(destination.clips().unwrap()[0].pinned);
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
