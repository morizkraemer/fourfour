use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use pioneer_library::{DuplicateCandidate, DuplicateIndex, DuplicateOf, DuplicateReason, LocalLibrary, PreparedMultiSync, SyncTarget};
use pioneer_usb_writer::models::Track;
use pioneer_usb_writer::scanner::{read_track_metadata, AUDIO_EXTENSIONS};
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct DuplicateRow {
    pub new_path: String,
    pub file_name: String,
    pub existing_id: u32,
    pub title: String,
    pub artist: String,
    pub duration_secs: f64,
    pub reason: DuplicateReason,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ImportPlan {
    pub token: u64,
    pub new_count: usize,
    pub already_in_library: usize,
    pub duplicates: Vec<DuplicateRow>,
    pub skipped: SkippedFiles,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct SkippedFiles {
    pub unsupported: usize,
    pub unsupported_exts: Vec<String>,
    pub unreadable: usize,
}

const SIDECAR_EXTENSIONS: &[&str] = &["asd", "reapeaks", "pkf", "sfk"];

/// What the dropped paths hold, from one walk: the audio files to read, grouped by dropped
/// path, and the files that are not audio. Hidden files and folders are left out; symlinked
/// folders are followed, symlink loops are not.
#[derive(Debug, Default)]
pub struct FileSurvey {
    audio: HashSet<PathBuf>,
    groups: Vec<ScanGroup>,
    unsupported: usize,
    unsupported_exts: BTreeSet<String>,
    unreadable: usize,
}

/// The audio files of one dropped path, read as one group: a folder's tracks come back sorted
/// by title, artist and path, loose files in the order given.
#[derive(Debug)]
struct ScanGroup {
    files: Vec<PathBuf>,
    sorted: bool,
}

pub fn survey_files(paths: &[String]) -> FileSurvey {
    let mut survey = FileSurvey::default();
    let mut loose = Vec::new();
    for p in paths {
        let path = Path::new(p);
        if path.is_dir() {
            let files = survey.visit_dropped_dir(path);
            survey.groups.push(ScanGroup { files, sorted: true });
        } else if path.is_file() {
            survey.visit_file(path, &mut loose);
        }
    }
    survey.groups.insert(0, ScanGroup { files: loose, sorted: false });
    survey
}

/// Listing folders waits on the disk; the subfolders of a dropped folder are walked in parallel.
const WALK_THREADS: usize = 8;

impl FileSurvey {
    pub fn audio_count(&self) -> usize {
        self.audio.len()
    }

    fn visit_dropped_dir(&mut self, root: &Path) -> Vec<PathBuf> {
        let mut files = Vec::new();
        let Ok(anchor) = root.canonicalize() else { return files };
        let mut chain = SymlinkChain { followed: Vec::new(), anchor, entered: Default::default() };
        if is_hidden(root) {
            return files;
        }
        let Ok(entries) = std::fs::read_dir(root) else {
            self.unreadable += 1;
            return files;
        };
        let mut subdirs = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            match entry_kind(&entry) {
                EntryKind::Dir => subdirs.push(path),
                EntryKind::SymlinkToDir => self.visit_symlinked_dir(root, &path, &mut chain, &mut files),
                EntryKind::File => self.visit_file(&path, &mut files),
                EntryKind::Other => {}
            }
        }
        let next = std::sync::atomic::AtomicUsize::new(0);
        let parts: Vec<(FileSurvey, Vec<PathBuf>)> = std::thread::scope(|scope| {
            let workers: Vec<_> = (0..WALK_THREADS.min(subdirs.len()))
                .map(|_| {
                    let (next, subdirs, mut chain) = (&next, &subdirs, chain.clone());
                    scope.spawn(move || {
                        let mut part = (FileSurvey::default(), Vec::new());
                        while let Some(dir) = subdirs.get(next.fetch_add(1, std::sync::atomic::Ordering::Relaxed)) {
                            part.0.visit_dir(dir, &mut chain, &mut part.1);
                        }
                        part
                    })
                })
                .collect();
            workers.into_iter().map(|w| w.join().expect("folder walk thread panicked")).collect()
        });
        for (part, part_files) in parts {
            self.audio.extend(part.audio);
            self.unsupported += part.unsupported;
            self.unsupported_exts.extend(part.unsupported_exts);
            self.unreadable += part.unreadable;
            files.extend(part_files);
        }
        files
    }

    fn visit_dir(&mut self, dir: &Path, chain: &mut SymlinkChain, out: &mut Vec<PathBuf>) {
        if is_hidden(dir) {
            return;
        }
        let Ok(entries) = std::fs::read_dir(dir) else {
            self.unreadable += 1;
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            match entry_kind(&entry) {
                EntryKind::Dir => self.visit_dir(&path, chain, out),
                EntryKind::SymlinkToDir => self.visit_symlinked_dir(dir, &path, chain, out),
                EntryKind::File => self.visit_file(&path, out),
                EntryKind::Other => {}
            }
        }
    }

    fn visit_symlinked_dir(&mut self, parent: &Path, link: &Path, chain: &mut SymlinkChain, out: &mut Vec<PathBuf>) {
        let (Ok(target), Ok(here)) = (link.canonicalize(), parent.canonicalize()) else { return };
        if chain.leads_back(&target, &here) || !chain.entered.lock().expect("symlink targets lock").insert(target.clone()) {
            return;
        }
        let anchor = std::mem::replace(&mut chain.anchor, target);
        chain.followed.push((anchor, here));
        self.visit_dir(link, chain, out);
        let (anchor, _) = chain.followed.pop().expect("pushed above");
        chain.anchor = anchor;
    }

    fn visit_file(&mut self, path: &Path, out: &mut Vec<PathBuf>) {
        if is_hidden(path) {
            return;
        }
        let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        if AUDIO_EXTENSIONS.contains(&ext.as_str()) {
            if path.to_str().is_none() {
                self.unreadable += 1;
                return;
            }
            self.audio.insert(path.to_path_buf());
            out.push(path.to_path_buf());
        } else if !SIDECAR_EXTENSIONS.contains(&ext.as_str()) {
            self.unsupported += 1;
            if !ext.is_empty() {
                self.unsupported_exts.insert(format!(".{ext}"));
            }
        }
    }
}

/// The folders on the way down from a dropped folder, as canonical stretches: each symlinked
/// folder followed starts a new stretch at its target. Plain subfolders are not canonicalised
/// (a library has about one folder per album); a symlink is.
#[derive(Clone)]
struct SymlinkChain {
    followed: Vec<(PathBuf, PathBuf)>,
    anchor: PathBuf,
    entered: std::sync::Arc<Mutex<HashSet<PathBuf>>>,
}

impl SymlinkChain {
    /// Whether `target` is one of the folders already on the way down (`here` = where the
    /// symlink is), so following it would walk in a loop.
    fn leads_back(&self, target: &Path, here: &Path) -> bool {
        let on_stretch = |anchor: &PathBuf, end: &Path| target.starts_with(anchor) && end.starts_with(target);
        on_stretch(&self.anchor, here) || self.followed.iter().any(|(anchor, end)| on_stretch(anchor, end))
    }
}

fn is_hidden(path: &Path) -> bool {
    path.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.'))
}

enum EntryKind {
    Dir,
    SymlinkToDir,
    File,
    Other,
}

/// What a directory entry is, symlinks followed, from the type the directory listing already
/// carries; only symlinks cost a `stat`.
fn entry_kind(entry: &std::fs::DirEntry) -> EntryKind {
    let Ok(kind) = entry.file_type() else { return EntryKind::Other };
    if kind.is_dir() {
        return EntryKind::Dir;
    }
    if kind.is_file() {
        return EntryKind::File;
    }
    match std::fs::metadata(entry.path()) {
        Ok(target) if kind.is_symlink() && target.is_dir() => EntryKind::SymlinkToDir,
        Ok(target) if kind.is_symlink() && target.is_file() => EntryKind::File,
        _ => EntryKind::Other,
    }
}

/// Tag reads wait on the disk, not the CPU: on a cold cache 10k files took 5.8 s on 8 threads,
/// 3.0 s on 16 and 2.2-2.6 s on 32 (dev box, R6 in `docs/architecture/perf-run-2026-11.md`).
const SCAN_THREADS: usize = 32;

/// The covers are read after the rows are on screen, next to whatever the user does next.
const COVER_THREADS: usize = 8;

/// Runs `read` on every file on up to `threads` threads and returns the results in file
/// order; `on_file` runs on the calling thread once per file, in completion order.
fn read_parallel<T: Send>(threads: usize, files: &[&Path], read: impl Fn(&Path) -> T + Sync, on_file: &mut dyn FnMut()) -> Vec<T> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let threads = threads.min(files.len());
    let next = AtomicUsize::new(0);
    let (sender, receiver) = std::sync::mpsc::channel::<(usize, T)>();
    let mut out: Vec<Option<T>> = (0..files.len()).map(|_| None).collect();
    std::thread::scope(|scope| {
        for _ in 0..threads {
            let sender = sender.clone();
            let (next, read) = (&next, &read);
            scope.spawn(move || loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                let Some(path) = files.get(i) else { break };
                if sender.send((i, read(path))).is_err() {
                    break;
                }
            });
        }
        drop(sender);
        for (i, value) in receiver {
            out[i] = Some(value);
            on_file();
        }
    });
    out.into_iter().map(|v| v.expect("every file was read")).collect()
}

