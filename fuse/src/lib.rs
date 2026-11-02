//! cdb-fuse — a read-path FUSE filesystem over cdb-core's
//! StorageProvider trait. Linux native way: unprivileged mount via
//! fusermount3, no root, no rclone in the loop.
//!
//! v0.2 scope (honest): readdir + lookup + getattr + open/read.
//! Writes are NOT implemented yet — the filesystem reports itself
//! read-only-capable and write calls return EROFS. Writable FUSE is
//! the first v0.3 item; the rclone mount remains the writable path
//! until then.

use cdb_core::{S3Object, StorageProvider};
use fuser::{
    FileAttr, FileType, Filesystem, ReplyAttr, ReplyData, ReplyDirectory, ReplyEntry, Request,
};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::time::{Duration, UNIX_EPOCH};

const TTL: Duration = Duration::from_secs(5);
const ROOT_INO: u64 = 1;

struct Node {
    obj: S3Object,
    parent_prefix: String,
}

pub struct S3Fuse<P: StorageProvider> {
    provider: P,
    nodes: HashMap<u64, Node>,
    by_key: HashMap<String, u64>,
    listed: HashMap<String, Vec<u64>>,
    next_ino: u64,
}

impl<P: StorageProvider> S3Fuse<P> {
    pub fn new(provider: P) -> S3Fuse<P> {
        let mut fs = S3Fuse {
            provider,
            nodes: HashMap::new(),
            by_key: HashMap::new(),
            listed: HashMap::new(),
            next_ino: 2,
        };
        fs.nodes.insert(ROOT_INO, Node {
            obj: S3Object {
                key: String::new(), size: 0, last_modified: String::new(),
                etag: String::new(), storage_class: String::new(), is_folder: true,
            },
            parent_prefix: String::new(),
        });
        fs.by_key.insert(String::new(), ROOT_INO);
        fs
    }

    fn prefix_of(&self, ino: u64) -> Option<String> {
        self.nodes.get(&ino).map(|n| {
            if n.obj.is_folder {
                let k = &n.obj.key;
                if k.is_empty() || k.ends_with('/') { k.clone() } else { format!("{}/", k) }
            } else {
                n.parent_prefix.clone()
            }
        })
    }

    fn ensure_listed(&mut self, prefix: &str) -> Result<Vec<u64>, i32> {
        if let Some(v) = self.listed.get(prefix) {
            return Ok(v.clone());
        }
        let listing = self.provider.list(prefix).map_err(|_| libc::EIO)?;
        let mut inos = Vec::new();
        let mut all: Vec<S3Object> = listing.folders;
        all.extend(listing.files);
        for obj in all {
            let ino = if let Some(i) = self.by_key.get(&obj.key) {
                *i
            } else {
                let i = self.next_ino;
                self.next_ino += 1;
                self.by_key.insert(obj.key.clone(), i);
                self.nodes.insert(i, Node { obj: obj.clone(), parent_prefix: prefix.to_string() });
                i
            };
            inos.push(ino);
        }
        self.listed.insert(prefix.to_string(), inos.clone());
        Ok(inos)
    }

    fn attr(&self, ino: u64) -> Option<FileAttr> {
        let n = self.nodes.get(&ino)?;
        let (kind, perm, size) = if n.obj.is_folder {
            (FileType::Directory, 0o755, 0)
        } else {
            (FileType::RegularFile, 0o444, n.obj.size)
        };
        Some(FileAttr {
            ino,
            size,
            blocks: size / 512 + 1,
            atime: UNIX_EPOCH,
            mtime: UNIX_EPOCH,
            ctime: UNIX_EPOCH,
            crtime: UNIX_EPOCH,
            kind,
            perm,
            nlink: if n.obj.is_folder { 2 } else { 1 },
            uid: 0,
            gid: 0,
            rdev: 0,
            blksize: 4096,
            flags: 0,
        })
    }
}

