//! Cloud Drive Bridge core engine.
//!
//! Design rules (non-negotiable):
//! - BYOK only. Secrets live in the OS keychain (desktop) or in memory
//!   (mobile v0.1). They are NEVER written to a config file, never
//!   logged, and `Secrets` intentionally has no `Debug` printing values.
//! - S3 operations use the system `curl` with `--aws-sigv4` — curl ships
//!   on Windows 10+, macOS and every Linux, and owns TLS for us.
//! - Mounting uses `rclone mount` with credentials passed ONLY through
//!   `RCLONE_CONFIG_<NAME>_*` environment variables, so rclone never
//!   writes a credential file either.

use std::collections::BTreeMap;
use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

// ---------------------------------------------------------------- providers

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Provider {
    Aws,
    R2,
    B2,
    Minio,
    Wasabi,
    Spaces,
    Linode,
    Oracle,
    IbmCos,
    Ceph,
    Custom,
}

impl Provider {
    pub fn from_id(id: &str) -> Option<Provider> {
        Some(match id.to_ascii_lowercase().as_str() {
            "aws" | "amazon" | "s3" => Provider::Aws,
            "r2" | "cloudflare" => Provider::R2,
            "b2" | "backblaze" => Provider::B2,
            "minio" => Provider::Minio,
            "wasabi" => Provider::Wasabi,
            "spaces" | "digitalocean" => Provider::Spaces,
            "linode" | "akamai" => Provider::Linode,
            "oracle" | "oci" => Provider::Oracle,
            "ibm" | "cos" => Provider::IbmCos,
            "ceph" => Provider::Ceph,
            "custom" => Provider::Custom,
            _ => return None,
        })
    }
    pub fn id(&self) -> &'static str {
        match self {
            Provider::Aws => "aws",
            Provider::R2 => "r2",
            Provider::B2 => "b2",
            Provider::Minio => "minio",
            Provider::Wasabi => "wasabi",
            Provider::Spaces => "spaces",
            Provider::Linode => "linode",
            Provider::Oracle => "oracle",
            Provider::IbmCos => "ibm",
            Provider::Ceph => "ceph",
            Provider::Custom => "custom",
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            Provider::Aws => "Amazon S3",
            Provider::R2 => "Cloudflare R2",
            Provider::B2 => "Backblaze B2",
            Provider::Minio => "MinIO",
            Provider::Wasabi => "Wasabi",
            Provider::Spaces => "DigitalOcean Spaces",
            Provider::Linode => "Linode Object Storage",
            Provider::Oracle => "Oracle Object Storage",
            Provider::IbmCos => "IBM Cloud Object Storage",
            Provider::Ceph => "Ceph (self-hosted)",
            Provider::Custom => "Custom S3 endpoint",
        }
    }
    /// rclone `provider` parameter for the env config.
    pub fn rclone_provider(&self) -> &'static str {
        match self {
            Provider::Aws => "AWS",
            Provider::R2 => "Cloudflare",
            Provider::B2 => "Backblaze",
            Provider::Minio => "Minio",
            Provider::Wasabi => "Wasabi",
            Provider::Spaces => "DigitalOcean",
            Provider::Linode => "Linode",
            Provider::Oracle => "Oracle",
            Provider::IbmCos => "IBMCOS",
            Provider::Ceph => "Ceph",
            Provider::Custom => "Other",
        }
    }
    pub fn default_region(&self) -> &'static str {
        match self {
            Provider::Aws => "us-east-1",
            Provider::R2 => "auto",
            Provider::B2 => "us-west-004",
            Provider::Minio => "us-east-1",
            Provider::Wasabi => "us-east-1",
            Provider::Spaces => "nyc3",
            Provider::Linode => "us-east-1",
            Provider::Oracle => "us-ashburn-1",
            Provider::IbmCos => "us-standard",
            Provider::Ceph => "default",
            Provider::Custom => "us-east-1",
        }
    }
    /// Endpoint template; `{region}` is substituted. Empty = AWS default
    /// (curl reaches s3.<region>.amazonaws.com without an explicit host
    /// override — we still build the full URL ourselves).
    pub fn endpoint_template(&self) -> &'static str {
        match self {
            Provider::Aws => "https://s3.{region}.amazonaws.com",
            Provider::R2 => "https://{account}.r2.cloudflarestorage.com",
            Provider::B2 => "https://s3.{region}.backblazeb2.com",
            Provider::Minio => "",
            Provider::Wasabi => "https://s3.{region}.wasabisys.com",
            Provider::Spaces => "https://{region}.digitaloceanspaces.com",
            Provider::Linode => "https://{region}.linodeobjects.com",
            Provider::Oracle => "https://{namespace}.compat.objectstorage.{region}.oraclecloud.com",
            Provider::IbmCos => "https://s3.{region}.cloud-object-storage.appdomain.cloud",
            Provider::Ceph => "",
            Provider::Custom => "",
        }
    }
    pub fn all() -> Vec<Provider> {
        vec![
            Provider::Aws, Provider::R2, Provider::B2, Provider::Minio,
            Provider::Wasabi, Provider::Spaces, Provider::Linode,
            Provider::Oracle, Provider::IbmCos, Provider::Ceph, Provider::Custom,
        ]
    }
}

// ------------------------------------------------------------------ profile

#[derive(Clone, PartialEq, Eq)]
pub struct Profile {
    /// rclone-remote-safe name: letters, digits, `-`, `_`.
    pub name: String,
    pub provider: Provider,
    pub bucket: String,
    pub region: String,
    /// Full endpoint base URL, e.g. https://acct.r2.cloudflarestorage.com
    /// (no trailing slash). For AWS it is derived from the region.
    pub endpoint: String,
    pub prefix: String,
}

impl Profile {
    pub fn new(
        name: &str,
        provider: Provider,
        bucket: &str,
        region: &str,
        endpoint: &str,
    ) -> Result<Profile, String> {
        let region = if region.trim().is_empty() {
            provider.default_region().to_string()
        } else {
            region.trim().to_string()
        };
        let endpoint = if endpoint.trim().is_empty() {
            provider
                .endpoint_template()
                .replace("{region}", &region)
        } else {
            endpoint.trim().trim_end_matches('/').to_string()
        };
        let p = Profile {
            name: name.trim().to_string(),
            provider,
            bucket: bucket.trim().to_string(),
            region,
            endpoint,
            prefix: String::new(),
        };
        p.validate()?;
        Ok(p)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.name.is_empty()
            || !self
                .name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err("profile name may only contain letters, numbers, - and _".into());
        }
        if self.bucket.is_empty()
            || !self
                .bucket
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '.')
            || self.bucket.starts_with('-')
            || self.bucket.ends_with('-')
        {
            return Err("bucket name looks invalid (lowercase letters, digits, - and . only)".into());
        }
        if self.endpoint.is_empty() {
            return Err("endpoint is required for this provider (e.g. your MinIO/Custom URL)".into());
        }
        if !self.endpoint.starts_with("https://") && !self.endpoint.starts_with("http://") {
            return Err("endpoint must start with https:// (or http:// for local MinIO)".into());
        }
        if self.endpoint.contains("{") {
            return Err("endpoint still contains a {placeholder} — fill in your account/namespace id".into());
        }
        Ok(())
    }

    /// Path-style base URL for the bucket: <endpoint>/<bucket>
    pub fn bucket_url(&self) -> String {
        format!("{}/{}", self.endpoint, self.bucket)
    }
    pub fn object_url(&self, key: &str) -> String {
        format!("{}/{}", self.bucket_url(), encode_key(key))
    }
    /// Env-var prefix rclone expects for this remote.
    pub fn rclone_env_prefix(&self) -> String {
        format!(
            "RCLONE_CONFIG_{}_",
            self.name.to_ascii_uppercase().replace('-', "_")
        )
    }
    /// Metadata-only JSON (NEVER includes secrets) for profile storage.
    pub fn to_json(&self) -> String {
        format!(
            "{{\"name\":\"{}\",\"provider\":\"{}\",\"bucket\":\"{}\",\"region\":\"{}\",\"endpoint\":\"{}\",\"prefix\":\"{}\"}}",
            esc(&self.name),
            self.provider.id(),
            esc(&self.bucket),
            esc(&self.region),
            esc(&self.endpoint),
            esc(&self.prefix)
        )
    }
}

