//! 本机登录账号记录与用户主动启用的「记住密码」。
//! JSON 不保存密码；此处的凭据服务独立于会话恢复，不用于自动登录或跳过二次验证。
use crate::zhishu::services::platform_auth::{error, PlatformAuthError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{path::Path, sync::Mutex};

static HISTORY_LOCK: Mutex<()> = Mutex::new(());
const SERVICE: &str = "com.zhishu.ccswitch.desktop.platform-login";

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedPlatformLogin {
    id: String,
    base_url: String,
    email: String,
    remember_password: bool,
}

trait PasswordStore {
    fn get(&self, id: &str) -> Result<Option<String>, PlatformAuthError>;
    fn set(&self, id: &str, password: &str) -> Result<(), PlatformAuthError>;
    fn remove(&self, id: &str) -> Result<(), PlatformAuthError>;
}
struct SystemPasswords;
fn vault_error() -> PlatformAuthError {
    error(
        "CREDENTIAL_STORE",
        "无法访问系统凭据库，请解锁后重试，或取消记住密码",
    )
}
impl PasswordStore for SystemPasswords {
    fn get(&self, id: &str) -> Result<Option<String>, PlatformAuthError> {
        match keyring::Entry::new(SERVICE, id)
            .map_err(|_| vault_error())?
            .get_password()
        {
            Ok(password) => Ok(Some(password)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(vault_error()),
        }
    }
    fn set(&self, id: &str, password: &str) -> Result<(), PlatformAuthError> {
        keyring::Entry::new(SERVICE, id)
            .map_err(|_| vault_error())?
            .set_password(password)
            .map_err(|_| vault_error())
    }
    fn remove(&self, id: &str) -> Result<(), PlatformAuthError> {
        match keyring::Entry::new(SERVICE, id)
            .map_err(|_| vault_error())?
            .delete_credential()
        {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(vault_error()),
        }
    }
}
fn history_error() -> PlatformAuthError {
    error(
        "LOGIN_HISTORY",
        "无法读写本机登录记录，请检查应用数据目录权限",
    )
}
fn read_history(path: &Path) -> Result<Vec<SavedPlatformLogin>, PlatformAuthError> {
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|_| history_error()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(_) => Err(history_error()),
    }
}
fn write_history(path: &Path, records: &[SavedPlatformLogin]) -> Result<(), PlatformAuthError> {
    let bytes = serde_json::to_vec_pretty(records).map_err(|_| history_error())?;
    crate::config::atomic_write_private(path, &bytes).map_err(|_| history_error())
}
fn save_login(
    path: &Path,
    vault: &impl PasswordStore,
    base_url: &str,
    email: &str,
    password: &str,
    remember_password: bool,
) -> Result<(), PlatformAuthError> {
    let mut records = read_history(path)?;
    let id = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&(base_url, email)).unwrap())
    );
    let previously_remembered = records.iter().any(|r| r.id == id && r.remember_password);
    let password_result = if remember_password {
        vault.set(&id, password)
    } else if previously_remembered {
        vault.remove(&id)
    } else {
        Ok(())
    };
    // 凭据库锁定时仍保存账号历史，但保留原记忆状态，并把密码存储失败告知调用方。
    let remember_password = if password_result.is_ok() {
        remember_password
    } else {
        previously_remembered
    };
    records.retain(|r| r.id != id);
    records.insert(
        0,
        SavedPlatformLogin {
            id,
            base_url: base_url.into(),
            email: email.into(),
            remember_password,
        },
    );
    write_history(path, &records)?;
    password_result
}
fn forget_password(
    path: &Path,
    vault: &impl PasswordStore,
    id: &str,
    remove_record: bool,
) -> Result<(), PlatformAuthError> {
    let mut records = read_history(path)?;
    if let Some(record) = records.iter_mut().find(|r| r.id == id) {
        if record.remember_password {
            vault.remove(id)?;
        }
        record.remember_password = false;
        if remove_record {
            records.retain(|r| r.id != id);
        }
        write_history(path, &records)?;
    }
    Ok(())
}
async fn with_history<T: Send + 'static>(
    handle: tauri::AppHandle,
    action: impl FnOnce(&Path) -> Result<T, PlatformAuthError> + Send + 'static,
) -> Result<T, PlatformAuthError> {
    let path =
        crate::zhishu::storage::platform_storage::data_file(&handle, "platform-login-history.json")
            .map_err(|_| history_error())?;
    tauri::async_runtime::spawn_blocking(move || {
        let _lock = HISTORY_LOCK.lock().map_err(|_| history_error())?;
        action(&path)
    })
    .await
    .map_err(|_| history_error())?
}

pub(crate) async fn remember_login(
    handle: tauri::AppHandle,
    base_url: String,
    email: String,
    password: String,
    remember_password: bool,
) -> Result<(), PlatformAuthError> {
    with_history(handle, move |path| {
        save_login(
            path,
            &SystemPasswords,
            &base_url,
            &email,
            &password,
            remember_password,
        )
    })
    .await
}

