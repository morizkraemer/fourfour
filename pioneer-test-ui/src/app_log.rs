use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const LOG_NAME: &str = "fourfour";
const MAX_FILE_BYTES: u64 = 5 * 1024 * 1024;
const ROTATED_FILES: usize = 2;
const FRONTEND_MESSAGE_CHARS: usize = 4000;
const FRONTEND_BURST: usize = 20;
const FRONTEND_WINDOW: Duration = Duration::from_secs(10);

static LOG: OnceLock<RotatingLog> = OnceLock::new();
static FRONTEND_LIMIT: Mutex<Option<RateLimit>> = Mutex::new(None);

struct Current {
    file: File,
    bytes: u64,
}

pub struct RotatingLog {
    dir: PathBuf,
    max_bytes: u64,
    rotated: usize,
    current: Mutex<Current>,
}

impl RotatingLog {
    pub fn open(dir: &Path, max_bytes: u64, rotated: usize) -> std::io::Result<Self> {
        fs::create_dir_all(dir)?;
        let file = open_append(&active_path(dir))?;
        let bytes = file.metadata()?.len();
        Ok(Self { dir: dir.to_path_buf(), max_bytes, rotated, current: Mutex::new(Current { file, bytes }) })
    }

    pub fn write(&self, level: &str, message: &str) {
        let record = format_record(chrono::Utc::now(), level, std::thread::current().name(), message);
        let mut current = self.current.lock().unwrap_or_else(|e| e.into_inner());
        if current.bytes > 0 && current.bytes + record.len() as u64 > self.max_bytes {
            if let Err(e) = self.rotate(&mut current) {
                eprintln!("log rotation failed: {e}");
            }
        }
        match current.file.write_all(record.as_bytes()) {
            Ok(()) => current.bytes += record.len() as u64,
            Err(e) => eprintln!("log write failed: {e}"),
        }
    }

    fn rotate(&self, current: &mut Current) -> std::io::Result<()> {
        for index in (1..=self.rotated).rev() {
            let from = if index == 1 { active_path(&self.dir) } else { rotated_path(&self.dir, index - 1) };
            if from.exists() {
                fs::rename(&from, rotated_path(&self.dir, index))?;
            }
        }
        current.file = open_append(&active_path(&self.dir))?;
        current.bytes = 0;
        Ok(())
    }
}

fn open_append(path: &Path) -> std::io::Result<File> {
    OpenOptions::new().create(true).append(true).open(path)
}

fn active_path(dir: &Path) -> PathBuf {
    dir.join(format!("{LOG_NAME}.log"))
}

fn rotated_path(dir: &Path, index: usize) -> PathBuf {
    dir.join(format!("{LOG_NAME}.{index}.log"))
}

fn format_record(at: chrono::DateTime<chrono::Utc>, level: &str, thread: Option<&str>, message: &str) -> String {
    let mut lines = message.lines();
    let mut record = format!(
        "{} {:<5} [{}] {}\n",
        at.format("%Y-%m-%dT%H:%M:%S%.3fZ"),
        level.to_uppercase(),
        thread.unwrap_or("unnamed"),
        lines.next().unwrap_or("")
    );
    for line in lines {
        record.push_str("    ");
        record.push_str(line);
        record.push('\n');
    }
    record
}

pub struct RateLimit {
    window: Duration,
    burst: usize,
    window_start: Instant,
    accepted: usize,
    dropped: usize,
}

pub enum Verdict {
    Accept { dropped_before: usize },
    Drop,
}

impl RateLimit {
    pub fn new(window: Duration, burst: usize, now: Instant) -> Self {
        Self { window, burst, window_start: now, accepted: 0, dropped: 0 }
    }

    pub fn check(&mut self, now: Instant) -> Verdict {
        if now.duration_since(self.window_start) >= self.window {
            self.window_start = now;
            self.accepted = 0;
        }
        if self.accepted >= self.burst {
            self.dropped += 1;
            return Verdict::Drop;
        }
        self.accepted += 1;
        Verdict::Accept { dropped_before: std::mem::take(&mut self.dropped) }
    }
}

pub fn init(dir: &Path) {
    match RotatingLog::open(dir, MAX_FILE_BYTES, ROTATED_FILES) {
        Ok(log) => {
            if LOG.set(log).is_ok() {
                info(&format!(
                    "fourfour {} started on {} {}, log folder {}",
                    env!("CARGO_PKG_VERSION"),
                    std::env::consts::OS,
                    std::env::consts::ARCH,
                    dir.display()
                ));
            }
        }
        Err(e) => eprintln!("could not open the log in {}: {e}", dir.display()),
    }
}