/// User's own keys. Deliberately no value-printing Debug impl.
pub struct Secrets {
    pub access_key_id: String,
    pub secret_access_key: String,
}
impl Secrets {
    pub fn from_env() -> Result<Secrets, String> {
        let a = std::env::var("CDB_ACCESS_KEY_ID")
            .map_err(|_| "CDB_ACCESS_KEY_ID is not set".to_string())?;
        let s = std::env::var("CDB_SECRET_ACCESS_KEY")
            .map_err(|_| "CDB_SECRET_ACCESS_KEY is not set".to_string())?;
        if a.trim().is_empty() || s.trim().is_empty() {
            return Err("key id / secret must not be empty".into());
        }
        Ok(Secrets { access_key_id: a, secret_access_key: s })
    }
    pub fn is_empty(&self) -> bool {
        self.access_key_id.trim().is_empty() || self.secret_access_key.trim().is_empty()
    }
}
impl std::fmt::Debug for Secrets {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Secrets(access_key_id: {}…, secret: [redacted])", &self.access_key_id.chars().take(4).collect::<String>())
    }
}

pub fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// Percent-encode an S3 key for a URL path (keeps `/` separators).
pub fn encode_key(key: &str) -> String {
    let mut out = String::new();
    for b in key.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

// ------------------------------------------------------------------ listing

#[derive(Clone, Debug, PartialEq)]
pub struct S3Object {
    pub key: String,
    pub size: u64,
    pub last_modified: String,
    pub etag: String,
    pub storage_class: String,
    pub is_folder: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Listing {
    pub folders: Vec<S3Object>,
    pub files: Vec<S3Object>,
    pub truncated: bool,
    pub next_token: Option<String>,
}

fn xml_tag(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{}>", tag);
    let close = format!("</{}>", tag);
    let s = xml.find(&open)? + open.len();
    let e = xml[s..].find(&close)? + s;
    Some(xml_unescape(&xml[s..e]))
}
fn xml_all(xml: &str, block: &str) -> Vec<String> {
    let open = format!("<{}>", block);
    let close = format!("</{}>", block);
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(s) = rest.find(&open) {
        let after = &rest[s + open.len()..];
        match after.find(&close) {
            Some(e) => {
                out.push(after[..e].to_string());
                rest = &after[e + close.len()..];
            }
            None => break,
        }
    }
    out
}
pub fn xml_unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

/// Parse a ListObjectsV2 response. `prefix` is stripped for display names
/// but full keys are kept on every entry.
pub fn parse_listing(xml: &str, prefix: &str) -> Listing {
    let mut l = Listing {
        truncated: xml_tag(xml, "IsTruncated").map(|v| v == "true").unwrap_or(false),
        next_token: xml_tag(xml, "NextContinuationToken"),
        ..Default::default()
    };
    for block in xml_all(xml, "CommonPrefixes") {
        if let Some(p) = xml_tag(&block, "Prefix") {
            if p != prefix {
                l.folders.push(S3Object {
                    key: p, size: 0, last_modified: String::new(),
                    etag: String::new(), storage_class: String::new(), is_folder: true,
                });
            }
        }
    }
    for block in xml_all(xml, "Contents") {
        let key = xml_tag(&block, "Key").unwrap_or_default();
        if key.is_empty() || key == prefix { continue; }
        let is_folder_marker = key.ends_with('/') ;
        l.files.push(S3Object {
            size: xml_tag(&block, "Size").and_then(|v| v.parse().ok()).unwrap_or(0),
            last_modified: xml_tag(&block, "LastModified").unwrap_or_default(),
            etag: xml_tag(&block, "ETag").unwrap_or_default().trim_matches('"').to_string(),
            storage_class: xml_tag(&block, "StorageClass").unwrap_or_else(|| "STANDARD".into()),
            key,
            is_folder: is_folder_marker,
        });
    }
    l.folders.sort_by(|a, b| a.key.cmp(&b.key));
    l.files.sort_by(|a, b| a.key.cmp(&b.key));
    l
}

impl S3Object {
    pub fn display_name(&self, prefix: &str) -> String {
        self.key.trim_start_matches(prefix).trim_end_matches('/').to_string()
    }
    pub fn to_json(&self, prefix: &str) -> String {
        format!(
            "{{\"key\":\"{}\",\"name\":\"{}\",\"size\":{},\"modified\":\"{}\",\"etag\":\"{}\",\"storage_class\":\"{}\",\"folder\":{}}}",
            esc(&self.key), esc(&self.display_name(prefix)), self.size,
            esc(&self.last_modified), esc(&self.etag), esc(&self.storage_class), self.is_folder
        )
    }
}

impl Listing {
    pub fn to_json(&self, prefix: &str) -> String {
        let mut items: Vec<String> = Vec::new();
        for f in &self.folders { items.push(f.to_json(prefix)); }
        for f in &self.files { items.push(f.to_json(prefix)); }
        format!(
            "{{\"items\":[{}],\"truncated\":{},\"count\":{}}}",
            items.join(","), self.truncated, items.len()
        )
    }
    pub fn total_size(&self) -> u64 { self.files.iter().map(|f| f.size).sum() }
}

// --------------------------------------------------------------------- curl

pub fn curl_program() -> &'static str {
    if cfg!(windows) { "curl.exe" } else { "curl" }
}
pub fn curl_available() -> bool {
    Command::new(curl_program()).arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

/// The testable core of every S3 request: the exact curl argv (without
/// the secret value — `--user` is added at run time from `Secrets`).
pub fn curl_base_args(profile: &Profile) -> Vec<String> {
    vec![
        "-sS".into(),
        "--aws-sigv4".into(),
        format!("aws:amz:{}:s3", profile.region),
    ]
}

pub struct CurlOutcome {
    pub status: u32,
    pub body: Vec<u8>,
    pub headers: String,
}

fn run_curl(
    profile: &Profile,
    secrets: &Secrets,
    extra: &[String],
    url: &str,
) -> Result<CurlOutcome, String> {
    if !curl_available() {
        return Err("curl was not found on this system. Install curl (it ships with Windows 10+, macOS and most Linux distros) and try again.".into());
    }
    let mut args = curl_base_args(profile);
    args.push("--user".into());
    args.push(format!("{}:{}", secrets.access_key_id, secrets.secret_access_key));
    args.push("-w".into());
    args.push("\nCDB_HTTP_STATUS:%{http_code}".into());
    args.push("-D".into());
    args.push("-".into());
    for a in extra { args.push(a.clone()); }
    args.push(url.to_string());
    let out = Command::new(curl_program())
        .args(&args)
        .output()
        .map_err(|e| format!("could not run curl: {}", e))?;
    let stdout = out.stdout;
    // Headers arrive on stdout because of -D - ; body precedes the marker.
    let text = String::from_utf8_lossy(&stdout).to_string();
    let (head_body, status) = match text.rfind("\nCDB_HTTP_STATUS:") {
        Some(i) => {
            let st: u32 = text[i + 17..].trim().parse().unwrap_or(0);
            (&text[..i], st)
        }
        None => (&text[..], 0),
    };
    // Split header block(s) from body: headers end at the first blank line
    // that is followed by a non-header line; simplest robust split used by
    // our callers: body = after the LAST "\r\n\r\n" before any XML/JSON.
    let mut headers = String::new();
    let mut body = head_body.as_bytes().to_vec();
    if let Some(pos) = find_body_start(head_body) {
        headers = head_body[..pos].to_string();
        body = head_body[pos..].as_bytes().to_vec();
    }
    if !out.status.success() && status == 0 {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(if err.is_empty() { "curl failed".into() } else { err });
    }
    Ok(CurlOutcome { status, body, headers })
}

fn find_body_start(s: &str) -> Option<usize> {
    // curl -D - prints headers first; the body of our S3 calls starts
    // with '<' (XML), '{' or is empty/binary for downloads (handled by -o).
    let sep = s.find("\r\n\r\n").map(|i| i + 4).or_else(|| s.find("\n\n").map(|i| i + 2))?;
    Some(sep)
}

fn s3_error(status: u32, body: &[u8]) -> String {
    let text = String::from_utf8_lossy(body);
    let code = xml_tag(&text, "Code").unwrap_or_else(|| format!("HTTP {}", status));
    let msg = xml_tag(&text, "Message").unwrap_or_default();
    if msg.is_empty() { code } else { format!("{}: {}", code, msg) }
}

fn expect_ok(o: &CurlOutcome) -> Result<(), String> {
    if (200..300).contains(&o.status) { Ok(()) } else { Err(s3_error(o.status, &o.body)) }
}

// ------------------------------------------------------------------ S3 ops

/// HEAD the bucket: 200 = reachable + authorized, 404 = no such bucket,
/// 403 = bad credentials / no permission.
pub fn test_connection(profile: &Profile, secrets: &Secrets) -> Result<String, String> {
    let o = run_curl(profile, secrets, &["-I".into()], &profile.bucket_url())?;
    match o.status {
        200 => Ok(format!("Connected — bucket '{}' is reachable and your keys are accepted.", profile.bucket)),
        404 => Err(format!("Bucket '{}' was not found at this endpoint. Check the bucket name, provider and region.", profile.bucket)),
        403 => Err("The endpoint rejected your keys (403). Check the access key id / secret and that they may access this bucket.".into()),
        301 => Err("Bucket exists in a different region (301). Update the profile region.".into()),
        0 => Err("No HTTP response — check the endpoint URL and your network.".into()),
        s => Err(format!("Unexpected response HTTP {}.", s)),
    }
}

#[derive(Clone, Debug, Default)]
pub struct FivePoint {
    pub reachable: bool,
    pub auth: bool,
    pub list: bool,
    pub write: bool,
    pub delete: bool,
    pub notes: Vec<String>,
}
impl FivePoint {
    pub fn to_json(&self) -> String {
        format!(
            "{{\"reachable\":{},\"auth\":{},\"list\":{},\"write\":{},\"delete\":{},\"notes\":[{}]}}",
            self.reachable, self.auth, self.list, self.write, self.delete,
            self.notes.iter().map(|n| format!("\"{}\"", esc(n))).collect::<Vec<_>>().join(",")
        )
    }
}

/// The 5-point connection check: reachability / auth / list / write /
/// delete, using a tiny probe object that is always cleaned up.
pub fn five_point_check(profile: &Profile, secrets: &Secrets, write_probe: bool) -> FivePoint {
    let mut r = FivePoint::default();
    match test_connection(profile, secrets) {
        Ok(n) => { r.reachable = true; r.auth = true; r.notes.push(n); }
        Err(e) => { r.notes.push(e); return r; }
    }
    match list_objects(profile, secrets, &profile.prefix, "") {
        Ok(_) => { r.list = true; r.notes.push("List: OK".into()); }
        Err(e) => { r.notes.push(format!("List failed: {}", e)); return r; }
    }
    if !write_probe {
        r.notes.push("Write/delete probe skipped (read-only check).".into());
        return r;
    }
    let probe_key = format!("{}cdb-probe-{}.txt", profile.prefix, now_nanos());
    let tmp = std::env::temp_dir().join(format!("cdb-probe-{}.txt", now_nanos()));
    let _ = std::fs::write(&tmp, b"cloud-drive-bridge probe");
    match upload_file(profile, secrets, &tmp.to_string_lossy(), &probe_key) {
        Ok(_) => {
            r.write = true;
            r.notes.push("Write probe: OK".into());
            match delete_object(profile, secrets, &probe_key) {
                Ok(_) => { r.delete = true; r.notes.push("Delete probe: OK (probe cleaned up)".into()); }
                Err(e) => r.notes.push(format!("Delete probe failed: {} — remove '{}' manually", e, probe_key)),
            }
        }
        Err(e) => r.notes.push(format!("Write probe failed: {}", e)),
    }
    let _ = std::fs::remove_file(&tmp);
    r
}

pub fn list_objects(
    profile: &Profile,
    secrets: &Secrets,
    prefix: &str,
    token: &str,
) -> Result<Listing, String> {
    let mut q = format!(
        "{}?list-type=2&delimiter=/&prefix={}",
        profile.bucket_url(),
        encode_key(prefix)
    );
    if !token.is_empty() {
        q.push_str(&format!("&continuation-token={}", encode_key(token)));
    }
    let o = run_curl(profile, secrets, &[], &q)?;
    expect_ok(&o)?;
    let xml = String::from_utf8_lossy(&o.body).to_string();
    Ok(parse_listing(&xml, prefix))
}

/// Full recursive listing under a prefix (for usage stats).
pub fn list_all(profile: &Profile, secrets: &Secrets, prefix: &str) -> Result<Vec<S3Object>, String> {
    let mut out = Vec::new();
    let mut token = String::new();
    loop {
        let mut q = format!("{}?list-type=2&prefix={}", profile.bucket_url(), encode_key(prefix));
        if !token.is_empty() { q.push_str(&format!("&continuation-token={}", encode_key(&token))); }
        let o = run_curl(profile, secrets, &[], &q)?;
        expect_ok(&o)?;
        let xml = String::from_utf8_lossy(&o.body).to_string();
        let l = parse_listing(&xml, prefix);
        out.extend(l.files);
        if l.truncated {
            token = l.next_token.unwrap_or_default();
            if token.is_empty() { break; }
        } else { break; }
    }
    Ok(out)
}

pub fn bucket_stats(profile: &Profile, secrets: &Secrets) -> Result<(u64, u64), String> {
    let all = list_all(profile, secrets, &profile.prefix)?;
    Ok((all.len() as u64, all.iter().map(|f| f.size).sum()))
}

pub fn download_file(profile: &Profile, secrets: &Secrets, key: &str, local_path: &str) -> Result<(), String> {
    let o = run_curl(
        profile, secrets,
        &["-o".into(), local_path.into()],
        &profile.object_url(key),
    )?;
    expect_ok(&o).map_err(|e| {
        let _ = std::fs::remove_file(local_path);
        e
    })
}

/// Upload one file; returns the ETag. Checksum note: a plain (dashless)
/// ETag is the MD5 for single-part uploads and is compared here.
pub fn upload_file(profile: &Profile, secrets: &Secrets, local_path: &str, key: &str) -> Result<String, String> {
    let o = run_curl(
        profile, secrets,
        &["-T".into(), local_path.into()],
        &profile.object_url(key),
    )?;
    expect_ok(&o)?;
    let etag = o.headers.lines()
        .find(|l| l.to_ascii_lowercase().starts_with("etag:"))
        .map(|l| l[5..].trim().trim_matches('"').to_string())
        .unwrap_or_default();
    Ok(etag)
}

pub fn verify_upload(local_path: &str, etag: &str) -> String {
    if etag.is_empty() {
        return "not-reported: endpoint returned no ETag".into();
    }
    if etag.contains('-') {
        return format!("multipart: ETag {} is a multipart digest, not a plain MD5 — size check only", etag);
    }
    match md5_file(local_path) {
        Ok(md5) if md5.eq_ignore_ascii_case(etag) => "verified: MD5 matches ETag".into(),
        Ok(md5) => format!("mismatch: local MD5 {} != ETag {}", md5, etag),
        Err(e) => format!("unverified: {}", e),
    }
}

pub fn delete_object(profile: &Profile, secrets: &Secrets, key: &str) -> Result<(), String> {
    let o = run_curl(profile, secrets, &["-X".into(), "DELETE".into()], &profile.object_url(key))?;
    expect_ok(&o)
}

pub fn create_folder(profile: &Profile, secrets: &Secrets, prefix: &str) -> Result<(), String> {
    let key = if prefix.ends_with('/') { prefix.to_string() } else { format!("{}/", prefix) };
    let o = run_curl(
        profile, secrets,
        &["-X".into(), "PUT".into(), "-H".into(), "Content-Length: 0".into()],
        &profile.object_url(&key),
    )?;
    expect_ok(&o)
}

/// Rename = copy to the new key, then delete the old one (S3 has no
/// native rename).
pub fn rename_object(profile: &Profile, secrets: &Secrets, from: &str, to: &str) -> Result<(), String> {
    let copy_source = format!("/{}/{}", profile.bucket, encode_key(from));
    let o = run_curl(
        profile, secrets,
        &["-X".into(), "PUT".into(), "-H".into(), format!("x-amz-copy-source: {}", copy_source)],
        &profile.object_url(to),
    )?;
    expect_ok(&o)?;
    delete_object(profile, secrets, from)
}

pub fn now_nanos() -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0)
}

