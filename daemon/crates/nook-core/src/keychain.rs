#[cfg(target_os = "macos")]
use security_framework::passwords::{get_generic_password, set_generic_password};

pub const SERVICE_NAME: &str = "Nook";
pub const ACCOUNT_NAME: &str = "model-api-key";

pub fn get_api_key() -> Option<String> {
    if let Ok(key) = std::env::var("NOOK_API_KEY") {
        if !key.trim().is_empty() {
            return Some(key);
        }
    }

    #[cfg(target_os = "macos")]
    {
        match get_generic_password(SERVICE_NAME, ACCOUNT_NAME) {
            Ok(bytes) => String::from_utf8(bytes).ok(),
            Err(_) => None,
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        None
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
