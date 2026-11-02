// Cloud Drive Bridge — Tauri library root. Desktop runs this through
// src/main.rs; Android loads this same code as a library target
// (Tauri mobile builds require a lib target).
// All S3 work runs in cdb-core on the user's own device. Keys live in
// the OS keychain; profile metadata (no secrets) lives in the app data
// dir as profiles.json.

use cdb_core::*;
use std::collections::HashMap;
use std::process::Child;
use std::sync::Mutex;
use tauri::Manager;

pub struct MountState {
    pub child: Option<Child>,
    pub profile: Option<Profile>,
    pub opts: Option<MountOptions>,
    pub intended: bool,
    pub failures: u32,
}
pub struct AppState {
    pub mount: Mutex<MountState>,
    pub transfers: Mutex<HashMap<u64, Transfer>>,
    pub next_job: Mutex<u64>,
    pub pins: Mutex<Vec<String>>,
}
impl Default for AppState {
    fn default() -> Self {
        AppState {
            mount: Mutex::new(MountState { child: None, profile: None, opts: None, intended: false, failures: 0 }),
            transfers: Mutex::new(HashMap::new()),
            next_job: Mutex::new(1),
            pins: Mutex::new(Vec::new()),
        }
    }
}

// ------------------------------------------------------------ profile store

fn profiles_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("profiles.json"))
}
fn load_profiles(app: &tauri::AppHandle) -> Vec<Profile> {
    profiles_path(app)
        .and_then(|p| std::fs::read_to_string(p).map_err(|e| e.to_string()))
        .map(|j| parse_profiles(&j))
        .unwrap_or_default()
}
fn store_profiles(app: &tauri::AppHandle, ps: &[Profile]) -> Result<(), String> {
    let path = profiles_path(app)?;
    std::fs::write(path, profiles_json(ps)).map_err(|e| e.to_string())
}
fn pins_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("pins.json"))
}
fn secrets_for(p: &Profile) -> Result<Secrets, String> {
    let (a, s) = keychain::load(&p.name)?;
    Ok(Secrets { access_key_id: a, secret_access_key: s })
}
fn find_profile(app: &tauri::AppHandle, name: &str) -> Result<Profile, String> {
    load_profiles(app).into_iter().find(|p| p.name == name)
        .ok_or_else(|| format!("profile '{}' not found — add it on the Profiles screen", name))
}

// ---------------------------------------------------------------- commands

#[tauri::command]
fn list_providers() -> String {
    let items: Vec<String> = Provider::all().iter().map(|p| {
        format!(
            "{{\"id\":\"{}\",\"label\":\"{}\",\"region\":\"{}\",\"endpoint_template\":\"{}\"}}",
            p.id(), esc(p.label()), p.default_region(), esc(p.endpoint_template())
        )
    }).collect();
    format!("[{}]", items.join(","))
}

#[tauri::command]
fn get_profiles(app: tauri::AppHandle) -> String {
    profiles_json(&load_profiles(&app))
}

#[tauri::command]
fn save_profile(
    app: tauri::AppHandle,
    name: String,
    provider: String,
    bucket: String,
    region: String,
    endpoint: String,
    access_key: String,
    secret_key: String,
) -> Result<String, String> {
    let prov = Provider::from_id(&provider).ok_or_else(|| "unknown provider".to_string())?;
    let p = Profile::new(&name, prov, &bucket, &region, &endpoint)?;
    if !access_key.trim().is_empty() || !secret_key.trim().is_empty() {
        if access_key.trim().is_empty() || secret_key.trim().is_empty() {
            return Err("enter both the access key id and the secret — or neither to keep the stored keys".into());
        }
        keychain::store(&p.name, access_key.trim(), secret_key.trim())?;
    }
    let mut ps = load_profiles(&app);
    ps.retain(|x| x.name != p.name);
    ps.push(p.clone());
    store_profiles(&app, &ps)?;
    Ok(format!("Profile '{}' saved. Keys are in your OS keychain — never in a file.", p.name))
}

#[tauri::command]
fn delete_profile(app: tauri::AppHandle, name: String) -> Result<String, String> {
    let mut ps = load_profiles(&app);
    ps.retain(|x| x.name != name);
    store_profiles(&app, &ps)?;
    let _ = keychain::remove(&name);
    Ok(format!("Profile '{}' and its keychain entry removed.", name))
}

