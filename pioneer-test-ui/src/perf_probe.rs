use std::io::Write;
use std::sync::{Mutex, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tauri::Runtime;

pub const LOG_ENV: &str = "FF_PERF_LOG";
pub const PROBE_ENV: &str = "FF_PERF_PROBE";

const PROBE_JS: &str = include_str!("perf_probe.js");

static START: OnceLock<Instant> = OnceLock::new();
static LOG: Mutex<()> = Mutex::new(());

pub fn start() {
    START.get_or_init(Instant::now);
}

fn log_path() -> Option<std::ffi::OsString> {
    std::env::var_os(LOG_ENV).filter(|p| !p.is_empty())
}

pub fn mark(name: &str, values: serde_json::Value) {
    let Some(path) = log_path() else { return };
    let since_start_ms = START.get().map_or(0.0, |t| t.elapsed().as_secs_f64() * 1000.0);
    let wall_ms = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis());
    let line = serde_json::json!({ "mark": name, "ms": since_start_ms, "wall_ms": wall_ms, "values": values });
    let _guard = LOG.lock().unwrap_or_else(|e| e.into_inner());
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{line}");
    }
}

#[tauri::command]
pub fn perf_mark(name: String, values: serde_json::Value) {
    mark(&name, values);
}

pub fn install<R: Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    if log_path().is_none() {
        return builder;
    }
    let config = std::env::var(PROBE_ENV).unwrap_or_else(|_| "{}".into());
    let config: serde_json::Value = serde_json::from_str(&config).unwrap_or_else(|_| serde_json::json!({}));
    builder.append_invoke_initialization_script(format!("window.__FF_PROBE__ = {config};\n{PROBE_JS}"))
}
