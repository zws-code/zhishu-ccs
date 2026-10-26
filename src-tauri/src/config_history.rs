//! 设备本地的配置写入历史、内部写入指纹及安全还原。
//! 每次写入保存独立的私密 JSON，避免底层原子写入器依赖数据库及其锁。
//! 文件快照、供应商同步报告和成员操作摘要分别记录，只有合格的文件快照可用于还原。

use crate::live::engine::{digest, lock_app, read_current, AppWriteGuard, DeviceStore, LiveFile};
use crate::live::patch::{Guarded, WholeFile};
use crate::mode::{
    operation::{self, FileChange},
    state::{self, PendingTarget},
};
use std::cell::RefCell;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use tauri::{AppHandle, Emitter};

use chrono::Local;
use serde::{Deserialize, Serialize};

use crate::config::{get_app_config_dir, get_device_data_dir, stage_write};
use crate::error::AppError;

const HISTORY_DIR: &str = "config-change-history";
const EVENTS_DIR: &str = "events";
const INTERNAL_WRITES_DIR: &str = "internal-writes";
const MAX_EVENTS: usize = 300;
const MAX_SNAPSHOT_BYTES: usize = 1024 * 1024;
const REDACTED: &str = "[已隐藏]";
const SENSITIVE_MARKERS: &[&str] = &[
    "api_key",
    "apikey",
    "access_token",
    "refresh_token",
    "id_token",
    "auth_token",
    "authorization",
    "password",
    "secret",
    "credential",
    "bearer_token",
];

static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();

pub fn init(handle: AppHandle) {
    let _ = APP_HANDLE.set(handle);
}

fn notify_updated(app: &str) {
    if let Some(handle) = APP_HANDLE.get() {
        if let Err(error) = handle.emit("config-change-history-updated", app) {
            log::warn!("通知配置记录更新失败: {error}");
        }
    }
}

static EVENT_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone)]
struct AuditContext {
    app: String,
    action: String,
    /// One user-facing operation may write several files; every record written
    /// under this context carries the same id so the history list can group
    /// them into a single entry. Nested same-app contexts inherit the outer id.
    operation_id: String,
    /// User-facing operation name (e.g. adding a provider) that differs from
    /// the low-level write action (e.g. `switch`). Records keep the mechanism
    /// action for the internal-write hash chain; only display prefers this.
    /// A nested same-app context never overrides the outer label.
    label: Option<String>,
    /// Pointer movement of a direct-mode switch, set via
    /// [`set_provider_pointer`] once the pre-switch provider is known.
    /// Inherited by nested same-app contexts like the label.
    pointer: Option<SwitchPointer>,
}

struct AuditContextGuard(Option<AuditContext>);

impl Drop for AuditContextGuard {
    fn drop(&mut self) {
        AUDIT_CONTEXT.with(|slot| {
            slot.replace(self.0.take());
        });
    }
}

thread_local! {
    static AUDIT_CONTEXT: RefCell<Option<AuditContext>> = const { RefCell::new(None) };
}

/// 为闭包内的文件写入附加操作上下文；同应用嵌套复用操作 ID，离开作用域后恢复原上下文。
pub(crate) fn with_audit_context<T>(
    app: impl Into<String>,
    action: impl Into<String>,
    work: impl FnOnce() -> T,
) -> T {
    with_audit_context_labeled(app, action, None, work)
}

/// Run a user-facing operation (provider add/remove) whose file writes are
/// mechanically switches, applies, or native rewrites. The label is
/// display-only: records keep their mechanism action.
pub(crate) fn with_operation_action<T>(
    app: impl Into<String>,
    label: impl Into<String>,
    work: impl FnOnce() -> T,
) -> T {
    let label = label.into();
    with_audit_context_labeled(app, label.clone(), Some(label), work)
}

fn with_audit_context_labeled<T>(
    app: impl Into<String>,
    action: impl Into<String>,
    label: Option<String>,
    work: impl FnOnce() -> T,
) -> T {
    let app = app.into();
    let previous = AUDIT_CONTEXT.with(|slot| {
        let inherited = slot
            .borrow()
            .as_ref()
            .filter(|context| context.app == app)
            .map(|context| {
                (
                    context.operation_id.clone(),
                    context.label.clone(),
                    context.pointer.clone(),
                )
            });
        let (operation_id, label, pointer) = match inherited {
            // A nested same-app context belongs to the same operation: it
            // reuses the outer id and never overrides the outer label or
            // pointer stamp.
            Some((operation_id, label, pointer)) => (operation_id, label, pointer),
            None => (event_id(), label, None),
        };
        slot.replace(Some(AuditContext {
            app,
            action: action.into(),
            operation_id,
            label,
            pointer,
        }))
    });
    let _guard = AuditContextGuard(previous);
    work()
}

/// Record the direct-provider pointer movement of the switch currently running
/// under an audit context, so a group restore can move the app window's
/// current selection back together with the files. No-op without a matching
/// context (e.g. recovery replay at startup).
pub(crate) fn set_provider_pointer(app: &str, from: Option<&str>, to: &str) {
    AUDIT_CONTEXT.with(|slot| {
        let mut slot = slot.borrow_mut();
        if let Some(context) = slot.as_mut() {
            if context.app == app {
                context.pointer = Some(SwitchPointer {
                    from: from.map(str::to_owned),
                    to: to.to_owned(),
                });
            }
        }
    });
}

/// Auxiliary native files are not restorable editor snapshots. Persist only
/// their hashes as a write audit; the file itself keeps no readable content.
#[derive(Serialize, Deserialize)]
struct InternalFileChange {
    id: String,
    app: String,
    path: PathBuf,
    action: String,
    before_hash: Option<String>,
    after_hash: Option<String>,
}

fn internal_writes_store() -> HistoryStore {
    HistoryStore {
        events: get_device_data_dir()
            .join(HISTORY_DIR)
            .join(INTERNAL_WRITES_DIR),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSyncReport {
    pub added: usize,
    pub updated: usize,
    pub removed: usize,
    pub unchanged: usize,
    pub changed_files: Vec<String>,
    pub backup_path: Option<String>,
    pub warnings: Vec<String>,
}

/// Roster-style summary of an operation whose visible effect is not a
/// restorable file diff, e.g. adding or removing an aggregation member: the
/// written files may be derived caches (Codex model catalog) or may not change
/// at all. Written as a standalone record like a platform sync report.
///
/// 两个字段始终序列化：前端按必有数组消费（`summary.removed.length`），省略
/// 空数组会让它读到 `undefined` 而崩溃；`default` 只用于读取旧记录。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigOperationSummary {
    #[serde(default)]
    pub added: Vec<String>,
    #[serde(default)]
    pub removed: Vec<String>,
}

/// Direct-provider pointer movement caused by one switch write, so a group
/// restore can move the app window's current selection back with the files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchPointer {
    /// Provider that was current before the switch; `None` if none was.
    pub from: Option<String>,
    pub to: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigChangeRecord {
    pub id: String,
    pub app: String,
    pub path: String,
    pub changed_at: String,
    pub action: String,
    pub before: Option<String>,
    pub after: Option<String>,
    pub sensitive: bool,
    /// Records of one operation share this id; `None` for legacy records and
    /// sync summaries, which each stand alone in the grouped list.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<String>,
    /// Display name of the user-facing operation (e.g. provider add/remove).
    /// `None` keeps showing the mechanism `action`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_action: Option<String>,
    /// Set on records of a direct-mode provider switch; group restore moves
    /// the current-provider pointer back to `from` when it still points to
    /// `to`. Proxy-mode route switches keep the pointer and carry no stamp.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub switch_pointer: Option<SwitchPointer>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sync: Option<ProviderSyncReport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<ConfigOperationSummary>,
}

/// One file of a grouped history entry. The id addresses the underlying
/// per-file record for detail, diff, and restore.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigChangeFileSummary {
    pub id: String,
    pub path: String,
    pub before_exists: bool,
    pub after_exists: bool,
    pub sensitive: bool,
}