#[tauri::command]
async fn test_profile(app: tauri::AppHandle, name: String, probe: Option<bool>) -> Result<String, String> {
    let p = find_profile(&app, &name)?;
    let s = secrets_for(&p)?;
    let r = tauri::async_runtime::spawn_blocking(move || five_point_check(&p, &s, probe.unwrap_or(false)))
        .await.map_err(|e| e.to_string())?;
    Ok(r.to_json())
}

#[tauri::command]
async fn list_objects_cmd(app: tauri::AppHandle, name: String, prefix: String) -> Result<String, String> {
    let p = find_profile(&app, &name)?;
    let s = secrets_for(&p)?;
    let full_prefix = if prefix.is_empty() { p.prefix.clone() } else { prefix };
    let fp = full_prefix.clone();
    let l = tauri::async_runtime::spawn_blocking(move || list_objects(&p, &s, &fp, ""))
        .await.map_err(|e| e.to_string())??;
    Ok(l.to_json(&full_prefix))
}

#[tauri::command]
async fn bucket_size_cmd(app: tauri::AppHandle, name: String) -> Result<String, String> {
    let p = find_profile(&app, &name)?;
    let s = secrets_for(&p)?;
    let (n, bytes) = tauri::async_runtime::spawn_blocking(move || bucket_stats(&p, &s))
        .await.map_err(|e| e.to_string())??;
    Ok(format!("{{\"objects\":{},\"bytes\":{}}}", n, bytes))
}

#[tauri::command]
async fn download_cmd(app: tauri::AppHandle, name: String, key: String) -> Result<String, String> {
    let p = find_profile(&app, &name)?;
    let s = secrets_for(&p)?;
    let dir = app.path().download_dir().or_else(|_| app.path().document_dir()).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let fname = key.rsplit('/').next().unwrap_or("download.bin").to_string();
    let local = dir.join(&fname).to_string_lossy().to_string();
    let local2 = local.clone();
    tauri::async_runtime::spawn_blocking(move || download_file(&p, &s, &key, &local2))
        .await.map_err(|e| e.to_string())??;
    Ok(format!("Saved to {}", local))
}

#[tauri::command]
async fn upload_cmd(app: tauri::AppHandle, name: String, local_path: String, key: String) -> Result<String, String> {
    let p = find_profile(&app, &name)?;
    let s = secrets_for(&p)?;
    let lp = local_path.clone();
    let etag = tauri::async_runtime::spawn_blocking(move || retry_backoff(3, || upload_file(&p, &s, &lp, &key)))
        .await.map_err(|e| e.to_string())??;
    Ok(format!("Uploaded. Checksum: {}", verify_upload(&local_path, &etag)))
}

#[tauri::command]
async fn delete_cmd(app: tauri::AppHandle, name: String, key: String) -> Result<String, String> {
    let p = find_profile(&app, &name)?;
    let s = secrets_for(&p)?;
    let msg = format!("Deleted {}", key);
    tauri::async_runtime::spawn_blocking(move || delete_object(&p, &s, &key))
        .await.map_err(|e| e.to_string())??;
    Ok(msg)
}

#[tauri::command]
async fn mkdir_cmd(app: tauri::AppHandle, name: String, prefix: String) -> Result<String, String> {
    let p = find_profile(&app, &name)?;
    let s = secrets_for(&p)?;
    let msg = format!("Folder '{}' created", prefix);
    tauri::async_runtime::spawn_blocking(move || create_folder(&p, &s, &prefix))
        .await.map_err(|e| e.to_string())??;
    Ok(msg)
}

#[tauri::command]
async fn rename_cmd(app: tauri::AppHandle, name: String, from: String, to: String) -> Result<String, String> {
    let p = find_profile(&app, &name)?;
    let s = secrets_for(&p)?;
    let msg = format!("Renamed {} -> {}", from, to);
    tauri::async_runtime::spawn_blocking(move || rename_object(&p, &s, &from, &to))
        .await.map_err(|e| e.to_string())??;
    Ok(msg)
}

#[tauri::command]
async fn share_cmd(app: tauri::AppHandle, name: String, key: String, expiry: String) -> Result<String, String> {
    let p = find_profile(&app, &name)?;
    let s = secrets_for(&p)?;
    tauri::async_runtime::spawn_blocking(move || share_link(&p, &s, &key, &expiry))
        .await.map_err(|e| e.to_string())?
}

