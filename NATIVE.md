# NATIVE.md — the no-root, native way

Owner directive: *"Not root way — native way."* Every platform gets the
integration its OS actually offers to well-behaved apps: **no kernel
drivers, no admin rights, no reduced security, no root** — and files
appear where the OS expects them.

## The one core

```
                ┌────────────────────────────────────────────┐
                │ cdb-core (Rust)                            │
                │  native S3 client: ureq + rustls + SigV4   │
                │  trait StorageProvider {                   │
                │    list · stat · read_range · write ·      │
                │    delete · rename · presign }             │
                └───────▲────────▲────────▲────────▲─────────┘
                        │        │        │        │
              Tauri app │  cdb-ffi (UniFFI)  │  cdb-fuse (Linux)
                        │   ├─ Kotlin (Android)      │
                        │   └─ Swift (Apple)         │
                        └─ cdb-windows-native (CfAPI contract)
```

- **Native HTTP, in-process.** Up to v0.1.0 the S3 engine shelled out to
  `curl --aws-sigv4`. That is dead on Android (no curl, no PowerShell —
  the exact failure network-diagnostics hit and fixed in its v0.3.2)
  and it never felt native. From v0.2.0 the client is **ureq + rustls
  with hand-rolled SigV4 signing**, verified against the AWS SigV4
  documentation test vector and a live local-HTTP integration test.
  Identical code on Windows, macOS, Linux and Android.
- **rclone is demoted** to what it's good at: the *optional desktop
  mount engine* (and WebDAV fallback) on Windows/macOS/Linux. It is
  never used for browse, transfer, test or share links anymore — share
  links are native SigV4 presigned URLs now.

## Per platform

### Windows — Cloud Files API (CfAPI / Cloud Filter)
- **The OneDrive way:** per-user sync-root registration, placeholder
  files, hydration on read via the `FETCH_DATA` callback into
  `StorageProvider::read_range`. User-space only: **no kernel driver,
  no admin, no reboot.**
- Rust wraps CfAPI via the `cloud-filter` crate.
- **v0.2 state: design + compilable contract.** The `windows-native/`
  crate defines `CloudFilesSource` (children / fetch_data / stat) over
  the core trait and the sync-root identity scheme. The shipping
  Windows mount remains WinFsp + rclone until the CfAPI provider is
  implemented against that contract. The app itself never requires
  admin in either path.

### macOS + iOS — File Provider extension
- **The native way:** a Swift File Provider extension (replicated on
  iOS, non-replicated acceptable on macOS) calling cdb-core as a
  staticlib through the UniFFI Swift bindings from `ffi/`. Buckets
  appear in **Finder > Locations** and `~/Library/CloudStorage`
  (macOS) and in the **Files app** (iOS), with on-demand download —
  **no kext, no reduced security.**
- The macOS 26 path beyond File Provider is an **FSKit volume**
  (fskit-s3 proves a Rust S3 FSKit volume is possible).
- **v0.2 state: design + bindings.** `native/macos-fileprovider/`
  documents the extension structure; the UniFFI Swift bindings
  generate from `cdb-ffi`. The shipping macOS mount remains
  macFUSE + rclone. An Apple build needs Xcode and the owner's Apple
  signing decision — **no App Store promise is made.**

### Android — DocumentsProvider (Storage Access Framework)
- **The native way:** a Kotlin `DocumentsProvider` calling cdb-core
  through the UniFFI Kotlin bindings from `ffi/`. Buckets appear as
  **roots in the system file picker for every app**, with
  system-mediated URI grants — the official API "designed for durable
  local or cloud documents", and **no root** (RSAF proves the
  rclone+SAF shape; ours swaps rclone for the native core). An
  offline-mirror folder mode covers apps that need real local files.
- **v0.2 state: scaffold, honestly NOT shipped.** The Kotlin provider
  class lives in `native/android-documentsprovider/` with its manifest
  registration commented out and every method wired to TODO UniFFI
  calls. It is the **first v0.3 item** because wiring generated
  Android code by hand is exactly how releases break; the shipped
  Android app remains the Tauri browse+transfer app — which now works
  fully, because its engine is the native client, not curl.

### Linux — our own FUSE filesystem (shipped engine, read path)
- **The native way:** `fuse/` — `cdb-fuse`, a Rust FUSE filesystem on
  the `fuser` crate implementing the `StorageProvider` trait directly
  against S3: readdir / lookup / getattr / read via signed range
  requests. Unprivileged mounts through `fusermount3` — **no root,
  no rclone.**
- **v0.2 state: compiles; read path implemented.** Writable FUSE is
  the first v0.3 item (the rclone mount stays the writable path until
  then). Mount verification needs a machine with `/dev/fuse` — the
  build sandbox has none, so no local mount is claimed here; the
  crate is compile-verified and the same provider code is exercised
  by the core's integration tests.

## Why native?

| | Native way | Old way |
|---|---|---|
| Rights | per-user, no admin, no root | drivers / admin installs |
| Security posture | platform-blessed APIs, no kext, no reduced security | kernel extensions, FUSE drivers per OS |
| File placement | Explorer sync root · Finder Locations · Files picker · Linux mount | a drive that exists only while a helper process behaves |
| Engine | one Rust core, every platform | per-platform subprocess patchwork |
