use std::collections::BTreeMap;
use std::io::Read;
use std::sync::{Arc, RwLock};
use thiserror::Error;

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum SecretError {
    #[error("Secret is missing")]
    Missing,
    #[error("Secret store is unavailable")]
    Unavailable,
    #[error("Secret cannot be read")]
    Unreadable,
}

pub trait SecretStore: Send + Sync {
    fn get(&self, name: &str) -> Result<String, SecretError>;
    fn set(&self, name: &str, value: &str) -> Result<(), SecretError>;
    fn delete(&self, name: &str) -> Result<(), SecretError>;
}

#[derive(Default)]
pub struct MemorySecretStore {
    values: RwLock<BTreeMap<String, String>>,
}

pub fn check_name(name: &str) -> Result<(), SecretError> {
    if name.is_empty() || name == "model-api-key" {
        Err(SecretError::Unreadable)
    } else {
        Ok(())
    }
}

impl SecretStore for MemorySecretStore {
    fn get(&self, name: &str) -> Result<String, SecretError> {
        check_name(name)?;
        self.values
            .read()
            .unwrap()
            .get(name)
            .cloned()
            .ok_or(SecretError::Missing)
    }
    fn set(&self, name: &str, value: &str) -> Result<(), SecretError> {
        check_name(name)?;
        self.values
            .write()
            .unwrap()
            .insert(name.to_string(), value.to_string());
        Ok(())
    }
    fn delete(&self, name: &str) -> Result<(), SecretError> {
        check_name(name)?;
        self.values.write().unwrap().remove(name);
        Ok(())
    }
}

#[cfg(feature = "os-store")]
const SERVICE_NAME: &str = "Nook MCP";

#[derive(Clone, Default)]
pub struct OsSecretStore {
    #[cfg(feature = "os-store")]
    store: Option<Arc<keyring_core::api::CredentialStore>>,
}

#[cfg(feature = "os-store")]
fn map_keyring_error(err: keyring_core::Error) -> SecretError {
    match err {
        keyring_core::Error::NoEntry => SecretError::Missing,
        keyring_core::Error::NoStorageAccess(_)
        | keyring_core::Error::PlatformFailure(_)
        | keyring_core::Error::NoDefaultStore => SecretError::Unavailable,
        keyring_core::Error::BadEncoding(_)
        | keyring_core::Error::BadDataFormat(_, _)
        | keyring_core::Error::BadStoreFormat(_)
        | keyring_core::Error::TooLong(_, _)
        | keyring_core::Error::Invalid(_, _)
        | keyring_core::Error::Ambiguous(_)
        | keyring_core::Error::NotSupportedByStore(_) => SecretError::Unreadable,
        _ => SecretError::Unreadable,
    }
}

impl OsSecretStore {
    #[cfg(feature = "os-store")]
    pub fn with_store(handle: Arc<keyring_core::api::CredentialStore>) -> Self {
        Self {
            store: Some(handle),
        }
    }

    pub fn unavailable() -> Self {
        Self {
            #[cfg(feature = "os-store")]
            store: None,
        }
    }

    pub fn default_for_os() -> Self {
        #[cfg(feature = "os-store")]
        {
            #[cfg(target_os = "macos")]
            let store = std::panic::catch_unwind(|| {
                apple_native_keyring_store::keychain::Store::new()
                    .map(|s| s as Arc<keyring_core::api::CredentialStore>)
                    .ok()
            })
            .ok()
            .flatten();

            #[cfg(target_os = "linux")]
            let store = std::panic::catch_unwind(|| {
                zbus_secret_service_keyring_store::Store::new()
                    .map(|s| s as Arc<keyring_core::api::CredentialStore>)
                    .ok()
            })
            .ok()
            .flatten();

            #[cfg(windows)]
            let store = std::panic::catch_unwind(|| {
                windows_native_keyring_store::Store::new()
                    .map(|s| s as Arc<keyring_core::api::CredentialStore>)
                    .ok()
            })
            .ok()
            .flatten();

            #[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
            let store: Option<Arc<keyring_core::api::CredentialStore>> = None;

            Self { store }
        }
        #[cfg(not(feature = "os-store"))]
        {
            Self {}
        }
    }
}

