# Roadmap — how Cloud Drive Bridge becomes available *everywhere*, honestly

Three states, used everywhere in this project: **✅ Shipped** (you can
download and use it in v0.1.0) · **🔜 Roadmap** (designed, proven by
someone else's working code, not built yet) · **❌ Not possible** (physics
or platform rules — we will never fake it).

| Platform | v0.1.0 | Path |
|---|---|---|
| Windows | ✅ WinFsp + rclone mount → drive letter. WebDAV fallback (localhost `rclone serve webdav` + `net use`) when WinFsp is missing | — |
| macOS | ✅ macFUSE mount. On macOS 15.4+ with macFUSE 5 we request the **FSKit backend** (`-o backend=fskit`, no legacy kext); older setups use the classic backend with a one-time system-extension approval | 🔜 v0.3 native FSKit volume (Rust, fskit-s3 precedent) or File Provider extension (Mountit precedent: no kext, no reduced security, Finder Locations + `~/Library/CloudStorage`) |
| Linux | ✅ FUSE3 native mount — the best-supported platform; systemd user unit for auto-mount | — |
| Android | ✅ Tauri app: browse + transfer. Transfers are resumable with retry/backoff. Secrets kept in memory in v0.1 (never on disk) | 🔜 v0.2 **SAF DocumentsProvider** (Kotlin module in the Tauri-generated Android project): buckets appear in the system file picker for *every* app via the Storage Access Framework — the real no-root "available everywhere" path. Proven by **RSAF** (rclone + SAF + VFS cache) and CloudMount · 🔜 Android Keystore storage · 🔜 foreground-service transfers |
| iOS | ❌ Not in v0.1 — no honest build exists yet | 🔜 v0.3 Swift `NSFileProviderReplicatedExtension` sharing the Rust core as a staticlib: buckets in the Files app with on-demand download (Mountit / unraid-drive pattern). Needs an Xcode/macOS build + the owner's Apple signing decision. **No App Store promise.** |
| Android true kernel mount | ❌ **Not possible without root** | — |

## v0.2

- Android SAF DocumentsProvider (Kotlin) — see above.
- **rclone crypt vaults:** client-side-encrypted profiles (zero-knowledge,
  Cryptomator-style) for buckets you don't fully trust.
- **Version restore:** browse and restore previous object versions on
  versioning-enabled buckets.
- **Sync mode:** two-way folder sync with conflict copies (rclone bisync).
- **Bandwidth scheduler:** mount/transfer bandwidth limits by time of day.
- **Native S3 transport for Android** (in-process SigV4 + HTTPS) so the
  Android app no longer depends on a system curl.

## v0.3

- macOS native volume: FSKit (Rust) or File Provider extension.
- iOS File Provider extension (signing decision required).
- Slim, properly signed Android release APK (v0.1 CI APK uses an
  ephemeral CI key; cross-version installs may need uninstall-first).

## Design notes

- The watchdog pattern (health check + auto-remount with backoff) is the
  permanent answer to rclone's flaky reconnect-after-sleep behaviour.
- Auto-mount hooks are generated per OS by the app and removed when the
  toggle is switched off: Windows `HKCU\...\Run`, macOS LaunchAgent
  plist, Linux systemd user unit.
- rclone is always configured through `RCLONE_CONFIG_<NAME>_*`
  environment variables. A credential file is never written, on any
  platform, in any version.
