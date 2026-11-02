//! Windows NATIVE WAY — Cloud Files API (CfAPI / Cloud Filter).
//!
//! This is how OneDrive integrates with Explorer: a per-user sync root
//! registration, placeholder files, and hydration on read. User-space
//! only — no kernel driver, no admin rights, no reboot.
//!
//! v0.2 state: **design + compilable callback contract** (this crate).
//! The shipping Windows mount in v0.2 remains WinFsp + rclone while the
//! CfAPI provider is built against this contract. Nothing in this crate
//! pretends to register a sync root yet — see native/windows-cfapi/
//! README.md for the full design.
//!
//! Intended wiring on Windows (feature `cfapi`, crate `cloud-filter`):
//!   CfRegisterSyncRoot(per-user) -> placeholder tree from
//!   StorageProvider::list -> FETCH_DATA callback ->
//!   StorageProvider::read_range -> CfExecute transfer.
//! Placeholders dehydrate under the normal cache policy; files appear
//! in Explorer exactly where the OS expects them.

use cdb_core::{S3Object, StorageProvider};
use std::sync::Arc;

/// What the CfAPI shell needs from the core. Implemented by wrapping a
/// `cdb_core::S3Provider`; kept as a trait so the Windows shell never
/// touches S3 types directly.
pub trait CloudFilesSource: Send + Sync {
    /// Children of a folder placeholder ("" = sync root).
    fn children(&self, prefix: &str) -> Result<Vec<S3Object>, String>;
    /// Hydration: bytes for [offset, offset+len) of one file.
    fn fetch_data(&self, key: &str, offset: u64, len: u64) -> Result<Vec<u8>, String>;
    /// File metadata for a placeholder.
    fn stat(&self, key: &str) -> Result<S3Object, String>;
}

pub struct CoreCloudFilesSource<P: StorageProvider> {
    pub provider: Arc<P>,
}
impl<P: StorageProvider + Send + Sync> CloudFilesSource for CoreCloudFilesSource<P> {
    fn children(&self, prefix: &str) -> Result<Vec<S3Object>, String> {
        let l = self.provider.list(prefix)?;
        let mut out = l.folders;
        out.extend(l.files);
        Ok(out)
    }
    fn fetch_data(&self, key: &str, offset: u64, len: u64) -> Result<Vec<u8>, String> {
        self.provider.read_range(key, offset, len)
    }
    fn stat(&self, key: &str) -> Result<S3Object, String> {
        self.provider.stat(key)
    }
}

/// Sync-root identity the CfAPI registration will use (per-user).
pub fn sync_root_id(profile_name: &str) -> String {
    format!("CloudDriveBridge!User!{}", profile_name)
}

/// Placeholder policy: files hydrate on read, folders are always
/// materialized, and the OS may dehydrate under disk pressure.
pub fn placeholder_policy() -> &'static str {
    "on-demand hydration; full folder placeholders; OS-managed dehydration"
}
