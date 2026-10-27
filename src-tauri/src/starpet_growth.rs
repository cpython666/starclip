use crate::store::Result;
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, sync::OnceLock};

pub const MAX_XP: u32 = 10_000_000;
pub const MAX_DAYS: u32 = 1_000_000;
pub const MAX_REVISION: u64 = 9_007_199_254_740_991;
pub const MAX_SAVE_BYTES: usize = 64 * 1024;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Catalog {
    max_level: u32,
    tasks: Vec<Task>,
    actions: Vec<Action>,
    items: Vec<Item>,
    achievements: Vec<Named>,
}
#[derive(Debug, Deserialize)]
struct Named {
    id: String,
}
#[derive(Debug, Deserialize)]
struct Task {
    id: String,
    xp: u32,
}
#[derive(Debug, Deserialize)]
struct Action {
    id: String,
    level: u32,
    task: String,
}
#[derive(Debug, Deserialize)]
struct Item {
    id: String,
    slot: String,
    level: u32,
    achievement: Option<String>,
}
fn catalog() -> &'static Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        serde_json::from_str(include_str!("../../src/starpet-growth-catalog.json"))
            .expect("bundled starpet catalog must be valid")
    })
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Equipment {
    pub back: Option<String>,
    pub face: Option<String>,
    pub hand: Option<String>,
    pub charm: Option<String>,
}
impl Equipment {
    fn slot_mut(&mut self, slot: &str) -> Result<&mut Option<String>> {
        match slot {
            "back" => Ok(&mut self.back),
            "face" => Ok(&mut self.face),
            "hand" => Ok(&mut self.hand),
            "charm" => Ok(&mut self.charm),
            _ => Err("不支持的星伴装扮位置。".into()),
        }
    }
    fn entries(&self) -> [(&str, &Option<String>); 4] {
        [
            ("back", &self.back),
            ("face", &self.face),
            ("hand", &self.hand),
            ("charm", &self.charm),
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GrowthState {
    pub version: u32,
    pub enabled: bool,
    pub xp: u32,
    pub day: String,
    pub completed_tasks: Vec<String>,
    pub interaction_days: u32,
    pub play_days: u32,
    pub last_interaction_day: String,
    pub achievements: Vec<String>,
    pub unlocked_items: Vec<String>,
    pub equipment: Equipment,
    pub revision: u64,
}
impl Default for GrowthState {
    fn default() -> Self {
        Self {
            version: 1,
            enabled: false,
            xp: 0,
            day: String::new(),
            completed_tasks: vec![],
            interaction_days: 0,
            play_days: 0,
            last_interaction_day: String::new(),
            achievements: vec![],
            unlocked_items: vec![],
            equipment: Equipment::default(),
            revision: 0,
        }
    }
}

pub fn level(xp: u32) -> u32 {
    (1..=catalog().max_level)
        .rev()
        .find(|&n| xp >= 40 * (n - 1) + 5 * (n - 1) * n.saturating_sub(2))
        .unwrap_or(1)
}
pub fn action_unlocked(xp: u32, action: &str) -> bool {
    catalog()
        .actions
        .iter()
        .any(|entry| entry.id == action && level(xp) >= entry.level)
}
fn valid_day(day: &str, empty_allowed: bool) -> bool {
    if day.is_empty() {
        return empty_allowed;
    }
    let b = day.as_bytes();
    if b.len() != 10
        || b[4] != b'-'
        || b[7] != b'-'
        || b.iter()
            .enumerate()
            .any(|(i, c)| i != 4 && i != 7 && !c.is_ascii_digit())
    {
        return false;
    }
    let year: u32 = day[..4].parse().unwrap_or(0);
    let month: usize = day[5..7].parse().unwrap_or(0);
    let date: u32 = day[8..].parse().unwrap_or(0);
    if year < 1970 || !(1..=12).contains(&month) {
        return false;
    }
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    (1..=days[month - 1]).contains(&date)
}
fn unique_known(values: &[String], allowed: impl Iterator<Item = String>) -> bool {
    let allowed: HashSet<_> = allowed.collect();
    let unique: HashSet<_> = values.iter().collect();
    values.len() == unique.len() && values.iter().all(|value| allowed.contains(value))
}
fn insert(values: &mut Vec<String>, value: &str) {
    if !values.iter().any(|v| v == value) {
        values.push(value.into());
    }
}
impl GrowthState {
    pub fn validate(&self) -> Result<()> {
        let c = catalog();
        if self.version != 1
            || self.xp > MAX_XP
            || self.interaction_days > MAX_DAYS
            || self.play_days > self.interaction_days
            || self.revision > MAX_REVISION
            || !valid_day(&self.day, true)
            || !valid_day(&self.last_interaction_day, true)
            || self.last_interaction_day > self.day
            || (self.day.is_empty() && !self.completed_tasks.is_empty())
            || ((self.interaction_days == 0) != self.last_interaction_day.is_empty())
            || !unique_known(&self.completed_tasks, c.tasks.iter().map(|t| t.id.clone()))
            || !unique_known(
                &self.achievements,
                c.achievements.iter().map(|a| a.id.clone()),
            )
            || !unique_known(&self.unlocked_items, c.items.iter().map(|i| i.id.clone()))
        {
            return Err("星伴成长数据无效或版本不受支持，现有进度未改变。".into());
        }
        for (slot, value) in self.equipment.entries() {
            if let Some(id) = value {
                if !self.unlocked_items.contains(id)
                    || !c
                        .items
                        .iter()
                        .any(|item| item.id == *id && item.slot == slot)
                {
                    return Err("成长存档包含未获得或位置不符的装饰。".into());
                }
            }
        }
        Ok(())
    }
    fn unlock(&mut self) {
        let l = level(self.xp);
        if self.interaction_days >= 7 {
            insert(&mut self.achievements, "seven-days");
        }
        if self.play_days >= 5 {
            insert(&mut self.achievements, "play-five");
        }
        if l >= 5 {
            insert(&mut self.achievements, "level-five");
        }
        if l >= 10 {
            insert(&mut self.achievements, "level-ten");
        }
        for item in &catalog().items {
            let earned = match &item.achievement {
                Some(id) => self.achievements.contains(id),
                None => l >= item.level,
            };
            if earned {
                insert(&mut self.unlocked_items, &item.id);
            }
        }
    }
    fn task(&mut self, task: &str, today: &str, interaction: bool) -> Result<()> {
        if !valid_day(today, false) {
            return Err("无法读取本机日期，成长进度未改变。".into());
        }
        if !self.enabled || today < self.day.as_str() {
            return Ok(());
        }
        if today > self.day.as_str() {
            self.day = today.into();
            self.completed_tasks.clear();
        }
        if interaction && today > self.last_interaction_day.as_str() {
            self.last_interaction_day = today.into();
            self.interaction_days = self.interaction_days.saturating_add(1).min(MAX_DAYS);
        }
        if !self.completed_tasks.iter().any(|id| id == task) {
            let reward = catalog()
                .tasks
                .iter()
                .find(|t| t.id == task)
                .ok_or("未知成长任务。")?;
            self.xp = self.xp.saturating_add(reward.xp).min(MAX_XP);
            self.completed_tasks.push(task.into());
            if task == "play" {
                self.play_days = self.play_days.saturating_add(1).min(MAX_DAYS);
            }
            if task == "greet" {
                insert(&mut self.achievements, "first-meeting");
            }
        }
        self.unlock();
        Ok(())
    }
    pub fn interact(&mut self, action: &str, today: &str) -> Result<()> {
        let action = catalog()
            .actions
            .iter()
            .find(|a| a.id == action)
            .ok_or("未知星伴动作。")?;
        if level(self.xp) < action.level {
            return Err(format!("这个动作将在 Lv.{} 解锁。", action.level));
        }
        self.task(&action.task, today, true)
    }
    pub fn complete_creation(&mut self, today: &str) -> Result<()> {
        self.task("create", today, false)
    }
    pub fn equip(&mut self, slot: &str, item_id: Option<&str>) -> Result<()> {
        if let Some(id) = item_id {
            if !self.unlocked_items.iter().any(|v| v == id)
                || !catalog()
                    .items
                    .iter()
                    .any(|item| item.id == id && item.slot == slot)
            {
                return Err("这件装饰尚未获得，或不能穿戴在这个位置。".into());
            }
        }
        *self.equipment.slot_mut(slot)? = item_id.map(str::to_owned);
        if self.enabled {
            if item_id.is_some() {
                insert(&mut self.achievements, "first-outfit");
            }
            self.unlock();
        }
        Ok(())
    }
}

pub enum GrowthChange<'a> {
    Enabled(bool),
    Interaction(&'a str),
    Equip(&'a str, Option<&'a str>),
    Creation,
    Import {
        data: &'a str,
        expected_revision: u64,
    },
    Reset {
        expected_revision: u64,
    },
}
impl GrowthChange<'_> {
    pub fn apply(self, state: &mut GrowthState, today: &str) -> Result<()> {
        match self {
            Self::Enabled(enabled) => state.enabled = enabled,
            Self::Interaction(action) => state.interact(action, today)?,
            Self::Equip(slot, id) => state.equip(slot, id)?,
            Self::Creation => state.complete_creation(today)?,
            Self::Import {
                data,
                expected_revision,
            } => {
                if state.revision != expected_revision {
                    return Err("成长进度已更新，请重新预览存档后再导入。".into());
                }
                let mut imported = decode_save(data)?;
                imported.revision = state.revision;
                *state = imported;
            }
            Self::Reset { expected_revision } => {
                if state.revision != expected_revision {
                    return Err("成长进度已更新，请重新确认后再重置。".into());
                }
                *state = GrowthState {
                    revision: state.revision,
                    ..GrowthState::default()
                };
            }
        }
        state.validate()
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SaveFile {
    format: String,
    version: u32,
    growth: GrowthState,
}
pub fn encode_save(state: GrowthState) -> Result<String> {
    state.validate()?;
    serde_json::to_string_pretty(&SaveFile {
        format: "starclip-starpet".into(),
        version: 1,
        growth: state,
    })
    .map_err(|_| "无法生成星伴成长存档。".into())
}
pub fn decode_save(data: &str) -> Result<GrowthState> {
    if data.len() > MAX_SAVE_BYTES {
        return Err("成长存档不能超过 64 KiB。".into());
    }
    let file: SaveFile =
        serde_json::from_str(data).map_err(|_| "成长存档格式无效或字段不完整。")?;
    if file.format != "starclip-starpet" || file.version != 1 {
        return Err("不支持的成长存档格式或版本。".into());
    }
    file.growth.validate()?;
    Ok(file.growth)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn daily_rewards_unlock_once_and_do_not_reward_offline_or_clock_rollback() {
        let mut s = GrowthState::default();
        s.interact("greet", "2026-10-26").unwrap();
        assert_eq!(s, GrowthState::default());
        s.enabled = true;
        s.interact("greet", "2026-10-26").unwrap();
        s.interact("highfive", "2026-10-26").unwrap();
        s.interact("stretch", "2026-10-26").unwrap();
        assert_eq!(
            (s.xp, level(s.xp), s.interaction_days, s.play_days),
            (45, 2, 1, 1)
        );
        assert_eq!(s.unlocked_items, ["welcome-star", "moon-glasses"]);
        s.enabled = false;
        s.equip("face", Some("moon-glasses")).unwrap();
        assert!(!s.achievements.iter().any(|id| id == "first-outfit"));
        s.enabled = true;
        s.equip("face", Some("moon-glasses")).unwrap();
        let snapshot = s.clone();
        for _ in 0..10 {
            s.interact("greet", "2026-10-26").unwrap();
            s.equip("face", Some("moon-glasses")).unwrap();
        }
        s.interact("greet", "2026-10-25").unwrap();
        assert_eq!(s, snapshot);
        assert!(s.interact("twirl", "2026-10-27").is_err());
        assert!(s.interact("create", "2026-10-27").is_err());
        assert!(s.equip("face", Some("night-cape")).is_err());
        s.complete_creation("2026-11-20").unwrap();
        assert_eq!((s.xp, s.interaction_days), (65, 1));
        s.interact("greet", "2026-11-20").unwrap();
        assert_eq!(
            (s.xp, s.interaction_days, s.completed_tasks.len()),
            (75, 2, 2)
        );
        for day in 21..=26 {
            s.interact("highfive", &format!("2026-11-{day}")).unwrap();
        }
        assert!(s.achievements.iter().any(|id| id == "seven-days"));
        assert!(s.achievements.iter().any(|id| id == "play-five"));
        s.validate().unwrap();
    }
    #[test]
    fn levels_and_import_validation_cover_limits_dates_and_equipment() {
        for (xp, l) in [
            (0, 1),
            (39, 1),
            (40, 2),
            (90, 3),
            (220, 5),
            (490, 8),
            (990, 12),
            (1650, 16),
            (2470, 20),
            (MAX_XP, 20),
        ] {
            assert_eq!(level(xp), l);
        }
        let baseline = encode_save(GrowthState::default()).unwrap();
        assert_eq!(decode_save(&baseline).unwrap(), GrowthState::default());
        assert!(decode_save(&" ".repeat(MAX_SAVE_BYTES + 1)).is_err());
        for (key, bad) in [
            ("xp", serde_json::json!(MAX_XP + 1)),
            ("version", serde_json::json!(2)),
            ("revision", serde_json::json!(MAX_REVISION + 1)),
            ("day", serde_json::json!("2026-02-29")),
            ("day", serde_json::json!("2026-13-01")),
            ("unlockedItems", serde_json::json!(["hello-kitty"])),
            (
                "achievements",
                serde_json::json!(["first-meeting", "first-meeting"]),
            ),
            (
                "equipment",
                serde_json::json!({"back":null,"face":"moon-glasses","hand":null,"charm":null}),
            ),
        ] {
            let mut value: serde_json::Value = serde_json::from_str(&baseline).unwrap();
            value["growth"][key] = bad;
            assert!(decode_save(&value.to_string()).is_err(), "{key}");
        }
        assert!(valid_day("2024-02-29", false));
        assert!(!valid_day("2100-02-29", false));
        assert!(!valid_day("２０２６-10-26", false));
        let mut s = GrowthState::default();
        s.revision = 2;
        assert!(GrowthChange::Import {
            data: &baseline,
            expected_revision: 1
        }
        .apply(&mut s, "2026-10-26")
        .is_err());
        assert_eq!(s.revision, 2);
    }
}