/// One user-facing operation: all records written under a single audit
/// context are collapsed into one entry. `id` is the newest record's id;
/// legacy records without an operation id each stand alone.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigChangeGroup {
    pub id: String,
    pub app: String,
    pub path: String,
    pub changed_at: String,
    pub action: String,
    pub sensitive: bool,
    pub files: Vec<ConfigChangeFileSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sync: Option<ProviderSyncReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<ConfigOperationSummary>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigChangeDetail {
    pub id: String,
    pub app: String,
    pub path: String,
    pub changed_at: String,
    pub action: String,
    pub before: Option<String>,
    pub after: Option<String>,
    pub sensitive: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sync: Option<ProviderSyncReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<ConfigOperationSummary>,
}

impl ConfigChangeDetail {
    fn from_record(record: ConfigChangeRecord, reveal_sensitive: bool) -> Self {
        let mask = !reveal_sensitive && record.sensitive;
        Self {
            id: record.id,
            app: record.app,
            path: record.path,
            changed_at: record.changed_at,
            action: record.operation_action.unwrap_or(record.action),
            before: record
                .before
                .map(|text| if mask { redact_sensitive(&text) } else { text }),
            after: record
                .after
                .map(|text| if mask { redact_sensitive(&text) } else { text }),
            sensitive: record.sensitive,
            sync: record.sync,
            summary: record.summary,
        }
    }
}

#[derive(Debug, Clone)]
struct HistoryStore {
    events: PathBuf,
}

impl HistoryStore {
    fn for_device() -> Self {
        Self::at(get_device_data_dir())
    }

    fn at(device_dir: impl AsRef<Path>) -> Self {
        Self {
            events: device_dir.as_ref().join(HISTORY_DIR).join(EVENTS_DIR),
        }
    }

    fn write(&self, record: &ConfigChangeRecord) -> Result<(), AppError> {
        fs::create_dir_all(&self.events).map_err(|error| AppError::io(&self.events, error))?;
        set_private_dir_permissions(&self.events);
        if let Some(parent) = self.events.parent() {
            set_private_dir_permissions(parent);
        }
        let path = self.events.join(format!("{}.json", record.id));
        let bytes = serde_json::to_vec_pretty(record)
            .map_err(|error| AppError::Config(format!("配置变更记录序列化失败: {error}")))?;
        stage_write(&path, &bytes, Some(0o600), true)?.commit()?;
        self.prune();
        Ok(())
    }

    fn list(&self, app: Option<&str>) -> Result<Vec<ConfigChangeRecord>, AppError> {
        let mut records = self.read_all()?;
        records.retain(|record| app.is_none_or(|expected| record.app == expected));
        records.sort_by(|left, right| right.changed_at.cmp(&left.changed_at));
        Ok(records)
    }

    fn get(&self, id: &str) -> Result<Option<ConfigChangeRecord>, AppError> {
        if !is_safe_id(id) {
            return Ok(None);
        }
        let path = self.events.join(format!("{id}.json"));
        match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map(Some)
                .map_err(|error| AppError::Config(format!("配置变更记录损坏: {error}"))),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(AppError::io(&path, error)),
        }
    }

    fn clear(&self) -> Result<usize, AppError> {
        if !self.events.exists() {
            return Ok(0);
        }
        let mut removed = 0usize;
        let entries =
            fs::read_dir(&self.events).map_err(|error| AppError::io(&self.events, error))?;
        for entry in entries {
            let entry = entry.map_err(|error| AppError::io(&self.events, error))?;
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            fs::remove_file(&path).map_err(|error| AppError::io(&path, error))?;
            removed += 1;
        }
        Ok(removed)
    }

    fn read_all(&self) -> Result<Vec<ConfigChangeRecord>, AppError> {
        if !self.events.exists() {
            return Ok(Vec::new());
        }
        let entries =
            fs::read_dir(&self.events).map_err(|error| AppError::io(&self.events, error))?;
        let mut records = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|error| AppError::io(&self.events, error))?;
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            match fs::read(&path).and_then(|bytes| {
                serde_json::from_slice(&bytes)
                    .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
            }) {
                Ok(record) => records.push(record),
                Err(error) => log::warn!("跳过损坏的配置变更记录 {}: {}", path.display(), error),
            }
        }
        Ok(records)
    }

    fn prune(&self) {
        let Ok(entries) = fs::read_dir(&self.events) else {
            return;
        };
        let mut paths: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("json"))
            .collect();
        if paths.len() <= MAX_EVENTS {
            return;
        }
        paths.sort();
        let remove_count = paths.len() - MAX_EVENTS;
        for path in paths.into_iter().take(remove_count) {
            if let Err(error) = fs::remove_file(&path) {
                log::warn!("清理旧配置变更记录失败 {}: {}", path.display(), error);
            }
        }
    }
}

pub(crate) struct ConfigChangeCapture {
    app: String,
    path: PathBuf,
    action: String,
    operation_id: Option<String>,
    operation_action: Option<String>,
    switch_pointer: Option<SwitchPointer>,
    before: Option<Vec<u8>>,
    hashes_only: bool,
}

/// 写入前捕获受监测文件；未受监测路径直接返回，避免普通状态、数据库或缓存写入额外读盘。
pub(crate) fn capture_before(path: &Path) -> Option<ConfigChangeCapture> {
    let tracked = app_for_path(path);
    let hashes_only = tracked.is_none();
    let app = tracked.or_else(|| auxiliary_app_for_path(path))?;
    Some(ConfigChangeCapture {
        action: current_action(&app),
        operation_id: current_operation_id(&app),
        operation_action: current_operation_label(&app),
        switch_pointer: current_switch_pointer(&app),
        app,
        path: path.to_path_buf(),
        before: fs::read(path).ok(),
        hashes_only,
    })
}