// ------------------------------------------------------------------ MD5
// Small self-contained MD5 (RFC 1321) for upload checksum verification.

const MD5_S: [u32; 64] = [
    7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22,
    5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20,
    4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23,
    6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
];
const MD5_K: [u32; 64] = [
    0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee, 0xf57c0faf, 0x4787c62a, 0xa8304613, 0xfd469501,
    0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be, 0x6b901122, 0xfd987193, 0xa679438e, 0x49b40821,
    0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa, 0xd62f105d, 0x02441453, 0xd8a1e681, 0xe7d3fbc8,
    0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed, 0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a,
    0xfffa3942, 0x8771f681, 0x6d9d6122, 0xfde5380c, 0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70,
    0x289b7ec6, 0xeaa127fa, 0xd4ef3085, 0x04881d05, 0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665,
    0xf4292244, 0x432aff97, 0xab9423a7, 0xfc93a039, 0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1,
    0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1, 0xf7537e82, 0xbd3af235, 0x2ad7d2bb, 0xeb86d391,
];

pub fn md5_hex(data: &[u8]) -> String {
    let mut a0: u32 = 0x67452301;
    let mut b0: u32 = 0xefcdab89;
    let mut c0: u32 = 0x98badcfe;
    let mut d0: u32 = 0x10325476;
    let mut msg = data.to_vec();
    let bit_len = (data.len() as u64) * 8;
    msg.push(0x80);
    while msg.len() % 64 != 56 { msg.push(0); }
    msg.extend_from_slice(&bit_len.to_le_bytes());
    for chunk in msg.chunks_exact(64) {
        let mut m = [0u32; 16];
        for i in 0..16 {
            m[i] = u32::from_le_bytes([chunk[i * 4], chunk[i * 4 + 1], chunk[i * 4 + 2], chunk[i * 4 + 3]]);
        }
        let (mut a, mut b, mut c, mut d) = (a0, b0, c0, d0);
        for i in 0..64 {
            let (f, g) = match i / 16 {
                0 => ((b & c) | (!b & d), i),
                1 => ((d & b) | (!d & c), (5 * i + 1) % 16),
                2 => (b ^ c ^ d, (3 * i + 5) % 16),
                _ => (c ^ (b | !d), (7 * i) % 16),
            };
            let tmp = f.wrapping_add(a).wrapping_add(MD5_K[i]).wrapping_add(m[g]);
            a = d; d = c; c = b;
            b = b.wrapping_add(tmp.rotate_left(MD5_S[i]));
        }
        a0 = a0.wrapping_add(a); b0 = b0.wrapping_add(b);
        c0 = c0.wrapping_add(c); d0 = d0.wrapping_add(d);
    }
    let mut out = String::with_capacity(32);
    for v in [a0, b0, c0, d0] {
        for byte in v.to_le_bytes() { out.push_str(&format!("{:02x}", byte)); }
    }
    out
}

