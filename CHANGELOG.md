# Changelog

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
