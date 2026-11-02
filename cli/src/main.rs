//! Cloud Drive Bridge CLI (`cdb`) — same cdb-core engine as the app.
//! Secrets: CDB_ACCESS_KEY_ID / CDB_SECRET_ACCESS_KEY env vars, or the
//! OS keychain with --from-keychain. Never a config file.

use cdb_core::*;
use std::process::ExitCode;

struct Opts {
    name: String,
    provider: Provider,
    bucket: String,
    region: String,
    endpoint: String,
    from_keychain: bool,
    rest: Vec<String>,
}

fn parse(rest: &[String]) -> Result<Opts, String> {
    let mut o = Opts {
        name: "cli".into(),
        provider: Provider::Aws,
        bucket: String::new(),
        region: String::new(),
        endpoint: String::new(),
        from_keychain: false,
        rest: Vec::new(),
    };
    let mut i = 0;
    while i < rest.len() {
        let a = &rest[i];
        let mut take = |i: &mut usize| -> Result<String, String> {
            *i += 1;
            rest.get(*i).cloned().ok_or_else(|| format!("missing value after {}", a))
        };
        match a.as_str() {
            "--profile" | "--name" => o.name = take(&mut i)?,
            "--provider" => {
                let v = take(&mut i)?;
                o.provider = Provider::from_id(&v).ok_or_else(|| format!("unknown provider '{}' (aws|r2|b2|minio|wasabi|spaces|linode|oracle|ibm|ceph|custom)", v))?;
            }
            "--bucket" => o.bucket = take(&mut i)?,
            "--region" => o.region = take(&mut i)?,
            "--endpoint" => o.endpoint = take(&mut i)?,
            "--from-keychain" => o.from_keychain = true,
            "--autostart" => {} // marker from auto-mount hooks
            other => o.rest.push(other.to_string()),
        }
        i += 1;
    }
    Ok(o)
}

fn profile_and_secrets(o: &Opts) -> Result<(Profile, Secrets), String> {
    // Keychain profiles carry their own metadata in CDB_PROFILES_JSON? No:
    // the app stores metadata itself; for CLI keychain use, bucket etc.
    // flags are still required — only the SECRETS come from the keychain.
    let p = Profile::new(&o.name, o.provider, &o.bucket, &o.region, &o.endpoint)?;
    let s = if o.from_keychain {
        let (a, sec) = keychain::load(&o.name)?;
        Secrets { access_key_id: a, secret_access_key: sec }
    } else {
        Secrets::from_env().map_err(|e| format!("{} — or store keys in the app and pass --from-keychain", e))?
    };
    Ok((p, s))
}

fn fmt_size(n: u64) -> String {
    const U: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = n as f64;
    let mut i = 0;
    while v >= 1024.0 && i < 4 { v /= 1024.0; i += 1; }
    format!("{:.1} {}", v, U[i])
}