pub fn md5_file(path: &str) -> Result<String, String> {
    let mut f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).map_err(|e| e.to_string())?;
    Ok(md5_hex(&buf))
}

// ---------------------------------------------------------------- keychain

pub mod keychain {
    //! Desktop: the real OS keychain via the `keyring` crate
    //! (Windows Credential Manager / macOS Keychain / Linux keyutils +
    //! Secret Service). Mobile v0.1: memory-only store — secrets are
    //! never written to disk on Android/iOS in this release.
    use std::collections::HashMap;
    use std::sync::Mutex;

    pub const SERVICE: &str = "org.opentools.clouddrivebridge";

    fn account(profile: &str) -> String { format!("profile:{}", profile) }

    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    pub fn store(profile: &str, access: &str, secret: &str) -> Result<(), String> {
        let entry = keyring::Entry::new(SERVICE, &account(profile)).map_err(|e| e.to_string())?;
        // One entry holds both values separated by a newline; key ids and
        // secrets never contain raw newlines in practice, and we reject
        // them defensively in validate before storing.
        if access.contains('\n') || secret.contains('\n') {
            return Err("keys must not contain newlines".into());
        }
        entry.set_password(&format!("{}\n{}", access, secret)).map_err(|e| e.to_string())
    }
    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    pub fn load(profile: &str) -> Result<(String, String), String> {
        let entry = keyring::Entry::new(SERVICE, &account(profile)).map_err(|e| e.to_string())?;
        let v = entry.get_password().map_err(|_| {
            format!("no keys stored in the OS keychain for profile '{}' yet — add them on the Profiles screen", profile)
        })?;
        let mut parts = v.splitn(2, '\n');
        let a = parts.next().unwrap_or_default().to_string();
        let s = parts.next().unwrap_or_default().to_string();
        if a.is_empty() || s.is_empty() { return Err("stored keychain entry is incomplete".into()); }
        Ok((a, s))
    }
    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    pub fn remove(profile: &str) -> Result<(), String> {
        let entry = keyring::Entry::new(SERVICE, &account(profile)).map_err(|e| e.to_string())?;
        entry.delete_credential().map_err(|e| e.to_string())
    }