impl ConfigChangeCapture {
    /// Audit failures never fail the primary write; they are logged and the
    /// client change remains valid.
    pub(crate) fn finish(self, after: Option<Vec<u8>>) {
        if self.before == after {
            return;
        }
        if self.hashes_only {
            let record = InternalFileChange {
                id: event_id(),
                app: self.app,
                path: self.path,
                action: self.action,
                before_hash: digest(self.before.as_deref()),
                after_hash: digest(after.as_deref()),
            };
            let store = internal_writes_store();
            let save = || -> Result<(), AppError> {
                fs::create_dir_all(&store.events)
                    .map_err(|error| AppError::io(&store.events, error))?;
                set_private_dir_permissions(&store.events);
                let bytes = serde_json::to_vec(&record)
                    .map_err(|_| AppError::Config("配置写入指纹序列化失败".into()))?;
                stage_write(
                    &store.events.join(format!("{}.json", record.id)),
                    &bytes,
                    Some(0o600),
                    true,
                )?
                .commit()?;
                store.prune();
                Ok(())
            };
            if let Err(error) = save() {
                log::warn!("保存配置写入指纹失败: {error}");
            }
            return;
        }
        if self
            .before
            .as_ref()
            .is_some_and(|bytes| bytes.len() > MAX_SNAPSHOT_BYTES)
            || after
                .as_ref()
                .is_some_and(|bytes| bytes.len() > MAX_SNAPSHOT_BYTES)
        {
            log::debug!(
                "跳过过大的配置变更快照 {} (before={}, after={})",
                self.path.display(),
                self.before.as_ref().map_or(0, Vec::len),
                after.as_ref().map_or(0, Vec::len)
            );
            return;
        }

        let decode = |bytes: Option<Vec<u8>>| bytes.map(String::from_utf8).transpose();
        let (Ok(before_text), Ok(after_text)) = (decode(self.before), decode(after)) else {
            log::warn!("跳过非 UTF-8 配置变更快照 {}", self.path.display());
            return;
        };
        let sensitive = before_text.as_deref().is_some_and(looks_sensitive)
            || after_text.as_deref().is_some_and(looks_sensitive);
        let record = ConfigChangeRecord {
            id: event_id(),
            app: self.app,
            path: self.path.to_string_lossy().into_owned(),
            changed_at: Local::now().to_rfc3339(),
            action: self.action,
            operation_id: self.operation_id,
            operation_action: self.operation_action,
            switch_pointer: self.switch_pointer,
            before: before_text,
            after: after_text,
            sensitive,
            sync: None,
            summary: None,
        };
        if let Err(error) = HistoryStore::for_device().write(&record) {
            log::warn!("写入配置变更记录失败 {}: {}", record.path, error);
        } else {
            notify_updated(&record.app);
        }
    }
}

pub fn list(app: Option<String>) -> Result<Vec<ConfigChangeGroup>, AppError> {
    Ok(group_records(
        HistoryStore::for_device().list(app.as_deref())?,
    ))
}

fn group_records(records: Vec<ConfigChangeRecord>) -> Vec<ConfigChangeGroup> {
    let mut groups: Vec<ConfigChangeGroup> = Vec::new();
    let mut group_of_operation: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for record in records {
        let file = ConfigChangeFileSummary {
            id: record.id.clone(),
            path: record.path.clone(),
            before_exists: record.before.is_some(),
            after_exists: record.after.is_some(),
            sensitive: record.sensitive,
        };
        // Records are newest-first; the first one seen for an operation is the
        // group head. Records without an operation id never share a group.
        let existing = record
            .operation_id
            .as_deref()
            .and_then(|id| group_of_operation.get(id).copied());
        match existing {
            Some(index) => {
                groups[index].files.push(file);
                groups[index].sensitive |= record.sensitive;
            }
            None => {
                if let Some(id) = record.operation_id.as_deref() {
                    group_of_operation.insert(id.to_string(), groups.len());
                }
                groups.push(ConfigChangeGroup {
                    id: record.id.clone(),
                    app: record.app.clone(),
                    path: record.path.clone(),
                    changed_at: record.changed_at.clone(),
                    action: record
                        .operation_action
                        .clone()
                        .unwrap_or_else(|| record.action.clone()),
                    sensitive: record.sensitive,
                    files: vec![file],
                    sync: record.sync.clone(),
                    summary: record.summary.clone(),
                });
            }
        }
    }
    for group in &mut groups {
        // Show files in write order within the operation.
        group.files.sort_by(|left, right| left.id.cmp(&right.id));
    }
    groups
}

pub fn detail(id: String, reveal_sensitive: bool) -> Result<Option<ConfigChangeDetail>, AppError> {
    Ok(HistoryStore::for_device()
        .get(&id)?
        .map(|record| ConfigChangeDetail::from_record(record, reveal_sensitive)))
}

pub fn clear() -> Result<usize, AppError> {
    let removed = HistoryStore::for_device().clear()?;
    internal_writes_store().clear()?;
    notify_updated("");
    Ok(removed)
}

/// 独立记录供应商同步的数据库与文件变化；兼容旧报告的备份路径，不伪造文件差异。
pub(crate) fn record_provider_sync(app: &str, report: ProviderSyncReport) -> Result<(), AppError> {
    let record = ConfigChangeRecord {
        id: event_id(),
        app: app.into(),
        path: report.backup_path.clone().unwrap_or_else(|| {
            DeviceStore::for_device()
                .file("platform-sync-history.json")
                .to_string_lossy()
                .into_owned()
        }),
        changed_at: Local::now().to_rfc3339(),
        action: "platform_sync".into(),
        before: None,
        after: None,
        sensitive: false,
        operation_id: None,
        operation_action: None,
        switch_pointer: None,
        sync: Some(report),
        summary: None,
    };
    HistoryStore::for_device().write(&record)?;
    notify_updated(app);
    Ok(())
}

/// A roster-style operation summary (e.g. aggregation membership) as a
/// standalone record: it has no restorable before/after content, and it is
/// excluded from the internal-write chain used to explain external edits.
/// `path` names the primary file the operation may have touched, for display.
pub(crate) fn record_operation_summary(
    app: &str,
    action: &str,
    path: &Path,
    summary: ConfigOperationSummary,
) -> Result<(), AppError> {
    let record = ConfigChangeRecord {
        id: event_id(),
        app: app.into(),
        path: path.to_string_lossy().into_owned(),
        changed_at: Local::now().to_rfc3339(),
        action: action.into(),
        before: None,
        after: None,
        sensitive: false,
        operation_id: None,
        operation_action: None,
        switch_pointer: None,
        sync: None,
        summary: Some(summary),
    };
    HistoryStore::for_device().write(&record)?;
    notify_updated(app);
    Ok(())
}

pub(crate) fn sync_backup_location(id: &str) -> Result<PathBuf, AppError> {
    let record = HistoryStore::for_device()
        .get(id)?
        .ok_or_else(|| AppError::Message("同步记录不存在".into()))?;
    let path = record
        .sync
        .and_then(|report| report.backup_path)
        .ok_or_else(|| AppError::Message("本次同步没有备份目录".into()))?;
    validated_sync_backup_dir(&path, &sync_backup_root())
        .ok_or_else(|| AppError::Message("同步备份路径无效或已经不存在".into()))
}

fn sync_backup_root() -> PathBuf {
    DeviceStore::for_device()
        .file("backups")
        .join("platform-sync")
}