impl SecretStore for OsSecretStore {
    fn get(&self, name: &str) -> Result<String, SecretError> {
        check_name(name)?;
        #[cfg(feature = "os-store")]
        {
            let store = self.store.as_ref().ok_or(SecretError::Unavailable)?;
            let entry = store
                .build(SERVICE_NAME, name, None)
                .map_err(map_keyring_error)?;
            entry.get_password().map_err(map_keyring_error)
        }
        #[cfg(not(feature = "os-store"))]
        {
            Err(SecretError::Unavailable)
        }
    }

    fn set(&self, name: &str, _value: &str) -> Result<(), SecretError> {
        check_name(name)?;
        #[cfg(feature = "os-store")]
        {
            let store = self.store.as_ref().ok_or(SecretError::Unavailable)?;
            let entry = store
                .build(SERVICE_NAME, name, None)
                .map_err(map_keyring_error)?;
            entry.set_password(_value).map_err(map_keyring_error)
        }
        #[cfg(not(feature = "os-store"))]
        {
            Err(SecretError::Unavailable)
        }
    }

    fn delete(&self, name: &str) -> Result<(), SecretError> {
        check_name(name)?;
        #[cfg(feature = "os-store")]
        {
            let store = self.store.as_ref().ok_or(SecretError::Unavailable)?;
            let entry = store
                .build(SERVICE_NAME, name, None)
                .map_err(map_keyring_error)?;
            entry.delete_credential().map_err(map_keyring_error)
        }
        #[cfg(not(feature = "os-store"))]
        {
            Err(SecretError::Unavailable)
        }
    }
}

pub type SecretMask = Arc<RwLock<Vec<String>>>;

#[derive(Clone)]
pub struct SecretContext {
    pub store: Arc<dyn SecretStore>,
    pub mask: SecretMask,
}

impl Default for SecretContext {
    fn default() -> Self {
        Self {
            store: Arc::new(MemorySecretStore::default()),
            mask: Arc::default(),
        }
    }
}

pub trait SecretMaskExt {
    fn add(&self, value: &str);
    fn scrub(&self, text: &str) -> String;
}

impl SecretMaskExt for SecretMask {
    fn add(&self, value: &str) {
        if value.chars().count() >= 4 {
            let mut guard = self.write().unwrap();
            if !guard.contains(&value.to_string()) {
                guard.push(value.to_string());
            }
        }
    }

    fn scrub(&self, text: &str) -> String {
        let Ok(guard) = self.read() else {
            return text.to_string();
        };
        let mut result = text.to_string();
        for secret in guard.iter() {
            if secret.chars().count() >= 4 {
                result = result.replace(secret, "***");
            }
        }
        result
    }
}