const MAX_TAG_CHARS: usize = 500;

fn clamp_text(text: &mut String) {
    if let Some((end, _)) = text.char_indices().nth(MAX_TAG_CHARS) {
        text.truncate(end);
    }
    if text.contains('\0') {
        text.retain(|c| c != '\0');
    }
}

fn clamp_tags(track: Track) -> Track {
    let mut track = track;
    for field in [&mut track.title, &mut track.artist, &mut track.album, &mut track.genre, &mut track.label, &mut track.remixer, &mut track.comment] {
        clamp_text(field);
    }
    let mut parts: Vec<String> = track.usb_path.splitn(4, '/').map(str::to_string).collect();
    if let [_, _, artist_dir, _] = parts.as_mut_slice() {
        clamp_text(artist_dir);
    }
    track.usb_path = parts.join("/");
    track
}

pub(crate) fn guarded_read(path: &Path, read: impl FnOnce() -> anyhow::Result<Track> + std::panic::UnwindSafe) -> Option<Track> {
    match std::panic::catch_unwind(read) {
        Ok(Ok(track)) => Some(clamp_tags(track)),
        Ok(Err(e)) => {
            eprintln!("Warning: skipping {}: {e}", path.display());
            None
        }
        Err(_) => {
            eprintln!("Warning: skipping {}: the tag reader crashed on it", path.display());
            None
        }
    }
}

