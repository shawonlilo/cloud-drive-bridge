//! cdb-fuse binary: mount a bucket with the native Linux FUSE engine.
//! Secrets come from CDB_ACCESS_KEY_ID / CDB_SECRET_ACCESS_KEY or the
//! OS keychain via --from-keychain (same rules as the cdb CLI).

use cdb_core::*;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let get = |flag: &str| -> Option<String> {
        args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned()
    };
    let flags_with_values = ["--bucket", "--provider", "--region", "--endpoint", "--profile"];
    let mut positional: Vec<String> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        if flags_with_values.contains(&args[i].as_str()) {
            i += 2;
        } else if args[i].starts_with("--") {
            i += 1;
        } else {
            positional.push(args[i].clone());
            i += 1;
        }
    }
    let mountpoint = match positional.first() {
        Some(m) => m.clone(),
        None => {
            eprintln!("usage: cdb-fuse <mountpoint> --bucket B [--provider r2] [--region R] [--endpoint URL] [--profile NAME] [--from-keychain]");
            std::process::exit(2);
        }
    };
    let provider = get("--provider").and_then(|v| Provider::from_id(&v)).unwrap_or(Provider::Aws);
    let profile = match Profile::new(
        &get("--profile").unwrap_or_else(|| "fusemnt".into()),
        provider,
        &get("--bucket").unwrap_or_default(),
        &get("--region").unwrap_or_default(),
        &get("--endpoint").unwrap_or_default(),
    ) {
        Ok(p) => p,
        Err(e) => { eprintln!("error: {}", e); std::process::exit(2); }
    };
    let secrets = if args.iter().any(|a| a == "--from-keychain") {
        match keychain::load(&profile.name) {
            Ok((a, s)) => Secrets { access_key_id: a, secret_access_key: s },
            Err(e) => { eprintln!("error: {}", e); std::process::exit(2); }
        }
    } else {
        match Secrets::from_env() {
            Ok(s) => s,
            Err(e) => { eprintln!("error: {}", e); std::process::exit(2); }
        }
    };
    println!("Mounting bucket '{}' at {} (native FUSE, read-only v0.2) — Ctrl+C / fusermount3 -u to unmount", profile.bucket, mountpoint);
    if let Err(e) = cdb_fuse::mount(S3Provider::new(profile, secrets), &mountpoint) {
        eprintln!("mount error: {}", e);
        std::process::exit(1);
    }
}
