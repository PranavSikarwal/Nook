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
}

impl SecretMaskExt for SecretMask {
    fn add(&self, value: &str) {
        if value.chars().count() >= 4 {
            self.write().unwrap().push(value.to_string());
        }
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
}
