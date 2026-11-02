# Windows native shell — Cloud Files API (CfAPI)

Design for the `cdb-windows-native` crate (see `../../windows-native/`).

## Registration (per user, no admin)
- `CfRegisterSyncRoot` with sync root id
  `CloudDriveBridge!User!<profile>` (see `sync_root_id()` in the crate),
  provider name "Cloud Drive Bridge", per-user scope.
- Sync root folder: `%USERPROFILE%\CloudDriveBridge\<profile>\`.

## Placeholder tree
- On registration and on folder expansion, children come from
  `CloudFilesSource::children(prefix)` → `StorageProvider::list`.
- Files are created as placeholders (`CfCreatePlaceholders`) with the
  real size in metadata; folders are full placeholders.

## Hydration (FETCH_DATA)
- Explorer/app reads a placeholder → CfAPI `FETCH_DATA` callback →
  `CloudFilesSource::fetch_data(key, offset, len)` →
  `StorageProvider::read_range` (signed Range GET) → `CfExecute`
  transfer. Progress + cancellation map to the ureq request lifecycle.

## Policies
- Hydration: on-demand. Dehydration: OS default (Files On-Demand
  behaviour) + pin = always-hydrate attribute forOffline pins.
- Write-back (create/modify inside the sync root → upload) is a v0.3
  item; v0.2 CfAPI target is read + hydrate, matching cdb-fuse.

## Status
Contract crate compiles on every platform (CfAPI calls are behind the
Windows-only `cfapi` feature using the `cloud-filter` crate). The
shipping Windows mount in v0.2 is WinFsp + rclone. This folder is a
design + contract, NOT a shipped shell.