fn read_metadata_guarded(path: &Path) -> Option<Track> {
    guarded_read(path, || read_track_metadata(path, 0))
}

/// The tags of one file without its cover: covers are read again and stored after the rows
/// are in the library ([`store_pending_covers`]), so a large import never holds them all.
fn read_tags(path: &Path) -> Option<Track> {
    read_metadata_guarded(path).map(|track| Track { artwork: None, ..track })
}

/// What a scan found: the tracks read (without covers) and the files skipped unread because
/// their path is already in the library.
#[derive(Debug, Default)]
pub struct Scanned {
    pub tracks: Vec<Track>,
    pub known: Vec<PathBuf>,
}

/// Reads the tags of the surveyed audio files in parallel, like `scanner::scan_files` and
/// `scanner::scan_directory` (same tracks, order and ids: loose files first, then each folder;
/// covers left out). Files whose path is in `known` are not read; they come back in `known`.
pub fn scan_paths(survey: &FileSurvey, known: &HashSet<PathBuf>, on_file: &mut dyn FnMut()) -> Scanned {
    let mut scanned = Scanned::default();
    let mut groups: Vec<(Vec<&Path>, bool)> = Vec::with_capacity(survey.groups.len());
    for group in &survey.groups {
        let mut unread = Vec::with_capacity(group.files.len());
        for file in &group.files {
            if known.contains(file) {
                scanned.known.push(file.clone());
                on_file();
            } else {
                unread.push(file.as_path());
            }
        }
        groups.push((unread, group.sorted));
    }
    let all: Vec<&Path> = groups.iter().flat_map(|(files, _)| files.iter().copied()).collect();
    let mut read = read_parallel(SCAN_THREADS, &all, read_tags, on_file).into_iter();
    for (files, sorted) in &groups {
        let mut tracks: Vec<Track> = read.by_ref().take(files.len()).flatten().collect();
        if *sorted {
            tracks.sort_by(|a, b| {
                a.title.cmp(&b.title).then_with(|| a.artist.cmp(&b.artist)).then_with(|| a.source_path.cmp(&b.source_path))
            });
        }
        for (i, track) in tracks.iter_mut().enumerate() {
            track.id = (i + 1) as u32;
        }
        scanned.tracks.extend(tracks);
    }
    scanned
}

