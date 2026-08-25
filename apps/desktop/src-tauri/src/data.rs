//! Read-only data layer: runs `maestro <verb> --json` per configured repo and
//! emits one `snapshot` event with all repos. Never opens the sqlite store; the
//! db/wal mtimes are only read to decide when to re-run the verbs.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};

const MTIME_POLL: Duration = Duration::from_millis(1000);
const FULL_REFRESH: Duration = Duration::from_secs(30);
const VERBS: [&[&str]; 5] = [
    &["status", "--json"],
    &["ready", "--json"],
    &["attention", "--json"],
    &["work", "list", "--json"],
    &["decision", "list", "--json"],
];

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    #[serde(default)]
    pub repos: Vec<String>,
    #[serde(default)]
    pub maestro_bin: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigInfo {
    pub path: String,
    pub repos: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoSnapshot {
    pub repo: String,
    pub path: String,
    pub at: String,
    pub works: Vec<Value>,
    pub ready: Vec<String>,
    pub gated: Vec<Value>,
    pub decisions: Vec<Value>,
    pub findings: Vec<Value>,
    pub sessions: Vec<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub struct Refresh(pub Arc<AtomicBool>);

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default()
}

fn expand(p: &str) -> PathBuf {
    match p.strip_prefix("~/") {
        Some(rest) => home().join(rest),
        None => PathBuf::from(p),
    }
}

pub fn load_config(app: &AppHandle) -> (PathBuf, Config) {
    let dir = app
        .path()
        .app_config_dir()
        .unwrap_or_else(|_| home().join(".config").join("maestro-desktop"));
    let path = dir.join("config.json");
    let cfg = match fs::read_to_string(&path) {
        Ok(text) => match serde_json::from_str::<Config>(&text) {
            Ok(cfg) => cfg,
            Err(err) => {
                eprintln!("[data] config parse error {}: {}", path.display(), err);
                Config::default()
            }
        },
        Err(_) => {
            let cfg = Config::default();
            let _ = fs::create_dir_all(&dir);
            let _ = fs::write(&path, serde_json::to_string_pretty(&cfg).unwrap_or_default() + "\n");
            cfg
        }
    };
    (path, cfg)
}

fn maestro_bin(cfg: &Config) -> PathBuf {
    cfg.maestro_bin
        .as_deref()
        .map(expand)
        .unwrap_or_else(|| home().join(".local/bin/maestro"))
}

/// GUI apps start without the user's shell PATH; the shim needs `bun` and its own dir.
fn path_env(bin: &Path) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(dir) = bin.parent() {
        parts.push(dir.display().to_string());
    }
    parts.push(home().join(".bun/bin").display().to_string());
    parts.push(home().join(".local/bin").display().to_string());
    for p in ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin", "/bin"] {
        parts.push(p.to_string());
    }
    if let Some(existing) = std::env::var_os("PATH") {
        parts.push(existing.to_string_lossy().to_string());
    }
    parts.join(":")
}

fn run_verb(bin: &Path, repo: &Path, args: &[&str]) -> Result<Value, String> {
    let out = Command::new(bin)
        .args(args)
        .current_dir(repo)
        .env("PATH", path_env(bin))
        .output()
        .map_err(|e| format!("spawn {}: {}", bin.display(), e))?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: Value = serde_json::from_str(stdout.trim())
        .map_err(|e| format!("{}: {} ({})", args.join(" "), e, String::from_utf8_lossy(&out.stderr).trim()))?;
    if value.get("ok") == Some(&Value::Bool(true)) {
        Ok(value.get("data").cloned().unwrap_or(Value::Null))
    } else {
        Err(value
            .pointer("/error/message")
            .and_then(Value::as_str)
            .unwrap_or("maestro returned ok=false")
            .to_string())
    }
}

fn arr(v: &Value, key: &str) -> Vec<Value> {
    v.get(key).and_then(Value::as_array).cloned().unwrap_or_default()
}