// ------------------------------------------------------------- mount + watch

#[tauri::command]
fn driver_info() -> String {
    let (ok, msg) = driver_status();
    format!("{{\"ok\":{},\"message\":\"{}\",\"rclone\":{}}}", ok, esc(&msg), rclone_available())
}

#[tauri::command]
fn mount_cmd(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    name: String,
    target: String,
    cache: String,
    bwlimit: String,
    read_only: bool,
    offline: bool,
) -> Result<String, String> {
    let p = find_profile(&app, &name)?;
    let s = secrets_for(&p)?;
    let mut opts = MountOptions::default();
    opts.target = target.clone();
    if !cache.trim().is_empty() { opts.cache_max_size = cache.trim().to_string(); }
    if !bwlimit.trim().is_empty() { opts.bwlimit = bwlimit.trim().to_string(); }
    opts.read_only = read_only;
    opts.offline = offline;
    let child = spawn_mount(&p, &s, &opts)?;
    let mut m = state.mount.lock().unwrap();
    m.child = Some(child);
    m.profile = Some(p);
    m.opts = Some(opts);
    m.intended = true;
    m.failures = 0;
    Ok(format!("Mounted at {} — watchdog is watching it and will auto-remount if it drops.", target))
}

#[tauri::command]
fn webdav_cmd(app: tauri::AppHandle, name: String) -> Result<String, String> {
    let p = find_profile(&app, &name)?;
    let s = secrets_for(&p)?;
    let mut child = spawn_webdav(&p, &s, 8340)?;
    std::thread::spawn(move || { let _ = child.wait(); });
    Ok("WebDAV mode running at http://127.0.0.1:8340/ — on Windows map it with: net use S: http://127.0.0.1:8340/ . It stops when the app closes. Limits: no mount performance profile, large-file streaming is weaker than a real mount.".into())
}

#[tauri::command]
fn unmount_cmd(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let mut m = state.mount.lock().unwrap();
    m.intended = false;
    if let Some(mut c) = m.child.take() {
        let _ = c.kill();
        let _ = c.wait();
    }
    Ok("Unmounted. Watchdog stood down.".into())
}

#[tauri::command]
fn mount_status(state: tauri::State<'_, AppState>) -> String {
    let mut m = state.mount.lock().unwrap();
    let alive = match m.child.as_mut() {
        Some(c) => c.try_wait().map(|s| s.is_none()).unwrap_or(false),
        None => false,
    };
    let target = m.opts.as_ref().map(|o| o.target.clone()).unwrap_or_default();
    let responsive = if alive && !target.is_empty() { mountpoint_responsive(&target) } else { false };
    format!(
        "{{\"mounted\":{},\"intended\":{},\"target\":\"{}\",\"responsive\":{},\"failures\":{}}}",
        alive, m.intended, esc(&target), responsive, m.failures
    )
}

#[tauri::command]
fn automount_cmd(name: String, enable: bool) -> Result<String, String> {
    set_automount(&name, enable)
}

// ---------------------------------------------------------------- transfers

#[tauri::command]
fn transfer_folder_cmd(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    name: String,
    from: String,
    to: String,
    folder: bool,
) -> Result<String, String> {
    let p = find_profile(&app, &name)?;
    let s = secrets_for(&p)?;
    let id = {
        let mut n = state.next_job.lock().unwrap();
        let v = *n; *n += 1; v
    };
    let label = format!("{} -> {}", from, to);
    let t = Transfer::spawn_rclone(&p, &s, &from, &to, folder, id, &label)?;
    state.transfers.lock().unwrap().insert(id, t);
    Ok(format!("{{\"job\":{},\"label\":\"{}\"}}", id, esc(&label)))
}

#[tauri::command]
fn transfer_control(state: tauri::State<'_, AppState>, job: u64, action: String) -> Result<String, String> {
    let mut map = state.transfers.lock().unwrap();
    let t = map.get_mut(&job).ok_or_else(|| "no such transfer job".to_string())?;
    match action.as_str() {
        "cancel" => { t.cancel()?; map.remove(&job); Ok("cancelled".into()) }
        "pause" => { t.pause()?; Ok("paused".into()) }
        "resume" => { t.resume()?; Ok("resumed".into()) }
        "status" => Ok(if t.is_done() { "done".into() } else if t.paused { "paused".into() } else { "running".into() }),
        _ => Err("action must be cancel|pause|resume|status".into()),
    }
}

