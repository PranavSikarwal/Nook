#[cfg(target_os = "macos")]
use security_framework::passwords::{get_generic_password, set_generic_password};

pub const SERVICE_NAME: &str = "Nook";
pub const ACCOUNT_NAME: &str = "model-api-key";

pub fn get_api_key() -> Option<String> {
    get_api_key_from(
        std::env::var("NOOK_DISABLE_KEYCHAIN").ok().as_deref(),
        std::env::var("NOOK_API_KEY").ok().as_deref(),
        #[cfg(target_os = "macos")]
        || match get_generic_password(SERVICE_NAME, ACCOUNT_NAME) {
            Ok(bytes) => String::from_utf8(bytes).ok(),
            Err(_) => None,
        },
        #[cfg(not(target_os = "macos"))]
        || None,
    )
}

fn get_api_key_from(
    disable_keychain: Option<&str>,
    environment_key: Option<&str>,
    keychain_key: impl FnOnce() -> Option<String>,
) -> Option<String> {
    if disable_keychain == Some("1") {
        return environment_key
            .filter(|key| !key.trim().is_empty())
            .map(str::to_owned);
    }

    if let Some(key) = environment_key.filter(|key| !key.trim().is_empty()) {
        return Some(key.to_owned());
    }

    keychain_key().filter(|key| !key.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::get_api_key_from;

    #[test]
    fn disabled_keychain_returns_only_environment_key() {
        let result = get_api_key_from(Some("1"), None, || Some("keychain-secret".into()));
        assert_eq!(result, None);
    }

    #[test]
    fn disabled_keychain_keeps_explicit_environment_key() {
        let result = get_api_key_from(Some("1"), Some("test-key"), || None);
        assert_eq!(result.as_deref(), Some("test-key"));
    }

    #[test]
    fn environment_key_takes_precedence() {
        let result = get_api_key_from(None, Some("test-key"), || Some("keychain-secret".into()));
        assert_eq!(result.as_deref(), Some("test-key"));
    }

    #[test]
    fn keychain_is_used_when_enabled_and_environment_key_is_missing() {
        let result = get_api_key_from(None, None, || Some("keychain-secret".into()));
        assert_eq!(result.as_deref(), Some("keychain-secret"));
    }

    #[test]
    fn empty_environment_key_falls_through_when_keychain_is_enabled() {
        let result = get_api_key_from(None, Some("  "), || Some("keychain-secret".into()));
        assert_eq!(result.as_deref(), Some("keychain-secret"));
    }

    #[test]
    fn empty_environment_key_is_absent_when_keychain_is_disabled() {
        let result = get_api_key_from(Some("1"), Some("  "), || Some("keychain-secret".into()));
        assert_eq!(result, None);
    }
}

pub fn set_api_key(key: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    #[cfg(target_os = "macos")]
    {
        set_generic_password(SERVICE_NAME, ACCOUNT_NAME, key.as_bytes())?;
        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = key;
        Ok(())
    }
}