/// About 6 MB of covers: the library stays locked for 25 ms (p50) to 60 ms (max) per batch.
const COVER_BATCH: usize = 32;

/// Reads the covers of the tracks queued by `LocalLibrary::add_tracks_covers_later` from their
/// files and stores them until the queue is empty: one thread reads the next batch while this
/// one stores the last, and the library is locked only to take a batch and to store it.
/// `on_stored` gets the ids of each batch's tracks that got a cover. Returns how many covers
/// were stored. With `only`, tracks outside that set stay queued.
pub fn store_pending_covers(
    lib: &Mutex<LocalLibrary>,
    only: Option<&HashSet<i64>>,
    on_stored: &mut dyn FnMut(Vec<u32>),
) -> Result<usize, String> {
    type Batch = Result<Vec<(i64, PathBuf, Option<Vec<u8>>)>, String>;
    let mut stored = 0;
    loop {
        let mut batches = 0;
        std::thread::scope(|scope| {
            let (sender, receiver) = std::sync::mpsc::sync_channel::<Batch>(1);
            scope.spawn(move || {
                let mut after = 0;
                loop {
                    let pending = lib.lock().map_err(|e| e.to_string()).and_then(|l| l.pending_artwork(after, COVER_BATCH).map_err(|e| e.to_string()));
                    let page = match pending {
                        Ok(page) if page.is_empty() => return,
                        Ok(page) => page,
                        Err(e) => {
                            sender.send(Err(e)).ok();
                            return;
                        }
                    };
                    after = page.last().map_or(after, |(id, _)| *id);
                    let batch: Vec<_> = page.into_iter().filter(|(id, _)| only.is_none_or(|ids| ids.contains(id))).collect();
                    if batch.is_empty() {
                        continue;
                    }
                    let files: Vec<&Path> = batch.iter().map(|(_, path)| path.as_path()).collect();
                    let covers = read_parallel(COVER_THREADS, &files, |path| read_metadata_guarded(path).and_then(|t| t.artwork), &mut || {});
                    let read = batch.into_iter().zip(covers).map(|((id, path), cover)| (id, path, cover)).collect();
                    if sender.send(Ok(read)).is_err() {
                        return;
                    }
                }
            });
            for covers in receiver {
                let covers = covers?;
                batches += 1;
                lib.lock().map_err(|e| e.to_string())?.store_pending_artwork(&covers).map_err(|e| e.to_string())?;
                let with_cover: Vec<u32> = covers.iter().filter(|(_, _, c)| c.is_some()).map(|(id, _, _)| *id as u32).collect();
                stored += with_cover.len();
                if !with_cover.is_empty() {
                    on_stored(with_cover);
                }
            }
            Ok::<(), String>(())
        })?;
        if batches == 0 {
            return Ok(stored);
        }
    }
}

/// Stores the queued covers of the tracks the targets may write; the others stay queued.
pub fn store_covers_for_sync(lib: &Mutex<LocalLibrary>, targets: &[SyncTarget]) -> Result<usize, String> {
    let wanted = lib.lock().map_err(|e| e.to_string())?.sync_track_ids(targets).map_err(|e| format!("{e:#}"))?;
    store_pending_covers(lib, Some(&wanted), &mut |_| {})
}

/// `prepare_multi_sync` after the covers of the tracks the targets will write are stored:
/// imported covers are filled in the background, and a stick written before that would get
/// no artwork. Tracks the targets do not write stay queued.
pub fn prepare_sync_with_covers(lib: &Mutex<LocalLibrary>, targets: &[SyncTarget]) -> Result<PreparedMultiSync, String> {
    store_covers_for_sync(lib, targets)?;
    lib.lock().map_err(|e| e.to_string())?.prepare_multi_sync(targets).map_err(|e| format!("{e:#}"))
}

#[derive(Debug)]
pub struct PendingImport {
    token: u64,
    fresh: Vec<Track>,
    duplicate_of: Vec<Option<u32>>,
}

impl PendingImport {
    /// The tracks to add and the existing ids of the duplicates left out.
    pub fn resolve(self, skip_duplicates: bool) -> (Vec<Track>, Vec<u32>) {
        if !skip_duplicates {
            return (self.fresh, Vec::new());
        }
        let mut add = Vec::new();
        let mut skipped = Vec::new();
        for (track, dup) in self.fresh.into_iter().zip(self.duplicate_of) {
            match dup {
                Some(id) => skipped.push(id),
                None => add.push(track),
            }
        }
        (add, skipped)
    }
}