pub fn log_dir_for(identifier: &str, env: impl Fn(&str) -> Option<std::ffi::OsString>) -> Option<PathBuf> {
    let absolute = |name: &str| env(name).map(PathBuf::from).filter(|p| p.is_absolute());
    #[cfg(target_os = "macos")]
    return absolute("HOME").map(|home| home.join("Library/Logs").join(identifier));
    #[cfg(windows)]
    return absolute("LOCALAPPDATA").map(|base| base.join(identifier).join("logs"));
    #[cfg(not(any(target_os = "macos", windows)))]
    return absolute("XDG_DATA_HOME")
        .or_else(|| absolute("HOME").map(|home| home.join(".local/share")))
        .map(|base| base.join(identifier).join("logs"));
}

pub fn start_early(identifier: &str) {
    match log_dir_for(identifier, |name| std::env::var_os(name)) {
        Some(dir) => init(&dir),
        None => eprintln!("no log folder: could not resolve the platform log directory"),
    }
    install_panic_hook();
}

pub fn directory() -> Option<PathBuf> {
    LOG.get().map(|log| log.dir.clone())
}

pub fn write(level: &str, message: &str) {
    if let Some(log) = LOG.get() {
        log.write(level, message);
    }
}

pub fn info(message: &str) {
    write("info", message);
}

pub fn warn(message: &str) {
    eprintln!("{message}");
    write("warn", message);
}

pub fn error(message: &str) {
    eprintln!("{message}");
    write("error", message);
}

pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        write("panic", &panic_report(info));
        previous(info);
    }));
}

fn panic_report(info: &std::panic::PanicHookInfo<'_>) -> String {
    let payload = info.payload();
    let message = payload
        .downcast_ref::<&str>()
        .map(|s| (*s).to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "non-text panic payload".to_string());
    let location = info.location().map_or_else(|| "unknown location".to_string(), |l| format!("{}:{}", l.file(), l.line()));
    format!(
        "panic in fourfour {}: {message} at {location}\n{}",
        env!("CARGO_PKG_VERSION"),
        std::backtrace::Backtrace::force_capture()
    )
}

#[tauri::command]
pub fn log_directory() -> Result<String, String> {
    directory().map(|dir| dir.to_string_lossy().into_owned()).ok_or_else(|| "The log is not available.".to_string())
}

#[tauri::command]
pub fn log_frontend(level: String, message: String) {
    let verdict = {
        let mut guard = FRONTEND_LIMIT.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        guard.get_or_insert_with(|| RateLimit::new(FRONTEND_WINDOW, FRONTEND_BURST, now)).check(now)
    };
    let Verdict::Accept { dropped_before } = verdict else { return };
    if dropped_before > 0 {
        write("warn", &format!("{dropped_before} frontend messages were suppressed by the rate limit"));
    }
    let level = if level == "warn" { "warn" } else { "error" };
    let message: String = message.chars().take(FRONTEND_MESSAGE_CHARS).collect();
    write(&format!("web-{level}"), &message);
}

#[cfg(test)]
mod tests {
    use super::*;

    struct ScratchDir(PathBuf);

    impl ScratchDir {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!("ff-applog-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for ScratchDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn sizes(dir: &Path) -> Vec<(String, u64)> {
        let mut found: Vec<(String, u64)> = fs::read_dir(dir)
            .unwrap()
            .map(|e| {
                let e = e.unwrap();
                (e.file_name().to_string_lossy().into_owned(), e.metadata().unwrap().len())
            })
            .collect();
        found.sort();
        found
    }

    #[test]
    fn a_record_starts_with_a_timestamp_and_indents_continuation_lines() {
        let at = chrono::DateTime::parse_from_rfc3339("2026-11-02T10:11:12.345Z").unwrap().to_utc();
        let record = format_record(at, "warn", Some("analysis"), "first\nsecond\nthird");
        assert_eq!(record, "2026-11-02T10:11:12.345Z WARN  [analysis] first\n    second\n    third\n");
    }

    #[test]
    fn rotation_keeps_every_file_under_the_cap_and_drops_the_oldest() {
        let dir = ScratchDir::new("rotation");
        let log = RotatingLog::open(&dir.0, 1000, 2).unwrap();
        for n in 0..200 {
            log.write("info", &format!("record {n:04} {}", "x".repeat(40)));
        }
        let files = sizes(&dir.0);
        let names: Vec<&str> = files.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["fourfour.1.log", "fourfour.2.log", "fourfour.log"]);
        assert!(files.iter().all(|(_, bytes)| *bytes <= 1000), "{files:?}");
        let newest = fs::read_to_string(dir.0.join("fourfour.log")).unwrap();
        assert!(newest.contains("record 0199"));
        let oldest_kept = fs::read_to_string(dir.0.join("fourfour.2.log")).unwrap();
        assert!(!oldest_kept.contains("record 0000"));
    }

    #[test]
    fn reopening_appends_and_counts_the_existing_size() {
        let dir = ScratchDir::new("reopen");
        RotatingLog::open(&dir.0, 300, 1).unwrap().write("info", &"a".repeat(200));
        let log = RotatingLog::open(&dir.0, 300, 1).unwrap();
        log.write("info", &"b".repeat(200));
        let names: Vec<String> = sizes(&dir.0).into_iter().map(|(n, _)| n).collect();
        assert_eq!(names, ["fourfour.1.log", "fourfour.log"]);
        assert!(fs::read_to_string(dir.0.join("fourfour.1.log")).unwrap().contains(&"a".repeat(200)));
    }