fn now_iso() -> String {
    let secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // RFC3339 without pulling a time crate; JS parses it fine.
    let days = secs / 86400;
    let (h, m, s) = ((secs % 86400) / 3600, (secs % 3600) / 60, secs % 60);
    let (y, mo, d) = civil_from_days(days as i64);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{m:02}:{s:02}Z")
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn snapshot_repo(bin: &Path, repo: &Path) -> RepoSnapshot {
    let name = repo
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| repo.display().to_string());
    let mut snap = RepoSnapshot { repo: name, path: repo.display().to_string(), at: now_iso(), ..Default::default() };
    let results: Vec<Result<Value, String>> = std::thread::scope(|scope| {
        let handles: Vec<_> = VERBS.iter().map(|args| scope.spawn(move || run_verb(bin, repo, args))).collect();
        handles.into_iter().map(|h| h.join().unwrap_or_else(|_| Err("verb thread panicked".into()))).collect()
    });
    let mut errors = Vec::new();
    let mut take = |i: usize| -> Value {
        match &results[i] {
            Ok(v) => v.clone(),
            Err(e) => {
                errors.push(e.clone());
                Value::Null
            }
        }
    };
    let status = take(0);
    let ready = take(1);
    let attention = take(2);
    let work = take(3);
    let decision = take(4);
    snap.sessions = arr(&status, "sessions");
    snap.ready = arr(&ready, "works")
        .iter()
        .filter_map(|w| w.as_str().map(str::to_string).or_else(|| w.get("id").and_then(Value::as_str).map(str::to_string)))
        .collect();
    snap.gated = arr(&ready, "gated");
    snap.findings = arr(&attention, "detections");
    snap.works = arr(&work, "works");
    snap.decisions = arr(&decision, "decisions");
    if !errors.is_empty() {
        snap.error = Some(errors.join("; "));
    }
    snap
}

fn store_stamp(repo: &Path) -> Option<SystemTime> {
    ["maestro.db", "maestro.db-wal"]
        .iter()
        .filter_map(|f| fs::metadata(repo.join(".maestro").join(f)).ok())
        .filter_map(|m| m.modified().ok())
        .max()
}

pub fn start(app: AppHandle) {
    let (path, cfg) = load_config(&app);
    let repos: Vec<PathBuf> = cfg.repos.iter().map(|r| expand(r)).collect();
    let bin = maestro_bin(&cfg);
    eprintln!("[data] config {} repos={} bin={}", path.display(), repos.len(), bin.display());
    let _ = app.emit(
        "config",
        ConfigInfo { path: path.display().to_string(), repos: repos.iter().map(|r| r.display().to_string()).collect() },
    );
    std::thread::spawn(move || {
        let mut stamps: Vec<Option<SystemTime>> = repos.iter().map(|_| None).collect();
        let mut last_full = Instant::now() - FULL_REFRESH;
        loop {
            let forced = app.state::<Refresh>().0.swap(false, Ordering::SeqCst);
            let current: Vec<Option<SystemTime>> = repos.iter().map(|r| store_stamp(r)).collect();
            let due = forced || current != stamps || last_full.elapsed() >= FULL_REFRESH;
            if due {
                let started = Instant::now();
                let snaps: Vec<RepoSnapshot> = std::thread::scope(|scope| {
                    let handles: Vec<_> = repos.iter().map(|r| scope.spawn(|| snapshot_repo(&bin, r))).collect();
                    handles.into_iter().filter_map(|h| h.join().ok()).collect()
                });
                eprintln!(
                    "[data] refresh {} repos in {} ms{}",
                    snaps.len(),
                    started.elapsed().as_millis(),
                    if forced { " (forced)" } else if current != stamps { " (store changed)" } else { "" }
                );
                let _ = app.emit("snapshot", &snaps);
                stamps = current;
                last_full = Instant::now();
            }
            std::thread::sleep(MTIME_POLL);
        }
    });
}