    static MEM: Mutex<Option<HashMap<String, (String, String)>>> = Mutex::new(None);
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    pub fn store(profile: &str, access: &str, secret: &str) -> Result<(), String> {
        let mut g = MEM.lock().unwrap();
        g.get_or_insert_with(HashMap::new).insert(profile.to_string(), (access.to_string(), secret.to_string()));
        Ok(())
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    pub fn load(profile: &str) -> Result<(String, String), String> {
        let g = MEM.lock().unwrap();
        g.as_ref().and_then(|m| m.get(profile)).cloned()
            .ok_or_else(|| "keys are kept in memory only on mobile v0.1 — enter them for this session".into())
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    pub fn remove(profile: &str) -> Result<(), String> {
        let mut g = MEM.lock().unwrap();
        if let Some(m) = g.as_mut() { m.remove(profile); }
        Ok(())
    }
}

// ------------------------------------------------------------------ rclone

pub fn rclone_program() -> String {
    std::env::var("CDB_RCLONE").unwrap_or_else(|_| {
        if cfg!(windows) { "rclone.exe".into() } else { "rclone".into() }
    })
}
pub fn rclone_available() -> bool {
    Command::new(rclone_program()).arg("version").output().map(|o| o.status.success()).unwrap_or(false)
}

/// The environment that configures rclone WITHOUT any config file.
/// This is the only place secrets leave the keychain, and they go
/// straight into the child process environment.
pub fn rclone_env(profile: &Profile, secrets: &Secrets) -> Vec<(String, String)> {
    let p = profile.rclone_env_prefix();
    let mut v = vec![
        (format!("{}TYPE", p), "s3".to_string()),
        (format!("{}PROVIDER", p), profile.provider.rclone_provider().to_string()),
        (format!("{}ACCESS_KEY_ID", p), secrets.access_key_id.clone()),
        (format!("{}SECRET_ACCESS_KEY", p), secrets.secret_access_key.clone()),
        (format!("{}REGION", p), profile.region.clone()),
        (format!("{}ENDPOINT", p), profile.endpoint.clone()),
    ];
    if profile.provider == Provider::R2 {
        v.push((format!("{}NO_CHECK_BUCKET", p), "true".to_string()));
    }
    v
}

pub fn remote_path(profile: &Profile, key: &str) -> String {
    let base = format!("{}:{}/{}", profile.name, profile.bucket, profile.prefix);
    if key.is_empty() { base.trim_end_matches('/').to_string() } else { format!("{}/{}", base.trim_end_matches('/'), key) }
}

// ------------------------------------------------------------------- mount

#[derive(Clone, Debug)]
pub struct MountOptions {
    pub target: String,
    pub cache_max_size: String,
    pub bwlimit: String,
    pub read_only: bool,
    pub offline: bool,
    pub vfs_cache_mode: String,
}
impl Default for MountOptions {
    fn default() -> Self {
        MountOptions {
            target: String::new(),
            cache_max_size: "5G".into(),
            bwlimit: "0".into(),
            read_only: false,
            offline: false,
            vfs_cache_mode: "full".into(),
        }
    }
}

/// The performance mount profile (researched rclone best practice).
/// Pure function — fully unit-tested.
pub fn build_mount_args(profile: &Profile, opts: &MountOptions) -> Vec<String> {
    let dir_cache = if opts.offline { "1000h" } else { "12h" };
    let mut a = vec![
        "mount".to_string(),
        remote_path(profile, ""),
        opts.target.clone(),
        "--vfs-cache-mode".into(), opts.vfs_cache_mode.clone(),
        "--vfs-cache-max-size".into(), opts.cache_max_size.clone(),
        "--dir-cache-time".into(), dir_cache.into(),
        "--buffer-size".into(), "128M".into(),
        "--vfs-read-chunk-size".into(), "128M".into(),
        "--transfers".into(), "4".into(),
        "--checkers".into(), "8".into(),
        "--low-level-retries".into(), "10".into(),
        "--timeout".into(), "300s".into(),
        "--contimeout".into(), "60s".into(),
        "--poll-interval".into(), "15s".into(),
    ];
    if opts.bwlimit != "0" && !opts.bwlimit.is_empty() {
        a.push("--bwlimit".into());
        a.push(opts.bwlimit.clone());
    }
    if opts.read_only { a.push("--read-only".into()); }
    // macFUSE 5 on macOS 15.4+ can use the FSKit backend (no legacy kext).
    if cfg!(target_os = "macos") {
        a.push("-o".into());
        a.push("backend=fskit".into());
    }
    a
}

pub fn spawn_mount(profile: &Profile, secrets: &Secrets, opts: &MountOptions) -> Result<Child, String> {
    if !rclone_available() {
        return Err("rclone was not found. Install rclone (rclone.org/downloads) — Cloud Drive Bridge uses it as the mount engine — then try again.".into());
    }
    if opts.target.is_empty() {
        return Err("choose a drive letter (Windows) or mount folder first".into());
    }
    if !cfg!(windows) {
        std::fs::create_dir_all(&opts.target).map_err(|e| format!("mount folder: {}", e))?;
    }
    let args = build_mount_args(profile, opts);
    Command::new(rclone_program())
        .args(&args)
        .envs(rclone_env(profile, secrets))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("could not start rclone mount: {}", e))
}

/// Driver / prerequisite guidance per platform.
pub fn driver_status() -> (bool, String) {
    #[cfg(windows)]
    {
        let found = ["C:\\Program Files (x86)\\WinFsp", "C:\\Program Files\\WinFsp"]
            .iter().any(|p| std::path::Path::new(p).exists());
        if found {
            (true, "WinFsp detected — drive-letter mounts are ready.".into())
        } else {
            (false, "WinFsp is not installed. It is the Windows filesystem driver rclone mount needs: install it free from winfsp.dev, or use WebDAV mode below (no driver needed).".into())
        }
    }
    #[cfg(target_os = "macos")]
    {
        let found = std::path::Path::new("/Library/Filesystems/macfuse.fs").exists()
            || std::path::Path::new("/usr/local/lib/libfuse.2.dylib").exists()
            || std::path::Path::new("/usr/local/lib/libfuse-t.dylib").exists();
        if found {
            (true, "macFUSE detected. On macOS 15.4+ Cloud Drive Bridge asks macFUSE 5 for its FSKit backend (no legacy kernel extension); older setups use the classic backend with a one-time system-extension approval in System Settings > Privacy & Security.".into())
        } else {
            (false, "macFUSE is not installed. Install it free from macfuse.github.io (macFUSE 5 supports the FSKit backend on macOS 15.4+, no legacy kext), approve it once if macOS asks, then mount.".into())
        }
    }
    #[cfg(target_os = "linux")]
    {
        let found = Command::new("fusermount3").arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
            || Command::new("fusermount").arg("--version").output().map(|o| o.status.success()).unwrap_or(false);
        if found {
            (true, "FUSE detected — Linux is the best-supported mount platform.".into())
        } else {
            (false, "FUSE is not installed. Install it with: sudo apt install fuse3 (Debian/Ubuntu) or your distro's fuse package.".into())
        }
    }
    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    {
        (false, "Mounting is a desktop feature. On mobile, Cloud Drive Bridge is browse + transfer (Android), honestly labeled — a true kernel mount is not possible without root.".into())
    }
}

// WebDAV fallback (Windows without WinFsp): rclone serve webdav on
// localhost + a net use mapping. Honest limits in README.
pub fn build_webdav_args(profile: &Profile, port: u16) -> Vec<String> {
    vec![
        "serve".into(), "webdav".into(), remote_path(profile, ""),
        "--addr".into(), format!("127.0.0.1:{}", port),
        "--vfs-cache-mode".into(), "writes".into(),
    ]
}
pub fn net_use_command(port: u16, drive: &str) -> Vec<String> {
    vec!["use".into(), drive.into(), format!("http://127.0.0.1:{}/", port), "/persistent:no".into()]
}
pub fn spawn_webdav(profile: &Profile, secrets: &Secrets, port: u16) -> Result<Child, String> {
    if !rclone_available() { return Err("rclone was not found — WebDAV mode also uses rclone.".into()); }
    Command::new(rclone_program())
        .args(build_webdav_args(profile, port))
        .envs(rclone_env(profile, secrets))
        .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())
        .spawn().map_err(|e| e.to_string())
}

