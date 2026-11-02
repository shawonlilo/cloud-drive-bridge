# Thanks 🙏

Cloud Drive Bridge stands on excellent open work. Full credit, always:

| Source | What we learned / use | License |
|---|---|---|
| [rclone](https://github.com/rclone/rclone) | The mount & transfer engine — configured via environment only, never a credential file | MIT |
| [WinFsp](https://github.com/winfsp/winfsp) | Windows filesystem driver that makes drive-letter mounts possible | GPLv3 (driver, installed separately) |
| [macFUSE](https://github.com/macfuse/macfuse) | macOS filesystem support, incl. the macFUSE 5 FSKit backend | BSD-style |
| [FUSE / libfuse](https://github.com/libfuse/libfuse) | Linux userspace filesystems (FUSE3) | LGPL/GPL (system component) |
| [RSAF](https://github.com/chenxiaolong/RSAF) by chenxiaolong | Proof that rclone + Android SAF DocumentsProvider + VFS cache works — our Android v0.2 blueprint | GPLv3 (reference only) |
| CloudMount | Concept proof for Android cloud mounting UX | (reference only) |
| [fskit-s3](https://github.com/) | Proof a Rust S3 FSKit volume works on modern macOS — our v0.3 blueprint | (reference only) |
| Mountit | File Provider extension precedent on Apple platforms (no kext, Finder Locations) | (reference only) |
| [mountpoint-s3](https://github.com/awslabs/mountpoint-s3) by AWS | S3-as-filesystem semantics research | Apache-2.0 (reference only) |
| [goofys](https://github.com/kahing/goofys) | High-performance S3 filesystem design lessons | Apache-2.0 (reference only) |
| [s3fs-fuse](https://github.com/s3fs-fuse/s3fs-fuse) | The original S3 FUSE filesystem — a decade of edge cases documented | GPLv2 (reference only) |
| [curl](https://github.com/curl/curl) | Our S3 transport (`--aws-sigv4`) — ships on Windows 10+, macOS, Linux | curl license |
| [Tauri](https://github.com/tauri-apps/tauri) | The app shell for desktop + Android | MIT/Apache-2.0 |
| [Rust](https://www.rust-lang.org/) | The language the whole engine is written in | MIT/Apache-2.0 |
| taste-skill by Leonxlnx | Design direction system | (pattern library) |
| Hallmark by Nutlope | Anti-slop design patterns | (pattern library) |
| anti-slop by miqdadbadjuber · no-ai-slop by petergyang | Copy/design honesty patterns | (pattern library) |
| aislop by scanaislop | CI slop-scan (advisory gate) | (tooling) |
| **Shawon's original `s3mount2nd.py`** | The seed: a Tkinter rclone S3 drive creator with profiles, drive letters, mount options and a bucket browser — rebuilt natively as this project | MIT (this repo) |

## Friends of the project

_Reserved — Shawon's own repo names land here on his word._
