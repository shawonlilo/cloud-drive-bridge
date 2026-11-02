# Android native shell — DocumentsProvider (SAF)

## Why
The Storage Access Framework is Android's official API "designed for
durable local or cloud documents": a `DocumentsProvider` exposes roots
and documents to the **system file picker of every app**, with
system-mediated URI grants and **no root**. RSAF (chenxiaolong) proves
a cloud provider over this API end-to-end; Cloud Drive Bridge's version
calls the native Rust core (via UniFFI Kotlin bindings from
`../../ffi/`) instead of rclone.

## Files here
- `CdbDocumentsProvider.kt` — the provider scaffold: roots = saved
  profiles, documents = bucket listings, `openDocument` streams via a
  presigned URL / pipe. Every UniFFI call is marked `TODO(ffi)` and the
  class is **not registered** (see the commented manifest snippet) —
  this is a scaffold for v0.3, NOT shipped functionality.

## Manifest registration (when v0.3 wires it — currently commented)
```xml
<!-- <provider
    android:name=".CdbDocumentsProvider"
    android:authorities="org.opentools.clouddrivebridge.documents"
    android:exported="true"
    android:grantUriPermissions="true"
    android:permission="android.permission.MANAGE_DOCUMENTS">
    <intent-filter>
        <action android:name="android.content.action.DOCUMENTS_PROVIDER" />
    </intent-filter>
</provider> -->
```

## Also in v0.3 scope
- Android Keystore-backed secret storage feeding `CdbSession.configure`.
- Foreground-service transfers for large copies started from the picker.
- Offline-mirror folder mode: a real local folder kept in sync for apps
  that cannot consume content URIs.