/// The import waiting on the duplicates question; a newer plan replaces it.
#[derive(Debug, Default)]
pub struct ImportPlans(Mutex<(u64, Option<PendingImport>)>);

impl ImportPlans {
    pub fn store(&self, build: impl FnOnce(u64) -> (PendingImport, ImportPlan)) -> Result<ImportPlan, String> {
        let mut state = self.0.lock().map_err(|e| e.to_string())?;
        state.0 += 1;
        let (pending, plan) = build(state.0);
        state.1 = Some(pending);
        Ok(plan)
    }

    pub fn take(&self, token: u64) -> Result<PendingImport, String> {
        let mut state = self.0.lock().map_err(|e| e.to_string())?;
        match state.1.take() {
            Some(pending) if pending.token == token => Ok(pending),
            other => {
                state.1 = other;
                Err("This import is no longer waiting. Import the files again.".into())
            }
        }
    }
}

const FINDS_PER_THREAD: usize = 1024;

/// `index.find` for every track, on several threads: each find normalises the track's tags.
fn find_all(index: &DuplicateIndex, tracks: &[Track]) -> Vec<Option<DuplicateOf>> {
    std::thread::scope(|scope| {
        let parts: Vec<_> = tracks
            .chunks(FINDS_PER_THREAD)
            .map(|part| scope.spawn(move || part.iter().map(|t| index.find(t)).collect::<Vec<_>>()))
            .collect();
        parts.into_iter().flat_map(|part| part.join().expect("duplicate check thread panicked")).collect()
    })
}

/// Splits scanned tracks into ones already in the library (same path, dropped silently),
/// likely duplicates (asked about) and new tracks.
pub fn plan_import(
    token: u64,
    index: &DuplicateIndex,
    existing: &dyn Fn(u32) -> Option<DuplicateCandidate>,
    scanned: Scanned,
    survey: &FileSurvey,
) -> (PendingImport, ImportPlan) {
    let mut seen = HashSet::new();
    let mut already_in_library = scanned.known.into_iter().filter(|path| seen.insert(path.clone())).count();
    let mut fresh = Vec::new();
    let mut duplicate_of = Vec::new();
    let mut duplicates = Vec::new();
    let hits = find_all(index, &scanned.tracks);
    for (track, found) in scanned.tracks.into_iter().zip(hits) {
        if !seen.insert(track.source_path.clone()) {
            continue;
        }
        match found {
            Some(hit) if hit.reason == DuplicateReason::SamePath => {
                already_in_library += 1;
                continue;
            }
            Some(hit) => {
                let existing_id = hit.existing_id as u32;
                let found = existing(existing_id);
                duplicates.push(DuplicateRow {
                    new_path: track.source_path.to_string_lossy().into_owned(),
                    file_name: track.source_path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                    existing_id,
                    title: found.as_ref().map(|t| t.title.clone()).unwrap_or_default(),
                    artist: found.as_ref().map(|t| t.artist.clone()).unwrap_or_default(),
                    duration_secs: found.map(|t| t.duration_secs).unwrap_or(track.duration_secs),
                    reason: hit.reason,
                });
                duplicate_of.push(Some(existing_id));
            }
            None => duplicate_of.push(None),
        }
        fresh.push(track);
    }
    let skipped = SkippedFiles {
        unsupported: survey.unsupported,
        unsupported_exts: survey.unsupported_exts.iter().cloned().collect(),
        unreadable: survey.audio.len().saturating_sub(seen.len()) + survey.unreadable,
    };
    let plan = ImportPlan { token, new_count: fresh.len() - duplicates.len(), already_in_library, duplicates, skipped };
    (PendingImport { token, fresh, duplicate_of }, plan)
}