/// Only a direct child of the platform-sync backup root with a manifest is a
/// real sync backup; anything else must not be read as one.
fn validated_sync_backup_dir(path: &str, root: &Path) -> Option<PathBuf> {
    let path = fs::canonicalize(path).ok()?;
    let root = fs::canonicalize(root).ok()?;
    if path.parent() != Some(root.as_path()) || !path.join("manifest.json").is_file() {
        return None;
    }
    Some(path)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigRestoreResult {
    pub changed: bool,
    pub backup_path: Option<String>,
}

/// One restored file of a group restore.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigRestoreFile {
    pub id: String,
    pub path: String,
    pub changed: bool,
    pub backup_path: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigRestoreSummary {
    pub files: Vec<ConfigRestoreFile>,
    pub changed: bool,
    /// The app window's current-provider selection was moved back to what it
    /// was before the restored switch; only set for direct-mode switches whose
    /// pointer still pointed at the switched-to provider.
    pub pointer_restored: bool,
}

/// Validated restore plan for one operation group: the records to restore
/// (newest first, one per path), plus the switch pointer to move back.
pub(crate) struct RestorePlan {
    pub app: String,
    pub records: Vec<ConfigChangeRecord>,
    pub pointer: Option<SwitchPointer>,
}

pub(crate) fn restore_plan(ids: &[String]) -> Result<RestorePlan, AppError> {
    restore_plan_at(&get_device_data_dir(), ids)
}

fn restore_record_at(device_dir: &Path, id: &str) -> Result<ConfigChangeRecord, AppError> {
    let record = HistoryStore::at(device_dir).get(id)?.ok_or_else(|| {
        AppError::localized(
            "configHistory.missing",
            "这条配置修改记录已经不存在",
            "This configuration change no longer exists",
        )
    })?;
    ensure_restorable(&record)?;
    validate_restore_path(&record)?;
    Ok(record)
}

/// Load and validate every record of the group. All records must belong to
/// one app; when several records touch the same path only the newest is
/// restorable, so older ones are dropped instead of failing later.
fn restore_plan_at(device_dir: &Path, ids: &[String]) -> Result<RestorePlan, AppError> {
    if ids.is_empty() {
        return Err(AppError::Message("没有选择要还原的记录".into()));
    }
    let mut records = Vec::with_capacity(ids.len());
    for id in ids {
        records.push(restore_record_at(device_dir, id)?);
    }
    plan_records(records)
}

/// Pure planning over already-validated records: one app per restore, newest
/// record first, only the newest record per path, and the group's switch
/// pointer to move back.
fn plan_records(records: Vec<ConfigChangeRecord>) -> Result<RestorePlan, AppError> {
    let mut records = records;
    let app = records[0].app.clone();
    if records.iter().any(|record| record.app != app) {
        return Err(AppError::Message("一次只能还原同一个应用的一次修改".into()));
    }
    records.sort_by(|left, right| right.id.cmp(&left.id));
    let mut newest_per_path: Vec<ConfigChangeRecord> = Vec::new();
    let mut seen_paths: HashSet<String> = HashSet::new();
    for record in records {
        if seen_paths.insert(record.path.clone()) {
            newest_per_path.push(record);
        }
    }
    let pointer = newest_per_path
        .iter()
        .rev()
        .find_map(|record| record.switch_pointer.clone());
    Ok(RestorePlan {
        app,
        records: newest_per_path,
        pointer,
    })
}

/// The caller holds the platform apply and provider switch locks, in that
/// order. Restore every file of the operation, newest first, under one audit
/// context so the restore itself becomes a single grouped history entry.
/// Files already restored stay restored when a later one refuses (e.g. newer
/// external changes); the error reports the failing file.
pub(crate) fn restore_records(
    plan: &RestorePlan,
    store: &DeviceStore,
) -> Result<Vec<ConfigRestoreFile>, AppError> {
    let guard = lock_app(&plan.app);
    let mut files = Vec::with_capacity(plan.records.len());
    with_audit_context(&plan.app, "restore", || {
        for record in &plan.records {
            let result = restore_file(record, store, &guard)
                .map_err(|error| AppError::Message(format!("{}：{error}", record.path)))?;
            files.push(ConfigRestoreFile {
                id: record.id.clone(),
                path: record.path.clone(),
                changed: result.changed,
                backup_path: result.backup_path,
            });
        }
        Ok::<(), AppError>(())
    })?;
    Ok(files)
}

fn ensure_restorable(record: &ConfigChangeRecord) -> Result<(), AppError> {
    if record.sync.is_some() {
        return Err(AppError::Message(
            "同步操作记录不能作为单个配置文件还原，请查看同步备份".into(),
        ));
    }
    if record.summary.is_some() {
        return Err(AppError::Message(
            "这条记录是操作摘要，没有可还原的文件内容".into(),
        ));
    }
    Ok(())
}

fn validate_restore_path(record: &ConfigChangeRecord) -> Result<(), AppError> {
    let path = Path::new(&record.path);
    if !path.is_absolute()
        || path
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir))
        || app_for_path(path).as_deref() != Some(&record.app)
    {
        return Err(AppError::localized(
            "configHistory.pathChanged",
            "配置路径已更改或不再属于该应用，无法还原这条记录",
            "The configuration path has changed or no longer belongs to this app",
        ));
    }
    Ok(())
}

/// The caller holds the platform apply and provider switch locks, in that
/// order. Restores only this file: provider rows and proxy state are not
/// historical snapshots, and mode/pointer safety is checked per file.
fn restore_file(
    record: &ConfigChangeRecord,
    store: &DeviceStore,
    guard: &AppWriteGuard,
) -> Result<ConfigRestoreResult, AppError> {
    if state::pending(store, &record.app)?.is_some() {
        return Err(AppError::localized(
            "configHistory.pending",
            "上一次配置写入尚未完成，请恢复后重试",
            "A previous configuration write is unfinished; recover it and retry",
        ));
    }
    if state::mode_state(store, &record.app)?.is_proxy() {
        return Err(AppError::localized(
            "configHistory.proxyActive",
            "请先将此应用切换到直连模式，再还原配置文件",
            "Switch this app to direct mode before restoring a configuration file",
        ));
    }
    let path = Path::new(&record.path);
    if fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        return Err(AppError::localized(
            "configHistory.symlink",
            "配置文件是符号链接，请先确认配置路径后再还原",
            "The configuration file is a symbolic link; check its path before restoring",
        ));
    }
    let current = read_current(path)?;
    let before = record.before.as_deref().map(str::as_bytes);
    let after = record.after.as_deref().map(str::as_bytes);
    if current.as_deref() == before {
        return Ok(ConfigRestoreResult {
            changed: false,
            backup_path: None,
        });
    }
    if current.as_deref() != after {
        return Err(AppError::localized("configHistory.conflict", "文件在这条记录之后已有其他修改，为避免覆盖，已停止还原。请从最新记录开始还原", "This file has newer changes. Restore was stopped to preserve them; start with the latest change"));
    }
    // History has no historical proxy contract/state. Never reinstall one while
    // the app's mode metadata says direct; use the existing mode switch instead.
    if matches!(
        record.app.as_str(),
        "claude" | "claude-desktop" | "codex" | "gemini" | "grokbuild"
    ) && (matches!(record.action.as_str(), "exit" | "detach")
        || record.before.as_deref().is_some_and(|text| {
            text.contains(crate::live::project::claude::PROXY_TOKEN_PLACEHOLDER)
        }))
    {
        return Err(AppError::localized("configHistory.proxySnapshot", "这条记录涉及路由模式，请使用应用页面的模式开关恢复路由，避免配置与模式状态不一致", "This change involves proxy mode. Use the app's mode switch to restore routing so configuration and mode state stay consistent"));
    }
    if let Some(text) = &record.before {
        validate_snapshot(path, text)?;
    }

    // 还原前必须保存独立备份；它不随历史清空或条数裁剪而删除，备份失败则不触碰原生文件。
    let backup_dir = store.file(HISTORY_DIR).join("restore-backups");
    fs::create_dir_all(&backup_dir).map_err(|error| AppError::io(&backup_dir, error))?;
    set_private_dir_permissions(&backup_dir);
    if let Some(parent) = backup_dir.parent() {
        set_private_dir_permissions(parent);
    }
    let backup_path = backup_dir.join(format!("{}.json", event_id()));
    let backup = ConfigChangeRecord {
        id: record.id.clone(),
        app: record.app.clone(),
        path: record.path.clone(),
        changed_at: Local::now().to_rfc3339(),
        action: "restore".into(),
        before: record.after.clone(),
        after: record.before.clone(),
        sensitive: record.sensitive,
        operation_id: None,
        operation_action: None,
        switch_pointer: None,
        sync: None,
        summary: None,
    };
    let bytes = serde_json::to_vec_pretty(&backup)
        .map_err(|_| AppError::Config("配置还原备份序列化失败".into()))?;
    stage_write(&backup_path, &bytes, Some(0o600), true)?.commit()?;
    let patch = Guarded {
        expected_pre: digest(after),
        then: record.before.as_ref().map_or(WholeFile::Delete, |text| {
            WholeFile::Write(text.as_bytes().to_vec())
        }),
    };
    // Reuse atomic publication, last-moment conflict checks, first-write backups,
    // and pending recovery. An empty target leaves provider/mode metadata alone.
    with_audit_context(&record.app, "restore", || {
        operation::run(
            store,
            guard,
            "restore",
            &[FileChange {
                file: LiveFile::private(path),
                patch: &patch,
            }],
            PendingTarget::default(),
            &|_| Ok(()),
        )
    })?;
    Ok(ConfigRestoreResult {
        changed: true,
        backup_path: Some(backup_path.to_string_lossy().into_owned()),
    })
}