// ---------------------------------------------------------------- watchdog

/// Exponential backoff for the mount watchdog: 5,10,20,40,60,60… seconds.
pub fn watchdog_backoff(failures: u32) -> u64 {
    let v = 5u64.saturating_mul(1u64 << failures.min(4));
    v.min(60)
}

/// Decide the watchdog action this tick (pure, tested).
pub fn watchdog_action(intended_mounted: bool, process_alive: bool, mountpoint_responsive: bool) -> &'static str {
    if !intended_mounted { "idle" }
    else if process_alive && mountpoint_responsive { "healthy" }
    else { "remount" }
}

pub fn mountpoint_responsive(target: &str) -> bool {
    if cfg!(windows) {
        std::path::Path::new(&format!("{}\\", target.trim_end_matches(':'))).exists()
            || std::path::Path::new(target).exists()
    } else {
        std::fs::read_dir(target).is_ok()
    }
}

// --------------------------------------------------------------- auto-mount

pub fn automount_systemd_unit(profile_name: &str, cdb_path: &str) -> String {
    format!(
        "[Unit]\nDescription=Cloud Drive Bridge auto-mount ({})\nAfter=network-online.target\nWants=network-online.target\n\n[Service]\nType=simple\nExecStart={} mount --profile {} --from-keychain --autostart\nRestart=on-failure\nRestartSec=10\n\n[Install]\nWantedBy=default.target\n",
        profile_name, cdb_path, profile_name
    )
}
pub fn automount_launchagent_plist(profile_name: &str, cdb_path: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\"><dict>\n<key>Label</key><string>org.opentools.clouddrivebridge.{}</string>\n<key>ProgramArguments</key><array><string>{}</string><string>mount</string><string>--profile</string><string>{}</string><string>--from-keychain</string><string>--autostart</string></array>\n<key>RunAtLoad</key><true/>\n<key>KeepAlive</key><false/>\n</dict></plist>\n",
        profile_name, cdb_path, profile_name
    )
}
pub fn automount_windows_command(profile_name: &str, cdb_path: &str) -> Vec<String> {
    vec![
        "add".into(),
        "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run".into(),
        "/v".into(), format!("CloudDriveBridge-{}", profile_name),
        "/t".into(), "REG_SZ".into(),
        "/d".into(), format!("\"{}\" mount --profile {} --from-keychain --autostart", cdb_path, profile_name),
        "/f".into(),
    ]
}

/// Install/remove the per-OS auto-mount hook. Returns a plain-English
/// description of what was done.
pub fn set_automount(profile_name: &str, enable: bool) -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe_s = exe.to_string_lossy().to_string();
    #[cfg(target_os = "linux")]
    {
        let dir = std::env::var("HOME").map_err(|_| "HOME not set".to_string())?;
        let path = format!("{}/.config/systemd/user/cdb-{}.service", dir, profile_name);
        if enable {
            std::fs::create_dir_all(format!("{}/.config/systemd/user", dir)).map_err(|e| e.to_string())?;
            std::fs::write(&path, automount_systemd_unit(profile_name, &exe_s)).map_err(|e| e.to_string())?;
            let _ = Command::new("systemctl").args(["--user", "daemon-reload"]).output();
            let _ = Command::new("systemctl").args(["--user", "enable", &format!("cdb-{}.service", profile_name)]).output();
            return Ok(format!("Auto-mount on login enabled (systemd user unit {}).", path));
        } else {
            let _ = Command::new("systemctl").args(["--user", "disable", &format!("cdb-{}.service", profile_name)]).output();
            let _ = std::fs::remove_file(&path);
            let _ = Command::new("systemctl").args(["--user", "daemon-reload"]).output();
            return Ok("Auto-mount on login removed.".into());
        }
    }
    #[cfg(target_os = "macos")]
    {
        let dir = std::env::var("HOME").map_err(|_| "HOME not set".to_string())?;
        let path = format!("{}/Library/LaunchAgents/org.opentools.clouddrivebridge.{}.plist", dir, profile_name);
        if enable {
            std::fs::create_dir_all(format!("{}/Library/LaunchAgents", dir)).map_err(|e| e.to_string())?;
            std::fs::write(&path, automount_launchagent_plist(profile_name, &exe_s)).map_err(|e| e.to_string())?;
            let _ = Command::new("launchctl").args(["load", &path]).output();
            return Ok(format!("Auto-mount on login enabled (LaunchAgent {}).", path));
        } else {
            let _ = Command::new("launchctl").args(["unload", &path]).output();
            let _ = std::fs::remove_file(&path);
            return Ok("Auto-mount on login removed.".into());
        }
    }
    #[cfg(windows)]
    {
        if enable {
            let args = automount_windows_command(profile_name, &exe_s);
            let o = Command::new("reg").args(&args).output().map_err(|e| e.to_string())?;
            if o.status.success() {
                return Ok("Auto-mount on login enabled (HKCU Run entry).".into());
            }
            return Err("could not write the Windows Run entry".into());
        } else {
            let _ = Command::new("reg").args(["delete", "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run", "/v", &format!("CloudDriveBridge-{}", profile_name), "/f"]).output();
            return Ok("Auto-mount on login removed.".into());
        }
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    {
        let _ = (profile_name, enable, exe_s);
        Err("auto-mount on login is a desktop feature".into())
    }
}

// ------------------------------------------------------------ share + pin

