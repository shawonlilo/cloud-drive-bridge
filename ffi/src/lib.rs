//! cdb-ffi — UniFFI surface over cdb-core.
//!
//! Native shells (Android DocumentsProvider in Kotlin, Apple File
//! Provider in Swift, Tauri desktop) all call THIS surface, which
//! delegates to the single cdb-core StorageProvider implementation.
//! No platform re-implements S3.
//!
//! Generate bindings:
//!   cargo run -p cdb-ffi --bin uniffi-bindgen -- generate \
//!     --library target/release/libcdb_ffi.so --language kotlin --out-dir out/kotlin
//!   (same with --language swift for Apple platforms)

use cdb_core::{Provider, StorageProvider};

uniffi::setup_scaffolding!();

#[uniffi::export]
pub fn cdb_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[uniffi::export]
pub fn cdb_provider_ids() -> Vec<String> {
    Provider::all().iter().map(|p| p.id().to_string()).collect()
}

#[uniffi::export]
pub fn cdb_default_region(provider_id: String) -> String {
    Provider::from_id(&provider_id)
        .map(|p| p.default_region().to_string())
        .unwrap_or_default()
}

#[uniffi::export]
pub fn cdb_endpoint_template(provider_id: String) -> String {
    Provider::from_id(&provider_id)
        .map(|p| p.endpoint_template().to_string())
        .unwrap_or_default()
}

#[uniffi::export]
pub fn cdb_valid_profile_name(name: String) -> bool {
    !name.is_empty()
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// A session handle for native shells: holds one profile + its keys in
/// memory (mobile keychain integration is the shell's job — Android
/// Keystore / iOS Keychain hand the values in, cdb-ffi never persists).
#[derive(uniffi::Object)]
pub struct CdbSession {
    inner: std::sync::Mutex<Option<cdb_core::S3Provider>>,
}

#[uniffi::export]
impl CdbSession {
    #[uniffi::constructor]
    pub fn new() -> std::sync::Arc<Self> {
        std::sync::Arc::new(CdbSession { inner: std::sync::Mutex::new(None) })
    }

    /// Configure with profile metadata + keys supplied by the shell's
    /// secure storage. Returns an error string (never panics across FFI).
    pub fn configure(
        &self,
        name: String,
        provider_id: String,
        bucket: String,
        region: String,
        endpoint: String,
        access_key: String,
        secret_key: String,
    ) -> Result<(), CdbFfiError> {
        let provider = Provider::from_id(&provider_id)
            .ok_or_else(|| CdbFfiError::Invalid(format!("unknown provider '{}'", provider_id)))?;
        let profile = cdb_core::Profile::new(&name, provider, &bucket, &region, &endpoint)
            .map_err(CdbFfiError::Invalid)?;
        let secrets = cdb_core::Secrets { access_key_id: access_key, secret_access_key: secret_key };
        if secrets.is_empty() {
            return Err(CdbFfiError::Invalid("keys must not be empty".into()));
        }
        *self.inner.lock().unwrap() = Some(cdb_core::S3Provider::new(profile, secrets));
        Ok(())
    }

    /// JSON listing for a prefix — shells render it natively.
    pub fn list_json(&self, prefix: String) -> Result<String, CdbFfiError> {
        let guard = self.inner.lock().unwrap();
        let provider = guard.as_ref().ok_or_else(|| CdbFfiError::Invalid("session not configured".into()))?;
        let listing = provider.list(&prefix).map_err(CdbFfiError::Backend)?;
        Ok(listing.to_json(&prefix))
    }

    /// Presigned GET URL (share links / handing a URL to a native player).
    pub fn presign(&self, key: String, expires_secs: u64) -> Result<String, CdbFfiError> {
        let guard = self.inner.lock().unwrap();
        let provider = guard.as_ref().ok_or_else(|| CdbFfiError::Invalid("session not configured".into()))?;
        provider.presign(&key, expires_secs).map_err(CdbFfiError::Backend)
    }
}

#[derive(Debug, uniffi::Error)]
pub enum CdbFfiError {
    Invalid(String),
    Backend(String),
}
impl std::fmt::Display for CdbFfiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CdbFfiError::Invalid(m) => write!(f, "invalid: {}", m),
            CdbFfiError::Backend(m) => write!(f, "backend: {}", m),
        }
    }
}
impl std::error::Error for CdbFfiError {}
