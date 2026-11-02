# Changelog

## 0.2.0 — 2026-11-02 — NATIVE release 🌉

The "not root way — native way" release:

- **Native S3 client, in-process:** ureq + rustls + hand-rolled AWS SigV4 (verified against the AWS documentation signing vector + a live local-HTTP integration test). The v0.1 `curl --aws-sigv4` subprocess engine is **gone** — it was dead on Android, exactly as network-diagnostics v0.3.2 documented. One client now runs identically on Windows, macOS, Linux and Android.
- **`StorageProvider` trait** in cdb-core (list · stat · read_range · write · delete · rename · presign) — every shell calls the one core; share links are native SigV4 presigned URLs (no rclone).
- **`cdb-ffi`** — UniFFI crate generating Kotlin + Swift bindings (`CdbSession`) over the core; compiles for the host target.
- **`cdb-fuse`** — our own Linux FUSE filesystem (fuser) implementing the trait: readdir/lookup/getattr/read via signed range requests, unprivileged via fusermount3. Compiles; read path. Writable FUSE = first v0.3 item; rclone remains the writable mount.
- **`cdb-windows-native`** — CfAPI (Cloud Files API) design + compilable callback contract (sync-root identity, FETCH_DATA contract); `native/` holds the CfAPI, File Provider and Android DocumentsProvider designs, incl. the Kotlin SAF provider scaffold (not registered — v0.3).
- **NATIVE.md** — the full no-root architecture: CfAPI (Windows), File Provider (Apple), DocumentsProvider (Android), FUSE (Linux).

## 0.1.0 — 2026-11-02 — First release 🌉

**Cloud Drive Bridge** — mount any S3-compatible bucket as a local drive, free and open source.

- **Providers (11 presets):** Amazon S3, Cloudflare R2, Backblaze B2, MinIO, Wasabi, DigitalOcean Spaces, Linode Object Storage, Oracle Object Storage, IBM COS, Ceph, Custom endpoint.
- **BYOK security:** your own keys, stored in the OS keychain on desktop (memory-only on mobile v0.1); keys are never written to a config file — rclone receives them through environment variables only — and never logged.
- **Browser:** folder navigation, instant filter, sort by name/size/date, upload with ETag checksum verification (honest multipart note), download, delete, create folder, rename, bucket usage stats, storage-class display.
- **5-point connection check:** reachability / auth / list / write / delete with a self-cleaning probe object.
- **Mount (desktop):** researched performance profile — VFS cache `full`, 128M buffers and read chunks, 4 transfers, 8 checkers, 12h directory cache (1000h in Offline mode), user-settable cache cap (default 5G) and bandwidth limit, read-only toggle.
- **Mount watchdog:** auto-remount with exponential backoff when the mount dies or stops responding; auto-mount on login (Windows Run entry / macOS LaunchAgent / Linux systemd user unit).
- **Share links:** presigned URLs via `rclone link` with 1h / 24h / 7d expiry.
- **Offline pin:** warm pinned files/folders into the VFS cache (or the app offline cache), managed by the app.
- **Transfers:** rclone folder copy/sync with retries + backoff, pause/resume (Unix) and cancel.
- **Windows WebDAV fallback** when WinFsp is missing: `rclone serve webdav` on localhost + `net use` mapping, with honest limits.
- **CLI (`cdb`)** with the same engine: `test / ls / get / put / rm / mkdir / rename / size / share / mount / unmount / providers`.
- **Android:** browse + transfer app (no fake mount). iOS: roadmap only.
- Rebuilt natively in Rust + Tauri 2 from Shawon's original Python/Tkinter `s3mount2nd.py` rclone GUI.
