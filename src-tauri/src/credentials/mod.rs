//! Secret storage. Production: Windows Credential Manager (per-user, DPAPI-protected by the OS).
//! Secrets never touch SQLite, logs, or the frontend after they are saved.

use crate::error::{AppError, AppResult};
use std::collections::HashMap;
use std::sync::Mutex;

pub trait SecretStore: Send + Sync {
    fn set(&self, key: &str, secret: &str) -> AppResult<()>;
    fn get(&self, key: &str) -> AppResult<Option<String>>;
    fn delete(&self, key: &str) -> AppResult<()>;
}

/// Credential target name for a session secret, e.g. `NexaTerm/<uuid>/password`.
pub fn target(session_id: &str, kind: &str) -> String {
    format!("NexaTerm/{session_id}/{kind}")
}

/// Volatile store used in tests (and as an explicit fallback that never persists).
#[derive(Default)]
pub struct MemoryStore(Mutex<HashMap<String, String>>);

impl SecretStore for MemoryStore {
    fn set(&self, key: &str, secret: &str) -> AppResult<()> {
        self.0.lock().unwrap().insert(key.into(), secret.into());
        Ok(())
    }
    fn get(&self, key: &str) -> AppResult<Option<String>> {
        Ok(self.0.lock().unwrap().get(key).cloned())
    }
    fn delete(&self, key: &str) -> AppResult<()> {
        self.0.lock().unwrap().remove(key);
        Ok(())
    }
}

#[cfg(windows)]
pub use win::WindowsCredentialStore;

#[cfg(target_os = "linux")]
pub use linux::SecretServiceStore;

/// Linux: freedesktop Secret Service (GNOME Keyring, KWallet, KeePassXC ...). Items carry the
/// attributes {application: "NexaTerm", target: "<key>"} and are stored in the default collection.
/// Each call runs on its own thread with a private tokio runtime, so it is safe to call from
/// synchronous code and from inside the app's async runtime alike.
#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use secret_service::{EncryptionType, SecretService};
    use std::collections::HashMap;

    pub struct SecretServiceStore;

    fn run<T: Send + 'static>(f: impl FnOnce(&tokio::runtime::Runtime) -> AppResult<T> + Send + 'static) -> AppResult<T> {
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().map_err(|e| AppError::Credential(e.to_string()))?;
            f(&rt)
        })
        .join()
        .map_err(|_| AppError::Credential("secret service thread panicked".into()))?
    }

    fn err(e: impl std::fmt::Display) -> AppError {
        AppError::Credential(format!(
            "Secret Service unavailable ({e}). Install and unlock a keyring such as GNOME Keyring or KWallet to save passwords."
        ))
    }

    fn attrs(key: &str) -> HashMap<&str, &str> {
        HashMap::from([("application", "NexaTerm"), ("target", key)])
    }

    impl SecretStore for SecretServiceStore {
        fn set(&self, key: &str, secret: &str) -> AppResult<()> {
            let (key, secret) = (key.to_string(), secret.to_string());
            run(move |rt| {
                rt.block_on(async {
                    let ss = SecretService::connect(EncryptionType::Dh).await.map_err(err)?;
                    let col = ss.get_default_collection().await.map_err(err)?;
                    if col.is_locked().await.map_err(err)? {
                        col.unlock().await.map_err(err)?;
                    }
                    col.create_item(&format!("NexaTerm: {key}"), attrs(&key), secret.as_bytes(), true, "text/plain").await.map_err(err)?;
                    Ok(())
                })
            })
        }

        fn get(&self, key: &str) -> AppResult<Option<String>> {
            let key = key.to_string();
            run(move |rt| {
                rt.block_on(async {
                    let ss = SecretService::connect(EncryptionType::Dh).await.map_err(err)?;
                    let found = ss.search_items(attrs(&key)).await.map_err(err)?;
                    let item = match (found.unlocked.into_iter().next(), found.locked.into_iter().next()) {
                        (Some(i), _) => i,
                        (None, Some(i)) => {
                            i.unlock().await.map_err(err)?;
                            i
                        }
                        (None, None) => return Ok(None),
                    };
                    let bytes = item.get_secret().await.map_err(err)?;
                    Ok(Some(String::from_utf8_lossy(&bytes).into_owned()))
                })
            })
        }

        fn delete(&self, key: &str) -> AppResult<()> {
            let key = key.to_string();
            run(move |rt| {
                rt.block_on(async {
                    let ss = SecretService::connect(EncryptionType::Dh).await.map_err(err)?;
                    let found = ss.search_items(attrs(&key)).await.map_err(err)?;
                    for item in found.unlocked.iter().chain(found.locked.iter()) {
                        item.delete().await.map_err(err)?;
                    }
                    Ok(())
                })
            })
        }
    }
}

