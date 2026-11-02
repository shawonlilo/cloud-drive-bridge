# Cloud Drive Bridge — Mount S3 as a Drive | Free S3 Drive Mounter for Cloudflare R2, Backblaze B2, MinIO & Wasabi 🌉

![Cloud Drive Bridge banner](assets/banner.webp)

[![License: MIT](https://img.shields.io/badge/License-MIT-38BDF8.svg)](LICENSE)
[![Platforms](https://img.shields.io/badge/Platforms-Windows%20·%20macOS%20·%20Linux%20·%20Debian-38BDF8.svg)](#-download)
[![Built with Rust](https://img.shields.io/badge/Built%20with-Rust-081120.svg)](#)
[![Privacy](https://img.shields.io/badge/Privacy-BYOK%20·%20keys%20in%20your%20OS%20keychain%20only-3ED598.svg)](#-security--byok-done-right)

**Cloud Drive Bridge is a free, open-source S3 drive mounter** ✨ — connect **any S3-compatible bucket** and use it like a local drive:

- 🌉 **Mount Amazon S3, Cloudflare R2, Backblaze B2, MinIO, Wasabi** (+ Spaces, Linode, Oracle, IBM COS, Ceph, custom endpoints) as a **drive letter on Windows** or a folder on macOS/Linux
- 📂 **Browse, upload, download, delete, rename** — with checksum-verified uploads
- 🔑 **BYOK done right:** your keys live in your **OS keychain only** — never in a config file, never logged (rclone gets them via environment variables only)
- 🛡️ **Mount watchdog** — auto-remounts with backoff when a mount drops (the fix for rclone's flaky reconnects)
- 🔗 **Share links** with 1h / 24h / 7d expiry, 📌 **offline pins**, ⚡ a researched performance mount profile
- 🆓 **The free Mountain Duck / RaiDrive / ExpanDrive alternative** — and a scriptable, honest **rclone companion CLI**

> Built for makers, homelabbers, founders and teams who keep real work in cheap object storage and are tired of syncing whole buckets or paying $39–$99/year just to see them as a drive. **Use buckets you own or are authorized to access.**

---

> **v0.5.0 — the GUI is back, in V1 Essential Light** (the owner's suite-wide design pick from the approved Stitch set): the same proven engine, now with the warm off-white desktop app + Android browse/transfer app. The CLI below remains the verified core — the GUI wraps it, and the MinIO end-to-end proof still gates every release.

## ✨ Features

| | |
|---|---|
| 🌉 Drive mount | rclone mount with performance profile: VFS cache `full`, 128M buffers/chunks, 4 transfers, 8 checkers, 12h dir-cache (1000h offline mode), 5G cache cap, optional bandwidth limit + read-only |
| 🛡️ Watchdog + auto-mount | Health check every few seconds, auto-remount with 5s→60s backoff; auto-mount on login (Windows Run / macOS LaunchAgent / Linux systemd user unit) |
| 🔑 11 provider presets | AWS S3 · Cloudflare R2 · Backblaze B2 · MinIO · Wasabi · DigitalOcean Spaces · Linode · Oracle · IBM COS · Ceph · Custom |
| 🧪 5-point check | Reachability / auth / list / write / delete — with a probe file that cleans itself up |
| 📂 Pro browser | Instant filter, sort by name/size/date, folder navigation, create-folder, rename, bucket usage stats, storage class |
| ✅ Verified uploads | ETag vs MD5 comparison for single-part uploads; multipart ETags honestly reported as *not* a plain MD5 |
| 🔗 Share links | Presigned URLs (1h / 24h / 7d), expiry always shown |
| 📌 Offline pin | Pin files/folders into the VFS cache so they open instantly and survive short offline periods |
| 🔁 Transfer engine | Folder copy/sync via rclone with retries + backoff, pause/resume (Unix), cancel |
| 🌐 WebDAV fallback | No WinFsp? `rclone serve webdav` on localhost + `net use` — no driver needed |
| 💻 CLI included | `cdb test · ls · get · put · rm · mkdir · rename · size · share · mount` |

## 🖥️ Platforms — the native, no-root way

| Platform | Native no-root way | v0.2 engine (shipping) | Native engine status |
|---|---|---|---|
| Windows | **Cloud Files API (CfAPI)** — OneDrive-style sync root, placeholders, hydration on read. No kernel driver, no admin | WinFsp + rclone mount → drive letter; WebDAV fallback | 🔜 CfAPI provider: design + compilable callback contract in `windows-native/` ([NATIVE.md](NATIVE.md)) |
| macOS | **File Provider extension** — Finder Locations, `~/Library/CloudStorage`, no kext, no reduced security (FSKit volume is the macOS 26 path) | macFUSE + rclone (FSKit backend requested on macOS 15.4+ / macFUSE 5) | 🔜 Design + UniFFI Swift bindings scaffold in `native/macos-fileprovider/` |
| Linux | **Our own Rust FUSE filesystem** — unprivileged via fusermount3, no root | **cdb-fuse (native, read path)** + rclone mount (writable) | ✅ `cdb-fuse` compiles and implements readdir/lookup/read over the core trait; writable FUSE is the first v0.3 item |
| Android | **DocumentsProvider (SAF)** — buckets as roots in every app's file picker, system URI grants, no root | — (no Android build ships; the native core compiles for Android via cdb-ffi) | 🔜 Kotlin provider scaffold in `native/android-documentsprovider/` (not registered, honestly a v0.3 item) |
| iOS | **File Provider extension** — Files app, on-demand download | — | 🔜 Roadmap v0.3 (needs the owner's Apple signing decision; no App Store promise) |
| Android true kernel mount | ❌ Not possible without root | — | ❌ We will never fake it |

### Why native?
No root. No admin. No kernel drivers. No reduced security. One Rust core (`StorageProvider` trait: list · stat · read_range · write · delete · rename · presign) called by every shell — the S3 engine is **in-process (ureq + rustls + hand-rolled SigV4)** and identical on every platform, Android included. The v0.1 subprocess design was killed for exactly the reason network-diagnostics v0.3.2 documented: shelling out is how apps die on mobile. Full architecture: **[NATIVE.md](NATIVE.md)**.

Roadmap beyond the native shells: **[ROADMAP.md](ROADMAP.md)** — rclone crypt vaults, version restore, two-way sync, bandwidth scheduler.

## ⚖️ Cloud Drive Bridge vs the alternatives

| | Cloud Drive Bridge | Mountain Duck / RaiDrive / ExpanDrive | rclone CLI |
|---|---|---|---|
| Price | **Free forever (MIT)** | $39–$99 / year or license | Free |
| Interface | ✅ GUI (V1 Essential Light) + scriptable CLI | ✅ GUI | ❌ command line only |
| Source code | ✅ open | ❌ closed | ✅ open |
| Keys storage | **OS keychain; rclone via env only** | App config | Plain-text config file |
| Watchdog auto-remount | ✅ | ❌ | ❌ (manual remount) |
| Windows no-driver fallback | ✅ WebDAV mode | ❌ | ❌ |
| Android app | ❌ (native DocumentsProvider is roadmap, not shipped) | ❌ | ⚠️ terminal only |

## 📥 Download

**Desktop app (V1 Essential Light GUI):**

| Platform | File |
|---|---|
| Windows (x64) | `Cloud.Drive.Bridge_0.5.0_x64-setup.exe` or `Cloud.Drive.Bridge_0.5.0_x64_en-US.msi` |
| macOS (Apple Silicon) | `Cloud.Drive.Bridge_0.5.0_aarch64.dmg` |
| macOS (Intel) | `Cloud.Drive.Bridge_0.5.0_x64.dmg` |
| Linux | `Cloud.Drive.Bridge_0.5.0_amd64.AppImage` or `Cloud.Drive.Bridge_0.5.0_amd64.deb` |
| Android | `cloud-drive-bridge-android.apk` (browse + transfer, CI-signed, sideload) |

**CLI (`cdb`)** — grab the binary for your platform, make it executable (`chmod +x`), and run it:

| Platform | File |
|---|---|
| Windows (x64) | `cdb-windows-x64.exe` |
| macOS (Apple Silicon) | `cdb-macos-arm64` |
| macOS (Intel) | `cdb-macos-x64` |
| Linux (x64) | `cdb-linux-x64` |
| Linux native FUSE engine | `cdb-fuse-linux-x64` |
| Checksums | `SHA256SUMS.txt` |

Prerequisites for **mounting only**: **rclone** (free, rclone.org/downloads) + your platform driver — **WinFsp** (Windows), **macFUSE** (macOS), **fuse3** (Linux). Browsing, transfers, tests and share links need nothing extra — the native S3 client is built in. On Linux you can also mount with our own `cdb-fuse` (read path) and skip rclone entirely.

### ✅ Proven end-to-end, not just unit-tested
[`scripts/e2e-minio.sh`](scripts/e2e-minio.sh) runs the real CLI against a real local S3 server (MinIO when present — its upstream binary distribution was withdrawn in 2025 — moto otherwise) and asserts every step: **test → put (checksum verified) → ls → get (byte-compare) → rename → share link (actually fetched, byte-compare) → size → rm**. It runs in CI on every release:

```
E2E RESULT: ALL STEPS PASS ✅
```

## 🚀 Quick start

1. Set your keys (env vars, or save them via a profile + `--from-keychain`): `export CDB_ACCESS_KEY_ID=... CDB_SECRET_ACCESS_KEY=...`
2. **Test:** `cdb test --bucket my-bucket --provider r2 --endpoint https://<acct>.r2.cloudflarestorage.com` (add `--probe` for the write/delete check).
3. **Use it:** `cdb ls / put / get / share` — or **mount:** install rclone + your platform driver, then `cdb mount X: --bucket my-bucket ...` and your bucket is a drive. 🌉

## 💻 CLI usage

```bash
export CDB_ACCESS_KEY_ID=…        # your own keys — or use --from-keychain
export CDB_SECRET_ACCESS_KEY=…

cdb providers
cdb test  --bucket my-bucket --provider r2 --endpoint https://<acct>.r2.cloudflarestorage.com --probe
cdb ls    --bucket my-bucket --provider aws --region eu-west-1  photos/
cdb put   ./report.pdf reports/report.pdf --bucket my-bucket --provider aws
cdb share reports/report.pdf 24h  --bucket my-bucket --provider aws
cdb mount S: --bucket my-bucket --provider aws --cache 10G --offline
```

## 🔒 Security — BYOK done right

- **Your keys, your machine.** Secrets are stored in the OS keychain (Windows Credential Manager / macOS Keychain / Linux key store). On mobile v0.1 they are kept **in memory only**.
- **No credential files, ever.** rclone is configured exclusively through `RCLONE_CONFIG_<NAME>_*` environment variables; S3 calls run in-process over TLS (rustls) with SigV4 signatures computed in the app. Nothing secret is written to disk or logged — the `Secrets` type can't even print itself.
- **Least privilege:** create a bucket-scoped IAM key for this app. The 5-point write probe is opt-in and deletes its probe file.
- Rebuilt from the author's original Python script with a full pre-publish credential sweep (zero hits).

## 🏗️ How it works

```
Profile (bucket + endpoint) ─► keys from OS keychain
        │
        ├─► Browse/transfer/test/share: cdb-core ─► native client (ureq + rustls + SigV4) ─► S3 API
        │
        └─► Mount: rclone mount (env-only config) ─► WinFsp / macFUSE / FUSE3 ─► drive letter / folder
                    ▲ watchdog: health check ─► auto-remount with backoff
                    ▲ auto-mount: Windows Run · macOS LaunchAgent · Linux systemd user unit
```

## 🙏 Thanks

rclone, WinFsp, macFUSE, FUSE/fuser, UniFFI, Tauri, Rust — plus RSAF, CloudMount, fskit-s3, Mountit, Nextcloud's DocumentsProvider, mountpoint-s3, goofys and s3fs-fuse as research precedents. Full credits: **[THANKS.md](THANKS.md)**.

## 📄 License

MIT — free for everyone, forever. Built by [Shawon / Cyberspace Work](https://github.com/shawonlilo) as part of the OpenTools suite (Network Diagnostics · CleanReach · Cloud Drive Bridge · Opt-in Messaging).

<sub>Keywords: mount S3 as drive · S3 drive mounter · cloud drive bridge · mount Cloudflare R2 as drive · Backblaze B2 drive · MinIO mount · Wasabi drive · free Mountain Duck alternative · RaiDrive alternative · ExpanDrive alternative · rclone companion CLI · S3 browser · mount object storage Windows macOS Linux Android</sub>