#[tauri::command]
pub async fn platform_login_history(
    handle: tauri::AppHandle,
) -> Result<Vec<SavedPlatformLogin>, PlatformAuthError> {
    with_history(handle, read_history).await
}
#[tauri::command]
pub async fn platform_saved_password(
    handle: tauri::AppHandle,
    id: String,
) -> Result<Option<String>, PlatformAuthError> {
    with_history(handle, move |path| {
        if read_history(path)?
            .iter()
            .any(|r| r.id == id && r.remember_password)
        {
            SystemPasswords.get(&id)
        } else {
            Ok(None)
        }
    })
    .await
}
#[tauri::command]
pub async fn platform_forget_password(
    handle: tauri::AppHandle,
    id: String,
) -> Result<(), PlatformAuthError> {
    with_history(handle, move |path| {
        forget_password(path, &SystemPasswords, &id, false)
    })
    .await
}
#[tauri::command]
pub async fn platform_remove_login(
    handle: tauri::AppHandle,
    id: String,
) -> Result<(), PlatformAuthError> {
    with_history(handle, move |path| {
        forget_password(path, &SystemPasswords, &id, true)
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, collections::HashMap};
    #[derive(Default)]
    struct MemoryPasswords(RefCell<HashMap<String, String>>);
    impl PasswordStore for MemoryPasswords {
        fn get(&self, id: &str) -> Result<Option<String>, PlatformAuthError> {
            Ok(self.0.borrow().get(id).cloned())
        }
        fn set(&self, id: &str, password: &str) -> Result<(), PlatformAuthError> {
            self.0.borrow_mut().insert(id.into(), password.into());
            Ok(())
        }
        fn remove(&self, id: &str) -> Result<(), PlatformAuthError> {
            self.0.borrow_mut().remove(id);
            Ok(())
        }
    }
    struct UnavailablePasswords;
    impl PasswordStore for UnavailablePasswords {
        fn get(&self, _: &str) -> Result<Option<String>, PlatformAuthError> {
            Err(vault_error())
        }
        fn set(&self, _: &str, _: &str) -> Result<(), PlatformAuthError> {
            Err(vault_error())
        }
        fn remove(&self, _: &str) -> Result<(), PlatformAuthError> {
            Err(vault_error())
        }
    }
    #[test]
    fn locked_vault_keeps_account_history_usable_without_claiming_password_was_saved() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.json");
        assert!(save_login(
            &path,
            &UnavailablePasswords,
            "https://one.example",
            "a@example.com",
            "secret",
            true
        )
        .is_err());
        let records = read_history(&path).unwrap();
        assert_eq!(records.len(), 1);
        assert!(!records[0].remember_password);
        forget_password(&path, &UnavailablePasswords, &records[0].id, true).unwrap();
        assert!(read_history(&path).unwrap().is_empty());
    }

    #[test]
    fn records_are_deduplicated_ordered_and_scoped_to_platform() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.json");
        let vault = MemoryPasswords::default();
        for base in [
            "https://one.example",
            "https://two.example",
            "https://one.example",
        ] {
            save_login(&path, &vault, base, "a@example.com", "secret", false).unwrap();
        }
        let records = read_history(&path).unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].base_url, "https://one.example");
        assert_ne!(records[0].id, records[1].id);
        assert!(vault.0.borrow().is_empty());
    }
    #[test]
    fn passwords_never_enter_metadata_and_can_be_forgotten_or_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.json");
        let vault = MemoryPasswords::default();
        save_login(
            &path,
            &vault,
            "https://one.example",
            "a@example.com",
            "secret-password",
            true,
        )
        .unwrap();
        let id = read_history(&path).unwrap()[0].id.clone();
        assert_eq!(vault.get(&id).unwrap().as_deref(), Some("secret-password"));
        assert!(!std::fs::read_to_string(&path)
            .unwrap()
            .contains("secret-password"));
        forget_password(&path, &vault, &id, false).unwrap();
        assert!(!read_history(&path).unwrap()[0].remember_password);
        assert!(vault.get(&id).unwrap().is_none());
        save_login(
            &path,
            &vault,
            "https://one.example",
            "a@example.com",
            "new-password",
            true,
        )
        .unwrap();
        save_login(
            &path,
            &vault,
            "https://one.example",
            "a@example.com",
            "ignored",
            false,
        )
        .unwrap();
        assert!(vault.get(&id).unwrap().is_none());
        save_login(
            &path,
            &vault,
            "https://one.example",
            "a@example.com",
            "new-password",
            true,
        )
        .unwrap();
        forget_password(&path, &vault, &id, true).unwrap();
        assert!(read_history(&path).unwrap().is_empty());
        assert!(vault.get(&id).unwrap().is_none());
    }
}