impl<P: StorageProvider + Send> Filesystem for S3Fuse<P> {
    fn lookup(&mut self, _req: &Request<'_>, parent: u64, name: &OsStr, reply: ReplyEntry) {
        let Some(prefix) = self.prefix_of(parent) else { reply.error(libc::ENOENT); return; };
        let Ok(inos) = self.ensure_listed(&prefix) else { reply.error(libc::EIO); return; };
        let want = name.to_string_lossy().to_string();
        for ino in inos {
            if let Some(node) = self.nodes.get(&ino) {
                let disp = node.obj.display_name(&prefix);
                if disp == want {
                    if let Some(attr) = self.attr(ino) {
                        reply.entry(&TTL, &attr, 0);
                        return;
                    }
                }
            }
        }
        reply.error(libc::ENOENT);
    }

    fn getattr(&mut self, _req: &Request<'_>, ino: u64, _fh: Option<u64>, reply: ReplyAttr) {
        // Prime the root listing so children resolve.
        if ino == ROOT_INO {
            let _ = self.ensure_listed("");
        }
        match self.attr(ino) {
            Some(attr) => reply.attr(&TTL, &attr),
            None => reply.error(libc::ENOENT),
        }
    }

    fn readdir(&mut self, _req: &Request<'_>, ino: u64, _fh: u64, offset: i64, mut reply: ReplyDirectory) {
        let Some(prefix) = self.prefix_of(ino) else { reply.error(libc::ENOENT); return; };
        let Ok(inos) = self.ensure_listed(&prefix) else { reply.error(libc::EIO); return; };
        let mut entries: Vec<(u64, FileType, String)> = vec![
            (ino, FileType::Directory, ".".into()),
            (ROOT_INO, FileType::Directory, "..".into()),
        ];
        for child in inos {
            if let Some(node) = self.nodes.get(&child) {
                let kind = if node.obj.is_folder { FileType::Directory } else { FileType::RegularFile };
                entries.push((child, kind, node.obj.display_name(&prefix)));
            }
        }
        for (i, (cino, kind, name)) in entries.iter().enumerate().skip(offset as usize) {
            if reply.add(*cino, (i + 1) as i64, *kind, name) {
                break;
            }
        }
        reply.ok();
    }

    fn open(&mut self, _req: &Request<'_>, ino: u64, flags: i32, reply: fuser::ReplyOpen) {
        if self.nodes.get(&ino).map(|n| n.obj.is_folder).unwrap_or(true) {
            reply.error(libc::EISDIR);
            return;
        }
        // Writes are out of scope for the v0.2 read path.
        if flags & libc::O_ACCMODE != libc::O_RDONLY {
            reply.error(libc::EROFS);
            return;
        }
        reply.opened(0, 0);
    }

    fn read(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        _fh: u64,
        offset: i64,
        size: u32,
        _flags: i32,
        _lock: Option<u64>,
        reply: ReplyData,
    ) {
        let Some(node) = self.nodes.get(&ino) else { reply.error(libc::ENOENT); return; };
        let key = node.obj.key.clone();
        match self.provider.read_range(&key, offset.max(0) as u64, size as u64) {
            Ok(bytes) => reply.data(&bytes),
            Err(_) => reply.error(libc::EIO),
        }
    }
}

/// Mount helper used by the cdb-fuse binary. Read-only, unprivileged
/// (fusermount3 setuid helper does the kernel side; our process stays
/// an ordinary user process).
pub fn mount<P: StorageProvider + Send + 'static>(
    provider: P,
    mountpoint: &str,
) -> std::io::Result<()> {
    std::fs::create_dir_all(mountpoint)?;
    let options = vec![
        fuser::MountOption::RO,
        fuser::MountOption::FSName("cloud-drive-bridge".to_string()),
        fuser::MountOption::AutoUnmount,
        fuser::MountOption::AllowOther,
    ];
    fuser::mount2(S3Fuse::new(provider), mountpoint, &options)
}