// --------------------------------------------------------------------- pins

#[tauri::command]
fn get_pins(app: tauri::AppHandle, state: tauri::State<'_, AppState>) -> String {
    let mut pins = state.pins.lock().unwrap();
    if pins.is_empty() {
        if let Ok(path) = pins_path(&app) {
            if let Ok(j) = std::fs::read_to_string(path) { *pins = parse_pins(&j); }
        }
    }
    pins_to_json(&pins)
}

#[tauri::command]
async fn pin_cmd(app: tauri::AppHandle, state: tauri::State<'_, AppState>, name: String, key: String) -> Result<String, String> {
    {
        let mut pins = state.pins.lock().unwrap();
        if !pins.contains(&key) { pins.push(key.clone()); }
        if let Ok(path) = pins_path(&app) {
            let _ = std::fs::write(path, pins_to_json(&pins));
        }
    }
    let p = find_profile(&app, &name)?;
    let s = secrets_for(&p)?;
    let (target, cache_dir) = {
        let m = state.mount.lock().unwrap();
        let t = if m.intended { m.opts.as_ref().map(|o| o.target.clone()) } else { None };
        let dir = app.path().app_cache_dir().map(|d| d.join("pinned").to_string_lossy().to_string()).unwrap_or_else(|_| "pinned".into());
        (t, dir)
    };
    tauri::async_runtime::spawn_blocking(move || pin_warm(&p, &s, &key, target.as_deref(), &cache_dir))
        .await.map_err(|e| e.to_string())?
}

#[tauri::command]
fn unpin_cmd(app: tauri::AppHandle, state: tauri::State<'_, AppState>, key: String) -> Result<String, String> {
    let mut pins = state.pins.lock().unwrap();
    pins.retain(|k| k != &key);
    if let Ok(path) = pins_path(&app) {
        let _ = std::fs::write(path, pins_to_json(&pins));
    }
    Ok(format!("'{}' unpinned. (The warmed cache copy ages out under the normal cache cap.)", key))
}

/// Watchdog thread: while a mount is intended, check the rclone child
/// and the mountpoint; remount with exponential backoff when either
/// fails. This is the fix for rclone's known flaky reconnect.
fn start_watchdog(handle: tauri::AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_secs(5));
        let state = handle.state::<AppState>();
        let action = {
            let mut m = state.mount.lock().unwrap();
            let alive = match m.child.as_mut() {
                Some(c) => c.try_wait().map(|s| s.is_none()).unwrap_or(false),
                None => false,
            };
            let target = m.opts.as_ref().map(|o| o.target.clone()).unwrap_or_default();
            let resp = if alive && !target.is_empty() { mountpoint_responsive(&target) } else { false };
            watchdog_action(m.intended, alive, resp)
        };
        if action == "remount" {
            let (profile, opts, failures) = {
                let mut m = state.mount.lock().unwrap();
                if let Some(mut c) = m.child.take() { let _ = c.kill(); let _ = c.wait(); }
                m.failures += 1;
                (m.profile.clone(), m.opts.clone(), m.failures)
            };
            if let (Some(p), Some(o)) = (profile, opts) {
                std::thread::sleep(std::time::Duration::from_secs(watchdog_backoff(failures)));
                if let Ok(s) = secrets_for(&p) {
                    if let Ok(child) = spawn_mount(&p, &s, &o) {
                        let mut m = state.mount.lock().unwrap();
                        m.child = Some(child);
                        m.failures = 0;
                    }
                }
            }
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::default())
        .setup(|app| {
            start_watchdog(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_providers, get_profiles, save_profile, delete_profile,
            test_profile, list_objects_cmd, bucket_size_cmd, download_cmd,
            upload_cmd, delete_cmd, mkdir_cmd, rename_cmd, share_cmd,
            driver_info, mount_cmd, webdav_cmd, unmount_cmd, mount_status,
            automount_cmd, transfer_folder_cmd, transfer_control,
            get_pins, pin_cmd, unpin_cmd
        ])
        .run(tauri::generate_context!())
        .expect("error while running Cloud Drive Bridge");
}
