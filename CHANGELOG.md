# Changelog

## 0.5.0 — 2026-11-02 — GUI returns in V1 Essential Light 🤍

- **Owner pick executed:** the desktop GUI is back, restyled to the approved **V1 Essential Light** design from the Stitch set (project 12348950776923825852): warm off-white #FAF9F6 surfaces, white cards with soft depth, deep ink #0F172A, sky accent #0284C7, teal success — sidebar layout per the Stitch V1 screen. Same screens, same commands, same proven engine; only the design layer changed. No Stitch mock claims shipped (no invented SOC2/audit/kill-switch copy).
- The GUI wraps cdb-core unchanged: profiles with OS-keychain keys, bucket browser, mount controls (no-root paths), share links, watchdog status.
- Android app (browse + transfer on the native client) returns via the CI lane; the APK is listed only because it is verified on this release page.
- Everything from Phase A stands: real-MinIO e2e green, 19/19 unit tests, CLI x4 + cdb-fuse + SHA256SUMS.txt.

## 0.4.1 — 2026-11-02

- Fix: the v0.4.0 workflow's macOS-x64 matrix patch silently failed to apply (string mismatch against the real job text), so no `cdb-macos-x64` was built. Matrix fixed against the actual file; this release's factory produces all four CLI binaries + cdb-fuse + checksums.

## 0.4.0 — 2026-11-02 — CLI proven against REAL MinIO + asset expansion

- **Real-MinIO end-to-end (not a mock):** MinIO's official binaries were withdrawn upstream (dl.min.io returns 410), so MinIO was built from source locally and the full flow ran against it: 5-point probe (write/delete true) → put (checksum verified) → ls → get (byte-identical) → rename → share (presigned URL really fetched, byte-identical) → size → rm → folder empty. **All steps pass** (transcript kept with the project records; `scripts/e2e-minio.sh` reproduces it — it prefers a real `minio` binary, moto is only the fallback).
- **Two real SigV4 bugs found by MinIO and fixed** (AWS/moto tolerated both): (1) the `delimiter` query value must be %2F-encoded in the canonical query; (2) query values (prefix, continuation-token) must use a strict query encoder — `encode_key` keeps `/` for object paths, which is wrong for query strings. New `encode_query_value` + regression tests (19/19 unit tests).
- **GUI moved to `gui-experimental/`** with an explicit "experimental, not in this release, not verified" note; out of the workspace and out of CI.
- **Assets expanded:** macOS Intel CLI (`cdb-macos-x64`), `cdb-fuse-linux-x64` (native Linux engine binary), and `SHA256SUMS.txt` generated from the actual release assets.
- cdb-fuse mount smoke in the build sandbox: honestly blocked — the VM has no /dev/fuse and no fusermount; the binary builds and reaches the mount call. Mount verification needs a real Linux host.

## 0.3.0 — 2026-11-02 — CLI-FIRST (owner correction: "fix it, no gui")

- **GUI dropped from the shipment.** The Tauri app is no longer built or distributed; `app/` stays in-tree as an unmaintained experiment. What ships: the `cdb` CLI binaries (Windows/macOS/Linux) — nothing else.
- **End-to-end proof, in CI and locally:** `scripts/e2e-minio.sh` runs the real CLI against a real local S3 server (MinIO when present — upstream withdrew its binary distribution in 2025 — moto otherwise) asserting test → put (checksum verified) → ls → get (byte-compare) → rename → share link (actually fetched, byte-compare) → size → rm. **All steps pass.**
- **Real bug found by the e2e and fixed:** the CLI rejected flags placed before the command (`cdb --bucket B test`). Flags now parse in any position.
- Native engine unchanged from 0.2.x: in-process ureq + rustls + SigV4, StorageProvider trait, cdb-ffi, cdb-fuse, rclone only for desktop mounts.

## 0.2.1 — 2026-11-02

- Fix: v0.2.0's desktop bundle filenames carried the old 0.1.0 version string (a minified tauri.conf.json escaped the version bump). Same native code as 0.2.0, correctly stamped. Supersedes v0.2.0.

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
