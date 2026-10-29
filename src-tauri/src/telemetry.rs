//! Optional, bounded feature counters. No clipboard data or persistent identifier.
use crate::store::Result;
use crate::telemetry_device::DeviceInfo;
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use url::Url;

const CACHE_TTL: u64 = 72 * 60 * 60;
const MAX_CACHE_BYTES: u64 = 2048;
const RETRY_DELAYS: [u64; 4] = [60, 300, 900, 3600];

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

const EVENTS: [&str; 9] = [
    "app_started",
    "studio_opened",
    "card_exported",
    "backup_exported",
    "backup_imported",
    "update_check_completed",
    "update_download_completed",
    "update_install_started",
    "font_downloaded",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TelemetryPreferences {
    pub revision: u64,
    pub enabled: bool,
}

impl Default for TelemetryPreferences {
    fn default() -> Self {
        Self {
            revision: 0,
            enabled: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EventCount {
    name: &'static str,
    count: u8,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TelemetryState {
    pub revision: u64,
    pub enabled: bool,
    pub configured: bool,
    pub endpoint_host: Option<String>,
    pub queued_events: Vec<EventCount>,
    pub last_result: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Envelope {
    version: u8,
    session_id: String,
    app_version: &'static str,
    platform: &'static str,
    device: DeviceInfo,
    events: Vec<EventCount>,
}

pub struct Batch {
    endpoint: Url,
    envelope: Envelope,
    generation: u64,
    created_at: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CachedCounts {
    version: u8,
    app_version: String,
    created_at: u64,
    counts: [u8; EVENTS.len()],
}

struct InFlight {
    session_id: String,
    generation: u64,
    created_at: u64,
    counts: [u8; EVENTS.len()],
}

pub struct Telemetry {
    endpoint: Option<Url>,
    session_id: Option<String>,
    enabled: bool,
    revision: u64,
    counts: [u8; EVENTS.len()],
    in_flight: Option<InFlight>,
    last_result: &'static str,
    device: DeviceInfo,
    cache_path: Option<PathBuf>,
    created_at: Option<u64>,
    generation: u64,
    failures: usize,
    next_attempt_at: u64,
    dirty: bool,
}

fn endpoint(value: Option<&str>) -> Option<Url> {
    let url = Url::parse(value?).ok()?;
    (url.scheme() == "https"
        && url.host_str().is_some()
        && url.username().is_empty()
        && url.password().is_none()
        && url.path() == "/collect"
        && url.query().is_none()
        && url.fragment().is_none())
    .then_some(url)
}

fn session_id() -> Result<String> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| "无法生成临时统计会话，请重试。".to_string())?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

impl Telemetry {
    pub fn new(preferences: Result<TelemetryPreferences>) -> Self {
        let mut telemetry =
            Self::with_endpoint(preferences, option_env!("STARCLIP_TELEMETRY_ENDPOINT"));
        if telemetry.enabled {
            telemetry.device = DeviceInfo::detect();
        }
        telemetry
    }

    fn with_endpoint(preferences: Result<TelemetryPreferences>, value: Option<&str>) -> Self {
        // Only the missing-key path may choose the product default. Corrupt or
        // unreadable preferences must never accidentally turn collection on.
        let preferences = preferences.unwrap_or(TelemetryPreferences {
            revision: 0,
            enabled: false,
        });
        let endpoint = endpoint(value);
        let session_id = session_id().ok();
        let enabled = preferences.enabled && endpoint.is_some() && session_id.is_some();
        Self {
            last_result: if endpoint.is_some() {
                "never"
            } else {
                "unconfigured"
            },
            endpoint,
            session_id,
            enabled,
            revision: preferences.revision,
            counts: [0; EVENTS.len()],
            in_flight: None,
            device: DeviceInfo {
                architecture: "unknown".into(),
                os_major: "unknown".into(),
                model: "unknown".into(),
            },
            cache_path: None,
            created_at: None,
            generation: 0,
            failures: 0,
            next_attempt_at: 0,
            dirty: false,
        }
    }

    /// A tiny independent cache outside clipboard backups. No session/device IDs on disk.
    pub fn with_cache(mut self, path: PathBuf) -> Self {
        self.cache_path = Some(path.clone());
        let restored = (|| -> Option<CachedCounts> {
            if !self.enabled {
                return None;
            }
            let meta = fs::symlink_metadata(&path).ok()?;
            if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > MAX_CACHE_BYTES {
                return None;
            }
            let mut bytes = Vec::new();
            fs::File::open(&path)
                .ok()?
                .take(MAX_CACHE_BYTES + 1)
                .read_to_end(&mut bytes)
                .ok()?;
            if bytes.len() as u64 > MAX_CACHE_BYTES {
                return None;
            }
            let saved: CachedCounts = serde_json::from_slice(&bytes).ok()?;
            let age = now().checked_sub(saved.created_at)?;
            (saved.version == 1
                && saved.app_version == env!("CARGO_PKG_VERSION")
                && age < CACHE_TTL
                && saved.counts.iter().all(|&n| n <= 100))
            .then_some(saved)
        })();
        if let Some(saved) = restored {
            self.counts = saved.counts;
            self.created_at = Some(saved.created_at);
        } else {
            let _ = fs::remove_file(&path);
        }
        let _ = fs::remove_file(path.with_extension("tmp"));
        self
    }

    fn expire(&mut self, at: u64) {
        if self
            .created_at
            .is_some_and(|start| at < start || at - start >= CACHE_TTL)
        {
            self.counts = [0; EVENTS.len()];
            self.created_at = None;
            self.dirty = true;
            self.bump();
        }
    }

    /// Called by the existing minute timer and at normal exit, never by a UI network wait.
    pub fn checkpoint(&mut self) {
        self.expire(now());
        if !self.dirty {
            return;
        }
        let Some(path) = &self.cache_path else {
            return;
        };
        let mut counts = self.counts;
        let mut created_at = self.created_at;
        if let Some(flight) = &self.in_flight {
            if flight.generation == self.generation && self.enabled {
                for (count, previous) in counts.iter_mut().zip(flight.counts) {
                    *count = count.saturating_add(previous).min(100);
                }
                created_at =
                    Some(created_at.map_or(flight.created_at, |at| at.min(flight.created_at)));
            }
        }
        if !self.enabled || !counts.iter().any(|&n| n > 0) {
            let removed = [path.clone(), path.with_extension("tmp")]
                .iter()
                .all(|file| match fs::remove_file(file) {
                    Ok(()) => true,
                    Err(error) => error.kind() == std::io::ErrorKind::NotFound,
                });
            self.dirty = !removed;
            return;
        }
        let saved = CachedCounts {
            version: 1,
            app_version: env!("CARGO_PKG_VERSION").into(),
            created_at: created_at.unwrap_or_else(now),
            counts,
        };
        let temporary = path.with_extension("tmp");
        let written = (|| -> std::io::Result<()> {
            let bytes = serde_json::to_vec(&saved)?;
            let _ = fs::remove_file(&temporary);
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options.open(&temporary)?;
            file.write_all(&bytes)?;
            drop(file);
            fs::rename(&temporary, path)
        })();
        if written.is_ok() {
            self.dirty = false;
        }
        let _ = fs::remove_file(temporary);
    }

    fn bump(&mut self) {
        self.revision = self.revision.saturating_add(1);
    }

    pub fn state(&self) -> TelemetryState {
        TelemetryState {
            revision: self.revision,
            enabled: self.enabled,
            configured: self.endpoint.is_some(),
            endpoint_host: self
                .endpoint
                .as_ref()
                .and_then(|url| url.host_str().map(str::to_owned)),
            queued_events: EVENTS
                .iter()
                .zip(self.counts)
                .filter_map(|(&name, count)| (count > 0).then_some(EventCount { name, count }))
                .collect(),
            last_result: self.last_result,
        }
    }

    pub fn validate_enable(&mut self) -> Result<()> {
        if self.endpoint.is_none() {
            return Err("此版本尚未配置统计接收服务，无法开启上报。".into());
        }
        if self.session_id.is_none() {
            self.session_id = Some(session_id()?);
        }
        #[cfg(not(test))]
        if self.device.architecture == "unknown" {
            self.device = DeviceInfo::detect();
        }
        Ok(())
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled && self.endpoint.is_some() && self.session_id.is_some();
        if self.enabled {
            self.last_result = "never";
        } else {
            self.clear();
            // Forget the old identity even if the OS cannot generate its replacement.
            self.session_id = session_id().ok();
            self.last_result = if self.endpoint.is_some() {
                "disabled"
            } else {
                "unconfigured"
            };
        }
        self.bump();
    }

    pub fn record(&mut self, name: &str) -> Result<()> {
        let index = EVENTS
            .iter()
            .position(|event| *event == name)
            .ok_or("不支持此统计事件。")?;
        self.expire(now());
        if self.enabled && self.counts[index] < 100 {
            self.counts[index] += 1;
            self.created_at.get_or_insert_with(now);
            self.dirty = true;
            self.bump();
        }
        Ok(())
    }

    pub fn record_frontend(&mut self, name: &str) -> Result<()> {
        if name != "studio_opened" {
            return Err("此事件只能由应用完成操作后记录。".into());
        }
        self.record(name)
    }

    pub fn clear(&mut self) {
        self.counts = [0; EVENTS.len()];
        self.created_at = None;
        self.generation = self.generation.wrapping_add(1);
        self.failures = 0;
        self.next_attempt_at = 0;
        self.dirty = true;
        self.checkpoint();
        self.bump();
    }

    pub fn clear_queue(&mut self) -> Result<()> {
        self.clear();
        if self.dirty && self.cache_path.is_some() {
            Err("本次待发统计已清空，但本地缓存未能删除；请恢复目录写入权限后重试，重启可能恢复旧计数。".into())
        } else {
            Ok(())
        }
    }

    pub fn ready(&self) -> bool {
        self.enabled
            && self.in_flight.is_none()
            && now() >= self.next_attempt_at
            && self.counts.iter().any(|count| *count > 0)
    }

    // Taking a batch is the request start boundary. Revocation clears everything
    // not yet started; an in-flight HTTP request cannot be recalled.
    pub fn take_batch(&mut self) -> Option<Batch> {
        self.checkpoint();
        if !self.ready() {
            return None;
        }
        let session_id = self.session_id.clone()?;
        let batch = Batch {
            endpoint: self.endpoint.clone()?,
            envelope: Envelope {
                version: 2,
                session_id: session_id.clone(),
                app_version: env!("CARGO_PKG_VERSION"),
                platform: std::env::consts::OS,
                device: self.device.clone(),
                events: self.state().queued_events,
            },
            generation: self.generation,
            created_at: self.created_at.unwrap_or_else(now),
        };
        self.in_flight = Some(InFlight {
            session_id,
            generation: self.generation,
            created_at: batch.created_at,
            counts: self.counts,
        });
        self.counts = [0; EVENTS.len()];
        self.created_at = None;
        self.bump();
        Some(batch)
    }

    pub fn finish(&mut self, batch: &Batch, success: bool) {
        if !self.in_flight.as_ref().is_some_and(|f| {
            f.session_id == batch.envelope.session_id && f.generation == batch.generation
        }) {
            return;
        }
        let flight = self.in_flight.take().unwrap();
        if self.enabled
            && self.generation == batch.generation
            && self.session_id.as_ref() == Some(&batch.envelope.session_id)
        {
            self.last_result = if success { "sent" } else { "failed" };
            if success {
                self.failures = 0;
                self.next_attempt_at = 0;
            } else {
                let at = now();
                if at >= flight.created_at && at - flight.created_at < CACHE_TTL {
                    for (count, previous) in self.counts.iter_mut().zip(flight.counts) {
                        *count = count.saturating_add(previous).min(100);
                    }
                    self.created_at = Some(
                        self.created_at
                            .map_or(flight.created_at, |time| time.min(flight.created_at)),
                    );
                }
                self.next_attempt_at =
                    at.saturating_add(RETRY_DELAYS[self.failures.min(RETRY_DELAYS.len() - 1)]);
                self.failures = (self.failures + 1).min(RETRY_DELAYS.len());
            }
            self.dirty = true;
            self.checkpoint();
        }
        self.bump();
    }
}

pub fn send(batch: &Batch) -> bool {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let Ok(client) = reqwest::blocking::Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(10))
        .build()
    else {
        return false;
    };
    let Ok(body) = serde_json::to_vec(&batch.envelope) else {
        return false;
    };
    client
        .post(batch.endpoint.clone())
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(body)
        .send()
        .is_ok_and(|response| response.status() == reqwest::StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn configured() -> Telemetry {
        Telemetry::with_endpoint(
            Ok(TelemetryPreferences::default()),
            Some("https://stats.example/collect"),
        )
    }

    #[test]
    fn defaults_and_endpoint_validation_prevent_unconfigured_collection() {
        for value in [
            None,
            Some("http://stats.example/collect"),
            Some("https://user:secret@stats.example/collect"),
            Some("https://stats.example/collect?secret=x"),
            Some("https://stats.example/collect#x"),
            Some("https://stats.example/other"),
        ] {
            let mut telemetry = Telemetry::with_endpoint(
                Ok(TelemetryPreferences {
                    revision: 1,
                    enabled: true,
                }),
                value,
            );
            assert!(!telemetry.state().configured);
            assert!(!telemetry.state().enabled);
            assert!(telemetry.validate_enable().is_err());
            telemetry.record("app_started").unwrap();
            assert!(telemetry.take_batch().is_none());
            assert!(telemetry.state().queued_events.is_empty());
        }
    }

    #[test]
    fn configured_default_is_enabled_but_saved_opt_out_and_read_errors_stay_disabled() {
        let mut telemetry = configured();
        assert!(telemetry.state().enabled);
        telemetry.record("app_started").unwrap();
        assert_eq!(
            telemetry.state().queued_events,
            vec![EventCount {
                name: "app_started",
                count: 1
            }]
        );

        for preferences in [
            Ok(TelemetryPreferences {
                revision: 4,
                enabled: false,
            }),
            Err("fixed preference read error".into()),
        ] {
            let mut telemetry =
                Telemetry::with_endpoint(preferences, Some("https://stats.example/collect"));
            assert!(telemetry.state().configured);
            assert!(!telemetry.state().enabled);
            telemetry.record("update_download_completed").unwrap();
            telemetry.record("update_install_started").unwrap();
            assert!(telemetry.state().queued_events.is_empty());
            assert!(telemetry.take_batch().is_none());
        }
    }

    #[test]
    fn only_bounded_allowlisted_counts_enter_the_payload_and_retries_back_off() {
        let mut telemetry = configured();
        telemetry.validate_enable().unwrap();
        telemetry.set_enabled(true);
        assert!(telemetry.record_frontend("card_exported").is_err());
        assert!(telemetry
            .record_frontend("update_download_completed")
            .is_err());
        assert!(telemetry.record_frontend("update_install_started").is_err());
        assert!(telemetry.record("clipboard_secret").is_err());
        for _ in 0..110 {
            telemetry.record_frontend("studio_opened").unwrap();
        }
        for event in EVENTS {
            telemetry.record(event).unwrap();
        }
        let batch = telemetry.take_batch().unwrap();
        let payload = serde_json::to_value(&batch.envelope).unwrap();
        assert_eq!(payload.as_object().unwrap().len(), 6);
        assert_eq!(payload["device"].as_object().unwrap().len(), 3);
        assert_eq!(batch.envelope.events.len(), 9);
        assert_eq!(batch.envelope.events[1].count, 100);
        assert_eq!(batch.envelope.session_id.len(), 32);
        assert!(batch
            .envelope
            .session_id
            .chars()
            .all(|c| c.is_ascii_hexdigit()));
        assert!(telemetry.take_batch().is_none());
        telemetry.finish(&batch, false);
        assert_eq!(telemetry.state().last_result, "failed");
        assert_eq!(telemetry.state().queued_events.len(), 9);
        assert!(telemetry.take_batch().is_none());
        for delay in [300, 900, 3600, 3600] {
            telemetry.next_attempt_at = 0;
            let retry = telemetry.take_batch().unwrap();
            telemetry.record("studio_opened").unwrap();
            telemetry.finish(&retry, false);
            assert!((now() + delay - 1..=now() + delay).contains(&telemetry.next_attempt_at));
            assert_eq!(telemetry.counts[1], 100);
        }
        telemetry.next_attempt_at = 0;
        let retry = telemetry.take_batch().unwrap();
        telemetry.finish(&retry, true);
        assert!(telemetry.state().queued_events.is_empty());
        assert_eq!(telemetry.failures, 0);
    }

    #[test]
    fn disable_clears_pending_rotates_identity_and_stale_completion_cannot_reenable() {
        let mut telemetry = configured();
        telemetry.set_enabled(true);
        telemetry.record("app_started").unwrap();
        let batch = telemetry.take_batch().unwrap();
        telemetry.record("card_exported").unwrap();
        telemetry.set_enabled(false);
        assert_ne!(
            telemetry.session_id.as_ref(),
            Some(&batch.envelope.session_id)
        );
        assert!(telemetry.state().queued_events.is_empty());
        assert!(!telemetry.ready());
        telemetry.finish(&batch, true);
        assert!(!telemetry.state().enabled);
        assert_eq!(telemetry.state().last_result, "disabled");
        telemetry.set_enabled(true);
        assert_eq!(telemetry.state().last_result, "never");
        telemetry.record("studio_opened").unwrap();
        telemetry.clear();
        assert!(telemetry.take_batch().is_none());
    }

    struct CacheFixture(PathBuf);
    impl CacheFixture {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("starclip-counts-{}", session_id().unwrap()));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn path(&self) -> PathBuf {
            self.0.join("usage-counts.json")
        }
    }
    impl Drop for CacheFixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn offline_cache_restores_counts_without_identity_and_success_removes_it() {
        let fixture = CacheFixture::new();
        let mut telemetry = configured().with_cache(fixture.path());
        telemetry.record("studio_opened").unwrap();
        let batch = telemetry.take_batch().unwrap();
        telemetry.record("card_exported").unwrap();
        telemetry.finish(&batch, false);
        let bytes = fs::read(fixture.path()).unwrap();
        assert!(bytes.len() < MAX_CACHE_BYTES as usize);
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value.as_object().unwrap().len(), 4);
        assert!(value.get("sessionId").is_none());
        assert!(value.get("device").is_none());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(fixture.path()).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        let mut restored = configured().with_cache(fixture.path());
        assert_ne!(restored.session_id, telemetry.session_id);
        assert_eq!(
            restored.state().queued_events,
            telemetry.state().queued_events
        );
        let batch = restored.take_batch().unwrap();
        restored.finish(&batch, true);
        assert!(!fixture.path().exists());
        assert!(restored.state().queued_events.is_empty());
    }

    #[test]
    fn cache_rejects_corruption_expiry_foreign_version_and_unexpected_fields() {
        let fixture = CacheFixture::new();
        let valid = serde_json::json!({
            "version": 1, "appVersion": env!("CARGO_PKG_VERSION"),
            "createdAt": now(), "counts": [1,0,0,0,0,0,0,0,0]
        });
        for (key, value) in [
            ("createdAt", serde_json::json!(now() - CACHE_TTL)),
            ("createdAt", serde_json::json!(now() + 100)),
            ("appVersion", serde_json::json!("0.0.0-fixture")),
            ("version", serde_json::json!(2)),
            ("counts", serde_json::json!([101, 0, 0, 0, 0, 0, 0, 0, 0])),
            ("sessionId", serde_json::json!("fixture-not-allowed")),
        ] {
            let mut invalid = valid.clone();
            invalid[key] = value;
            fs::write(fixture.path(), serde_json::to_vec(&invalid).unwrap()).unwrap();
            let restored = configured().with_cache(fixture.path());
            assert!(restored.state().queued_events.is_empty(), "{key}");
            assert!(!fixture.path().exists());
        }
        for bytes in [
            b"not-json".to_vec(),
            vec![b' '; MAX_CACHE_BYTES as usize + 1],
        ] {
            fs::write(fixture.path(), bytes).unwrap();
            assert!(configured()
                .with_cache(fixture.path())
                .state()
                .queued_events
                .is_empty());
            assert!(!fixture.path().exists());
        }
    }

    #[test]
    fn clearing_and_disabling_forget_cached_and_in_flight_counts() {
        let fixture = CacheFixture::new();
        for disable in [false, true] {
            let mut telemetry = configured().with_cache(fixture.path());
            telemetry.record("app_started").unwrap();
            let batch = telemetry.take_batch().unwrap();
            assert!(fixture.path().exists());
            if disable {
                telemetry.set_enabled(false);
            } else {
                telemetry.clear_queue().unwrap();
            }
            assert!(!fixture.path().exists());
            telemetry.finish(&batch, false);
            assert!(!fixture.path().exists());
            assert!(telemetry.state().queued_events.is_empty());
        }
        let mut telemetry = configured().with_cache(fixture.path());
        telemetry.record("studio_opened").unwrap();
        telemetry.checkpoint();
        let restored = Telemetry::with_endpoint(
            Ok(TelemetryPreferences {
                revision: 1,
                enabled: false,
            }),
            Some("https://stats.example/collect"),
        )
        .with_cache(fixture.path());
        assert!(restored.state().queued_events.is_empty());
        assert!(!fixture.path().exists());
    }

    #[test]
    fn cache_write_failures_stay_bounded_and_failed_deletions_remain_retryable() {
        let fixture = CacheFixture::new();
        let path = fixture.0.join("missing").join("usage-counts.json");
        let mut telemetry = configured().with_cache(path);
        for _ in 0..200 {
            telemetry.record("app_started").unwrap();
        }
        telemetry.checkpoint();
        assert!(telemetry.dirty);
        assert_eq!(telemetry.counts[0], 100);
        assert!(
            telemetry.take_batch().is_some(),
            "cache failure cannot block use or network delivery"
        );

        let mut telemetry = configured().with_cache(fixture.path());
        fs::create_dir(fixture.path()).unwrap();
        assert!(telemetry.clear_queue().is_err());
        assert!(telemetry.dirty);
        fs::remove_dir(fixture.path()).unwrap();
        telemetry.checkpoint();
        assert!(!telemetry.dirty);
        telemetry.record("app_started").unwrap();
        telemetry.created_at = Some(now() - CACHE_TTL);
        telemetry.checkpoint();
        assert!(!fixture.path().exists());
        assert!(telemetry.take_batch().is_none());
    }

    #[cfg(unix)]
    #[test]
    fn cache_symlinks_are_not_read_or_followed() {
        let fixture = CacheFixture::new();
        let target = fixture.0.join("original-fixture.txt");
        fs::write(&target, b"original fixed fixture").unwrap();
        std::os::unix::fs::symlink(&target, fixture.path()).unwrap();
        let mut telemetry = configured().with_cache(fixture.path());
        assert!(telemetry.state().queued_events.is_empty());
        telemetry.record("app_started").unwrap();
        telemetry.checkpoint();
        assert_eq!(fs::read(target).unwrap(), b"original fixed fixture");
        assert!(!fs::symlink_metadata(fixture.path())
            .unwrap()
            .file_type()
            .is_symlink());
    }

    #[test]
    #[ignore = "Explicit live relay check; requires STARCLIP_TELEMETRY_TEST_ENDPOINT and sends synthetic counts"]
    fn live_relay_accepts_original_fixed_counts() {
        let endpoint = std::env::var("STARCLIP_TELEMETRY_TEST_ENDPOINT")
            .expect("Set STARCLIP_TELEMETRY_TEST_ENDPOINT to the test relay HTTPS /collect URL");
        let mut telemetry =
            Telemetry::with_endpoint(Ok(TelemetryPreferences::default()), Some(&endpoint));
        telemetry.validate_enable().unwrap();
        assert!(telemetry.state().enabled, "configured default must be on");
        for event in EVENTS {
            telemetry.record(event).unwrap();
        }
        let mut batch = telemetry.take_batch().unwrap();
        batch.envelope.device = DeviceInfo {
            architecture: "unknown".into(),
            os_major: "unknown".into(),
            model: match std::env::consts::OS {
                "macos" => "Mac",
                "windows" => "WindowsPC",
                _ => "LinuxPC",
            }
            .into(),
        };
        // Only this test substitutes a clearly synthetic version. Production
        // batching and the real HTTPS sender remain unchanged; no store is opened.
        batch.envelope.app_version = "0.0.0-telemetry-verification";
        assert_eq!(batch.envelope.events.len(), 9);
        assert!(batch.envelope.events.iter().all(|event| event.count == 1));
        let accepted = send(&batch);
        telemetry.finish(&batch, accepted);
        assert!(accepted, "relay did not return HTTP 204; no retry is made");
        assert_eq!(telemetry.state().last_result, "sent");
        assert!(telemetry.take_batch().is_none());

        telemetry.record("studio_opened").unwrap();
        telemetry.set_enabled(false);
        for event in EVENTS {
            telemetry.record(event).unwrap();
        }
        assert!(!telemetry.state().enabled);
        assert!(telemetry.state().queued_events.is_empty());
        assert!(telemetry.take_batch().is_none());
        // HTTP acceptance does not prove ingestion into either provider's reports.
    }
}