fn help() {
    println!(
        "Cloud Drive Bridge — cdb {}\n\nUSAGE:\n  cdb <command> --bucket B [--provider aws|r2|b2|minio|wasabi|spaces|linode|oracle|ibm|ceph|custom] [--region R] [--endpoint URL] [--profile NAME] [--from-keychain]\n\nCOMMANDS:\n  test                      5-point check (reachability/auth/list, +write/delete probe with --probe)\n  ls [prefix]               list a folder\n  get <key> <local-path>    download one object\n  put <local-path> <key>    upload one file (ETag checksum verified)\n  rm <key>                  delete one object\n  mkdir <prefix>            create a folder marker\n  rename <from> <to>        rename (copy + delete)\n  size                      bucket object count + total size\n  share <key> [1h|24h|7d]   native SigV4 presigned share link (default 24h)\n  mount <target> [options]  rclone mount (drive letter or folder)\n      --cache 5G --bwlimit 0 --read-only --offline --webdav\n  unmount <target>          unmount (fusermount/taskkill guidance per OS)\n\nSECRETS: CDB_ACCESS_KEY_ID + CDB_SECRET_ACCESS_KEY env vars, or the OS\nkeychain (--from-keychain). Keys are never written to a config file.",
        env!("CARGO_PKG_VERSION")
    );
}

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if argv.is_empty() || argv[0] == "--help" || argv[0] == "-h" || argv[0] == "help" {
        help();
        return ExitCode::SUCCESS;
    }
    let cmd = argv[0].clone();
    let o = match parse(&argv[1..]) {
        Ok(o) => o,
        Err(e) => { eprintln!("error: {}", e); return ExitCode::FAILURE; }
    };
    if cmd == "providers" {
        for p in Provider::all() {
            println!("{:<8} {:<28} rclone-provider={} default-region={}", p.id(), p.label(), p.rclone_provider(), p.default_region());
        }
        return ExitCode::SUCCESS;
    }
    let (profile, secrets) = match profile_and_secrets(&o) {
        Ok(v) => v,
        Err(e) => { eprintln!("error: {}", e); return ExitCode::FAILURE; }
    };
    let rest = &o.rest;
    let arg = |i: usize| -> Result<String, String> {
        rest.get(i).cloned().ok_or_else(|| "missing argument".to_string())
    };
    let result: Result<String, String> = (|| {
        Ok(match cmd.as_str() {
            "test" => {
                let probe = rest.iter().any(|a| a == "--probe");
                let r = five_point_check(&profile, &secrets, probe);
                format!("{}\n{}", r.to_json(), r.notes.join("\n"))
            }
            "ls" => {
                let prefix = rest.first().cloned().unwrap_or_else(|| profile.prefix.clone());
                let l = list_objects(&profile, &secrets, &prefix, "")?;
                let mut out = String::new();
                for f in &l.folders { out.push_str(&format!("DIR  {}\n", f.key)); }
                for f in &l.files {
                    out.push_str(&format!("{:>10}  {}  {}  [{}]\n", fmt_size(f.size), f.last_modified, f.key, f.storage_class));
                }
                out.push_str(&format!("{} item(s)", l.folders.len() + l.files.len()));
                out
            }
            "get" => {
                download_file(&profile, &secrets, &arg(0)?, &arg(1)?)?;
                format!("downloaded {} -> {}", arg(0)?, arg(1)?)
            }
            "put" => {
                let etag = retry_backoff(3, || upload_file(&profile, &secrets, &arg(0)?, &arg(1)?))?;
                format!("uploaded {} -> {} (etag {})\nchecksum: {}", arg(0)?, arg(1)?, etag, verify_upload(&arg(0)?, &etag))
            }
            "rm" => { delete_object(&profile, &secrets, &arg(0)?)?; format!("deleted {}", arg(0)?) }
            "mkdir" => { create_folder(&profile, &secrets, &arg(0)?)?; format!("created folder {}", arg(0)?) }
            "rename" => { rename_object(&profile, &secrets, &arg(0)?, &arg(1)?)?; format!("renamed {} -> {}", arg(0)?, arg(1)?) }
            "size" => {
                let (n, bytes) = bucket_stats(&profile, &secrets)?;
                format!("{} objects, {} total", n, fmt_size(bytes))
            }
            "share" => {
                let preset = rest.get(1).map(|s| s.as_str()).unwrap_or("24h");
                let url = share_link(&profile, &secrets, &arg(0)?, preset)?;
                format!("{} (expires in {})", url, preset)
            }
            "mount" => {
                let target = arg(0)?;
                if rest.iter().any(|a| a == "--webdav") {
                    let mut child = spawn_webdav(&profile, &secrets, 8340)?;
                    println!("WebDAV mode on http://127.0.0.1:8340/ — map it with: net use {} http://127.0.0.1:8340/", target);
                    println!("Press Ctrl+C to stop.");
                    let _ = child.wait();
                    return Ok("webdav stopped".into());
                }
                let mut opts = MountOptions::default();
                opts.target = target.clone();
                opts.read_only = rest.iter().any(|a| a == "--read-only");
                opts.offline = rest.iter().any(|a| a == "--offline");
                if let Some(i) = rest.iter().position(|a| a == "--cache") { opts.cache_max_size = rest.get(i + 1).cloned().unwrap_or(opts.cache_max_size); }
                if let Some(i) = rest.iter().position(|a| a == "--bwlimit") { opts.bwlimit = rest.get(i + 1).cloned().unwrap_or(opts.bwlimit); }
                let (ok, guidance) = driver_status();
                if !ok { println!("note: {}", guidance); }
                let mut child = spawn_mount(&profile, &secrets, &opts)?;
                println!("Mounted {} at {} (watchdog active in the app; in CLI mode the mount lives while this process runs)", profile.bucket, target);
                println!("Press Ctrl+C to unmount.");
                loop {
                    match child.try_wait() {
                        Ok(Some(s)) => return Err(format!("rclone mount exited ({}) — check WinFsp/macFUSE/FUSE and your keys", s)),
                        Ok(None) => std::thread::sleep(std::time::Duration::from_secs(2)),
                        Err(e) => return Err(e.to_string()),
                    }
                }
            }
            "unmount" => {
                let target = arg(0)?;
                if cfg!(windows) {
                    format!("On Windows, stop the cdb/rclone process that mounted {} (Task Manager or: taskkill /IM rclone.exe). The drive letter is released when rclone exits.", target)
                } else {
                    let cmdname = if cfg!(target_os = "macos") { "umount" } else { "fusermount3 -u" };
                    match std::process::Command::new("sh").args(["-c", &format!("{} {}", cmdname, target)]).status() {
                        Ok(s) if s.success() => format!("unmounted {}", target),
                        _ => format!("run: {} {}", cmdname, target),
                    }
                }
            }
            _ => { help(); return Err(format!("unknown command '{}'", cmd)); }
        })
    })();
    match result {
        Ok(msg) => { println!("{}", msg); ExitCode::SUCCESS }
        Err(e) => { eprintln!("error: {}", e); ExitCode::FAILURE }
    }
}
