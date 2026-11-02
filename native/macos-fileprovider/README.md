# Apple native shell — File Provider extension (macOS + iOS)

## Shape
- App extension target (Swift) implementing `NSFileProviderReplicatedExtension`
  (iOS + macOS; non-replicated variant acceptable for a macOS-only step).
- The extension links `libcdb_ffi` (staticlib from `../../ffi/`) and the
  UniFFI-generated Swift bindings: `CdbSession.configure(...)`,
  `list_json(prefix)`, `presign(key, expires)`.
- Enumerator maps `list_json` results to `NSFileProviderItem`s;
  `fetchContents` streams via presigned URL or a ranged read added to
  the FFI surface when the extension lands.

## Where files appear
- macOS: Finder sidebar > Locations, `~/Library/CloudStorage/<domain>`.
- iOS: Files app > Locations.
- No kernel extension, no reduced security, no admin.

## Prerequisites (owner decisions, not code)
- Xcode/macOS build machine or macOS CI runner.
- Apple Developer signing identity — the owner's call, later.
  No App Store promise is made anywhere in this project.

## Status
Design + bindings scaffold (v0.2). The shipping macOS mount in v0.2 is
macFUSE + rclone. The FSKit volume path (macOS 26, fskit-s3 precedent)
is the documented alternative if File Provider semantics fall short.
