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

use pioneer_usb_writer::models::{AnalysisResult, Playlist, SyncAction, SyncReport, Track};
use pioneer_usb_writer::{reader, writer};

use crate::sticks::{self, LastSync, OwnedFile, StickManifest};
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

/// Writes the stick. The last argument lists stick paths of removed tracks whose
/// files must stay because fourfour did not write them.
pub type WriterFn =
    fn(&Path, &[Track], &[AnalysisResult], &[Playlist], &HashSet<String>) -> Result<SyncReport>;

pub type FreeSpaceProbe = dyn Fn(&Path) -> Result<u64> + Sync;

/// One track in a [`UsbSyncPlan`]; `track_id` is the library id when known.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanTrack {
    pub track_id: Option<i64>,
    pub title: String,
    pub artist: String,
    pub size_bytes: Option<u64>,
}

/// Dry run of one stick: what a sync with the same selection would do.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsbSyncPlan {
    pub volume: String,
    pub stick_id: Option<String>,
    pub playlist_count: u32,
    pub add: Vec<PlanTrack>,
    pub remove: Vec<PlanTrack>,
    pub update_count: u32,
    pub unchanged_count: u32,
    pub skipped_unanalyzed: Vec<PlanTrack>,
    /// Tracks in the stick's database that fourfour did not write: their files stay.
    pub foreign_kept_count: u32,
    pub bytes_to_add: u64,
    pub bytes_to_free: u64,
    pub bytes_needed: u64,
    pub free_bytes: Option<u64>,
    pub fits: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct VolumeCapacity {
    pub total_bytes: u64,
    pub free_bytes: u64,
}

struct PreparedSelection {
    tracks: Vec<Track>,
    analyses: Vec<AnalysisResult>,
    playlists: Vec<Playlist>,
    library_ids: Vec<i64>,
    skipped: Vec<PlanTrack>,
}

