//! 平台会话的跨重启恢复与七天闲置过期管理。
//! JSON 仅保存账户、凭据引用和活动时间；刷新令牌始终保存在系统凭据库。
use super::platform_auth::{error, PlatformAuthError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{path::PathBuf, sync::Arc};

pub(super) const IDLE_SECONDS: i64 = 7 * 24 * 60 * 60;
const SERVICE: &str = "com.zhishu.ccswitch.desktop.platform-session";

pub(super) fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

/// 会话恢复、账户轮询和本地身份校验共用七天边界；时钟倒退或计算溢出时按过期处理。
pub(super) fn idle_expired(last_used_at: i64, timestamp: i64) -> bool {
    timestamp
        .checked_sub(last_used_at)
        .is_none_or(|elapsed| !(0..=IDLE_SECONDS).contains(&elapsed))
}

fn storage_error() -> PlatformAuthError {
    error(
        "SESSION_STORE",
        "无法保存或恢复登录状态，请检查系统凭据库和应用数据目录后重试",
    )
}

pub(super) trait SessionVault: Send + Sync {
    fn get(&self, id: &str) -> Result<Option<String>, PlatformAuthError>;
    fn set(&self, id: &str, token: &str) -> Result<(), PlatformAuthError>;
    fn remove(&self, id: &str) -> Result<(), PlatformAuthError>;
}
struct SystemVault;
impl SessionVault for SystemVault {
    fn get(&self, id: &str) -> Result<Option<String>, PlatformAuthError> {
        match keyring::Entry::new(SERVICE, id)
            .map_err(|_| storage_error())?
            .get_password()
        {
            Ok(token) => Ok(Some(token)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(storage_error()),
        }
    }
    fn set(&self, id: &str, token: &str) -> Result<(), PlatformAuthError> {
        keyring::Entry::new(SERVICE, id)
            .map_err(|_| storage_error())?
            .set_password(token)
            .map_err(|_| storage_error())
    }
    fn remove(&self, id: &str) -> Result<(), PlatformAuthError> {
        match keyring::Entry::new(SERVICE, id)
            .map_err(|_| storage_error())?
            .delete_credential()
        {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(storage_error()),
        }
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SessionRecord {
    credential_id: String,
    pub base_url: String,
    pub user_id: i64,
    pub last_used_at: i64,
}
impl SessionRecord {
    pub fn expired(&self, timestamp: i64) -> bool {
        idle_expired(self.last_used_at, timestamp)
    }
}

#[derive(Clone)]
pub(super) struct SessionStore {
    path: PathBuf,
    vault: Arc<dyn SessionVault>,
}
impl SessionStore {
    pub fn for_app(handle: &tauri::AppHandle) -> Result<Self, PlatformAuthError> {
        Ok(Self {
            path: super::platform_storage::data_file(handle, "platform-session.json")
                .map_err(|_| storage_error())?,
            vault: Arc::new(SystemVault),
        })
    }
    #[cfg(test)]
    pub fn for_test(path: PathBuf, vault: Arc<dyn SessionVault>) -> Self {
        Self { path, vault }
    }
    fn read(&self) -> Result<Option<SessionRecord>, PlatformAuthError> {
        match std::fs::read(&self.path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|_| storage_error()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(_) => Err(storage_error()),
        }
    }
    fn write(&self, record: Option<&SessionRecord>) -> Result<(), PlatformAuthError> {
        let bytes = serde_json::to_vec(&record).map_err(|_| storage_error())?;
        crate::config::atomic_write_private(&self.path, &bytes).map_err(|_| storage_error())
    }
    async fn blocking<T: Send + 'static>(
        &self,
        action: impl FnOnce(Self) -> Result<T, PlatformAuthError> + Send + 'static,
    ) -> Result<T, PlatformAuthError> {
        let store = self.clone();
        tauri::async_runtime::spawn_blocking(move || action(store))
            .await
            .map_err(|_| storage_error())?
    }
    pub async fn load(&self) -> Result<Option<(SessionRecord, String)>, PlatformAuthError> {
        self.blocking(|store| {
            let Some(record) = store.read()? else {
                return Ok(None);
            };
            if record.expired(now()) {
                store.clear_sync()?;
                return Ok(None);
            }
            match store.vault.get(&record.credential_id)? {
                Some(token) if !token.is_empty() => Ok(Some((record, token))),
                _ => {
                    store.clear_sync()?;
                    Ok(None)
                }
            }
        })
        .await
    }
    fn clear_sync(&self) -> Result<(), PlatformAuthError> {
        let record = self.read();
        if matches!(&record, Ok(None)) {
            return Ok(());
        }
        // 先失效本机会话指针，避免凭据库锁定、无法删除令牌时再次恢复已退出的账户。
        let invalidated = self.write(None);
        let removed = record
            .ok()
            .flatten()
            .map(|record| self.vault.remove(&record.credential_id));
        match invalidated {
            Ok(()) => {
                if matches!(removed, Some(Err(_))) {
                    log::warn!(
                        "Platform session vault cleanup deferred; local session is disabled"
                    );
                }
                Ok(())
            }
            Err(_) if matches!(removed, Some(Ok(()))) => {
                // 元数据只读时，只要凭据删除成功，同样能阻止下次恢复。
                log::warn!(
                    "Platform session metadata cleanup deferred; vault credential was removed"
                );
                Ok(())
            }
            Err(err) => Err(err),
        }
    }
    // 保留只读账户元数据入口；退出登录不再据此清理工具凭证。
    #[allow(dead_code)]
    pub async fn identity(&self) -> Result<Option<(String, i64)>, PlatformAuthError> {
        self.blocking(|store| {
            Ok(store
                .read()?
                .map(|record| (record.base_url, record.user_id)))
        })
        .await
    }
    pub async fn clear(&self) -> Result<(), PlatformAuthError> {
        self.blocking(|store| store.clear_sync()).await
    }
    pub async fn create(
        &self,
        base_url: String,
        user_id: i64,
        token: String,
    ) -> Result<SessionRecord, PlatformAuthError> {
        self.blocking(move |store| {
            if token.is_empty() {
                return Err(error(
                    "SESSION_STORE",
                    "平台未返回刷新凭证，本次登录无法跨重启保持",
                ));
            }
            // 凭据引用包含存储路径摘要，让开发隔离目录与正式目录使用不同条目。
            let scope = format!(
                "{:x}",
                Sha256::digest(store.path.to_string_lossy().as_bytes())
            );
            let record = SessionRecord {
                credential_id: format!("{scope}:{}", uuid::Uuid::new_v4()),
                base_url,
                user_id,
                last_used_at: now(),
            };
            store.vault.set(&record.credential_id, &token)?;
            if let Err(err) = store.write(Some(&record)) {
                let _ = store.vault.remove(&record.credential_id);
                return Err(err);
            }
            Ok(record)
        })
        .await
    }
    pub async fn save_token(
        &self,
        record: &SessionRecord,
        token: String,
    ) -> Result<(), PlatformAuthError> {
        let id = record.credential_id.clone();
        self.blocking(move |store| store.vault.set(&id, &token))
            .await
    }
    pub async fn touch(&self, record: &mut SessionRecord) -> Result<(), PlatformAuthError> {
        let mut next = record.clone();
        next.last_used_at = now();
        let saved = next.clone();
        self.blocking(move |store| store.write(Some(&saved)))
            .await?;
        *record = next;
        Ok(())
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use std::{
        collections::HashMap,
        sync::{
            atomic::{AtomicBool, Ordering},
            Mutex,
        },
    };

    #[derive(Default)]
    pub struct MemoryVault {
        tokens: Mutex<HashMap<String, String>>,
        pub unavailable: AtomicBool,
    }
    impl SessionVault for MemoryVault {
        fn get(&self, id: &str) -> Result<Option<String>, PlatformAuthError> {
            if self.unavailable.load(Ordering::SeqCst) {
                return Err(storage_error());
            }
            Ok(self.tokens.lock().unwrap().get(id).cloned())
        }
        fn set(&self, id: &str, token: &str) -> Result<(), PlatformAuthError> {
            if self.unavailable.load(Ordering::SeqCst) {
                return Err(storage_error());
            }
            self.tokens.lock().unwrap().insert(id.into(), token.into());
            Ok(())
        }
        fn remove(&self, id: &str) -> Result<(), PlatformAuthError> {
            if self.unavailable.load(Ordering::SeqCst) {
                return Err(storage_error());
            }
            self.tokens.lock().unwrap().remove(id);
            Ok(())
        }
    }
    pub async fn saved_store(
        base: &str,
        last_used_at: i64,
    ) -> (tempfile::TempDir, SessionStore, Arc<MemoryVault>) {
        let dir = tempfile::tempdir().unwrap();
        let vault = Arc::new(MemoryVault::default());
        let store = SessionStore::for_test(dir.path().join("session.json"), vault.clone());
        let mut record = store
            .create(base.into(), 1, "saved-refresh".into())
            .await
            .unwrap();
        record.last_used_at = last_used_at;
        store.write(Some(&record)).unwrap();
        (dir, store, vault)
    }
    #[test]
    fn inactivity_expires_only_after_seven_days_and_rejects_clock_reversal() {
        let record = SessionRecord {
            credential_id: "test".into(),
            base_url: "https://example.com".into(),
            user_id: 1,
            last_used_at: 100,
        };
        assert!(!record.expired(100 + IDLE_SECONDS));
        assert!(record.expired(101 + IDLE_SECONDS));
        assert!(record.expired(99));
    }
    #[tokio::test]
    async fn metadata_contains_no_token_and_has_private_permissions() {
        let (_dir, store, _vault) = saved_store("https://example.com", now()).await;
        let text = std::fs::read_to_string(&store.path).unwrap();
        assert!(!text.contains("saved-refresh"));
        assert!(!text.contains("access_token"));
        assert_eq!(store.load().await.unwrap().unwrap().1, "saved-refresh");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&store.path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }
    #[tokio::test]
    async fn expired_session_is_removed_without_reading_locked_vault() {
        let (_dir, store, vault) =
            saved_store("https://example.com", now() - IDLE_SECONDS - 1).await;
        vault.unavailable.store(true, Ordering::SeqCst);
        assert!(store.load().await.unwrap().is_none());
        assert!(store.read().unwrap().is_none());
    }
    #[tokio::test]
    async fn locked_vault_does_not_erase_a_valid_session() {
        let (_dir, store, vault) = saved_store("https://example.com", now()).await;
        vault.unavailable.store(true, Ordering::SeqCst);
        assert!(store.load().await.is_err());
        assert!(store.read().unwrap().is_some());
        vault.unavailable.store(false, Ordering::SeqCst);
        assert!(store.load().await.unwrap().is_some());
    }
    #[tokio::test]
    async fn logout_tombstone_prevents_restore_even_if_vault_deletion_fails() {
        let (_dir, store, vault) = saved_store("https://example.com", now()).await;
        vault.unavailable.store(true, Ordering::SeqCst);
        store.clear().await.unwrap();
        vault.unavailable.store(false, Ordering::SeqCst);
        assert!(store.load().await.unwrap().is_none());
    }
    #[tokio::test]
    async fn failed_metadata_write_removes_the_unpublished_credential() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("not-a-directory");
        std::fs::write(&path, b"blocked").unwrap();
        let vault = Arc::new(MemoryVault::default());
        let store = SessionStore::for_test(path.join("session.json"), vault.clone());
        assert!(store
            .create("https://example.com".into(), 1, "secret".into())
            .await
            .is_err());
        assert!(vault.tokens.lock().unwrap().is_empty());
    }
    #[tokio::test]
    async fn explicit_logout_can_disable_corrupted_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::for_test(
            dir.path().join("session.json"),
            Arc::new(MemoryVault::default()),
        );
        std::fs::write(&store.path, b"corrupt JSON").unwrap();
        assert!(store.load().await.is_err());
        store.clear().await.unwrap();
        assert!(store.load().await.unwrap().is_none());
    }
}