pub fn set_from_reader(
    store: &dyn SecretStore,
    name: &str,
    mut reader: impl Read,
) -> Result<(), SecretError> {
    check_name(name)?;
    let mut value = String::new();
    reader
        .read_to_string(&mut value)
        .map_err(|_| SecretError::Unreadable)?;
    if value.ends_with("\r\n") {
        value.truncate(value.len() - 2);
    } else if value.ends_with('\n') {
        value.pop();
    }
    if value.is_empty() {
        return Err(SecretError::Unreadable);
    }
    store.set(name, &value)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn the_memory_store_returns_what_was_set_and_nothing_after_delete() {
        let store = MemorySecretStore::default();
        store.set("secret", "value").unwrap();
        assert_eq!(store.get("secret"), Ok("value".into()));
        store.delete("secret").unwrap();
        assert_eq!(store.get("secret"), Err(SecretError::Missing));
    }
    #[test]
    fn set_from_reader_trims_one_trailing_newline_or_crlf_and_refuses_an_empty_value() {
        let store = MemorySecretStore::default();
        set_from_reader(&store, "a", &b"one\n\n"[..]).unwrap();
        set_from_reader(&store, "b", &b"two\r\n"[..]).unwrap();
        assert_eq!(store.get("a"), Ok("one\n".into()));
        assert_eq!(store.get("b"), Ok("two".into()));
        assert_eq!(
            set_from_reader(&store, "c", &b"\n"[..]),
            Err(SecretError::Unreadable)
        );
    }
    #[test]
    fn the_model_key_name_is_refused_by_every_store_call() {
        let store = MemorySecretStore::default();
        assert_eq!(store.get("model-api-key"), Err(SecretError::Unreadable));
        assert_eq!(
            store.set("model-api-key", "x"),
            Err(SecretError::Unreadable)
        );
        assert_eq!(store.delete("model-api-key"), Err(SecretError::Unreadable));
        assert_eq!(store.get(""), Err(SecretError::Unreadable));
    }
    #[test]
    fn secret_mask_adds_values_of_at_least_four_chars_and_scrubs_them() {
        let mask = Arc::new(RwLock::new(Vec::new()));
        mask.add("supersecret");
        mask.add("abc"); // skipped because < 4 chars
        assert_eq!(
            mask.scrub("hello supersecret abc world"),
            "hello *** abc world"
        );
    }

    #[test]
    #[cfg(feature = "os-store")]
    fn the_os_store_round_trips_through_the_keyring_core_mock_store() {
        let mock = keyring_core::mock::Store::new().unwrap();
        let store = OsSecretStore::with_store(mock);

        // Refused names
        assert_eq!(store.get(""), Err(SecretError::Unreadable));
        assert_eq!(store.set("", "value"), Err(SecretError::Unreadable));
        assert_eq!(store.delete(""), Err(SecretError::Unreadable));
        assert_eq!(store.get("model-api-key"), Err(SecretError::Unreadable));
        assert_eq!(
            store.set("model-api-key", "value"),
            Err(SecretError::Unreadable)
        );
        assert_eq!(store.delete("model-api-key"), Err(SecretError::Unreadable));

        // Missing
        assert_eq!(store.get("not-found"), Err(SecretError::Missing));

        // Set & Get
        store.set("my-secret", "supersecret").unwrap();
        assert_eq!(store.get("my-secret"), Ok("supersecret".to_string()));

        // Delete
        store.delete("my-secret").unwrap();
        assert_eq!(store.get("my-secret"), Err(SecretError::Missing));
    }

    #[test]
    #[cfg(feature = "os-store")]
    fn a_store_error_that_holds_the_secret_bytes_never_leaks_in_secrets_rs() {
        use keyring_core::api::CredentialStoreApi;
        let mock = keyring_core::mock::Store::new().unwrap();
        let secret_bytes = b"super-sensitive-token-999";
        let entry = mock.build("Nook MCP", "test-secret", None).unwrap();
        let cred: &keyring_core::mock::Cred = entry.as_any().downcast_ref().unwrap();
        cred.set_error(keyring_core::Error::BadEncoding(secret_bytes.to_vec()));

        let store = OsSecretStore::with_store(mock);
        let err = store.get("test-secret").unwrap_err();
        assert_eq!(err, SecretError::Unreadable);
        let display = format!("{err}");
        let debug = format!("{err:?}");
        let leaked_str = "super-sensitive-token-999";
        assert!(!display.contains(leaked_str));
        assert!(!debug.contains(leaked_str));
        assert_eq!(display, "Secret cannot be read");
        assert_eq!(debug, "Unreadable");
    }

    #[test]
    #[cfg(feature = "os-store")]
    fn an_unbuildable_real_store_returns_unavailable() {
        let store = OsSecretStore::unavailable();
        assert_eq!(store.get("my-secret"), Err(SecretError::Unavailable));
        assert_eq!(store.set("my-secret", "val"), Err(SecretError::Unavailable));
        assert_eq!(store.delete("my-secret"), Err(SecretError::Unavailable));

        let default_store = OsSecretStore::default_for_os();
        assert_eq!(
            default_store.get("my-secret"),
            Err(SecretError::Unavailable)
        );
    }
}