impl PreparedSelection {
    fn skipped_unanalyzed(&self) -> u32 {
        self.skipped.len() as u32
    }
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
                        self,
                        &target.selection,
                        &tracks,
                        &analyses,
                        &playlists,
                        &analyzed_ids,
                    )?);
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
        self.run_with(sink, &statvfs_available_bytes, writer::filesystem::sync_usb_keeping)
    }

    /// Dry run per stick, in target order. Writes nothing.
    pub fn plan(&self) -> Vec<Result<UsbSyncPlan>> {
        self.plan_with(&statvfs_available_bytes)
    }

    pub fn plan_with(&self, free_space: &FreeSpaceProbe) -> Vec<Result<UsbSyncPlan>> {
        self.sticks.iter().map(|stick| stick_plan(stick, free_space).map(|(plan, _)| plan)).collect()
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
    library: &LocalLibrary,
    selection: &SyncSelection,
    tracks: &[Track],
    analyses: &[AnalysisResult],
    playlists: &[Playlist],
    analyzed_ids: &HashSet<u32>,
) -> Result<PreparedSelection> {
    match selection {
        SyncSelection::WholeLibrary => {
            let library_ids = tracks.iter().map(|t| t.id as i64).collect();
            let (tracks, analyses, playlists) =
                prepare_for_usb(tracks.to_vec(), analyses.to_vec(), playlists.to_vec());
            Ok(PreparedSelection { tracks, analyses, playlists, library_ids, skipped: Vec::new() })
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
            let mut skipped_ids: Vec<u32> =
                wanted.iter().copied().filter(|id| !analyzed_ids.contains(id)).collect();
            skipped_ids.sort_unstable();
            let mut skipped = Vec::with_capacity(skipped_ids.len());
            for id in skipped_ids {
                if let Some(track) = library.get_track(id as i64)? {
                    skipped.push(plan_track(Some(id as i64), &track.title, &track.artist, Some(track.file_size)));
                }
            }
            let (kept_tracks, kept_analyses): (Vec<Track>, Vec<AnalysisResult>) = tracks
                .iter()
                .zip(analyses)
                .filter(|(t, _)| wanted.contains(&t.id))
                .map(|(t, a)| (t.clone(), a.clone()))
                .unzip();
            let library_ids = kept_tracks.iter().map(|t| t.id as i64).collect();
            let (tracks, analyses, playlists) = prepare_for_usb(kept_tracks, kept_analyses, chosen);
            Ok(PreparedSelection { tracks, analyses, playlists, library_ids, skipped })
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
    let (_, keep_files) = match stick_plan(stick, &|_| Ok(u64::MAX)) {
        Ok(planned) => planned,
        Err(e) => return fail(format!("{e:#}")),
    };
    let manifest = match claim_selection(stick) {
        Ok(manifest) => manifest,
        Err(e) => return fail(format!("{e:#}")),
    };

    let finished = AtomicBool::new(false);
    let result = std::thread::scope(|scope| {
        scope.spawn(|| watch_copy_progress(stick, sink, &finished));
        let _stop_watcher = StopOnDrop(&finished);
        write(
            &stick.root,
            &stick.selection.tracks,
            &stick.selection.analyses,
            &stick.selection.playlists,
            &keep_files,
        )
    });
    let result = result.and_then(|report| {
        record_sync(stick, manifest, &report)?;
        Ok(report)
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
                skipped_unanalyzed: stick.selection.skipped_unanalyzed(),
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

fn plan_track(track_id: Option<i64>, title: &str, artist: &str, size_bytes: Option<u64>) -> PlanTrack {
    PlanTrack { track_id, title: title.to_string(), artist: artist.to_string(), size_bytes }
}

fn bytes_needed(stick: &PreparedStick) -> u64 {
    bytes_to_add(&stick.root, &stick.selection.tracks)
        + SPACE_MARGIN_BASE_BYTES
        + SPACE_MARGIN_PER_TRACK_BYTES * stick.selection.tracks.len() as u64
}

fn stick_plan(stick: &PreparedStick, free_space: &FreeSpaceProbe) -> Result<(UsbSyncPlan, HashSet<String>)> {
    if let Some(message) = &stick.setup_error {
        bail!("{message}");
    }
    let selection = &stick.selection;
    let existing = reader::read_usb_state(&stick.root)
        .with_context(|| format!("Cannot read the library on {}", stick.name))?;
    let manifest = sticks::read_manifest(&stick.root)?;
    let diff = writer::sync::compute_sync_plan(
        &selection.tracks,
        &selection.analyses,
        &selection.playlists,
        existing.as_ref(),
    );

    let mut add = Vec::new();
    let mut bytes_to_add = 0;
    let mut update_count = 0;
    let mut unchanged_count = 0;
    for (entry, library_id) in diff.entries.iter().zip(&selection.library_ids) {
        match entry.action {
            SyncAction::Add | SyncAction::Replace => {
                bytes_to_add += entry.track.file_size;
                add.push(plan_track(
                    Some(*library_id),
                    &entry.track.title,
                    &entry.track.artist,
                    Some(entry.track.file_size),
                ));
            }
            SyncAction::Update => update_count += 1,
            SyncAction::Skip => unchanged_count += 1,
        }
    }

    let mut remove = Vec::new();
    let mut bytes_to_free = 0;
    let mut keep_files = HashSet::new();
    for gone in &diff.removals {
        let owned = manifest.as_ref().and_then(|m| m.files.iter().find(|f| f.usb_path == gone.usb_path));
        match owned {
            Some(file) => {
                bytes_to_free += gone.file_size;
                remove.push(plan_track(Some(file.track_id), &gone.title, &gone.artist, Some(gone.file_size)));
            }
            None => {
                keep_files.insert(gone.usb_path.clone());
            }
        }
    }

    let needed = bytes_needed(stick);
    let free_bytes = free_space(&stick.root).ok();
    let plan = UsbSyncPlan {
        volume: stick.name.clone(),
        stick_id: manifest.map(|m| m.stick_id),
        playlist_count: selection.playlists.len() as u32,
        add,
        remove,
        update_count,
        unchanged_count,
        skipped_unanalyzed: selection.skipped.clone(),
        foreign_kept_count: keep_files.len() as u32,
        bytes_to_add,
        bytes_to_free,
        bytes_needed: needed,
        free_bytes,
        fits: free_bytes.map(|free| needed <= free),
    };
    Ok((plan, keep_files))
}

fn owned_files(stick: &PreparedStick) -> Vec<OwnedFile> {
    stick
        .selection
        .tracks
        .iter()
        .zip(&stick.selection.library_ids)
        .map(|(track, &track_id)| OwnedFile {
            usb_path: track.usb_path.clone(),
            track_id,
            size_bytes: track.file_size,
        })
        .collect()
}

fn claim_selection(stick: &PreparedStick) -> Result<StickManifest> {
    let mut manifest = sticks::manifest_or_fresh(&stick.root)?;
    for file in owned_files(stick) {
        if !manifest.owns(&file.usb_path) {
            manifest.files.push(file);
        }
    }
    sticks::write_manifest(&stick.root, &manifest)?;
    Ok(manifest)
}

fn record_sync(stick: &PreparedStick, mut manifest: StickManifest, report: &SyncReport) -> Result<()> {
    manifest.files = owned_files(stick);
    manifest.last_sync = Some(LastSync { at: chrono::Utc::now().to_rfc3339(), report: report.clone() });
    sticks::write_manifest(&stick.root, &manifest)
}

fn check_free_space(stick: &PreparedStick, free_space: &FreeSpaceProbe) -> Result<()> {
    let needed = bytes_needed(stick);
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
        skipped_unanalyzed: stick.selection.skipped_unanalyzed(),
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

/// Total and free bytes of the filesystem holding `path` (macOS and Linux).
#[cfg(unix)]
pub fn volume_capacity(path: &Path) -> Result<VolumeCapacity> {
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
    let block = stats.f_frsize as u64;
    Ok(VolumeCapacity {
        total_bytes: stats.f_blocks as u64 * block,
        free_bytes: stats.f_bavail as u64 * block,
    })
}

#[cfg(not(unix))]
pub fn volume_capacity(path: &Path) -> Result<VolumeCapacity> {
    bail!("Capacity is not available for {}", path.display())
}