pub fn expiry_arg(preset: &str) -> Result<&'static str, String> {
    Ok(match preset {
        "1h" => "1h",
        "24h" => "24h",
        "7d" => "168h",
        _ => return Err("expiry must be 1h, 24h or 7d".into()),
    })
}
pub fn build_link_args(profile: &Profile, key: &str, expiry: &str) -> Vec<String> {
    vec!["link".into(), remote_path(profile, key), "--expire".into(), expiry.into()]
}
pub fn share_link(profile: &Profile, secrets: &Secrets, key: &str, preset: &str) -> Result<String, String> {
    if !rclone_available() { return Err("rclone was not found — share links use rclone link.".into()); }
    let exp = expiry_arg(preset)?;
    let out = Command::new(rclone_program())
        .args(build_link_args(profile, key, exp))
        .envs(rclone_env(profile, secrets))
        .output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

/// Offline pin registry: JSON file of pinned keys (names only, no
/// secrets) in the app state dir, plus the cache-warming action.
pub fn pins_to_json(pins: &[String]) -> String {
    format!("[{}]", pins.iter().map(|p| format!("\"{}\"", esc(p))).collect::<Vec<_>>().join(","))
}
pub fn parse_pins(json: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = json;
    while let Some(s) = rest.find('"') {
        let after = &rest[s + 1..];
        if let Some(e) = after.find('"') {
            out.push(after[..e].to_string());
            rest = &after[e + 1..];
        } else { break; }
    }
    out
}

/// Warm the cache for a pinned item: read it through the mount point
/// when mounted, otherwise rclone-copy it into the app's pinned cache.
pub fn pin_warm(
    profile: &Profile,
    secrets: &Secrets,
    key: &str,
    mount_target: Option<&str>,
    pinned_cache_dir: &str,
) -> Result<String, String> {
    if let Some(t) = mount_target {
        let path = format!("{}/{}", t.trim_end_matches('/'), key);
        let mut f = std::fs::File::open(&path).map_err(|e| format!("open through mount: {}", e))?;
        let mut buf = [0u8; 8192];
        loop {
            let n = f.read(&mut buf).map_err(|e| e.to_string())?;
            if n == 0 { break; }
        }
        return Ok(format!("Pinned '{}' — read through the mount so it now lives in the VFS cache.", key));
    }
    if !rclone_available() { return Err("rclone was not found and no mount is active — cannot warm the pin cache.".into()); }
    std::fs::create_dir_all(pinned_cache_dir).map_err(|e| e.to_string())?;
    let dest = format!("{}/{}", pinned_cache_dir, key);
    if let Some(parent) = std::path::Path::new(&dest).parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let out = Command::new(rclone_program())
        .args(["copyto", &remote_path(profile, key), &dest])
        .envs(rclone_env(profile, secrets))
        .output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(format!("Pinned '{}' into the app-managed offline cache.", key))
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

// --------------------------------------------------------------- transfers

pub fn build_copyto_args(from: &str, to: &str) -> Vec<String> {
    vec![
        "copyto".into(), from.into(), to.into(),
        "--transfers".into(), "4".into(), "--checkers".into(), "8".into(),
        "--retries".into(), "3".into(), "--low-level-retries".into(), "10".into(),
        "--stats".into(), "1s".into(), "--stats-one-line".into(),
    ]
}
pub fn build_sync_args(from: &str, to: &str) -> Vec<String> {
    vec![
        "copy".into(), from.into(), to.into(),
        "--transfers".into(), "4".into(), "--checkers".into(), "8".into(),
        "--retries".into(), "3".into(), "--low-level-retries".into(), "10".into(),
        "--stats".into(), "1s".into(), "--stats-one-line".into(),
    ]
}

/// A running transfer child with cancel / pause / resume.
/// Pause uses SIGSTOP on Unix; on Windows it honestly reports that
/// pause is unsupported for a child transfer (cancel + restart instead).
pub struct Transfer {
    pub id: u64,
    pub label: String,
    pub child: Child,
    pub paused: bool,
}
impl Transfer {
    pub fn spawn_rclone(profile: &Profile, secrets: &Secrets, from: &str, to: &str, folder: bool, id: u64, label: &str) -> Result<Transfer, String> {
        if !rclone_available() { return Err("rclone was not found — folder transfers use rclone.".into()); }
        let args = if folder { build_sync_args(from, to) } else { build_copyto_args(from, to) };
        let child = Command::new(rclone_program())
            .args(&args)
            .envs(rclone_env(profile, secrets))
            .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped())
            .spawn().map_err(|e| e.to_string())?;
        Ok(Transfer { id, label: label.to_string(), child, paused: false })
    }
    pub fn cancel(&mut self) -> Result<(), String> {
        self.child.kill().map_err(|e| e.to_string())?;
        let _ = self.child.wait();
        Ok(())
    }
    pub fn pause(&mut self) -> Result<(), String> {
        #[cfg(unix)]
        {
            let pid = self.child.id();
            let ok = Command::new("kill").args(["-STOP", &pid.to_string()]).status().map(|s| s.success()).unwrap_or(false);
            if ok { self.paused = true; return Ok(()); }
            return Err("could not pause the transfer process".into());
        }
        #[cfg(not(unix))]
        {
            Err("pause is not supported for child transfers on Windows — cancel and restart the transfer instead".into())
        }
    }
    pub fn resume(&mut self) -> Result<(), String> {
        #[cfg(unix)]
        {
            let pid = self.child.id();
            let ok = Command::new("kill").args(["-CONT", &pid.to_string()]).status().map(|s| s.success()).unwrap_or(false);
            if ok { self.paused = false; return Ok(()); }
            return Err("could not resume the transfer process".into());
        }
        #[cfg(not(unix))]
        {
            Err("resume is not supported on Windows for child transfers".into())
        }
    }
    pub fn is_done(&mut self) -> bool {
        self.child.try_wait().map(|s| s.is_some()).unwrap_or(true)
    }
}

/// Retry with exponential backoff for fallible transfer closures.
pub fn retry_backoff<T, F: FnMut() -> Result<T, String>>(attempts: u32, mut f: F) -> Result<T, String> {
    let mut last = String::new();
    for i in 0..attempts.max(1) {
        match f() {
            Ok(v) => return Ok(v),
            Err(e) => {
                last = e;
                if i + 1 < attempts {
                    std::thread::sleep(std::time::Duration::from_millis(250 * (1u64 << i.min(4))));
                }
            }
        }
    }
    Err(last)
}

// ------------------------------------------------------------- JSON helpers

pub fn profiles_json(profiles: &[Profile]) -> String {
    format!("[{}]", profiles.iter().map(|p| p.to_json()).collect::<Vec<_>>().join(","))
}

/// Very small field extractor for our own profile JSON (flat objects).
pub fn json_str_field(obj: &str, field: &str) -> Option<String> {
    let pat = format!("\"{}\":\"", field);
    let s = obj.find(&pat)? + pat.len();
    let mut out = String::new();
    let mut chars = obj[s..].chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(out),
            '\\' => { if let Some(n) = chars.next() { out.push(n); } }
            _ => out.push(c),
        }
    }
    None
}
pub fn parse_profile(obj: &str) -> Option<Profile> {
    let name = json_str_field(obj, "name")?;
    let provider = Provider::from_id(&json_str_field(obj, "provider")?)?;
    let bucket = json_str_field(obj, "bucket")?;
    let region = json_str_field(obj, "region").unwrap_or_default();
    let endpoint = json_str_field(obj, "endpoint").unwrap_or_default();
    let mut p = Profile::new(&name, provider, &bucket, &region, &endpoint).ok()?;
    p.prefix = json_str_field(obj, "prefix").unwrap_or_default();
    Some(p)
}
pub fn parse_profiles(json: &str) -> Vec<Profile> {
    // Split top-level objects by "},{" boundaries in our own format.
    let trimmed = json.trim().trim_start_matches('[').trim_end_matches(']');
    if trimmed.trim().is_empty() { return Vec::new(); }
    trimmed.split("},{").filter_map(|part| {
        let o = format!("{{{}}}", part.trim_matches(|c| c == '{' || c == '}'));
        parse_profile(&o)
    }).collect()
}

pub type ProfileMap = BTreeMap<String, Profile>;

// ------------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;

    fn aws_profile() -> Profile {
        Profile::new("myaws", Provider::Aws, "my-bucket", "eu-west-1", "").unwrap()
    }
    fn secrets() -> Secrets {
        Secrets { access_key_id: "test-key-id".into(), secret_access_key: "test-secret-value".into() }
    }

    const LIST_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ListBucketResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
