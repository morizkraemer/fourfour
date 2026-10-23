//! Sync several USB sticks at once.
//!
//! The library is read once on the caller's thread ([`LocalLibrary::prepare_multi_sync`]).
//! The prepared value owns plain data only, so the actual writing
//! ([`PreparedMultiSync::run`]) runs without the library and without any lock,
//! one thread per stick. A failing stick never stops the others.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use pioneer_usb_writer::models::{AnalysisResult, Playlist, SyncReport, Track};
use pioneer_usb_writer::writer;

use crate::{LocalLibrary, prepare_for_usb};

const SPACE_MARGIN_BASE_BYTES: u64 = 16 * 1024 * 1024;
const SPACE_MARGIN_PER_TRACK_BYTES: u64 = 128 * 1024;
const PROGRESS_POLL: Duration = Duration::from_millis(250);

/// What one stick should hold.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SyncSelection {
    WholeLibrary,
    Playlists {
        playlist_ids: Vec<i64>,
        #[serde(default)]
        track_ids: Vec<i64>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncTarget {
    pub volume: PathBuf,
    pub selection: SyncSelection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SyncPhase {
    CheckingSpace,
    Copying,
    WritingDatabase,
    Done,
    Failed,
}

/// Event payload of `usb-sync-progress`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StickProgress {
    pub volume: String,
    pub phase: SyncPhase,
    pub done: u32,
    pub total: u32,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StickStatus {
    Done,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StickReport {
    pub volume: String,
    pub path: PathBuf,
    pub status: StickStatus,
    pub report: Option<SyncReport>,
    pub error: Option<String>,
    pub skipped_unanalyzed: u32,
    pub elapsed_ms: u64,
}

pub trait ProgressSink: Sync {
    fn progress(&self, progress: &StickProgress);
    fn finished(&self, _report: &StickReport) {}
}

pub struct NoProgress;

impl ProgressSink for NoProgress {
    fn progress(&self, _progress: &StickProgress) {}
}

pub type WriterFn =
    fn(&Path, &[Track], &[AnalysisResult], &[Playlist]) -> Result<SyncReport>;

pub type FreeSpaceProbe = dyn Fn(&Path) -> Result<u64> + Sync;

struct PreparedSelection {
    tracks: Vec<Track>,
    analyses: Vec<AnalysisResult>,
    playlists: Vec<Playlist>,
    skipped_unanalyzed: u32,
}

struct PreparedStick {
    root: PathBuf,
    name: String,
    selection: Arc<PreparedSelection>,
    setup_error: Option<String>,
}

pub struct PreparedMultiSync {
    sticks: Vec<PreparedStick>,
}

impl LocalLibrary {
    /// Read the library once and prepare every distinct selection once.
    ///
    /// Rejects the whole call when two targets are the same directory or one
    /// contains the other. A target that is not a directory fails on its own
    /// when run.
    pub fn prepare_multi_sync(&self, targets: &[SyncTarget]) -> Result<PreparedMultiSync> {
        if targets.is_empty() {
            bail!("No USB sticks selected.");
        }
        let resolved = resolve_roots(targets)?;

        let (tracks, analyses) = self.get_analyzed_tracks()?;
        let playlists = self.get_all_playlists()?;
        let analyzed_ids: HashSet<u32> = tracks.iter().map(|t| t.id).collect();

        let mut cache: Vec<(SyncSelection, Arc<PreparedSelection>)> = Vec::new();
        let mut sticks = Vec::with_capacity(targets.len());
        for (target, (root, setup_error)) in targets.iter().zip(resolved) {
            let selection = match cache.iter().find(|(s, _)| *s == target.selection) {
                Some((_, prepared)) => prepared.clone(),
                None => {
                    let prepared = Arc::new(prepare_selection(
                        &target.selection,
                        &tracks,
                        &analyses,
                        &playlists,
                        &analyzed_ids,
                    ));
                    cache.push((target.selection.clone(), prepared.clone()));
                    prepared
                }
            };
            sticks.push(PreparedStick {
                name: volume_name(&root),
                root,
                selection,
                setup_error,
            });
        }
        Ok(PreparedMultiSync { sticks })
    }
}

impl PreparedMultiSync {
    pub fn run(&self, sink: &dyn ProgressSink) -> Vec<StickReport> {
        self.run_with(sink, &statvfs_available_bytes, writer::filesystem::sync_usb)
    }

    pub fn run_with(
        &self,
        sink: &dyn ProgressSink,
        free_space: &FreeSpaceProbe,
        write: WriterFn,
    ) -> Vec<StickReport> {
        std::thread::scope(|scope| {
            let handles: Vec<_> = self
                .sticks
                .iter()
                .map(|stick| scope.spawn(move || run_stick(stick, sink, free_space, write)))
                .collect();
            handles
                .into_iter()
                .zip(&self.sticks)
                .map(|(handle, stick)| {
                    handle.join().unwrap_or_else(|_| {
                        let report = failed_report(stick, "Sync thread panicked.".into(), 0);
                        emit_failed(sink, stick, &report);
                        report
                    })
                })
                .collect()
        })
    }
}

fn resolve_roots(targets: &[SyncTarget]) -> Result<Vec<(PathBuf, Option<String>)>> {
    let resolved: Vec<(PathBuf, Option<String>)> = targets
        .iter()
        .map(|t| match t.volume.canonicalize() {
            Ok(path) if path.is_dir() => (path, None),
            Ok(path) => (path, Some(format!("{} is not a folder.", t.volume.display()))),
            Err(e) => (
                t.volume.clone(),
                Some(format!("Cannot open {}: {e}", t.volume.display())),
            ),
        })
        .collect();

    for (i, (a, a_err)) in resolved.iter().enumerate() {
        for (b, b_err) in &resolved[i + 1..] {
            if a_err.is_some() || b_err.is_some() {
                continue;
            }
            if a == b {
                bail!("{} was selected twice.", a.display());
            }
            if a.starts_with(b) || b.starts_with(a) {
                bail!(
                    "{} and {} overlap: one is inside the other.",
                    a.display(),
                    b.display()
                );
            }
        }
    }
    Ok(resolved)
}

fn prepare_selection(
    selection: &SyncSelection,
    tracks: &[Track],
    analyses: &[AnalysisResult],
    playlists: &[Playlist],
    analyzed_ids: &HashSet<u32>,
) -> PreparedSelection {
    match selection {
        SyncSelection::WholeLibrary => {
            let (tracks, analyses, playlists) =
                prepare_for_usb(tracks.to_vec(), analyses.to_vec(), playlists.to_vec());
            PreparedSelection { tracks, analyses, playlists, skipped_unanalyzed: 0 }
        }
        SyncSelection::Playlists { playlist_ids, track_ids } => {
            let chosen: Vec<Playlist> = playlists
                .iter()
                .filter(|p| playlist_ids.contains(&(p.id as i64)))
                .cloned()
                .collect();
            let mut wanted: HashSet<u32> = track_ids.iter().map(|&id| id as u32).collect();
            for playlist in &chosen {
                wanted.extend(playlist.track_ids.iter().copied());
            }
            let skipped_unanalyzed =
                wanted.iter().filter(|id| !analyzed_ids.contains(id)).count() as u32;
            let (kept_tracks, kept_analyses): (Vec<Track>, Vec<AnalysisResult>) = tracks
                .iter()
                .zip(analyses)
                .filter(|(t, _)| wanted.contains(&t.id))
                .map(|(t, a)| (t.clone(), a.clone()))
                .unzip();
            let (tracks, analyses, playlists) = prepare_for_usb(kept_tracks, kept_analyses, chosen);
            PreparedSelection { tracks, analyses, playlists, skipped_unanalyzed }
        }
    }
}

fn run_stick(
    stick: &PreparedStick,
    sink: &dyn ProgressSink,
    free_space: &FreeSpaceProbe,
    write: WriterFn,
) -> StickReport {
    let started = Instant::now();
    let total = stick.selection.tracks.len() as u32;
    let fail = |message: String| {
        let report = failed_report(stick, message, started.elapsed().as_millis() as u64);
        emit_failed(sink, stick, &report);
        report
    };

    if let Some(message) = &stick.setup_error {
        return fail(message.clone());
    }
    let Some(_claim) = RootClaim::acquire(&stick.root) else {
        return fail(format!("{} is already being synced.", stick.name));
    };

    sink.progress(&progress(stick, SyncPhase::CheckingSpace, 0, total, "Checking free space"));
    if let Err(e) = check_free_space(stick, free_space) {
        return fail(format!("{e}"));
    }

    let finished = AtomicBool::new(false);
    let result = std::thread::scope(|scope| {
        scope.spawn(|| watch_copy_progress(stick, sink, &finished));
        let _stop_watcher = StopOnDrop(&finished);
        write(
            &stick.root,
            &stick.selection.tracks,
            &stick.selection.analyses,
            &stick.selection.playlists,
        )
    });

    let elapsed_ms = started.elapsed().as_millis() as u64;
    match result {
        Ok(report) => {
            let report = StickReport {
                volume: stick.name.clone(),
                path: stick.root.clone(),
                status: StickStatus::Done,
                report: Some(report),
                error: None,
                skipped_unanalyzed: stick.selection.skipped_unanalyzed,
                elapsed_ms,
            };
            sink.progress(&progress(stick, SyncPhase::Done, total, total, "Safe to eject"));
            sink.finished(&report);
            report
        }
        Err(e) => {
            let report = failed_report(stick, format!("{e:#}"), elapsed_ms);
            emit_failed(sink, stick, &report);
            report
        }
    }
}

struct StopOnDrop<'a>(&'a AtomicBool);

impl Drop for StopOnDrop<'_> {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

fn watch_copy_progress(stick: &PreparedStick, sink: &dyn ProgressSink, finished: &AtomicBool) {
    let total = stick.selection.tracks.len() as u32;
    let mut last: Option<u32> = None;
    while !finished.load(Ordering::Acquire) {
        let done = stick
            .selection
            .tracks
            .iter()
            .filter(|t| stick.root.join(t.usb_path.trim_start_matches('/')).exists())
            .count() as u32;
        if last != Some(done) {
            last = Some(done);
            let (phase, message) = if done < total {
                (SyncPhase::Copying, "Copying tracks")
            } else {
                (SyncPhase::WritingDatabase, "Writing database")
            };
            sink.progress(&progress(stick, phase, done, total, message));
        }
        std::thread::sleep(PROGRESS_POLL);
    }
}

fn check_free_space(stick: &PreparedStick, free_space: &FreeSpaceProbe) -> Result<()> {
    let needed = bytes_to_add(&stick.root, &stick.selection.tracks)
        + SPACE_MARGIN_BASE_BYTES
        + SPACE_MARGIN_PER_TRACK_BYTES * stick.selection.tracks.len() as u64;
    let free = free_space(&stick.root)
        .with_context(|| format!("Cannot read free space on {}", stick.name))?;
    if needed > free {
        bail!(
            "Not enough space on {}: needs {}, {} free. Nothing was written.",
            stick.name,
            human_bytes(needed),
            human_bytes(free)
        );
    }
    Ok(())
}

fn bytes_to_add(root: &Path, tracks: &[Track]) -> u64 {
    tracks
        .iter()
        .filter(|t| {
            std::fs::metadata(root.join(t.usb_path.trim_start_matches('/')))
                .map(|m| m.len() != t.file_size)
                .unwrap_or(true)
        })
        .map(|t| t.file_size)
        .sum()
}

fn progress(
    stick: &PreparedStick,
    phase: SyncPhase,
    done: u32,
    total: u32,
    message: &str,
) -> StickProgress {
    StickProgress {
        volume: stick.name.clone(),
        phase,
        done,
        total,
        message: message.to_string(),
    }
}

fn failed_report(stick: &PreparedStick, error: String, elapsed_ms: u64) -> StickReport {
    StickReport {
        volume: stick.name.clone(),
        path: stick.root.clone(),
        status: StickStatus::Failed,
        report: None,
        error: Some(error),
        skipped_unanalyzed: stick.selection.skipped_unanalyzed,
        elapsed_ms,
    }
}

fn emit_failed(sink: &dyn ProgressSink, stick: &PreparedStick, report: &StickReport) {
    let total = stick.selection.tracks.len() as u32;
    sink.progress(&progress(
        stick,
        SyncPhase::Failed,
        0,
        total,
        report.error.as_deref().unwrap_or("Sync failed"),
    ));
    sink.finished(report);
}

fn volume_name(root: &Path) -> String {
    root.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| root.display().to_string())
}

fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

static ACTIVE_ROOTS: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

struct RootClaim(PathBuf);

impl RootClaim {
    fn acquire(root: &Path) -> Option<Self> {
        let mut active = ACTIVE_ROOTS.lock().unwrap_or_else(|e| e.into_inner());
        if active.iter().any(|r| r == root) {
            return None;
        }
        active.push(root.to_path_buf());
        Some(Self(root.to_path_buf()))
    }
}

impl Drop for RootClaim {
    fn drop(&mut self) {
        let mut active = ACTIVE_ROOTS.lock().unwrap_or_else(|e| e.into_inner());
        active.retain(|r| r != &self.0);
    }
}

/// Free bytes available to an unprivileged user on the filesystem holding `path`.
#[cfg(unix)]
pub fn statvfs_available_bytes(path: &Path) -> Result<u64> {
    use std::os::unix::ffi::OsStrExt;
    let c_path = std::ffi::CString::new(path.as_os_str().as_bytes())
        .context("path contains a NUL byte")?;
    let mut stats = std::mem::MaybeUninit::<libc::statvfs>::zeroed();
    let rc = unsafe { libc::statvfs(c_path.as_ptr(), stats.as_mut_ptr()) };
    if rc != 0 {
        return Err(std::io::Error::last_os_error())
            .with_context(|| format!("statvfs {}", path.display()));
    }
    let stats = unsafe { stats.assume_init() };
    Ok(stats.f_bavail as u64 * stats.f_frsize as u64)
}

#[cfg(not(unix))]
pub fn statvfs_available_bytes(_path: &Path) -> Result<u64> {
    Ok(u64::MAX)
}