    #[test]
    fn a_panic_writes_message_location_and_version_to_the_log() {
        let dir = ScratchDir::new("panic");
        init(&dir.0);
        install_panic_hook();
        let outcome = std::thread::Builder::new().name("r48-worker".into()).spawn(|| panic!("r48 marker panic")).unwrap().join();
        assert!(outcome.is_err());
        let text = fs::read_to_string(dir.0.join("fourfour.log")).unwrap();
        let line = text.lines().find(|l| l.contains("r48 marker panic")).expect("panic line in the log");
        assert!(line.contains("PANIC"), "{line}");
        assert!(line.contains("[r48-worker]"), "{line}");
        assert!(line.contains(env!("CARGO_PKG_VERSION")), "{line}");
        assert!(line.contains("app_log.rs"), "{line}");
        assert!(text.contains("\n    "), "backtrace lines are indented under the panic record");
    }

    fn env_of(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<std::ffi::OsString> {
        move |name| pairs.iter().find(|(k, _)| *k == name).map(|(_, v)| std::ffi::OsString::from(*v))
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn the_log_dir_is_library_logs_under_the_identifier_on_macos() {
        let dir = log_dir_for("com.example.app", env_of(&[("HOME", "/Users/dj")]));
        assert_eq!(dir, Some(PathBuf::from("/Users/dj/Library/Logs/com.example.app")));
        assert_eq!(log_dir_for("com.example.app", env_of(&[])), None);
    }

    #[cfg(windows)]
    #[test]
    fn the_log_dir_is_localappdata_identifier_logs_on_windows() {
        let base = "C:\\Users\\dj\\AppData\\Local";
        let dir = log_dir_for("com.example.app", env_of(&[("LOCALAPPDATA", "C:\\Users\\dj\\AppData\\Local")]));
        assert_eq!(dir, Some(PathBuf::from(base).join("com.example.app").join("logs")));
        assert_eq!(log_dir_for("com.example.app", env_of(&[])), None);
    }

    #[cfg(not(any(target_os = "macos", windows)))]
    #[test]
    fn the_log_dir_follows_xdg_data_home_then_home_on_linux() {
        let xdg = log_dir_for("com.example.app", env_of(&[("XDG_DATA_HOME", "/xdg"), ("HOME", "/home/dj")]));
        assert_eq!(xdg, Some(PathBuf::from("/xdg/com.example.app/logs")));
        let home = log_dir_for("com.example.app", env_of(&[("HOME", "/home/dj")]));
        assert_eq!(home, Some(PathBuf::from("/home/dj/.local/share/com.example.app/logs")));
        let relative = log_dir_for("com.example.app", env_of(&[("XDG_DATA_HOME", "rel"), ("HOME", "/home/dj")]));
        assert_eq!(relative, home);
        assert_eq!(log_dir_for("com.example.app", env_of(&[])), None);
    }

    const CHILD_DIR_VAR: &str = "FF_TEST_EARLY_PANIC_DIR";

    #[test]
    fn early_panic_child_entry() {
        let Some(dir) = std::env::var_os(CHILD_DIR_VAR) else { return };
        init(Path::new(&dir));
        install_panic_hook();
        panic!("r56 early marker panic");
    }

    #[test]
    fn a_panic_before_setup_in_a_separate_process_reaches_the_log_file() {
        let dir = ScratchDir::new("early-child");
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "app_log::tests::early_panic_child_entry", "--test-threads=1"])
            .env(CHILD_DIR_VAR, &dir.0)
            .output()
            .unwrap();
        assert!(!output.status.success());
        let text = fs::read_to_string(dir.0.join("fourfour.log")).unwrap();
        let line = text.lines().find(|l| l.contains("r56 early marker panic")).expect("panic line in the log");
        assert!(line.contains("PANIC"), "{line}");
    }

    #[test]
    fn the_rate_limit_accepts_a_burst_then_drops_and_reports_the_count_after_the_window() {
        let start = Instant::now();
        let mut limit = RateLimit::new(Duration::from_secs(10), 3, start);
        for _ in 0..3 {
            assert!(matches!(limit.check(start), Verdict::Accept { dropped_before: 0 }));
        }
        assert!(matches!(limit.check(start + Duration::from_secs(1)), Verdict::Drop));
        assert!(matches!(limit.check(start + Duration::from_secs(9)), Verdict::Drop));
        assert!(matches!(limit.check(start + Duration::from_secs(10)), Verdict::Accept { dropped_before: 2 }));
        assert!(matches!(limit.check(start + Duration::from_secs(10)), Verdict::Accept { dropped_before: 0 }));
    }
}