fn validate_snapshot(path: &Path, text: &str) -> Result<(), AppError> {
    let valid = match path.extension().and_then(|value| value.to_str()) {
        Some("json" | "jsonc") => json5::from_str::<serde_json::Value>(text).is_ok(),
        Some("toml") => text.parse::<toml::Value>().is_ok(),
        Some("yaml" | "yml") => serde_yaml::from_str::<serde_yaml::Value>(text).is_ok(),
        _ => true,
    };
    if valid {
        Ok(())
    } else {
        Err(AppError::localized(
            "configHistory.invalidSnapshot",
            "修改前的快照无法解析，已停止还原",
            "The previous snapshot cannot be parsed; restore was stopped",
        ))
    }
}

fn current_action(app: &str) -> String {
    AUDIT_CONTEXT.with(|slot| {
        slot.borrow()
            .as_ref()
            .filter(|context| context.app == app)
            .map_or_else(
                || "config_update".to_string(),
                |context| context.action.clone(),
            )
    })
}

fn current_operation_id(app: &str) -> Option<String> {
    AUDIT_CONTEXT.with(|slot| {
        slot.borrow()
            .as_ref()
            .filter(|context| context.app == app)
            .map(|context| context.operation_id.clone())
    })
}

fn current_operation_label(app: &str) -> Option<String> {
    AUDIT_CONTEXT.with(|slot| {
        slot.borrow()
            .as_ref()
            .filter(|context| context.app == app)
            .and_then(|context| context.label.clone())
    })
}

fn current_switch_pointer(app: &str) -> Option<SwitchPointer> {
    AUDIT_CONTEXT.with(|slot| {
        slot.borrow()
            .as_ref()
            .filter(|context| context.app == app)
            .and_then(|context| context.pointer.clone())
    })
}

fn event_id() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let counter = EVENT_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{nanos:020}-{:x}-{counter:08x}", std::process::id())
}

fn is_safe_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 96
        && id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || byte.is_ascii_hexdigit() || byte == b'-')
}

#[cfg(unix)]
fn set_private_dir_permissions(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o700));
}

#[cfg(not(unix))]
fn set_private_dir_permissions(_path: &Path) {}

fn app_for_path(path: &Path) -> Option<String> {
    // zhishu-ccs' own settings/database/history files live in the same device
    // directory on some platforms. Returning early also avoids taking the
    // settings read lock while `mutate_settings` holds its write lock.
    if normalize_path(path).starts_with(normalize_path(&get_app_config_dir()))
        || normalize_path(path).starts_with(normalize_path(&get_device_data_dir()))
    {
        return None;
    }

    if same_file(path, &crate::config::get_claude_settings_path())
        || same_file(path, &crate::config::get_claude_mcp_path())
    {
        return Some("claude".into());
    }
    if crate::claude_desktop_config::live_file_paths()
        .ok()
        .is_some_and(|paths| paths.iter().any(|candidate| same_file(path, candidate)))
    {
        return Some("claude-desktop".into());
    }

    let rules: [(&str, PathBuf, &[&str]); 12] = [
        (
            "codex",
            crate::codex_config::get_codex_config_dir(),
            &["config.toml", "auth.json"],
        ),
        (
            "gemini",
            crate::gemini_config::get_gemini_dir(),
            &["settings.json", ".env"],
        ),
        (
            "grokbuild",
            crate::grok_config::get_grok_config_dir(),
            &["config.toml"],
        ),
        (
            "opencode",
            crate::opencode_config::get_opencode_dir(),
            &["opencode.json", "opencode.jsonc", ".env"],
        ),
        (
            "openclaw",
            crate::openclaw_config::get_openclaw_dir(),
            &["openclaw.json", "env.json", "tools.json", "agents.json"],
        ),
        (
            "hermes",
            crate::hermes_config::get_hermes_dir(),
            &["config.yaml"],
        ),
        (
            "pi",
            crate::pi_config::get_pi_agent_dir()
                .ok()
                .unwrap_or_default(),
            &["models.json", "settings.json", "auth.json"],
        ),
        (
            "deepseek-harness",
            crate::deepseek_harness_config::get_dsh_home(),
            &[".credentials.yaml", "settings.yaml", "cordis.patch.yml"],
        ),
        (
            "workbuddy",
            crate::workbuddy_config::get_workbuddy_home(),
            &["models.json"],
        ),
        ("mcode", crate::mcode_config::data_dir(), &["config.yaml"]),
        ("kimi", crate::kimi_config::data_dir(), &["config.toml"]),
        (
            "agy",
            crate::agy_config::agy_dir(),
            &["settings.json", "auth.json", ".env"],
        ),
    ];

    rules.into_iter().find_map(|(app, root, files)| {
        let allowed: HashSet<&str> = files.iter().copied().collect();
        file_is_within_root_and_allowed(path, &root, &allowed).then(|| app.to_string())
    })
}

fn auxiliary_app_for_path(path: &Path) -> Option<String> {
    // Check device paths before reading application settings: settings writes
    // hold their own write lock, and audit callbacks must never reenter it.
    if same_file(
        path,
        &crate::codex_config::get_codex_managed_oauth_live_auth_marker_path(),
    ) || same_file(
        path,
        &DeviceStore::for_device().file("codex-login-stash.json"),
    ) {
        return Some("codex".into());
    }
    if normalize_path(path).starts_with(normalize_path(&get_app_config_dir()))
        || normalize_path(path).starts_with(normalize_path(&get_device_data_dir()))
    {
        return None;
    }
    same_file(path, &crate::codex_config::get_codex_model_catalog_path()).then(|| "codex".into())
}