#[cfg(windows)]
mod win {
    use super::*;
    use windows::core::{PCWSTR, PWSTR};
    use windows::Win32::Foundation::{ERROR_NOT_FOUND, FILETIME};
    use windows::Win32::Security::Credentials::{
        CredDeleteW, CredFree, CredReadW, CredWriteW, CREDENTIALW, CRED_FLAGS, CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC,
    };

    pub struct WindowsCredentialStore;

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn is_not_found(e: &windows::core::Error) -> bool {
        e.code() == ERROR_NOT_FOUND.to_hresult()
    }

    impl SecretStore for WindowsCredentialStore {
        fn set(&self, key: &str, secret: &str) -> AppResult<()> {
            let mut target = wide(key);
            let mut user = wide("NexaTerm");
            // Stored as UTF-16 bytes, matching what Windows' own tools expect for generic creds.
            let mut blob: Vec<u8> = secret.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
            if blob.len() > 5 * 512 {
                return Err(AppError::Credential("Secret is too long".into()));
            }
            let cred = CREDENTIALW {
                Flags: CRED_FLAGS(0),
                Type: CRED_TYPE_GENERIC,
                TargetName: PWSTR(target.as_mut_ptr()),
                Comment: PWSTR::null(),
                LastWritten: FILETIME::default(),
                CredentialBlobSize: blob.len() as u32,
                CredentialBlob: blob.as_mut_ptr(),
                Persist: CRED_PERSIST_LOCAL_MACHINE,
                AttributeCount: 0,
                Attributes: std::ptr::null_mut(),
                TargetAlias: PWSTR::null(),
                UserName: PWSTR(user.as_mut_ptr()),
            };
            let res = unsafe { CredWriteW(&cred, 0) };
            // Best-effort wipe of our copy of the secret.
            blob.iter_mut().for_each(|b| *b = 0);
            res.map_err(|e| AppError::Credential(format!("CredWrite failed: {}", e.message())))
        }

        fn get(&self, key: &str) -> AppResult<Option<String>> {
            let target = wide(key);
            let mut pcred: *mut CREDENTIALW = std::ptr::null_mut();
            match unsafe { CredReadW(PCWSTR(target.as_ptr()), CRED_TYPE_GENERIC, None, &mut pcred) } {
                Ok(()) => {
                    let s = unsafe {
                        let c = &*pcred;
                        let bytes = std::slice::from_raw_parts(c.CredentialBlob, c.CredentialBlobSize as usize);
                        let units: Vec<u16> = bytes.chunks_exact(2).map(|ch| u16::from_le_bytes([ch[0], ch[1]])).collect();
                        let s = String::from_utf16_lossy(&units);
                        CredFree(pcred as *const _);
                        s
                    };
                    Ok(Some(s))
                }
                Err(e) if is_not_found(&e) => Ok(None),
                Err(e) => Err(AppError::Credential(format!("CredRead failed: {}", e.message()))),
            }
        }

        fn delete(&self, key: &str) -> AppResult<()> {
            let target = wide(key);
            match unsafe { CredDeleteW(PCWSTR(target.as_ptr()), CRED_TYPE_GENERIC, None) } {
                Ok(()) => Ok(()),
                Err(e) if is_not_found(&e) => Ok(()),
                Err(e) => Err(AppError::Credential(format!("CredDelete failed: {}", e.message()))),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exercise(store: &dyn SecretStore, key: &str) {
        store.delete(key).unwrap();
        assert_eq!(store.get(key).unwrap(), None);
        store.set(key, "s3cr3t-ไทย").unwrap();
        assert_eq!(store.get(key).unwrap().as_deref(), Some("s3cr3t-ไทย"));
        store.set(key, "second").unwrap();
        assert_eq!(store.get(key).unwrap().as_deref(), Some("second"));
        store.delete(key).unwrap();
        assert_eq!(store.get(key).unwrap(), None);
    }

    #[test]
    fn memory_store() {
        exercise(&MemoryStore::default(), "k");
    }

    /// Runs in CI under dbus-run-session with an unlocked gnome-keyring
    /// (NEXATERM_TEST_SECRET_SERVICE=1); skipped elsewhere.
    #[cfg(target_os = "linux")]
    #[test]
    fn secret_service_roundtrip() {
        if std::env::var_os("NEXATERM_TEST_SECRET_SERVICE").is_none() {
            eprintln!("skipping: NEXATERM_TEST_SECRET_SERVICE not set");
            return;
        }
        let key = format!("NexaTerm-test/{}", uuid::Uuid::new_v4());
        exercise(&SecretServiceStore, &key);
    }

    #[cfg(windows)]
    #[test]
    fn windows_credential_manager_roundtrip() {
        let key = format!("NexaTerm-test/{}", uuid::Uuid::new_v4());
        exercise(&WindowsCredentialStore, &key);
    }
}