pub fn plan_for_library(
    plans: &ImportPlans,
    lib: &LocalLibrary,
    scanned: Scanned,
    survey: &FileSurvey,
) -> Result<ImportPlan, String> {
    if scanned.tracks.is_empty() {
        return plans.store(|token| plan_import(token, &DuplicateIndex::default(), &|_| None, scanned, survey));
    }
    let existing = |id: u32| lib.duplicate_candidate(id).ok().flatten();
    lib.with_duplicate_index(|index| plans.store(|token| plan_import(token, index, &existing, scanned, survey))).map_err(|e| e.to_string())?

}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn track(id: u32, path: &str, artist: &str, title: &str, secs: f64) -> Track {
        Track {
            id,
            source_path: PathBuf::from(path),
            usb_path: String::new(),
            title: title.into(),
            artist: artist.into(),
            album: String::new(),
            genre: String::new(),
            label: String::new(),
            remixer: String::new(),
            comment: String::new(),
            year: 0,
            disc_number: 0,
            track_number: 0,
            tempo: 0,
            key: String::new(),
            duration_secs: secs,
            sample_rate: 44100,
            bitrate: 320,
            file_size: 1000 + id as u64,
            artwork: None,
        }
    }

    fn sample() -> (PendingImport, ImportPlan) {
        let stored = DuplicateCandidate::from(&track(7, "/Music/Glass Harbor.aiff", "Ode & Ash", "Glass Harbor", 425.0));
        let library = DuplicateIndex::new(&[stored.clone()]);
        let existing = |id: u32| (id == stored.id).then(|| stored.clone());
        let tracks = vec![
            track(0, "/Music/Glass Harbor.aiff", "Ode & Ash", "Glass Harbor", 425.0),
            track(0, "/New/glass_harbor.mp3", "Ode & Ash", "Glass Harbor (Original Mix)", 426.0),
            track(0, "/New/fresh.mp3", "Hale", "Copper Lines", 380.0),
            track(0, "/New/fresh.mp3", "Hale", "Copper Lines", 380.0),
        ];
        plan_import(3, &library, &existing, Scanned { tracks, known: vec![] }, &FileSurvey::default())
    }

    #[test]
    fn plan_drops_same_path_and_lists_likely_duplicates_with_your_track() {
        let (_, plan) = sample();
        assert_eq!(plan.token, 3);
        assert_eq!(plan.already_in_library, 1);
        assert_eq!(plan.new_count, 1);
        assert_eq!(
            plan.duplicates,
            vec![DuplicateRow {
                new_path: "/New/glass_harbor.mp3".into(),
                file_name: "glass_harbor.mp3".into(),
                existing_id: 7,
                title: "Glass Harbor".into(),
                artist: "Ode & Ash".into(),
                duration_secs: 425.0,
                reason: DuplicateReason::SameTags,
            }]
        );
    }

    #[test]
    fn files_skipped_unread_count_as_already_in_library_once_and_not_as_unreadable() {
        let mut survey = FileSurvey::default();
        survey.audio.extend(["/Music/a.mp3", "/Music/b.mp3"].map(PathBuf::from));
        let scanned = Scanned {
            tracks: vec![track(0, "/Music/b.mp3", "Hale", "Copper Lines", 380.0)],
            known: vec![PathBuf::from("/Music/a.mp3"), PathBuf::from("/Music/a.mp3")],
        };
        let (pending, plan) = plan_import(1, &DuplicateIndex::default(), &|_| None, scanned, &survey);
        assert_eq!((plan.already_in_library, plan.new_count, plan.skipped.unreadable), (1, 1, 0));
        assert_eq!(pending.resolve(false).0.len(), 1);
    }

    #[test]
    fn skip_duplicates_adds_only_new_tracks_and_reports_the_existing_ids() {
        let (pending, _) = sample();
        let (add, skipped) = pending.resolve(true);
        assert_eq!(add.iter().map(|t| t.source_path.to_str().unwrap()).collect::<Vec<_>>(), ["/New/fresh.mp3"]);
        assert_eq!(skipped, [7]);
    }

    #[test]
    fn import_anyway_adds_duplicates_too() {
        let (pending, _) = sample();
        let (add, skipped) = pending.resolve(false);
        assert_eq!(add.len(), 2);
        assert!(skipped.is_empty());
    }

    #[test]
    fn only_the_latest_plan_can_be_committed() {
        let plans = ImportPlans::default();
        let first = plans.store(|token| plan_import(token, &DuplicateIndex::default(), &|_| None, Scanned::default(), &FileSurvey::default())).unwrap();
        let second = plans.store(|token| plan_import(token, &DuplicateIndex::default(), &|_| None, Scanned::default(), &FileSurvey::default())).unwrap();
        assert!(plans.take(first.token).is_err());
        assert!(plans.take(second.token).is_ok());
        assert!(plans.take(second.token).is_err());
    }

    #[test]
    fn a_plan_from_the_cached_index_equals_one_from_a_fresh_index_after_library_writes() {
        let lib = LocalLibrary::open_in_memory().unwrap();
        let stored = |n: u32| track(0, &format!("/Music/{n}.mp3"), &format!("Artist {}", n % 4), &format!("Title {}", n % 7), 300.0 + (n % 3) as f64);
        let ids = lib.add_tracks(&(1..=40).map(stored).collect::<Vec<_>>()).unwrap();
        let incoming = || -> Vec<Track> {
            (0..30).map(|n| track(0, &format!("/New/{n}.mp3"), &format!("artist {}", n % 5), &format!("TITLE {}", n % 9), 300.0 + (n % 4) as f64)).collect()
        };
        let plan_with = |index: &DuplicateIndex| {
            let existing = |id: u32| lib.duplicate_candidate(id).ok().flatten();
            plan_import(1, index, &existing, Scanned { tracks: incoming(), known: vec![] }, &FileSurvey::default()).1
        };
        let cached = |lib: &LocalLibrary| {
            let plans = ImportPlans::default();
            let plan = plan_for_library(&plans, lib, Scanned { tracks: incoming(), known: vec![] }, &FileSurvey::default()).unwrap();
            ImportPlan { token: 1, ..plan }
        };
        assert_eq!(cached(&lib), plan_with(&lib.duplicate_index().unwrap()));
        lib.update_track(ids[0], &track(0, "/Music/renamed.mp3", "Artist 0", "Title 0", 301.0)).unwrap();
        lib.remove_tracks(&[ids[1], ids[5]]).unwrap();
        lib.add_tracks(&[track(0, "/Music/extra.mp3", "Artist 2", "Title 3", 300.0)]).unwrap();
        assert_eq!(cached(&lib), plan_with(&lib.duplicate_index().unwrap()));
    }

    #[test]
    fn survey_counts_unsupported_and_unreadable_files_and_ignores_hidden_ones() {
        let dir = std::env::temp_dir().join(format!("ff-survey-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        for name in ["a.mp3", "b.MP3", "notes.txt", "sub/c.m4p", "sub/d.txt", ".DS_Store", "README", "a.mp3.asd", "x.reapeaks", "y.pkf", "._a.mp3"] {
            std::fs::write(dir.join(name), b"x").unwrap();
        }
        let survey = survey_files(&[dir.to_string_lossy().into_owned()]);
        let scanned = Scanned { tracks: vec![track(0, &dir.join("a.mp3").to_string_lossy(), "", "a", 1.0)], known: vec![] };
        let (_, plan) = plan_import(1, &DuplicateIndex::default(), &|_| None, scanned, &survey);
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(
            plan.skipped,
            SkippedFiles { unsupported: 4, unsupported_exts: vec![".m4p".into(), ".txt".into()], unreadable: 1 }
        );
    }

    fn wav(seconds: u32) -> Vec<u8> {
        let rate = 8000u32;
        let data_len = rate * seconds * 2;
        let mut out = Vec::new();
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + data_len).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&rate.to_le_bytes());
        out.extend_from_slice(&(rate * 2).to_le_bytes());
        out.extend_from_slice(&2u16.to_le_bytes());
        out.extend_from_slice(&16u16.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&data_len.to_le_bytes());
        out.resize(out.len() + data_len as usize, 0);
        out
    }

    #[test]
    fn parallel_scan_returns_what_the_scanner_returns() {
        use pioneer_usb_writer::scanner;
        let dir = std::env::temp_dir().join(format!("ff-scan-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let folder = dir.join("folder");
        std::fs::create_dir_all(folder.join("sub/deeper")).unwrap();
        std::fs::create_dir_all(dir.join("loose")).unwrap();
        for (i, name) in ["b.wav", "A.WAV", "sub/c.wav", "sub/deeper/a.wav", "sub/z.wav"].iter().enumerate() {
            std::fs::write(folder.join(name), wav(i as u32 + 1)).unwrap();
        }
        std::fs::write(folder.join("broken.mp3"), b"not audio").unwrap();
        std::fs::write(folder.join("notes.txt"), b"x").unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&folder, folder.join("sub/loop")).unwrap();
            std::os::unix::fs::symlink(folder.join("sub/deeper"), folder.join("linked")).unwrap();
        }
        for name in ["2.wav", "1.wav"] {
            std::fs::write(dir.join("loose").join(name), wav(2)).unwrap();
        }
        std::fs::write(dir.join("loose/bad.flac"), b"nope").unwrap();
        let loose: Vec<PathBuf> =
            ["2.wav", "notes.txt", "bad.flac", "1.wav", "missing.wav"].iter().map(|n| dir.join("loose").join(n)).collect();
        let mut paths: Vec<String> = loose.iter().map(|p| p.to_string_lossy().into_owned()).collect();
        paths.insert(1, folder.to_string_lossy().into_owned());

        let mut expected = scanner::scan_files(&loose).unwrap();
        expected.extend(scanner::scan_directory(&folder).unwrap());
        let survey = survey_files(&paths);
        let mut files_seen = 0;
        let scanned = scan_paths(&survey, &HashSet::new(), &mut || files_seen += 1);
        let known: HashSet<PathBuf> = [folder.join("b.wav"), dir.join("loose/1.wav")].into();
        let mut files_seen_known = 0;
        let partly_known = scan_paths(&survey, &known, &mut || files_seen_known += 1);
        std::fs::remove_dir_all(&dir).unwrap();

        assert_eq!(format!("{:?}", scanned.tracks), format!("{expected:?}"));
        assert!(scanned.known.is_empty());
        assert_eq!(scanned.tracks.len(), if cfg!(unix) { 8 } else { 7 });
        assert_eq!(files_seen, scanned.tracks.len() + 2);
        assert_eq!(files_seen, survey.audio_count());

        let mut unread: Vec<PathBuf> = partly_known.known.clone();
        unread.sort();
        let mut expected_unread: Vec<PathBuf> = known.into_iter().collect();
        expected_unread.sort();
        assert_eq!(unread, expected_unread);
        assert_eq!(partly_known.tracks.len(), scanned.tracks.len() - 2);
        assert!(partly_known.tracks.iter().all(|t| !expected_unread.contains(&t.source_path)));
        assert_eq!(files_seen_known, files_seen);
    }

    fn wav_with_cover(cover: &[u8]) -> Vec<u8> {
        let mut apic = vec![0u8];
        apic.extend_from_slice(b"image/jpeg\0");
        apic.push(3);
        apic.push(0);
        apic.extend_from_slice(cover);
        let mut frame = b"APIC".to_vec();
        frame.extend_from_slice(&(apic.len() as u32).to_be_bytes());
        frame.extend_from_slice(&[0, 0]);
        frame.extend_from_slice(&apic);
        let size = frame.len() as u32;
        let mut tag = b"ID3\x03\x00\x00".to_vec();
        tag.extend((0..4).rev().map(|i| ((size >> (7 * i)) & 0x7f) as u8));
        tag.extend_from_slice(&frame);
        if tag.len() % 2 == 1 {
            tag.push(0);
        }
        let mut out = wav(1);
        out.extend_from_slice(b"id3 ");
        out.extend_from_slice(&(tag.len() as u32).to_le_bytes());
        out.extend_from_slice(&tag);
        let riff_len = (out.len() - 8) as u32;
        out[4..8].copy_from_slice(&riff_len.to_le_bytes());
        out
    }

    #[test]
    fn covers_are_left_out_of_the_scan_and_stored_after_the_rows() {
        let dir = std::env::temp_dir().join(format!("ff-covers-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let cover = vec![0xFF, 0xD8, 0xFF, 0xE0, 1, 2, 3, 4];
        std::fs::write(dir.join("with.wav"), wav_with_cover(&cover)).unwrap();
        std::fs::write(dir.join("without.wav"), wav(1)).unwrap();
        let survey = survey_files(&[dir.to_string_lossy().into_owned()]);
        let scanned = scan_paths(&survey, &HashSet::new(), &mut || {});
        assert_eq!(scanned.tracks.len(), 2);
        assert!(scanned.tracks.iter().all(|t| t.artwork.is_none()));

        let lib = LocalLibrary::open_in_memory().unwrap();
        let ids = lib.add_tracks_covers_later(&scanned.tracks).unwrap();
        let lib = Mutex::new(lib);
        let mut announced = Vec::new();
        let stored = store_pending_covers(&lib, None, &mut |ids| announced.extend(ids)).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();

        let lib = lib.into_inner().unwrap();
        let with = scanned.tracks.iter().position(|t| t.source_path.ends_with("with.wav")).unwrap();
        assert_eq!(stored, 1);
        assert_eq!(announced, vec![ids[with] as u32]);
        assert_eq!(lib.get_artwork(ids[with]).unwrap(), Some(cover));
        assert!(lib.get_artwork(ids[1 - with]).unwrap().is_none());
        assert!(lib.pending_artwork(0, 10).unwrap().is_empty());
    }
}