fn same_file(left: &Path, right: &Path) -> bool {
    normalize_path(left) == normalize_path(right)
}

fn file_is_within_root_and_allowed(path: &Path, root: &Path, allowed: &HashSet<&str>) -> bool {
    if root.as_os_str().is_empty() || !normalize_path(path).starts_with(normalize_path(root)) {
        return false;
    }
    path.file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|name| allowed.contains(name))
}

fn normalize_path(path: &Path) -> PathBuf {
    path.components().collect()
}

fn looks_sensitive(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    SENSITIVE_MARKERS
        .iter()
        .any(|marker| lower.contains(marker))
}

fn redact_sensitive(text: &str) -> String {
    text.lines()
        .map(|line| {
            let lower = line.to_ascii_lowercase();
            if !SENSITIVE_MARKERS
                .iter()
                .any(|marker| lower.contains(marker))
            {
                return line.to_string();
            }

            if let Some(separator) = line.find(['=', ':']) {
                let (prefix, suffix) = line.split_at(separator + 1);
                if suffix.trim().is_empty() {
                    return line.to_string();
                }
                return format!("{prefix} {REDACTED}");
            }
            REDACTED.to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(id: &str, changed_at: &str, before: &str, after: &str) -> ConfigChangeRecord {
        ConfigChangeRecord {
            id: id.into(),
            app: "codex".into(),
            path: "/tmp/config.toml".into(),
            changed_at: changed_at.into(),
            action: "switch".into(),
            before: Some(before.into()),
            after: Some(after.into()),
            sensitive: false,
            operation_id: None,
            operation_action: None,
            switch_pointer: None,
            sync: None,
            summary: None,
        }
    }

    #[test]
    fn nested_audit_contexts_share_one_operation_id() {
        let (outer_id, inner_id, other_app_id) = with_audit_context("codex", "switch", || {
            let outer = current_operation_id("codex").unwrap();
            let inner =
                with_audit_context("codex", "apply", || current_operation_id("codex").unwrap());
            let other_app = with_audit_context("claude", "switch", || {
                current_operation_id("claude").unwrap()
            });
            (outer, inner, other_app)
        });
        assert_eq!(outer_id, inner_id, "same-app nesting is one operation");
        assert_ne!(outer_id, other_app_id, "another app is another operation");
        assert_eq!(current_operation_id("codex"), None);
    }

    #[test]
    fn operation_label_is_inherited_by_nested_contexts() {
        let (label, mechanism, nested_label) =
            with_operation_action("codex", "add_provider", || {
                let label = current_operation_label("codex").unwrap();
                let (mechanism, nested_label) = with_audit_context("codex", "switch", || {
                    (
                        current_action("codex"),
                        current_operation_label("codex").unwrap(),
                    )
                });
                (label, mechanism, nested_label)
            });
        assert_eq!(label, "add_provider");
        assert_eq!(mechanism, "switch", "hash-chain action stays the mechanism");
        assert_eq!(
            nested_label, "add_provider",
            "nested contexts inherit the label"
        );
        assert_eq!(current_operation_label("codex"), None);

        // An outer context without a label is never overridden by a nested
        // operation label: platform sync keeps its own display.
        let inherited = with_audit_context("codex", "platform_sync", || {
            with_operation_action("codex", "add_provider", || current_operation_label("codex"))
        });
        assert_eq!(inherited, None);
    }

    #[test]
    fn grouped_display_prefers_the_operation_label() {
        let mut head = record("0002", "2026-10-25T10:00:01+08:00", "c", "d");
        head.action = "switch".into();
        head.operation_id = Some("op-add".into());
        head.operation_action = Some("add_provider".into());
        let mut tail = record("0001", "2026-10-25T10:00:00+08:00", "a", "b");
        tail.path = "/tmp/auth.json".into();
        tail.operation_id = Some("op-add".into());
        tail.operation_action = Some("add_provider".into());

        let groups = group_records(vec![head.clone(), tail]);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].action, "add_provider");
        assert_eq!(groups[0].files.len(), 2);

        // Detail shows the user operation too; unlabeled records keep `action`.
        let detail = ConfigChangeDetail::from_record(head, false);
        assert_eq!(detail.action, "add_provider");
        let plain = ConfigChangeDetail::from_record(record("0003", "1", "a", "b"), false);
        assert_eq!(plain.action, "switch");
    }

    #[test]
    fn list_groups_records_of_one_operation_into_a_single_entry() {
        let dir = tempfile::tempdir().unwrap();
        let store = HistoryStore::at(dir.path());
        let mut older = record("0001", "2026-10-25T10:00:00+08:00", "a", "b");
        older.path = "/tmp/auth.json".into();
        older.operation_id = Some("op-1".into());
        let mut newer = record("0002", "2026-10-25T10:00:01+08:00", "c", "d");
        newer.path = "/tmp/config.toml".into();
        newer.sensitive = true;
        newer.operation_id = Some("op-1".into());
        // Legacy and standalone records never merge with anything.
        let standalone = record("0003", "2026-10-25T11:00:00+08:00", "e", "f");
        store.write(&standalone).unwrap();
        store.write(&older).unwrap();
        store.write(&newer).unwrap();

        let groups = group_records(store.list(None).unwrap());
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].id, "0003", "newest standalone entry first");
        let grouped = &groups[1];
        assert_eq!(grouped.id, "0002");
        assert_eq!(grouped.action, "switch");
        assert!(grouped.sensitive, "any sensitive file marks the group");
        assert_eq!(
            grouped
                .files
                .iter()
                .map(|file| (file.id.as_str(), file.path.as_str()))
                .collect::<Vec<_>>(),
            vec![("0001", "/tmp/auth.json"), ("0002", "/tmp/config.toml")],
            "files follow write order"
        );
    }

    fn summary_record(id: &str, app: &str, path: &Path) -> ConfigChangeRecord {
        ConfigChangeRecord {
            id: id.into(),
            app: app.into(),
            path: path.to_string_lossy().into_owned(),
            changed_at: "2026-10-25T12:00:00+08:00".into(),
            action: "stack".into(),
            before: None,
            after: None,
            sensitive: false,
            operation_id: None,
            operation_action: None,
            switch_pointer: None,
            sync: None,
            summary: Some(ConfigOperationSummary {
                added: vec!["DeepSeek".into()],
                removed: Vec::new(),
            }),
        }
    }

    #[test]
    fn operation_summary_groups_standalone_and_is_not_restorable() {
        let dir = tempfile::tempdir().unwrap();
        let store = HistoryStore::at(dir.path());
        let path = Path::new("/isolated/cc-switch-model-catalog.json");
        store.write(&summary_record("0001", "codex", path)).unwrap();
        store
            .write(&record("0002", "2026-10-25T13:00:00+08:00", "a", "b"))
            .unwrap();

        let groups = group_records(store.list(None).unwrap());
        assert_eq!(
            groups.len(),
            2,
            "summary records never merge with file records"
        );
        let summary = &groups[1];
        assert_eq!(summary.action, "stack");
        assert_eq!(
            summary.summary.as_ref().unwrap().added,
            vec!["DeepSeek".to_string()]
        );
        assert!(!summary.files[0].before_exists && !summary.files[0].after_exists);

        let detail = ConfigChangeDetail::from_record(store.get("0001").unwrap().unwrap(), false);
        assert_eq!(
            detail.summary.as_ref().unwrap().added,
            vec!["DeepSeek".to_string()]
        );
        assert!(ensure_restorable(&store.get("0001").unwrap().unwrap()).is_err());
        assert!(ensure_restorable(&store.get("0002").unwrap().unwrap()).is_ok());
    }

    /// 前端 `ConfigOperationSummary` 声明两个数组必有；空的一侧若被省略，
    /// `summary.removed.length` 这类读取会直接崩溃（见错误边界日志）。
    #[test]
    fn operation_summary_serializes_empty_sides() {
        let json = serde_json::to_value(ConfigOperationSummary {
            added: vec!["DeepSeek".into()],
            removed: Vec::new(),
        })
        .unwrap();
        assert_eq!(
            json,
            serde_json::json!({ "added": ["DeepSeek"], "removed": [] })
        );

        let json = serde_json::to_value(ConfigOperationSummary {
            added: Vec::new(),
            removed: Vec::new(),
        })
        .unwrap();
        assert_eq!(json, serde_json::json!({ "added": [], "removed": [] }));

        // 旧记录缺字段时仍按空数组读取。
        let parsed: ConfigOperationSummary =
            serde_json::from_value(serde_json::json!({ "removed": ["DeepSeek"] })).unwrap();
        assert_eq!(parsed.added, Vec::<String>::new());
        assert_eq!(parsed.removed, vec!["DeepSeek".to_string()]);
    }

    #[test]
    fn store_round_trips_and_filters_by_app() {
        let dir = tempfile::tempdir().unwrap();
        let store = HistoryStore::at(dir.path());
        store
            .write(&record("0001", "2026-10-23T10:00:00+08:00", "a", "b"))
            .unwrap();
        let records = store.list(Some("codex")).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].after.as_deref(), Some("b"));
        assert!(store.list(Some("claude")).unwrap().is_empty());
    }

    #[test]
    fn detail_masks_sensitive_values_until_revealed() {
        let dir = tempfile::tempdir().unwrap();
        let store = HistoryStore::at(dir.path());
        let mut item = record(
            "0001",
            "2026-10-23T10:00:00+08:00",
            "OPENAI_API_KEY=old\nmodel = \"a\"",
            "OPENAI_API_KEY=new\nmodel = \"b\"",
        );
        item.sensitive = true;
        store.write(&item).unwrap();

        let masked = ConfigChangeDetail::from_record(store.get("0001").unwrap().unwrap(), false);
        assert!(masked.before.unwrap().contains(REDACTED));
        let shown = ConfigChangeDetail::from_record(store.get("0001").unwrap().unwrap(), true);
        assert!(shown.after.unwrap().contains("new"));
    }

    #[test]
    fn ids_cannot_escape_the_event_directory() {
        assert!(is_safe_id("123-abc-00000001"));
        assert!(!is_safe_id("../secret"));
        assert!(!is_safe_id("a/b"));
    }
    fn restore_fixture(
        before: Option<&str>,
        after: Option<&str>,
    ) -> (tempfile::TempDir, DeviceStore, ConfigChangeRecord) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        if let Some(text) = after {
            fs::write(&path, text).unwrap();
        }
        let store = DeviceStore::at(dir.path().join("device"));
        let item = ConfigChangeRecord {
            id: "0001".into(),
            app: format!(
                "history-{}",
                dir.path().file_name().unwrap().to_string_lossy()
            ),
            path: path.to_string_lossy().into_owned(),
            changed_at: "2026-10-25T12:00:00+08:00".into(),
            action: "switch".into(),
            before: before.map(str::to_owned),
            after: after.map(str::to_owned),
            sensitive: false,
            operation_id: None,
            operation_action: None,
            switch_pointer: None,
            sync: None,
            summary: None,
        };
        (dir, store, item)
    }

    struct ResetFailpoints;
    impl Drop for ResetFailpoints {
        fn drop(&mut self) {
            operation::failpoint::crash_at(None);
            operation::failpoint::on_before_publish(None);
        }
    }

    #[test]
    fn plan_records_keeps_the_newest_record_per_path_and_the_switch_pointer() {
        let mut old_config = record("0001", "2026-10-25T10:00:00+08:00", "a", "b");
        old_config.path = "/tmp/config.toml".into();
        old_config.operation_id = Some("op-1".into());
        let mut new_config = record("0002", "2026-10-25T10:00:01+08:00", "b", "c");
        new_config.path = "/tmp/config.toml".into();
        new_config.operation_id = Some("op-1".into());
        new_config.switch_pointer = Some(SwitchPointer {
            from: Some("provider-a".into()),
            to: "provider-b".into(),
        });
        let mut auth = record("0003", "2026-10-25T10:00:02+08:00", "d", "e");
        auth.path = "/tmp/auth.json".into();
        auth.operation_id = Some("op-1".into());

        // Same path twice: only the newest record stays restorable.
        let plan = plan_records(vec![old_config, auth.clone(), new_config]).unwrap();
        assert_eq!(plan.app, "codex");
        assert_eq!(plan.records.len(), 2);
        assert_eq!(plan.records[0].id, "0003", "records restore newest first");
        assert_eq!(plan.records[1].id, "0002");
        assert_eq!(
            plan.pointer,
            Some(SwitchPointer {
                from: Some("provider-a".into()),
                to: "provider-b".into(),
            })
        );

        let mut foreign = record("0004", "2026-10-25T11:00:00+08:00", "f", "g");
        foreign.app = "claude".into();
        foreign.path = "/tmp/settings.json".into();
        let mixed = plan_records(vec![auth, foreign]);
        assert!(mixed.is_err(), "one restore cannot span two apps");
    }

    #[test]
    fn restore_records_restores_every_file_of_one_operation() {
        let dir = tempfile::tempdir().unwrap();
        let app = format!(
            "history-{}",
            dir.path().file_name().unwrap().to_string_lossy()
        );
        let first_path = dir.path().join("config.toml");
        let second_path = dir.path().join("settings.toml");
        fs::write(&first_path, "model='new'\n").unwrap();
        fs::write(&second_path, "token = 'new'\n").unwrap();
        let store = DeviceStore::at(dir.path().join("device"));
        let item = |id: &str, path: &Path, before: &str, after: &str| ConfigChangeRecord {
            id: id.into(),
            app: app.clone(),
            path: path.to_string_lossy().into_owned(),
            changed_at: "2026-10-25T12:00:00+08:00".into(),
            action: "switch".into(),
            before: Some(before.into()),
            after: Some(after.into()),
            sensitive: false,
            operation_id: Some("op-1".into()),
            operation_action: None,
            switch_pointer: None,
            sync: None,
            summary: None,
        };
        let plan = RestorePlan {
            app: app.clone(),
            records: vec![
                item("0002", &second_path, "token = 'old'\n", "token = 'new'\n"),
                item("0001", &first_path, "model='old'\n", "model='new'\n"),
            ],
            pointer: None,
        };
        let files = restore_records(&plan, &store).unwrap();
        assert_eq!(files.len(), 2);
        assert!(files.iter().all(|file| file.changed));
        assert!(files.iter().all(|file| file.backup_path.is_some()));
        assert_eq!(
            fs::read_to_string(&first_path).unwrap(),
            "model='old'\n",
            "every file of the operation is restored"
        );
        assert_eq!(fs::read_to_string(&second_path).unwrap(), "token = 'old'\n");
        // A file with newer external changes stops the restore after the
        // files that already succeeded stay restored.
        fs::write(&second_path, "token='external'\n").unwrap();
        let failed = restore_records(&plan, &store);
        assert!(failed.is_err());
        assert_eq!(fs::read_to_string(&first_path).unwrap(), "model='old'\n");
        assert_eq!(
            fs::read_to_string(&second_path).unwrap(),
            "token='external'\n"
        );
    }

    #[test]
    fn switch_pointer_stamp_is_inherited_by_nested_contexts() {
        let pointer = with_audit_context("codex", "switch", || {
            set_provider_pointer("codex", Some("provider-a"), "provider-b");
            let root = current_switch_pointer("codex").unwrap();
            let nested = with_audit_context("codex", "apply", || {
                current_switch_pointer("codex").unwrap()
            });
            assert_eq!(root, nested);
            // Another app keeps its own (empty) stamp.
            assert!(with_audit_context("claude", "switch", || {
                current_switch_pointer("claude")
            })
            .is_none());
            root
        });
        assert_eq!(pointer.from.as_deref(), Some("provider-a"));
        assert_eq!(pointer.to, "provider-b");
        assert!(current_switch_pointer("codex").is_none());
        // Without a context the stamp is a no-op.
        set_provider_pointer("codex", Some("x"), "y");
        assert!(current_switch_pointer("codex").is_none());
    }

    #[test]
    fn restore_preserves_exact_bytes_and_writes_a_private_independent_backup() {
        let before = "# user comment\r\nmodel = \"old\"\r\nunknown = true\r\n";
        let after = "# user comment\r\nmodel = \"new\"\r\nunknown = true\r\n";
        let (_dir, store, item) = restore_fixture(Some(before), Some(after));
        let guard = lock_app(&item.app);
        let result = restore_file(&item, &store, &guard).unwrap();
        assert!(result.changed);
        assert_eq!(fs::read(&item.path).unwrap(), before.as_bytes());
        let backup_path = PathBuf::from(result.backup_path.unwrap());
        let backup: ConfigChangeRecord =
            serde_json::from_slice(&fs::read(&backup_path).unwrap()).unwrap();
        assert_eq!(backup.before.as_deref(), Some(after));
        assert_eq!(backup.after.as_deref(), Some(before));
        let history = HistoryStore::at(store.file(HISTORY_DIR).parent().unwrap());
        history.write(&item).unwrap();
        assert_eq!(history.clear().unwrap(), 1);
        assert!(
            backup_path.exists(),
            "history clearing must not remove restore backups"
        );
        assert!(store.first_write_backup_dir().exists());
        assert!(state::pending(&store, &item.app).unwrap().is_none());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(backup_path).unwrap().permissions().mode() & 0o777,
                0o600
            );
            assert_eq!(
                fs::metadata(&item.path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }

    #[test]
    fn restore_refuses_newer_changes_and_is_idempotent() {
        let (_dir, store, item) = restore_fixture(Some("model='old'"), Some("model='new'"));
        let guard = lock_app(&item.app);
        fs::write(&item.path, "model='external'").unwrap();
        assert!(restore_file(&item, &store, &guard).is_err());
        assert_eq!(fs::read_to_string(&item.path).unwrap(), "model='external'");
        assert!(!store.file(HISTORY_DIR).exists());
        fs::write(&item.path, item.before.as_deref().unwrap()).unwrap();
        assert!(!restore_file(&item, &store, &guard).unwrap().changed);
    }

    #[test]
    fn restore_handles_creation_and_deletion() {
        let (_dir, store, item) = restore_fixture(None, Some("model='new'"));
        let guard = lock_app(&item.app);
        assert!(restore_file(&item, &store, &guard).unwrap().changed);
        assert!(!Path::new(&item.path).exists());
        let (_dir, store, item) = restore_fixture(Some("model='old'"), None);
        let guard = lock_app(&item.app);
        assert!(restore_file(&item, &store, &guard).unwrap().changed);
        assert_eq!(fs::read_to_string(&item.path).unwrap(), "model='old'");
    }

    #[test]
    fn restore_aborts_on_invalid_snapshot_or_backup_failure() {
        let (_dir, store, mut item) = restore_fixture(Some("[broken"), Some("model='new'"));
        let guard = lock_app(&item.app);
        assert!(restore_file(&item, &store, &guard).is_err());
        assert_eq!(fs::read_to_string(&item.path).unwrap(), "model='new'");
        item.before = Some("model='old'".into());
        fs::create_dir_all(store.file(HISTORY_DIR).parent().unwrap()).unwrap();
        fs::write(store.file(HISTORY_DIR), "not a directory").unwrap();
        assert!(restore_file(&item, &store, &guard).is_err());
        assert_eq!(fs::read_to_string(&item.path).unwrap(), "model='new'");
    }

    #[test]
    fn restore_refuses_a_concurrent_edit_before_publish() {
        let _reset = ResetFailpoints;
        let (_dir, store, item) = restore_fixture(Some("model='old'"), Some("model='new'"));
        let guard = lock_app(&item.app);
        operation::failpoint::on_before_publish(Some(Box::new(|_, path| {
            fs::write(path, "model='external'").unwrap();
        })));
        assert!(restore_file(&item, &store, &guard).is_err());
        assert_eq!(fs::read_to_string(&item.path).unwrap(), "model='external'");
        assert!(state::pending(&store, &item.app).unwrap().is_none());
    }

    #[test]
    fn interrupted_restore_uses_existing_pending_recovery_and_preserves_external_edits() {
        let _reset = ResetFailpoints;
        let (_dir, store, item) = restore_fixture(Some("model='old'"), Some("model='new'"));
        let guard = lock_app(&item.app);
        operation::failpoint::crash_at(Some("marked"));
        assert!(restore_file(&item, &store, &guard).is_err());
        assert!(state::pending(&store, &item.app).unwrap().is_some());
        operation::failpoint::crash_at(None);
        fs::write(&item.path, "model='external'").unwrap();
        operation::recover(&store, &guard, &|_| Ok(())).unwrap();
        assert_eq!(fs::read_to_string(&item.path).unwrap(), "model='external'");
        assert!(state::pending(&store, &item.app).unwrap().is_none());
    }

    #[test]
    fn restore_does_not_write_when_proxy_mode_is_active() {
        let (_dir, store, item) = restore_fixture(Some("model='old'"), Some("model='new'"));
        let guard = lock_app(&item.app);
        state::update(&store, |live| {
            live.apps.entry(item.app.clone()).or_default().mode = Some(state::Mode::Proxy);
        })
        .unwrap();
        assert!(restore_file(&item, &store, &guard).is_err());
        assert_eq!(fs::read_to_string(&item.path).unwrap(), "model='new'");
    }

    #[test]
    fn restore_paths_must_still_belong_to_the_same_app() {
        let mut item = record("0001", "2026-10-25T00:00:00Z", "a", "b");
        item.path = crate::codex_config::get_codex_config_dir()
            .join("config.toml")
            .to_string_lossy()
            .into_owned();
        assert!(validate_restore_path(&item).is_ok());
        item.app = "claude".into();
        assert!(validate_restore_path(&item).is_err());
        item.path = "/tmp/../config.toml".into();
        assert!(validate_restore_path(&item).is_err());
    }
}