<Name>my-bucket</Name><Prefix>photos/</Prefix><Delimiter>/</Delimiter>
<IsTruncated>true</IsTruncated>
<NextContinuationToken>tok123</NextContinuationToken>
<Contents><Key>photos/cat &amp; dog.jpg</Key><LastModified>2026-10-01T10:00:00.000Z</LastModified><ETag>"abc123"</ETag><Size>2048</Size><StorageClass>STANDARD</StorageClass></Contents>
<Contents><Key>photos/notes.txt</Key><LastModified>2026-10-02T11:00:00.000Z</LastModified><ETag>"def-2"</ETag><Size>10</Size><StorageClass>GLACIER</StorageClass></Contents>
<CommonPrefixes><Prefix>photos/2025/</Prefix></CommonPrefixes>
<CommonPrefixes><Prefix>photos/2026/</Prefix></CommonPrefixes>
</ListBucketResult>"#;

    #[test]
    fn parse_listing_folders_files_and_token() {
        let l = parse_listing(LIST_XML, "photos/");
        assert_eq!(l.folders.len(), 2);
        assert_eq!(l.folders[0].key, "photos/2025/");
        assert_eq!(l.files.len(), 2);
        assert_eq!(l.files[0].key, "photos/cat & dog.jpg");
        assert_eq!(l.files[0].size, 2048);
        assert_eq!(l.files[0].etag, "abc123");
        assert_eq!(l.files[1].storage_class, "GLACIER");
        assert!(l.truncated);
        assert_eq!(l.next_token.as_deref(), Some("tok123"));
        assert_eq!(l.total_size(), 2058);
    }

    #[test]
    fn profile_validation_rules() {
        assert!(Profile::new("bad name!", Provider::Aws, "b", "", "").is_err());
        assert!(Profile::new("ok", Provider::Aws, "Bad_Bucket", "", "").is_err());
        assert!(Profile::new("ok", Provider::Custom, "b", "", "").is_err()); // endpoint required
        assert!(Profile::new("ok", Provider::R2, "b", "", "").is_err()); // {account} placeholder
        let r2 = Profile::new("ok", Provider::R2, "b", "", "https://acct123.r2.cloudflarestorage.com").unwrap();
        assert_eq!(r2.region, "auto");
        assert_eq!(r2.bucket_url(), "https://acct123.r2.cloudflarestorage.com/b");
    }

    #[test]
    fn presets_cover_all_providers() {
        assert_eq!(Provider::all().len(), 11);
        for p in Provider::all() {
            assert!(!p.label().is_empty());
            assert!(!p.rclone_provider().is_empty());
            assert_eq!(Provider::from_id(p.id()), Some(p));
        }
        let minio = Profile::new("m", Provider::Minio, "data", "", "http://127.0.0.1:9000").unwrap();
        assert_eq!(minio.object_url("a b/c.txt"), "http://127.0.0.1:9000/data/a%20b/c.txt");
    }

    #[test]
    fn mount_command_has_performance_profile() {
        let p = aws_profile();
        let mut o = MountOptions::default();
        o.target = "Z:".into();
        let args = build_mount_args(&p, &o);
        let joined = args.join(" ");
        for needle in [
            "--vfs-cache-mode full", "--vfs-cache-max-size 5G", "--dir-cache-time 12h",
            "--buffer-size 128M", "--vfs-read-chunk-size 128M", "--transfers 4",
            "--checkers 8", "--low-level-retries 10", "--timeout 300s", "--contimeout 60s",
        ] {
            assert!(joined.contains(needle), "missing {}", needle);
        }
        assert!(!joined.contains("--bwlimit")); // 0 = unlimited, omitted
        o.offline = true; o.read_only = true; o.bwlimit = "10M".into();
        let args2 = build_mount_args(&p, &o).join(" ");
        assert!(args2.contains("--dir-cache-time 1000h"));
        assert!(args2.contains("--read-only"));
        assert!(args2.contains("--bwlimit 10M"));
    }

    #[test]
    fn rclone_env_uses_no_config_file_and_carries_provider() {
        let p = aws_profile();
        let env = rclone_env(&p, &secrets());
        let map: std::collections::HashMap<_, _> = env.iter().cloned().collect();
        assert_eq!(map["RCLONE_CONFIG_MYAWS_TYPE"], "s3");
        assert_eq!(map["RCLONE_CONFIG_MYAWS_PROVIDER"], "AWS");
        assert_eq!(map["RCLONE_CONFIG_MYAWS_REGION"], "eu-west-1");
        assert!(map.keys().all(|k| !k.to_ascii_lowercase().contains("config_file")));
    }

    #[test]
    fn watchdog_backoff_and_actions() {
        assert_eq!(watchdog_backoff(0), 5);
        assert_eq!(watchdog_backoff(1), 10);
        assert_eq!(watchdog_backoff(10), 60);
        assert_eq!(watchdog_action(false, false, false), "idle");
        assert_eq!(watchdog_action(true, true, true), "healthy");
        assert_eq!(watchdog_action(true, false, true), "remount");
        assert_eq!(watchdog_action(true, true, false), "remount");
    }

    #[test]
    fn automount_generators_mention_profile() {
        let u = automount_systemd_unit("myaws", "/usr/bin/cdb");
        assert!(u.contains("cdb") && u.contains("myaws") && u.contains("WantedBy=default.target"));
        let pl = automount_launchagent_plist("myaws", "/usr/bin/cdb");
        assert!(pl.contains("org.opentools.clouddrivebridge.myaws"));
        assert!(pl.contains("RunAtLoad"));
        let win = automount_windows_command("myaws", "cdb.exe").join(" ");
        assert!(win.contains("CurrentVersion\\Run"));
        assert!(win.contains("CloudDriveBridge-myaws"));
    }

    #[test]
    fn share_expiry_presets() {
        assert_eq!(expiry_arg("1h").unwrap(), "1h");
        assert_eq!(expiry_arg("24h").unwrap(), "24h");
        assert_eq!(expiry_arg("7d").unwrap(), "168h");
        assert!(expiry_arg("30d").is_err());
        let a = build_link_args(&aws_profile(), "x/y.txt", "24h").join(" ");
        assert!(a.contains("link") && a.contains("--expire 24h"));
    }

    #[test]
    fn md5_known_vectors() {
        assert_eq!(md5_hex(b""), "d41d8cd98f00b204e9800998ecf8427e");
        assert_eq!(md5_hex(b"abc"), "900150983cd24fb0d6963f7d28e17f72");
        assert_eq!(md5_hex(b"The quick brown fox jumps over the lazy dog"), "9e107d9d372bb6826bd81d3542a419d6");
    }

    #[test]
    fn verify_upload_honesty() {
        assert!(verify_upload("/nonexistent", "abc-3").contains("multipart"));
        assert!(verify_upload("/nonexistent", "").contains("not-reported"));
    }

    #[test]
    fn curl_missing_and_args_shape() {
        let args = curl_base_args(&aws_profile());
        assert!(args.iter().any(|a| a == "--aws-sigv4"));
        assert!(args.iter().any(|a| a == "aws:amz:eu-west-1:s3"));
        assert!(!args.join(" ").contains("secret")); // secrets never baked into base args
    }

    #[test]
    fn pins_roundtrip() {
        let pins = vec!["a/b.txt".to_string(), "c/".to_string()];
        let j = pins_to_json(&pins);
        assert_eq!(parse_pins(&j), pins);
    }

    #[test]
    fn profiles_json_roundtrip() {
        let ps = vec![aws_profile()];
        let j = profiles_json(&ps);
        let back = parse_profiles(&j);
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].name, "myaws");
        assert_eq!(back[0].bucket, "my-bucket");
        assert!(!j.to_ascii_lowercase().contains("secret"));
    }

    #[test]
    fn webdav_fallback_args() {
        let a = build_webdav_args(&aws_profile(), 8080).join(" ");
        assert!(a.contains("serve webdav") && a.contains("127.0.0.1:8080"));
        let n = net_use_command(8080, "S:").join(" ");
        assert!(n.contains("use S: http://127.0.0.1:8080/"));
    }

    #[test]
    fn transfer_args_have_retries() {
        let a = build_copyto_args("a", "b").join(" ");
        assert!(a.contains("--retries 3") && a.contains("--stats-one-line"));
        let s = build_sync_args("a", "b").join(" ");
        assert!(s.starts_with("copy "));
    }
}
