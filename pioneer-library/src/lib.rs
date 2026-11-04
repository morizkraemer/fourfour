//! Persistent local music library for Pioneer CDJ workflows.
//!
//! Backed by a plain SQLite database. Provides CRUD for tracks, analyses,
//! artwork, and playlists, plus convenience methods to export or sync to a
//! Pioneer-formatted USB drive via [`pioneer_usb_writer`].
//!
//! Analysis data (beat grids, waveforms, cue points) is stored as ANLZ binary
//! files on disk alongside the database. The SQLite `analyses` table keeps only
//! a lightweight index (bpm + key) for fast queries.

mod analysis_write;
pub mod backup;
mod cover_store;
mod duplicates;
mod edits;
pub mod failure;
mod file_health;
mod multi_sync;
mod queries;
pub mod rekordbox;
mod row_preview;
mod schema;
pub mod sticks;
mod user_edits;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use anyhow::{Context, Result};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use pioneer_usb_writer::models::{
    AnalysisResult, Beat, BeatGrid, ColorWaveform, Playlist, SyncReport, Track, WaveformPreview,
};
use pioneer_usb_writer::{reader, writer};

pub use analysis_write::{AnalysisWrite, StagedAnalysis, remove_stale_staging};
pub use cover_store::{COVERS_DIR, covers_dir_of};
pub use schema::{SCHEMA_VERSION, TAG_FOLD_VERSION};
use cover_store::CoverStore;
pub use duplicates::{DURATION_TOLERANCE_SECS, DuplicateCandidate, DuplicateIndex, DuplicateOf, DuplicateReason, normalise_tag, file_name_key, path_key};
pub use edits::{EDITS_FILE, HOT_CUE_SLOTS, UserEdits};
pub use file_health::{FileStatus, Relinked, RelocateReport, TrackFile, file_status};
use rekordbox::ImportedCue;

/// One beat of an edited grid: where it is and its place in the bar (1 to 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BeatPosition {
    pub time_ms: u32,
    pub bar_position: u8,
}

pub use multi_sync::{
    NoProgress, PlanTrack, PreparedMultiSync, ProgressSink, StickProgress, StickReport,
    StickStatus, SyncPhase, SyncSelection, SyncTarget, UsbSyncPlan, VolumeCapacity, WriterFn,
    statvfs_available_bytes, volume_capacity,
};
pub use sticks::StickContents;
pub use user_edits::{LABELS_CACHE_FILE, MAX_BPM, MIN_BPM, TrackLabels, TrackLabelsChange, TrackLabelsReader, is_camelot};
pub use row_preview::{ROW_BAND_COLUMNS, RowPreview, effective_preview, row_bands};

/// Runs `read` on the ANLZ directory of every track in `ids`, on as many threads as there are cores, and keeps the
/// tracks it returns something for.
fn read_track_dirs<T: Send>(anlz_dir: &Path, ids: &[i64], read: impl Fn(&Path) -> Option<T> + Sync) -> HashMap<i64, T> {
    let workers = std::thread::available_parallelism().map_or(4, |n| n.get());
    let chunk_len = ids.len().div_ceil(workers).max(1);

    let mut out = HashMap::with_capacity(ids.len());
    std::thread::scope(|scope| {
        let handles: Vec<_> = ids
            .chunks(chunk_len)
            .map(|chunk| {
                let read = &read;
                scope.spawn(move || {
                    chunk
                        .iter()
                        .filter_map(|id| read(&anlz_dir.join(id.to_string())).map(|a| (*id, a)))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        for handle in handles {
            out.extend(handle.join().expect("track directory reader thread panicked"));
        }
    });
    out
}

/// A track as the track list shows it: the tags it lists, its flags, and the stored row preview.
#[derive(Debug, Clone, PartialEq)]
pub struct TrackListRow {
    pub id: u32,
    pub source_path: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub genre: String,
    /// BPM * 100: the analysed BPM when there is one, else the tag's.
    pub tempo: u32,
    /// The analysed key when there is one, else the tag's.
    pub key: String,
    pub duration_secs: f64,
    pub bitrate: u32,
    pub file_size: u64,
    pub has_artwork: bool,
    /// Stage bits of the analysis row ([`analysis_stage`]); `None` when the track has none.
    pub analysis_stages: Option<u8>,
    pub row_preview: Option<RowPreview>,
}

/// A [`TrackListRow`] whose texts borrow from the query: valid inside the visit that receives it.
pub struct TrackListRowRef<'a> {
    pub id: u32,
    pub source_path: &'a str,
    pub title: &'a str,
    pub artist: &'a str,
    pub album: &'a str,
    pub genre: &'a str,
    pub tempo: u32,
    pub key: &'a str,
    pub duration_secs: f64,
    pub bitrate: u32,
    pub file_size: u64,
    pub has_artwork: bool,
    pub analysis_stages: Option<u8>,
    pub row_preview: Option<&'a [u8]>,
}

/// See [`LocalLibrary::track_list_scanner`].
pub struct TrackListScanner {
    db_path: PathBuf,
}

impl TrackListScanner {
    /// [`LocalLibrary::for_each_track_list_row`] on a new read-only connection.
    pub fn for_each_track_list_row(
        &self,
        ids: std::ops::RangeInclusive<i64>,
        visit: impl FnMut(TrackListRowRef<'_>),
    ) -> Result<usize> {
        let conn = Connection::open_with_flags(
            &self.db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .with_context(|| format!("Failed to open {} to read tracks", self.db_path.display()))?;
        queries::for_each_track_list_row(&conn, ids, false, visit)
    }
}

/// Bits of `analyses.stages`: which parts of a track's analysis are stored.
pub mod analysis_stage {
    pub const WAVEFORM: u8 = 1;
    pub const KEY: u8 = 2;
    pub const GRID: u8 = 4;
    pub const ALL: u8 = WAVEFORM | KEY | GRID;
}

/// A track whose analysis failed, as the analysis run stores it so the next launch does not retry the same file
/// with the same analyzer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalysisFailure {
    pub track_id: i64,
    pub source_path: PathBuf,
    pub file_size: Option<u64>,
    pub modified_ms: Option<u64>,
    pub kind: String,
    pub attempts: u32,
    pub analyzer_version: String,
    pub failed_at_ms: u64,
}

/// Where the move of v7 cover blobs into the cover store stands after a call to
/// [`LocalLibrary::move_legacy_covers`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoverMove {
    pub moved: u64,
    pub remaining: u64,
}

/// Free space the cover move keeps on top of the bytes it is about to write (the `VACUUM` at the end writes a copy
/// of what is left of the database).
pub const COVER_MOVE_SPARE_BYTES: u64 = 256 * 1024 * 1024;

/// Summary report from importing a Pioneer USB into the local library.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportReport {
    /// Number of tracks imported from the USB.
    pub tracks_imported: u32,
    /// Number of tracks skipped because they were already in the library.
    pub tracks_skipped: u32,
    /// Number of playlists imported from the USB.
    pub playlists_imported: u32,
}

/// A persistent local music library backed by SQLite.
///
/// Manages tracks, analyses, playlists, and artwork on the user's computer.
/// Provides convenience methods to export the library to a Pioneer-formatted USB
/// or import from an existing USB.
pub struct LocalLibrary {
    conn: Connection,
    /// Path to the database file. `None` for in-memory databases (tests).
    db_path: Option<PathBuf>,
    /// Folded keys of every track for duplicate lookups, built by the first
    /// [`Self::with_duplicate_index`] and kept current by every write that changes a track's
    /// path, size, artist, title or duration, or adds or removes a track.
    duplicates: Mutex<Option<DuplicateIndex>>,
    covers: CoverStore,
}

impl LocalLibrary {
    /// Open (or create) a library database at the given path.
    ///
    /// Runs migrations automatically if the schema is outdated.
    pub fn open(db_path: &Path) -> Result<Self> {
        let conn =
            Connection::open(db_path).with_context(|| format!("Failed to open {}", db_path.display()))?;
        schema::initialize(&conn, Some(db_path))?;
        Ok(Self {
            conn,
            db_path: Some(db_path.to_path_buf()),
            duplicates: Mutex::default(),
            covers: CoverStore::for_library(Some(db_path)),
        })
    }

    /// Open an in-memory library (for testing).
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        schema::initialize(&conn, None)?;
        Ok(Self {
            conn,
            db_path: None,
            duplicates: Mutex::default(),
            covers: CoverStore::for_library(None),
        })
    }

    // -----------------------------------------------------------------------
    // ANLZ directory helpers
    // -----------------------------------------------------------------------

    /// Root directory for ANLZ files: `{db_parent}/anlz/`.
    fn anlz_dir(&self) -> Option<PathBuf> {
        self.db_path
            .as_ref()
            .map(|p| p.parent().unwrap_or(Path::new(".")).join("anlz"))
    }

    /// Per-track ANLZ directory: `{db_parent}/anlz/{track_id}/`.
    fn track_anlz_dir(&self, track_id: i64) -> Option<PathBuf> {
        self.anlz_dir().map(|d| d.join(track_id.to_string()))
    }

    /// Remove ANLZ files for a track (best-effort, ignores errors).
    fn remove_track_anlz(&self, track_id: i64) {
        if let Some(dir) = self.track_anlz_dir(track_id) {
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    // -----------------------------------------------------------------------
    // Track CRUD
    // -----------------------------------------------------------------------

    /// Add a track to the library. Returns the library-assigned track ID.
    ///
    /// If `track.artwork` is `Some`, artwork is stored automatically.
    /// The `track.id` field is ignored — the library assigns its own IDs.
    pub fn add_track(&self, track: &Track) -> Result<i64> {
        let id = queries::insert_track(&self.conn, track)?;
        self.note_duplicates_added(&[DuplicateCandidate::stored(id, track)]);
        if let Some(ref art) = track.artwork {
            self.store_cover(&self.conn, id, art)?;
        }
        Ok(id)
    }

    /// Add multiple tracks in a single transaction. Returns assigned IDs.
    ///
    /// Embedded covers make the write-ahead log large (~25 MB per 1,000 imported files), and
    /// the commit would copy all of it into the database file and sync both. The commit only
    /// writes the log (`synchronous=NORMAL`); the checkpoint, which syncs the log first and
    /// then the database, runs right after on a thread of its own.
    pub fn add_tracks(&self, tracks: &[Track]) -> Result<Vec<i64>> {
        self.add_tracks_then(tracks, |tx, id, track| match &track.artwork {
            Some(art) => self.store_cover(tx, id, art).map(|_| ()),
            None => Ok(()),
        })
    }

    /// Adds tracks like [`Self::add_tracks`] but leaves their covers out (`track.artwork` is
    /// ignored) and queues every new track for [`Self::pending_artwork`]: the rows are there
    /// at once, the covers follow through [`Self::store_pending_artwork`].
    pub fn add_tracks_covers_later(&self, tracks: &[Track]) -> Result<Vec<i64>> {
        self.add_tracks_then(tracks, |tx, id, _| queries::queue_pending_artwork(tx, id))
    }

    fn add_tracks_then(&self, tracks: &[Track], then: impl Fn(&Connection, i64, &Track) -> Result<()>) -> Result<Vec<i64>> {
        let ids = self.write_bulk(|| self.insert_tracks(tracks, then))?;
        let added: Vec<DuplicateCandidate> = ids.iter().zip(tracks).map(|(id, track)| DuplicateCandidate::stored(*id, track)).collect();
        self.note_duplicates_added(&added);
        Ok(ids)
    }

    /// Runs a large write that only has to reach the log (`synchronous=NORMAL`), with no
    /// checkpoint inside it; the checkpoint runs right after on a thread of its own.
    fn write_bulk<T>(&self, write: impl FnOnce() -> Result<T>) -> Result<T> {
        let Some(db_path) = &self.db_path else {
            return write();
        };
        let pragma = |name: &str| self.conn.query_row(&format!("PRAGMA {name}"), [], |row| row.get::<_, i64>(0));
        let (autocheckpoint, synchronous, cache) = (pragma("wal_autocheckpoint")?, pragma("synchronous")?, pragma("cache_size")?);
        self.conn.execute_batch(&format!(
            "PRAGMA wal_autocheckpoint=0; PRAGMA synchronous=NORMAL; PRAGMA cache_size={}",
            BULK_WRITE_CACHE_KIB.min(cache)
        ))?;
        let written = write();
        self.conn.execute_batch(&format!(
            "PRAGMA wal_autocheckpoint={autocheckpoint}; PRAGMA synchronous={synchronous}; PRAGMA cache_size={cache}"
        ))?;
        let value = written?;
        checkpoint_in_background(db_path.clone());
        Ok(value)
    }

    fn insert_tracks(&self, tracks: &[Track], then: impl Fn(&Connection, i64, &Track) -> Result<()>) -> Result<Vec<i64>> {
        let tx = self.conn.unchecked_transaction()?;
        let mut ids = Vec::with_capacity(tracks.len());
        for track in tracks {
            let id = queries::insert_track(&tx, track)?;
            then(&tx, id, track)?;
            ids.push(id);
        }
        tx.commit()?;
        Ok(ids)
    }

    /// Up to `limit` tracks with an id above `after` whose covers are still to be read, in id
    /// order, with their audio files.
    pub fn pending_artwork(&self, after: i64, limit: usize) -> Result<Vec<(i64, PathBuf)>> {
        queries::select_pending_artwork(&self.conn, after, limit)
    }

    /// Stores the covers read for queued tracks and takes them off the queue, in one
    /// transaction; each cover comes with the track's id and the file it was read from, and
    /// `None` = the file has no cover. A track removed or pointing at another file in the
    /// meantime is skipped, and a cover set by hand in the meantime is kept.
    pub fn store_pending_artwork(&self, covers: &[(i64, PathBuf, Option<Vec<u8>>)]) -> Result<()> {
        let mut unused = Vec::new();
        self.write_bulk(|| {
            let tx = self.conn.unchecked_transaction()?;
            for (id, path, cover) in covers {
                if let Some(cover) = cover {
                    let hash = self.covers.put(cover)?;
                    if !queries::insert_pending_artwork(&tx, *id, path, &hash)? {
                        unused.push(hash);
                    }
                }
                queries::unqueue_pending_artwork(&tx, *id, path)?;
            }
            tx.commit()?;
            Ok(())
        })?;
        self.release_covers(&unused)
    }

    /// The audio paths of every library track.
    pub fn track_paths(&self) -> Result<HashSet<PathBuf>> {
        queries::select_track_paths(&self.conn)
    }

    /// Update metadata for an existing track (identified by library ID).
    pub fn update_track(&self, id: i64, track: &Track) -> Result<()> {
        let old = self.duplicates_before(&[id])?;
        queries::update_track(&self.conn, id, track)?;
        self.duplicates_after(old, |_| Some(DuplicateCandidate::stored(id, track)));
        Ok(())
    }

    /// Remove a track (and its analysis + artwork via CASCADE), including ANLZ files.
    pub fn remove_track(&self, id: i64) -> Result<()> {
        self.remove_tracks(&[id])
    }

    /// Remove multiple tracks in a single transaction, including ANLZ files.
    pub fn remove_tracks(&self, ids: &[i64]) -> Result<()> {
        let old = self.duplicates_before(ids)?;
        let covers = queries::select_artwork_hashes_of(&self.conn, ids)?;
        queries::delete_tracks(&self.conn, ids)?;
        self.duplicates_after(old, |_| None);
        for id in ids {
            self.remove_track_anlz(*id);
        }
        self.release_covers(&covers)
    }

    /// Retrieve a single track by library ID. Artwork is not loaded (use `get_artwork`).
    pub fn get_track(&self, id: i64) -> Result<Option<Track>> {
        queries::select_track(&self.conn, id)
    }

    /// Retrieve all tracks in the library. Artwork is not loaded for performance.
    pub fn get_all_tracks(&self) -> Result<Vec<Track>> {
        queries::select_all_tracks(&self.conn)
    }

    /// Check whether a source path is already in the library.
    pub fn track_exists_by_path(&self, source_path: &Path) -> Result<bool> {
        queries::track_exists_by_path(&self.conn, source_path)
    }

    /// Every track's duplicate-detection fields, in id order: the slim form of `get_all_tracks`.
    pub fn duplicate_candidates(&self) -> Result<Vec<DuplicateCandidate>> {
        queries::select_duplicate_candidates(&self.conn)
    }

    /// A fresh index over the library, read from the database.
    pub fn duplicate_index(&self) -> Result<DuplicateIndex> {
        Ok(DuplicateIndex::new(&self.duplicate_candidates()?))
    }

    /// Runs `read` on the library's duplicate index. The index is built on first use and
    /// kept current by the writes after that; it is rebuilt when its size no longer matches
    /// the library's track count.
    pub fn with_duplicate_index<T>(&self, read: impl FnOnce(&DuplicateIndex) -> T) -> Result<T> {
        let mut cache = self.lock_duplicates();
        let tracks = queries::track_count(&self.conn)? as usize;
        if cache.as_ref().is_none_or(|index| index.len() != tracks) {
            *cache = Some(self.duplicate_index()?);
        }
        Ok(read(cache.as_ref().expect("index built above")))
    }

    fn lock_duplicates(&self) -> MutexGuard<'_, Option<DuplicateIndex>> {
        self.duplicates.lock().expect("duplicate index lock")
    }

    pub(crate) fn forget_duplicates(&self) {
        *self.lock_duplicates() = None;
    }

    fn note_duplicates_added(&self, candidates: &[DuplicateCandidate]) {
        if let Some(index) = self.lock_duplicates().as_mut() {
            index.insert_all(candidates);
        }
    }

    /// What the cached index holds for `ids`, read from the database before a write changes
    /// them (`None` when no index is cached). Hand it to [`Self::duplicates_after`].
    pub(crate) fn duplicates_before(&self, ids: &[i64]) -> Result<Option<Vec<DuplicateCandidate>>> {
        if self.lock_duplicates().is_none() {
            return Ok(None);
        }
        queries::select_duplicate_candidates_of(&self.conn, ids).map(Some)
    }

    /// After the write: takes the `old` tracks out of the cached index and puts back what
    /// `rewrite` makes of each (`None` leaves it out).
    pub(crate) fn duplicates_after(&self, old: Option<Vec<DuplicateCandidate>>, rewrite: impl Fn(&DuplicateCandidate) -> Option<DuplicateCandidate>) {
        let Some(old) = old else { return };
        let mut cache = self.lock_duplicates();
        let Some(index) = cache.as_mut() else { return };
        for candidate in &old {
            index.remove(candidate);
        }
        index.insert_all(&old.iter().filter_map(rewrite).collect::<Vec<_>>());
    }

    fn duplicates_after_path_change(&self, old: Option<Vec<DuplicateCandidate>>, new_paths: &HashMap<i64, PathBuf>) {
        self.duplicates_after(old, |candidate| {
            let path = new_paths.get(&(candidate.id as i64))?;
            Some(candidate.clone().with_source_path(PathBuf::from(path.to_string_lossy().into_owned())))
        });
    }

    /// A track's duplicate-detection fields.
    pub fn duplicate_candidate(&self, id: u32) -> Result<Option<DuplicateCandidate>> {
        Ok(queries::select_duplicate_candidates_of(&self.conn, &[id as i64])?.pop())
    }

    /// Every track's audio path and stored size, for the file health check.
    pub fn track_files(&self) -> Result<Vec<TrackFile>> {
        Ok(queries::select_track_files(&self.conn)?
            .into_iter()
            .map(|(track_id, path, size)| TrackFile { track_id, path: PathBuf::from(path), file_size: size.max(0) as u64 })
            .collect())
    }

    /// Point a track at its audio file's new location. Only the path changes: the track id,
    /// and with it analysis, cues, grid, edits and playlist membership, stay as they are.
    pub fn relink_track(&self, track_id: i64, new_path: &Path) -> Result<()> {
        if file_status(new_path) != FileStatus::Ok {
            anyhow::bail!("{} is not a readable file", new_path.display());
        }
        let old = self.duplicates_before(&[track_id])?;
        if !queries::update_source_path(&self.conn, track_id, new_path)? {
            anyhow::bail!("Track {track_id} is not in the library");
        }
        self.duplicates_after_path_change(old, &HashMap::from([(track_id, new_path.to_path_buf())]));
        Ok(())
    }

    /// Searches `folder` for the files of `track_ids` that are missing and relinks every
    /// unambiguous match in one transaction.
    pub fn relocate_in_folder(&self, track_ids: &[i64], folder: &Path) -> Result<RelocateReport> {
        let wanted: HashSet<i64> = track_ids.iter().copied().collect();
        let files = self.track_files()?;
        let taken: HashSet<PathBuf> = files.iter().map(|f| f.path.clone()).collect();
        let missing: Vec<TrackFile> = files
            .into_iter()
            .filter(|f| wanted.contains(&f.track_id) && file_status(&f.path) != FileStatus::Ok)
            .collect();
        let (matched, ambiguous, not_found) = file_health::plan_relocation(&missing, folder, &taken);
        let matched_ids: Vec<i64> = matched.iter().map(|(track_id, _)| *track_id).collect();
        let old = self.duplicates_before(&matched_ids)?;
        let tx = self.conn.unchecked_transaction()?;
        for (track_id, path) in &matched {
            queries::update_source_path(&tx, *track_id, path)?;
        }
        tx.commit()?;
        self.duplicates_after_path_change(old, &matched.iter().cloned().collect());
        Ok(RelocateReport {
            matched: matched
                .into_iter()
                .map(|(track_id, path)| Relinked { track_id, source_path: path.to_string_lossy().into_owned() })
                .collect(),
            ambiguous,
            not_found,
        })
    }

    /// Return the number of tracks in the library.
    pub fn track_count(&self) -> Result<u32> {
        queries::track_count(&self.conn)
    }

    // -----------------------------------------------------------------------
    // Artwork
    // -----------------------------------------------------------------------

    /// Store (or replace) artwork for a track.
    pub fn set_artwork(&self, track_id: i64, image_data: &[u8]) -> Result<()> {
        self.store_cover(&self.conn, track_id, image_data)?;
        Ok(())
    }

    /// Retrieve artwork for a track. Returns `None` if no artwork stored.
    pub fn get_artwork(&self, track_id: i64) -> Result<Option<Vec<u8>>> {
        match queries::select_artwork_hash(&self.conn, track_id)? {
            None => Ok(None),
            Some(Some(hash)) => self.covers.get(&hash),
            Some(None) => queries::select_legacy_artwork(&self.conn, track_id),
        }
    }

    /// Remove artwork for a track.
    pub fn remove_artwork(&self, track_id: i64) -> Result<()> {
        let old = queries::select_artwork_hashes_of(&self.conn, &[track_id])?;
        queries::delete_artwork(&self.conn, track_id)?;
        self.release_covers(&old)
    }

    /// Writes the image to the cover store and points the track at it, on `conn` (the library's connection, maybe
    /// inside a transaction). The image the track had before is deleted once the write is committed and no other
    /// track uses it: here when `conn` is not in a transaction, else by the next sweep.
    fn store_cover(&self, conn: &Connection, track_id: i64, image_data: &[u8]) -> Result<String> {
        let old = queries::select_artwork_hashes_of(conn, &[track_id])?;
        let hash = self.covers.put(image_data)?;
        queries::upsert_artwork(conn, track_id, &hash)?;
        if conn.is_autocommit() {
            self.release_covers(&old)?;
        }
        Ok(hash)
    }

    /// Deletes the stored images among `hashes` that no track references any more.
    fn release_covers(&self, hashes: &[String]) -> Result<()> {
        for hash in hashes {
            if !queries::cover_is_referenced(&self.conn, hash)? {
                self.covers.remove(hash);
            }
        }
        Ok(())
    }

    /// Deletes stored images no track references (left by a write that did not commit) and temp files of killed
    /// writes. Returns how many files went.
    pub fn sweep_covers(&self) -> Result<usize> {
        let referenced = queries::select_referenced_covers(&self.conn)?;
        let (stored, temps) = self.covers.list();
        let mut removed = 0;
        for hash in stored.iter().filter(|hash| !referenced.contains(*hash)) {
            self.covers.remove(hash);
            removed += 1;
        }
        for temp in &temps {
            if std::fs::remove_file(temp).is_ok() {
                removed += 1;
            }
        }
        Ok(removed)
    }

    /// The cover store folder (`None` for an in-memory library).
    pub fn covers_dir(&self) -> Option<&Path> {
        self.covers.dir()
    }

    /// Rows and bytes of v7 covers still in the database (`None` when all are moved).
    pub fn legacy_covers(&self) -> Result<Option<(u64, u64)>> {
        if !queries::legacy_artwork_exists(&self.conn)? {
            return Ok(None);
        }
        queries::legacy_artwork_size(&self.conn).map(Some)
    }

    /// Checks that the cover move fits on the library's volume: the bytes still in the database plus
    /// [`COVER_MOVE_SPARE_BYTES`]. `available` returns the free bytes of a folder (`statvfs_available_bytes`).
    pub fn check_space_for_legacy_covers(&self, available: &dyn Fn(&Path) -> Result<u64>) -> Result<()> {
        let (Some((_, bytes)), Some(dir)) = (self.legacy_covers()?, self.db_path.as_deref().and_then(Path::parent)) else {
            return Ok(());
        };
        let free = available(dir)?;
        let needed = bytes + COVER_MOVE_SPARE_BYTES;
        if free < needed {
            return Err(failure::Failure::error(
                failure::LibraryFailure::DiskFull,
                format!(
                    "Not enough space to move the covers out of the library: {} MB needed, {} MB free; they stay in the database for now",
                    needed / 1_000_000,
                    free / 1_000_000
                ),
            ));
        }
        Ok(())
    }

    /// Moves up to `batch` v7 cover blobs into the cover store, in one transaction: each image is written first,
    /// then the track's artwork row points at it and the blob row goes. A cover set or removed since the migration
    /// is kept as it is. When the last blob is moved the blob table is dropped, the database vacuumed and the store
    /// swept. Safe to stop at any point: the next call continues with what is left.
    pub fn move_legacy_covers(&self, batch: usize) -> Result<CoverMove> {
        if !queries::legacy_artwork_exists(&self.conn)? {
            return Ok(CoverMove { moved: 0, remaining: 0 });
        }
        let blobs = queries::select_legacy_artwork_batch(&self.conn, batch.max(1))?;
        let tx = self.conn.unchecked_transaction()?;
        for (track_id, image) in &blobs {
            let hash = self.covers.put(image)?;
            queries::finish_legacy_artwork(&tx, *track_id, &hash)?;
        }
        tx.commit().context("Failed to store moved covers")?;
        let (remaining, _) = queries::legacy_artwork_size(&self.conn)?;
        if remaining == 0 {
            queries::drop_legacy_artwork(&self.conn)?;
            if self.db_path.is_some() {
                self.conn.execute_batch("VACUUM").context("Failed to compact the library after moving the covers")?;
            }
            self.sweep_covers()?;
        }
        Ok(CoverMove { moved: blobs.len() as u64, remaining })
    }

    // -----------------------------------------------------------------------
    // Analysis failures
    // -----------------------------------------------------------------------

    pub fn analysis_failure(&self, track_id: i64) -> Result<Option<AnalysisFailure>> {
        queries::select_analysis_failure(&self.conn, track_id)
    }

    pub fn analysis_failures(&self) -> Result<Vec<AnalysisFailure>> {
        queries::select_analysis_failures(&self.conn)
    }

    /// Stores (or replaces) the track's failure record; nothing is stored for a track that is not in the library.
    pub fn record_analysis_failure(&self, failure: &AnalysisFailure) -> Result<()> {
        queries::upsert_analysis_failure(&self.conn, failure)
    }

    /// Forgets the failures of `track_ids`; returns how many there were.
    pub fn clear_analysis_failures(&self, track_ids: &[i64]) -> Result<usize> {
        let tx = self.conn.unchecked_transaction()?;
        let removed = queries::delete_analysis_failures(&tx, track_ids)?;
        tx.commit()?;
        Ok(removed)
    }

    // -----------------------------------------------------------------------
    // Analysis
    // -----------------------------------------------------------------------

    /// Store (or replace) analysis results for a track.
    ///
    /// Writes ANLZ binary files to disk (if the library is file-backed) and
    /// updates the lightweight bpm/key index in SQLite.
    pub fn set_analysis(&self, track_id: i64, analysis: &AnalysisResult) -> Result<()> {
        self.set_analysis_stages(track_id, analysis, analysis_stage::ALL)
    }

    /// Store an analysis that has only some stages (`analysis_stage` bits). The track stays pending
    /// for the missing stages: it is listed by `get_unanalyzed_track_ids` and left out of USB export.
    pub fn set_analysis_stages(&self, track_id: i64, analysis: &AnalysisResult, stages: u8) -> Result<()> {
        let analysis = &self.keep_user_edits(track_id, &self.keep_imported_data(track_id, analysis)?)?;
        self.write_analysis(track_id, analysis, stages)
    }

    /// Replace the beat grid of an analyzed track with an edited one (nudged, or set from the playhead).
    /// The tempo of every beat stays the stored one. An edit is user data, so it is written as it is (not
    /// replaced by a locked imported grid) and the analysis stages stay as they were.
    /// Replace the beat grid by hand (locks it). `offset_ms` is the grid's place between whole milliseconds (see
    /// [`UserEdits::grid_offset_ms`]); anything outside half a millisecond either way is refused. `anchor_ms` is the
    /// downbeat a later BPM change keeps ([`UserEdits::grid_anchor_ms`]).
    pub fn set_beat_grid(
        &self,
        track_id: i64,
        positions: &[BeatPosition],
        offset_ms: Option<f64>,
        anchor_ms: Option<f64>,
    ) -> Result<()> {
        if let Some(anchor) = anchor_ms {
            if !anchor.is_finite() {
                anyhow::bail!("Beat grid anchor {anchor} ms is not a number");
            }
        }
        if let Some(offset) = offset_ms {
            if !(offset.is_finite() && offset.abs() <= 0.5) {
                anyhow::bail!("Beat grid offset {offset} ms is not within half a millisecond");
            }
        }
        let mut analysis = self
            .get_analysis(track_id)?
            .context("Track has no analysis to edit the beat grid of")?;
        let tempo = analysis
            .beat_grid
            .beats
            .first()
            .map(|b| b.tempo)
            .unwrap_or_else(|| (analysis.bpm * 100.0).round() as u32);
        analysis.beat_grid = BeatGrid {
            beats: positions
                .iter()
                .map(|p| Beat { bar_position: p.bar_position, time_ms: p.time_ms, tempo })
                .collect(),
        };
        let stages = queries::select_analysis_stages(&self.conn, track_id)?.unwrap_or(analysis_stage::ALL);
        self.write_analysis(track_id, &analysis, stages)?;
        if let Some(dir) = self.track_anlz_dir(track_id) {
            let mut edits = UserEdits::read(&dir).unwrap_or_default();
            edits.grid_locked = true;
            edits.grid_offset_ms = offset_ms.filter(|o| *o != 0.0);
            edits.grid_anchor_ms = anchor_ms;
            edits.write(&dir)?;
        }
        Ok(())
    }

    /// The cues of a track with their names and colours: the user's list if they edited it, else the
    /// imported one, else what the analysis stored (no names or colours).
    pub fn get_cues(&self, track_id: i64) -> Result<Vec<ImportedCue>> {
        let dir = self.track_anlz_dir(track_id);
        if let Some(edits) = dir.as_deref().and_then(UserEdits::read).filter(|e| e.cues_locked) {
            return Ok(edits.cues);
        }
        if let Some(meta) = self.imported_meta(track_id).filter(|m| m.cues_locked) {
            return Ok(meta.cues);
        }
        Ok(self
            .get_analysis(track_id)?
            .map(|a| {
                a.cue_points
                    .iter()
                    .map(|c| ImportedCue {
                        hot_cue_number: c.hot_cue_number,
                        time_ms: c.time_ms,
                        loop_time_ms: c.loop_time_ms,
                        color: c.color.clone(),
                        name: c.name.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    /// Replace all cues of a track with an edited list (hot cue slots 1 to 8 are A to H, 0 is a memory
    /// cue). The list is user data: re-analysis keeps it. Names and colours live in `edits.json`; the
    /// ANLZ files get the positions.
    pub fn set_cues(&self, track_id: i64, cues: &[ImportedCue]) -> Result<()> {
        let cues = edits::normalize_cues(cues)?;
        let dir = self
            .track_anlz_dir(track_id)
            .context("This library keeps no files, so cues cannot be stored")?;
        let mut user_edits = UserEdits::read(&dir).unwrap_or_default();
        user_edits.cues_locked = true;
        user_edits.cues = cues.clone();
        user_edits.write(&dir)?;
        if let Some(mut analysis) = self.get_analysis(track_id)? {
            analysis.cue_points = rekordbox::to_cue_points(&cues);
            let stages = queries::select_analysis_stages(&self.conn, track_id)?.unwrap_or(analysis_stage::ALL);
            self.write_analysis(track_id, &analysis, stages)?;
        }
        Ok(())
    }

    /// Let re-analysis replace the beat grid, cues, BPM and key the user edited.
    pub fn release_user_edits(&self, track_id: i64) -> Result<()> {
        let Some(dir) = self.track_anlz_dir(track_id) else { return Ok(()) };
        if let Some(mut user_edits) = UserEdits::read(&dir) {
            user_edits.grid_locked = false;
            user_edits.grid_offset_ms = None;
            user_edits.grid_anchor_ms = None;
            user_edits.cues_locked = false;
            user_edits.bpm_locked = false;
            user_edits.key_locked = false;
            user_edits.write(&dir)?;
        }
        Ok(())
    }

    fn write_analysis(&self, track_id: i64, analysis: &AnalysisResult, stages: u8) -> Result<()> {
        self.write_anlz_files(track_id, analysis)?;

        let row_preview = match self.track_anlz_dir(track_id) {
            Some(dir) => read_waveform_analysis(&dir.join("ANLZ0000.DAT")).map_or(RowPreview::None, |a| RowPreview::from_analysis(&a)),
            None => RowPreview::from_analysis(analysis),
        };

        queries::upsert_analysis_index(&self.conn, track_id, analysis.bpm, &analysis.key, analysis.energy, &row_preview.to_blob(), stages)?;

        Ok(())
    }

    /// Re-analysis must not replace what the user edited or imported from Rekordbox: a locked beat
    /// grid (and its tempo) stays as stored, locked cues come from the edit or import metadata.
    fn keep_imported_data(&self, track_id: i64, analysis: &AnalysisResult) -> Result<AnalysisResult> {
        let mut kept = analysis.clone();
        let edits = self.track_anlz_dir(track_id).and_then(|dir| UserEdits::read(&dir)).unwrap_or_default();
        let meta = self.imported_meta(track_id);
        let imported_grid = meta.as_ref().filter(|m| m.grid_locked);
        if edits.grid_locked || imported_grid.is_some() {
            match self.get_analysis(track_id)? {
                Some(current) if !current.beat_grid.beats.is_empty() => {
                    kept.beat_grid = current.beat_grid;
                    kept.bpm = current.bpm;
                }
                _ => {
                    if let Some(meta) = imported_grid.filter(|m| !m.grid.is_empty()) {
                        kept.bpm = meta.grid[0].tempo as f64 / 100.0;
                        kept.beat_grid = BeatGrid { beats: meta.grid.clone() };
                    }
                }
            }
        }
        if edits.cues_locked {
            kept.cue_points = rekordbox::to_cue_points(&edits.cues);
        } else if let Some(meta) = meta.filter(|m| m.cues_locked) {
            kept.cue_points = rekordbox::to_cue_points(&meta.cues);
        }
        Ok(kept)
    }

    /// Store analyses for multiple tracks in a single transaction.
    pub fn set_analyses(&self, entries: &[(i64, &AnalysisResult)]) -> Result<()> {
        for (track_id, analysis) in entries {
            self.set_analysis(*track_id, analysis)?;
        }
        Ok(())
    }

    /// Retrieve analysis for a track. Returns `None` if not yet analyzed.
    ///
    /// Reads bpm/key from the DB index and full beat grid / waveform / cue data
    /// from the ANLZ files on disk. For in-memory libraries (or if ANLZ files
    /// are missing), returns a minimal result with just bpm and key.
    pub fn get_analysis(&self, track_id: i64) -> Result<Option<AnalysisResult>> {
        // Check the DB index first.
        let Some((bpm, key, energy)) = queries::select_analysis_index(&self.conn, track_id)? else {
            return Ok(None);
        };

        // Try reading full data from ANLZ files.
        if let Some(dir) = self.track_anlz_dir(track_id) {
            let dat_path = dir.join("ANLZ0000.DAT");
            if dat_path.exists() {
                let mut result = reader::anlz::read_anlz(&dat_path)?;
                // Override bpm/key/energy from the DB index (authoritative source).
                result.bpm = bpm;
                result.key = key;
                result.energy = energy;
                return Ok(Some(result));
            }
        }

        // Fallback for in-memory or missing files: return minimal result.
        Ok(Some(AnalysisResult {
            beat_grid: BeatGrid { beats: Vec::new() },
            waveform: WaveformPreview { data: [0u8; 400] },
            bpm,
            key,
            energy,
            cue_points: Vec::new(),
            color_waveform: None,
        }))
    }

    /// Waveform-only analyses for every analyzed track, for building list views.
    ///
    /// One SQL query plus a parallel file read, instead of one `get_analysis`
    /// call (query + full ANLZ parse) per track. Only `waveform` and
    /// `color_waveform` are populated; beat grid, cues, bpm and key are left
    /// empty. The color waveform carries just the `PWV6` overview (empty
    /// `detail`) unless the DAT's PWAV preview is flat, where the full reader
    /// fills it. Tracks whose ANLZ files are missing or unreadable are omitted.
    pub fn get_waveform_analyses(&self) -> Result<HashMap<i64, AnalysisResult>> {
        let ids = queries::select_analyzed_track_ids(&self.conn)?;
        Ok(self.read_waveform_analyses(&ids))
    }

    fn read_waveform_analyses(&self, ids: &[i64]) -> HashMap<i64, AnalysisResult> {
        let Some(anlz_dir) = self.anlz_dir() else {
            return HashMap::new();
        };
        read_track_dirs(&anlz_dir, ids, |dir| read_waveform_analysis(&dir.join("ANLZ0000.DAT")))
    }

    /// The stored row preview of every analyzed track: one SQL query, no file reads.
    /// Tracks stored before the column existed are missing until
    /// [`backfill_row_previews`](Self::backfill_row_previews) has run.
    pub fn get_row_previews(&self) -> Result<HashMap<i64, RowPreview>> {
        Ok(queries::select_row_previews(&self.conn)?
            .into_iter()
            .map(|(id, blob)| (id, RowPreview::from_blob(blob)))
            .collect())
    }

    /// Fill `analyses.row_preview` for tracks stored before the column existed,
    /// from their ANLZ files (parallel read, one transaction). Returns how many
    /// tracks were filled; later calls find nothing to do and cost one query.
    pub fn backfill_row_previews(&self) -> Result<usize> {
        let missing = queries::select_missing_row_preview_ids(&self.conn)?;
        if missing.is_empty() {
            return Ok(0);
        }
        let mut analyses = self.read_waveform_analyses(&missing);
        let tx = self.conn.unchecked_transaction()?;
        for id in &missing {
            let preview = analyses.remove(id).map_or(RowPreview::None, |a| RowPreview::from_analysis(&a));
            queries::update_row_preview(&tx, *id, &preview.to_blob())?;
        }
        tx.commit()?;
        Ok(missing.len())
    }

    /// Stored row previews of the given tracks; tracks without one are left out.
    pub fn get_row_previews_of(&self, track_ids: &[i64]) -> Result<Vec<(i64, RowPreview)>> {
        Ok(queries::select_row_previews_of(&self.conn, track_ids)?
            .into_iter()
            .map(|(id, blob)| (id, RowPreview::from_blob(blob)))
            .collect())
    }

    /// Retrieve all tracks that have analysis data, with artwork included.
    /// Returns parallel vecs suitable for passing to the writer.
    pub fn get_analyzed_tracks(&self) -> Result<(Vec<Track>, Vec<AnalysisResult>)> {
        let track_ids = queries::select_complete_analysis_track_ids(&self.conn, analysis_stage::ALL)?;

        let mut tracks = Vec::new();
        let mut analyses = Vec::new();

        for id in track_ids {
            if let (Some(mut track), Some(analysis)) = (self.get_track(id)?, self.get_analysis(id)?) {
                // Load artwork for USB export.
                track.artwork = self.get_artwork(id)?;
                tracks.push(track);
                analyses.push(analysis);
            }
        }

        Ok((tracks, analyses))
    }

    /// Ids of the tracks with every analysis stage stored: the ones an export can write.
    pub fn complete_analysis_track_ids(&self) -> Result<Vec<i64>> {
        queries::select_complete_analysis_track_ids(&self.conn, analysis_stage::ALL)
    }

    /// Return IDs of tracks that have no analysis data yet or only some of its stages.
    pub fn get_unanalyzed_track_ids(&self) -> Result<Vec<i64>> {
        queries::select_unanalyzed_track_ids(&self.conn, analysis_stage::ALL)
    }

    /// Stage bits stored for a track, `None` when it has no analysis row.
    pub fn analysis_stages(&self, track_id: i64) -> Result<Option<u8>> {
        queries::select_analysis_stages(&self.conn, track_id)
    }

    /// Mark every stage of a track's analysis as missing, so a re-analysis runs the staged passes
    /// again and the track counts as pending until they are stored. Hand-set values stay locked.
    pub fn reset_analysis_stages(&self, track_id: i64) -> Result<()> {
        queries::reset_analysis_stages(&self.conn, track_id)?;
        Ok(())
    }

    /// Tracks that have an analysis row with stages still missing.
    pub fn get_incomplete_analysis_ids(&self) -> Result<Vec<i64>> {
        queries::select_incomplete_analysis_ids(&self.conn, analysis_stage::ALL)
    }

    /// Every track with what the track list needs of it (see [`TrackListRow`]), in one query. Row previews of tracks
    /// stored before that column existed are missing until [`backfill_row_previews`](Self::backfill_row_previews)
    /// has run.
    pub fn get_track_list_rows(&self) -> Result<Vec<TrackListRow>> {
        queries::select_track_list_rows(&self.conn, true)
    }

    /// [`get_track_list_rows`](Self::get_track_list_rows) without the row previews, which the list fetches for the
    /// rows on screen.
    pub fn get_track_list_rows_without_previews(&self) -> Result<Vec<TrackListRow>> {
        queries::select_track_list_rows(&self.conn, false)
    }

    /// The rows of [`get_track_list_rows_without_previews`](Self::get_track_list_rows_without_previews) of the tracks
    /// with these ids, handed over one by one without copying their texts. Returns how many of them have an analysis
    /// whose row preview is still to be filled by [`backfill_row_previews`](Self::backfill_row_previews).
    pub fn for_each_track_list_row(
        &self,
        ids: std::ops::RangeInclusive<i64>,
        visit: impl FnMut(TrackListRowRef<'_>),
    ) -> Result<usize> {
        queries::for_each_track_list_row(&self.conn, ids, false, visit)
    }

    /// The lowest and highest track id; `None` without tracks.
    pub fn track_id_bounds(&self) -> Result<Option<(i64, i64)>> {
        queries::track_id_bounds(&self.conn)
    }

    /// Reads track list rows on its own connection, so that several threads can scan id ranges at once; `None` for a
    /// library without a file.
    pub fn track_list_scanner(&self) -> Option<TrackListScanner> {
        self.db_path.clone().map(|db_path| TrackListScanner { db_path })
    }

    /// Retrieve all tracks with flags indicating artwork/analysis/cue presence.
    /// Useful for building UI list views without loading heavy data.
    /// Returns `(Track, has_artwork, has_analysis, has_cues)` tuples.
    ///
    /// Note: `has_cues` is always `false` since cue data now lives in ANLZ files.
    /// Callers that need cue info should read the full analysis via `get_analysis`.
    pub fn get_all_tracks_with_flags(&self) -> Result<Vec<(Track, bool, bool, bool)>> {
        queries::select_all_tracks_with_flags(&self.conn)
    }

    // -----------------------------------------------------------------------
    // Playlists
    // -----------------------------------------------------------------------

    /// Create a new playlist. Returns the library-assigned playlist ID.
    pub fn create_playlist(&self, name: &str) -> Result<i64> {
        queries::insert_playlist(&self.conn, name)
    }

    /// Create a folder under `parent_id` (0 is the root). Returns its ID.
    pub fn create_folder(&self, name: &str, parent_id: u32) -> Result<i64> {
        queries::insert_playlist_row(&self.conn, name, parent_id, true)
    }

    /// Create a playlist inside the folder `parent_id` (0 is the root). Returns its ID.
    pub fn create_playlist_in(&self, name: &str, parent_id: u32) -> Result<i64> {
        queries::insert_playlist_row(&self.conn, name, parent_id, false)
    }

    /// Rename a playlist.
    pub fn rename_playlist(&self, id: i64, name: &str) -> Result<()> {
        queries::rename_playlist(&self.conn, id, name)
    }

    /// Delete a playlist (track membership removed via CASCADE).
    pub fn delete_playlist(&self, id: i64) -> Result<()> {
        queries::delete_playlist(&self.conn, id)
    }

    /// Set the tracks in a playlist (replaces existing membership).
    /// `track_ids` are library track IDs, in desired order.
    pub fn set_playlist_tracks(&self, playlist_id: i64, track_ids: &[i64]) -> Result<()> {
        queries::set_playlist_tracks(&self.conn, playlist_id, track_ids)
    }

    /// Make the stored playlists exactly `playlists`, in that order, keeping their ids. Playlists
    /// that stay keep what refers to them (the sticks they are on).
    pub fn replace_playlists(&self, playlists: &[Playlist]) -> Result<()> {
        queries::replace_playlists(&self.conn, playlists)
    }

    /// Retrieve all playlists with their track IDs.
    pub fn get_all_playlists(&self) -> Result<Vec<Playlist>> {
        queries::select_all_playlists(&self.conn)
    }

    /// Retrieve a single playlist by ID.
    pub fn get_playlist(&self, id: i64) -> Result<Option<Playlist>> {
        queries::select_playlist(&self.conn, id)
    }

    // -----------------------------------------------------------------------
    // USB Export
    // -----------------------------------------------------------------------

    /// Write a fresh Pioneer USB from the library contents.
    ///
    /// Only analyzed tracks are included. Library IDs are remapped to
    /// sequential 1-based USB IDs.
    pub fn write_usb(&self, output_dir: &Path) -> Result<()> {
        let (tracks, analyses) = self.get_analyzed_tracks()?;
        let playlists = self.get_all_playlists()?;
        let (usb_tracks, usb_analyses, usb_playlists) =
            prepare_for_usb(tracks, analyses, playlists);
        writer::filesystem::write_usb(output_dir, &usb_tracks, &usb_analyses, &usb_playlists)
    }

    /// Incrementally sync the library to a USB drive.
    ///
    /// Only analyzed tracks are included. Returns a `SyncReport`.
    pub fn sync_usb(&self, output_dir: &Path) -> Result<SyncReport> {
        let (tracks, analyses) = self.get_analyzed_tracks()?;
        let playlists = self.get_all_playlists()?;
        let (usb_tracks, usb_analyses, usb_playlists) =
            prepare_for_usb(tracks, analyses, playlists);
        writer::filesystem::sync_usb(output_dir, &usb_tracks, &usb_analyses, &usb_playlists)
    }

    // -----------------------------------------------------------------------
    // USB Import
    // -----------------------------------------------------------------------

    /// Import tracks and playlists from an existing Pioneer USB into the library.
    ///
    /// Reads the OneLibrary database and artwork files. Tracks whose source path
    /// already exists in the library are skipped.
    ///
    /// Analysis data (beat grids, waveforms, cues) is NOT imported — imported
    /// tracks will need re-analysis.
    pub fn import_from_usb(&self, usb_dir: &Path) -> Result<ImportReport> {
        let existing = reader::read_usb_state(usb_dir)?;
        let Some(state) = existing else {
            return Ok(ImportReport {
                tracks_imported: 0,
                tracks_skipped: 0,
                playlists_imported: 0,
            });
        };

        let artwork_dir = usb_dir.join("PIONEER/Artwork/00001");

        let tx = self.conn.unchecked_transaction()?;
        let mut tracks_imported: u32 = 0;
        let mut tracks_skipped: u32 = 0;
        let mut usb_id_to_library_id: HashMap<u32, i64> = HashMap::new();

        for existing_track in &state.tracks {
            let audio_path = usb_dir.join(existing_track.usb_path.trim_start_matches('/'));

            // Skip if already in library
            if queries::track_exists_by_path(&tx, &audio_path)? {
                tracks_skipped += 1;
                continue;
            }

            let track = Track {
                id: 0,
                source_path: audio_path,
                usb_path: existing_track.usb_path.clone(),
                title: existing_track.title.clone(),
                artist: existing_track.artist.clone(),
                album: existing_track.album.clone(),
                genre: existing_track.genre.clone(),
                label: existing_track.label.clone(),
                remixer: existing_track.remixer.clone(),
                comment: existing_track.comment.clone(),
                year: existing_track.year,
                disc_number: existing_track.disc_number,
                track_number: existing_track.track_number,
                tempo: existing_track.tempo,
                key: existing_track.key.clone(),
                duration_secs: existing_track.duration_secs,
                sample_rate: existing_track.sample_rate,
                bitrate: existing_track.bitrate,
                file_size: existing_track.file_size,
                artwork: None,
            };

            let library_id = queries::insert_track(&tx, &track)?;
            usb_id_to_library_id.insert(existing_track.id, library_id);

            // Try to read artwork from USB
            if existing_track.has_artwork {
                let art_path = artwork_dir.join(format!("a{}_m.jpg", existing_track.id));
                if let Ok(art_data) = std::fs::read(&art_path) {
                    let _ = self.store_cover(&tx, library_id, &art_data);
                }
            }

            tracks_imported += 1;
        }

        // Import playlists, remapping USB track IDs to library IDs
        let mut playlists_imported: u32 = 0;
        for existing_pl in &state.playlists {
            let remapped_ids: Vec<i64> = existing_pl
                .track_ids
                .iter()
                .filter_map(|usb_id| usb_id_to_library_id.get(usb_id).copied())
                .collect();

            if remapped_ids.is_empty() {
                continue;
            }

            let sort_order: u32 = tx.query_row(
                "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM playlists",
                [],
                |row| row.get(0),
            )?;
            tx.execute(
                "INSERT INTO playlists (name, sort_order) VALUES (?1, ?2)",
                rusqlite::params![existing_pl.name, sort_order],
            )?;
            let pl_id = tx.last_insert_rowid();

            for (pos, track_id) in remapped_ids.iter().enumerate() {
                tx.execute(
                    "INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?1, ?2, ?3)",
                    rusqlite::params![pl_id, track_id, pos as i64],
                )?;
            }

            playlists_imported += 1;
        }

        tx.commit()?;
        self.forget_duplicates();

        Ok(ImportReport {
            tracks_imported,
            tracks_skipped,
            playlists_imported,
        })
    }
}

/// Remap sparse library IDs to dense 1-based USB IDs.
/// Copies the write-ahead log into the database file from a second connection. Passive: it
/// never waits for, or blocks, the library's own reads and writes.
/// Page cache for a bulk write, as SQLite's negative KiB: the default 2 MB spills the track
/// indexes of a 25k-file import to the log mid-transaction.
const BULK_WRITE_CACHE_KIB: i64 = -65_536;

fn checkpoint_in_background(db_path: PathBuf) {
    std::thread::spawn(move || {
        let flags = rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX;
        let checkpointed = Connection::open_with_flags(&db_path, flags)
            .and_then(|conn| conn.query_row("PRAGMA wal_checkpoint(PASSIVE)", [], |_| Ok(())));
        if let Err(e) = checkpointed {
            eprintln!("Warning: checkpoint of {} after adding tracks failed: {e}", db_path.display());
        }
    });
}

fn prepare_for_usb(
    mut tracks: Vec<Track>,
    analyses: Vec<AnalysisResult>,
    mut playlists: Vec<Playlist>,
) -> (Vec<Track>, Vec<AnalysisResult>, Vec<Playlist>) {
    let mut id_map: HashMap<u32, u32> = HashMap::with_capacity(tracks.len());

    for (i, track) in tracks.iter_mut().enumerate() {
        let usb_id = (i + 1) as u32;
        id_map.insert(track.id, usb_id);
        track.id = usb_id;
    }

    for playlist in &mut playlists {
        playlist.track_ids = playlist
            .track_ids
            .iter()
            .filter_map(|lib_id| id_map.get(lib_id).copied())
            .collect();
    }

    let playlist_id_map: HashMap<u32, u32> = playlists
        .iter()
        .enumerate()
        .map(|(i, playlist)| (playlist.id, (i + 1) as u32))
        .collect();
    for playlist in &mut playlists {
        playlist.parent_id = playlist_id_map.get(&playlist.parent_id).copied().unwrap_or(0);
        playlist.id = playlist_id_map[&playlist.id];
    }

    (tracks, analyses, playlists)
}

/// The playlists a stick selection puts on the drive: the chosen ones, everything inside
/// chosen folders, and the folders around them, in library order.
pub(crate) fn selected_playlist_tree(playlists: &[Playlist], chosen_ids: &[i64]) -> Vec<Playlist> {
    let by_id: HashMap<u32, &Playlist> = playlists.iter().map(|p| (p.id, p)).collect();
    let mut included: HashSet<u32> = HashSet::new();
    for playlist in playlists {
        let mut chain = vec![playlist.id];
        let mut ancestor = playlist.parent_id;
        while ancestor != 0 && chain.len() <= playlists.len() {
            chain.push(ancestor);
            ancestor = by_id.get(&ancestor).map_or(0, |p| p.parent_id);
        }
        if chain.iter().any(|id| chosen_ids.contains(&(*id as i64))) {
            included.extend(chain);
        }
    }
    playlists.iter().filter(|p| included.contains(&p.id)).cloned().collect()
}

/// PWAV preview of an ANLZ `.DAT` image: all-zero when the section is absent,
/// `None` when the file is not a PMAI image.
fn dat_pwav_preview(data: &[u8]) -> Option<[u8; 400]> {
    const PWAV_HEADER_LEN: usize = 20;
    let be_u32 = |at: usize| -> Option<usize> {
        let bytes: [u8; 4] = data.get(at..at + 4)?.try_into().ok()?;
        Some(u32::from_be_bytes(bytes) as usize)
    };
    if data.len() < 28 || &data[0..4] != b"PMAI" {
        return None;
    }
    let mut preview = [0u8; 400];
    let mut offset = be_u32(4)?;
    while offset + 12 <= data.len() {
        let section_len = be_u32(offset + 8)?;
        if section_len == 0 || offset + section_len > data.len() {
            break;
        }
        if &data[offset..offset + 4] == b"PWAV" && section_len >= PWAV_HEADER_LEN + 400 {
            let start = offset + PWAV_HEADER_LEN;
            preview.copy_from_slice(&data[start..start + 400]);
        }
        offset += section_len;
    }
    Some(preview)
}

/// Read the PWAV preview from a DAT file and the PWV6 overview from its sibling
/// `.2EX` (`detail` stays empty); fall back to the full ANLZ reader (which also
/// loads the color waveform) when the preview is flat.
fn read_waveform_analysis(dat_path: &Path) -> Option<AnalysisResult> {
    let bytes = std::fs::read(dat_path).ok()?;
    let waveform = dat_pwav_preview(&bytes)?;
    if waveform.iter().any(|&b| b & 0x1f != 0) {
        let color_waveform = reader::anlz::read_pwv6_overview(&dat_path.with_extension("2EX")).map(|overview| {
            ColorWaveform { detail: Vec::new(), overview, exact: None }
        });
        return Some(AnalysisResult {
            beat_grid: BeatGrid { beats: Vec::new() },
            waveform: WaveformPreview { data: waveform },
            bpm: 0.0,
            key: String::new(),
            energy: None,
            cue_points: Vec::new(),
            color_waveform,
        });
    }
    reader::anlz::read_anlz(dat_path).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pioneer_usb_writer::models::{AnalysisResult, Beat, BeatGrid, CuePoint, WaveformPreview};
    use std::path::PathBuf;

    pub(crate) fn make_track(title: &str, source: &str) -> Track {
        Track {
            id: 0,
            source_path: PathBuf::from(source),
            usb_path: format!("/Contents/Artist/{}", source.rsplit('/').next().unwrap_or(source)),
            title: title.to_string(),
            artist: "Test Artist".to_string(),
            album: "Test Album".to_string(),
            genre: "Electronic".to_string(),
            label: String::new(),
            remixer: String::new(),
            comment: String::new(),
            year: 2024,
            disc_number: 1,
            track_number: 1,
            tempo: 12800,
            key: "1A".to_string(),
            duration_secs: 300.0,
            sample_rate: 44100,
            bitrate: 320,
            file_size: 10_000_000,
            artwork: None,
        }
    }

    pub(crate) fn make_analysis() -> AnalysisResult {
        AnalysisResult {
            beat_grid: BeatGrid {
                beats: vec![
                    Beat { bar_position: 1, time_ms: 0, tempo: 12800 },
                    Beat { bar_position: 2, time_ms: 469, tempo: 12800 },
                    Beat { bar_position: 3, time_ms: 938, tempo: 12800 },
                    Beat { bar_position: 4, time_ms: 1406, tempo: 12800 },
                ],
            },
            waveform: WaveformPreview { data: {
                let mut d = [0u8; 400];
                for i in 0..400 { d[i] = (i % 32) as u8; }
                d
            }},
            bpm: 128.0,
            key: "1A".to_string(),
            energy: Some(7),
            cue_points: vec![
                CuePoint { hot_cue_number: 0, time_ms: 1000, loop_time_ms: None, ..Default::default() },
                CuePoint { hot_cue_number: 1, time_ms: 60_000, loop_time_ms: None, ..Default::default() },
                CuePoint { hot_cue_number: 2, time_ms: 90_000, loop_time_ms: Some(100_000), ..Default::default() },
            ],
            color_waveform: None,
        }
    }

    #[test]
    fn open_in_memory_creates_schema() {
        let lib = LocalLibrary::open_in_memory().unwrap();
        assert_eq!(lib.track_count().unwrap(), 0);
    }

    #[test]
    fn open_twice_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("library.db");
        let _lib1 = LocalLibrary::open(&db_path).unwrap();
        drop(_lib1);
        let lib2 = LocalLibrary::open(&db_path).unwrap();
        assert_eq!(lib2.track_count().unwrap(), 0);
    }

    #[test]
    fn covers_live_in_the_cover_store_and_the_database_keeps_only_their_hash() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("library.db");
        let lib = LocalLibrary::open(&db_path).unwrap();
        let a = lib.add_track(&make_track("A", "/music/a.mp3")).unwrap();
        let b = lib.add_track(&make_track("B", "/music/b.mp3")).unwrap();
        lib.set_artwork(a, &[1, 2, 3]).unwrap();
        lib.set_artwork(b, &[1, 2, 3]).unwrap();
        let columns: Vec<String> = lib
            .conn
            .prepare("SELECT name FROM pragma_table_info('artwork')")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(columns, ["track_id", "cover_hash"]);
        let hash = cover_store::sha256_hex(&[1, 2, 3]);
        let file = dir.path().join(COVERS_DIR).join(&hash[..2]).join(&hash);
        assert_eq!(std::fs::read(&file).unwrap(), [1, 2, 3]);
        assert!(lib.get_track_list_rows().unwrap().iter().all(|row| row.has_artwork));

        lib.set_artwork(a, &[4]).unwrap();
        assert!(file.is_file(), "still used by the other track");
        lib.remove_track(b).unwrap();
        assert!(!file.exists(), "no track uses it any more");
        lib.remove_artwork(a).unwrap();
        assert!(lib.covers.list().0.is_empty());
        assert_eq!(lib.get_artwork(a).unwrap(), None);
    }

    #[test]
    fn add_and_get_track() {
        let lib = LocalLibrary::open_in_memory().unwrap();
        let track = make_track("Track 1", "/music/track1.mp3");
        let id = lib.add_track(&track).unwrap();
        assert!(id > 0);

        let fetched = lib.get_track(id).unwrap().unwrap();
        assert_eq!(fetched.title, "Track 1");
        assert_eq!(fetched.artist, "Test Artist");
        assert_eq!(fetched.artwork, None);
    }

    #[test]
    fn add_tracks_batch() {
        let lib = LocalLibrary::open_in_memory().unwrap();
        let tracks = vec![
            make_track("A", "/music/a.mp3"),
            make_track("B", "/music/b.mp3"),
            make_track("C", "/music/c.mp3"),
        ];
        let ids = lib.add_tracks(&tracks).unwrap();
        assert_eq!(ids.len(), 3);
        assert_eq!(lib.track_count().unwrap(), 3);
    }

    #[test]
    fn add_tracks_on_disk_stores_covers_and_restores_the_sync_settings() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("library.db");
        let lib = LocalLibrary::open(&db_path).unwrap();
        let pragmas = |lib: &LocalLibrary| -> (i64, i64) {
            let get = |name: &str| lib.conn.query_row(&format!("PRAGMA {name}"), [], |row| row.get(0)).unwrap();
            (get("wal_autocheckpoint"), get("synchronous"))
        };
        let before = pragmas(&lib);
        let cover: Vec<u8> = (0..200_000u32).map(|i| (i % 251) as u8).collect();
        let tracks: Vec<Track> = (0..40)
            .map(|i| Track {
                artwork: (i % 2 == 0).then(|| cover.clone()),
                ..make_track(&format!("T{i}"), &format!("/music/{i}.mp3"))
            })
            .collect();
        let ids = lib.add_tracks(&tracks).unwrap();
        assert_eq!(pragmas(&lib), before);
        assert!(lib.add_tracks(&tracks[..1]).is_err());
        assert_eq!(pragmas(&lib), before);
        drop(lib);

        let reopened = LocalLibrary::open(&db_path).unwrap();
        assert_eq!(reopened.track_count().unwrap(), 40);
        for (i, id) in ids.iter().enumerate() {
            assert_eq!(reopened.get_artwork(*id).unwrap(), tracks[i].artwork);
        }
    }

    #[test]
    fn duplicate_candidates_are_the_library_tracks_in_id_order() {
        let lib = LocalLibrary::open_in_memory().unwrap();
        let tracks = vec![
            Track { artist: "Hale".into(), duration_secs: 380.5, file_size: 42, ..make_track("B", "/music/b.mp3") },
            make_track("A", "/music/a.mp3"),
        ];
        lib.add_tracks(&tracks).unwrap();
        let expected: Vec<DuplicateCandidate> = lib.get_all_tracks().unwrap().iter().map(DuplicateCandidate::from).collect();
        assert_eq!(lib.duplicate_candidates().unwrap(), expected);
        assert_eq!(expected.len(), 2);
    }

    #[test]
    fn the_duplicate_index_reads_the_stored_folded_keys_instead_of_folding_again() {
        let lib = LocalLibrary::open_in_memory().unwrap();
        let id = lib.add_track(&Track { artist: "Hålé".into(), ..make_track("Cöpper Lines (Original Mix)", "/music/a.mp3") }).unwrap();
        let stored: (String, String) =
            lib.conn.query_row("SELECT artist_key, title_key FROM tracks WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        assert_eq!(stored, ("hale".to_string(), "copper lines".to_string()));

        lib.conn.execute("UPDATE tracks SET artist_key = 'stored', title_key = 'key' WHERE id = ?1", [id]).unwrap();
        let probe = |artist: &str, title: &str| Track { artist: artist.into(), ..make_track(title, "/elsewhere/x.mp3") };
        let index = lib.duplicate_index().unwrap();
        assert_eq!(index.find(&probe("Stored", "Key")).map(|hit| hit.existing_id), Some(id));
        assert_eq!(index.find(&probe("Hale", "Copper Lines")), None);

        let update = Track { artist: "Other".into(), ..lib.get_track(id).unwrap().unwrap() };
        lib.update_track(id, &update).unwrap();
        let stored: String = lib.conn.query_row("SELECT artist_key FROM tracks WHERE id = ?1", [id], |r| r.get(0)).unwrap();
        assert_eq!(stored, "other", "an edit writes the key again");
    }

    #[test]
    fn the_duplicate_index_reads_the_stored_path_and_file_name_keys() {
        let lib = LocalLibrary::open_in_memory().unwrap();
        let id = lib.add_track(&Track { file_size: 500, ..make_track("A", "/Music/Ä/Song.MP3") }).unwrap();
        let stored: (String, String) =
            lib.conn.query_row("SELECT path_key, file_name_key FROM tracks WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        assert_eq!(stored, ("/music/\u{e4}/song.mp3".to_string(), "song.mp3".to_string()));

        lib.conn.execute("UPDATE tracks SET path_key = '/stored/path.mp3', file_name_key = 'stored.mp3' WHERE id = ?1", [id]).unwrap();
        let probe = |path: &str, size: u64| Track { artist: "Other".into(), file_size: size, ..make_track("Different", path) };
        let index = lib.duplicate_index().unwrap();
        assert_eq!(index.find(&probe("/Stored/Path.mp3", 1)).map(|hit| (hit.existing_id, hit.reason)), Some((id, DuplicateReason::SamePath)));
        assert_eq!(index.find(&probe("/elsewhere/STORED.mp3", 500)).map(|hit| hit.reason), Some(DuplicateReason::SameFile));
        assert_eq!(index.find(&probe("/Music/Ä/Song.MP3", 500)), None);

        let moved = Track { source_path: PathBuf::from("/new/Place.mp3"), ..lib.get_track(id).unwrap().unwrap() };
        lib.update_track(id, &moved).unwrap();
        let stored: (String, String) =
            lib.conn.query_row("SELECT path_key, file_name_key FROM tracks WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        assert_eq!(stored, ("/new/place.mp3".to_string(), "place.mp3".to_string()), "an edit writes the keys again");
    }

    #[test]
    fn relinking_a_track_writes_its_path_keys_and_moves_it_in_the_cached_index() {
        let dir = tempfile::tempdir().unwrap();
        let new_path = dir.path().join("New Home.wav");
        std::fs::write(&new_path, b"audio").unwrap();
        let lib = LocalLibrary::open_in_memory().unwrap();
        let id = lib.add_track(&Track { file_size: 5, ..make_track("A", "/gone/Old.wav") }).unwrap();
        assert!(lib.duplicate_index().unwrap().find(&make_track("Z", "/gone/old.wav")).is_some());
        lib.relink_track(id, &new_path).unwrap();
        let stored: (String, String) =
            lib.conn.query_row("SELECT path_key, file_name_key FROM tracks WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        assert_eq!(stored, (crate::path_key(&new_path), "new home.wav".to_string()));
        let index = lib.duplicate_index().unwrap();
        assert_eq!(index.find(&make_track("Z", "/gone/old.wav")), None);
        let hit = index.find(&Track { artist: "Other".into(), ..make_track("Z", &new_path.to_string_lossy()) });
        assert_eq!(hit.map(|h| (h.existing_id, h.reason)), Some((id, DuplicateReason::SamePath)));
        assert_eq!(lib.duplicate_candidate(id as u32).unwrap().unwrap().file_name_key, "new home.wav");
    }

    #[test]
    fn stored_keys_are_refilled_when_the_fold_version_changes() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("library.db");
        let lib = LocalLibrary::open(&db_path).unwrap();
        let id = lib.add_track(&Track { artist: "Ärtist".into(), ..make_track("Títle", "/music/a.mp3") }).unwrap();
        lib.conn
            .execute_batch("UPDATE tracks SET artist_key = 'stale', title_key = 'stale', path_key = 'stale', file_name_key = 'stale'; UPDATE library_meta SET value = '0' WHERE name = 'tag_fold';")
            .unwrap();
        drop(lib);
        let lib = LocalLibrary::open(&db_path).unwrap();
        let path_keys: (String, String) =
            lib.conn.query_row("SELECT path_key, file_name_key FROM tracks WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        assert_eq!(path_keys, ("/music/a.mp3".to_string(), "a.mp3".to_string()));
        let keys: (String, String, String) = lib
            .conn
            .query_row(
                "SELECT artist_key, title_key, (SELECT value FROM library_meta WHERE name = 'tag_fold') FROM tracks WHERE id = ?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(keys, ("artist".to_string(), "title".to_string(), TAG_FOLD_VERSION.to_string()));
    }

    #[test]
    fn analysis_failures_are_stored_replaced_cleared_and_go_with_their_track() {
        let lib = LocalLibrary::open_in_memory().unwrap();
        let id = lib.add_track(&make_track("A", "/music/a.mp3")).unwrap();
        let failure = AnalysisFailure {
            track_id: id,
            source_path: PathBuf::from("/music/a.mp3"),
            file_size: Some(10),
            modified_ms: None,
            kind: "decode_failed".into(),
            attempts: 1,
            analyzer_version: "analysis-1".into(),
            failed_at_ms: 5,
        };
        lib.record_analysis_failure(&failure).unwrap();
        lib.record_analysis_failure(&AnalysisFailure { attempts: 2, ..failure.clone() }).unwrap();
        assert_eq!(lib.analysis_failure(id).unwrap().map(|f| f.attempts), Some(2));
        lib.record_analysis_failure(&AnalysisFailure { track_id: 999, ..failure.clone() }).unwrap();
        assert_eq!(lib.analysis_failures().unwrap().len(), 1, "no row for a track that is not there");
        assert_eq!(lib.clear_analysis_failures(&[id, 999]).unwrap(), 1);
        assert_eq!(lib.analysis_failure(id).unwrap(), None);
        lib.record_analysis_failure(&failure).unwrap();
        lib.remove_track(id).unwrap();
        assert!(lib.analysis_failures().unwrap().is_empty());
    }

    #[test]
    fn duplicate_source_path_errors() {
        let lib = LocalLibrary::open_in_memory().unwrap();
        let track = make_track("Track 1", "/music/track1.mp3");
        lib.add_track(&track).unwrap();
        assert!(lib.add_track(&track).is_err());
    }

    #[test]
    fn update_track_metadata() {
        let lib = LocalLibrary::open_in_memory().unwrap();
        let mut track = make_track("Old Title", "/music/track.mp3");
        let id = lib.add_track(&track).unwrap();

        track.title = "New Title".to_string();
        lib.update_track(id, &track).unwrap();

        let fetched = lib.get_track(id).unwrap().unwrap();
        assert_eq!(fetched.title, "New Title");
    }

    #[test]
    fn remove_tracks_cascades() {
        let lib = LocalLibrary::open_in_memory().unwrap();
        let track = make_track("Track 1", "/music/track1.mp3");
        let id = lib.add_track(&track).unwrap();

        lib.set_artwork(id, &[0xFF, 0xD8, 0xFF]).unwrap();
        lib.set_analysis(id, &make_analysis()).unwrap();

        let pl_id = lib.create_playlist("Test PL").unwrap();
        lib.set_playlist_tracks(pl_id, &[id]).unwrap();

        lib.remove_track(id).unwrap();
        assert_eq!(lib.track_count().unwrap(), 0);
        assert!(lib.get_artwork(id).unwrap().is_none());
        assert!(lib.get_analysis(id).unwrap().is_none());
        let pl = lib.get_playlist(pl_id).unwrap().unwrap();
        assert!(pl.track_ids.is_empty());
    }

    #[test]
    fn track_exists_by_path_works() {
        let lib = LocalLibrary::open_in_memory().unwrap();
        assert!(!lib.track_exists_by_path(Path::new("/music/track.mp3")).unwrap());
        lib.add_track(&make_track("T", "/music/track.mp3")).unwrap();
        assert!(lib.track_exists_by_path(Path::new("/music/track.mp3")).unwrap());
    }

    #[test]
    fn artwork_crud() {
        let lib = LocalLibrary::open_in_memory().unwrap();
        let id = lib.add_track(&make_track("T", "/music/t.mp3")).unwrap();

        assert!(lib.get_artwork(id).unwrap().is_none());
        lib.set_artwork(id, &[1, 2, 3, 4]).unwrap();
        assert_eq!(lib.get_artwork(id).unwrap().unwrap(), vec![1, 2, 3, 4]);
        lib.set_artwork(id, &[5, 6]).unwrap();
        assert_eq!(lib.get_artwork(id).unwrap().unwrap(), vec![5, 6]);
        lib.remove_artwork(id).unwrap();
        assert!(lib.get_artwork(id).unwrap().is_none());
    }

    /// In-memory: analysis round-trip stores bpm/key in the DB index.
    /// Without disk ANLZ files, get_analysis returns a minimal result.
    #[test]
    fn analysis_round_trip_in_memory() {
        let lib = LocalLibrary::open_in_memory().unwrap();
        let id = lib.add_track(&make_track("T", "/music/t.mp3")).unwrap();
        let analysis = make_analysis();

        lib.set_analysis(id, &analysis).unwrap();
        let fetched = lib.get_analysis(id).unwrap().unwrap();

        // bpm and key round-trip through the DB index.
        assert_eq!(fetched.bpm, analysis.bpm);
        assert_eq!(fetched.key, analysis.key);
        // In-memory: no ANLZ files, so beat grid/waveform/cues are empty defaults.
        assert!(fetched.beat_grid.beats.is_empty());
        assert_eq!(fetched.waveform.data, [0u8; 400]);
        assert!(fetched.cue_points.is_empty());
    }

    /// On-disk: full ANLZ round-trip through binary files.
    #[test]
    fn analysis_round_trip_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("library.db");
        let lib = LocalLibrary::open(&db_path).unwrap();

        let id = lib.add_track(&make_track("T", "/music/t.mp3")).unwrap();
        let analysis = make_analysis();

        lib.set_analysis(id, &analysis).unwrap();

        // Verify ANLZ files were created.
        let anlz_dir = dir.path().join("anlz").join(id.to_string());
        assert!(anlz_dir.join("ANLZ0000.DAT").exists());
        assert!(anlz_dir.join("ANLZ0000.EXT").exists());

        // Read back and verify full data.
        let fetched = lib.get_analysis(id).unwrap().unwrap();
        assert_eq!(fetched.bpm, analysis.bpm);
        assert_eq!(fetched.key, analysis.key);
        assert_eq!(fetched.beat_grid.beats.len(), analysis.beat_grid.beats.len());
        assert_eq!(fetched.beat_grid.beats[0].time_ms, 0);
        assert_eq!(fetched.beat_grid.beats[0].tempo, 12800);
        assert_eq!(fetched.waveform.data, analysis.waveform.data);
        assert_eq!(fetched.cue_points.len(), 3);
        // Cue order may differ after ANLZ round-trip (hot cues before memory cues).
        let loop_cue = fetched.cue_points.iter().find(|c| c.hot_cue_number == 2).unwrap();
        assert_eq!(loop_cue.loop_time_ms, Some(100_000));
        assert_eq!(loop_cue.time_ms, 90_000);
    }

    #[test]
    fn waveform_analyses_match_per_track_reads() {
        use pioneer_usb_writer::models::ColorWaveform;

        let dir = tempfile::tempdir().unwrap();
        let lib = LocalLibrary::open(&dir.path().join("library.db")).unwrap();

        let real = lib.add_track(&make_track("Real", "/music/real.mp3")).unwrap();
        lib.set_analysis(real, &make_analysis()).unwrap();

        let mut flat = make_analysis();
        flat.waveform = WaveformPreview { data: [0u8; 400] };
        flat.color_waveform = Some(ColorWaveform {
            detail: (0..600).map(|n| [(n % 100) as u8, 40, 20]).collect(),
            overview: (0..1200).map(|n| [(n % 100) as u8, 40, 20]).collect(),
            exact: None,
        });
        let flat_id = lib.add_track(&make_track("Flat", "/music/flat.mp3")).unwrap();
        lib.set_analysis(flat_id, &flat).unwrap();

        let unanalyzed = lib.add_track(&make_track("None", "/music/none.mp3")).unwrap();

        let batch = lib.get_waveform_analyses().unwrap();
        assert!(!batch.contains_key(&unanalyzed));
        for id in [real, flat_id] {
            let single = lib.get_analysis(id).unwrap().unwrap();
            let batched = &batch[&id];
            assert_eq!(batched.waveform.data, single.waveform.data);
            if single.waveform.data.iter().all(|&b| b & 0x1f == 0) {
                let (a, b) = (batched.color_waveform.as_ref(), single.color_waveform.as_ref());
                assert!(a.is_some());
                assert_eq!(a.unwrap().overview, b.unwrap().overview);
                assert_eq!(a.unwrap().detail, b.unwrap().detail);
            }
        }
    }

    pub(crate) fn banded_analysis() -> AnalysisResult {
        use pioneer_usb_writer::models::ColorWaveform;
        let mut a = make_analysis();
        a.color_waveform = Some(ColorWaveform {
            detail: (0..900).map(|n| [(n % 100) as u8, 40, (n % 60) as u8]).collect(),
            overview: (0..1200).map(|n| [(n % 100) as u8, 40, (n % 60) as u8]).collect(),
            exact: None,
        });
        a
    }

    #[test]
    fn a_hand_set_grid_keeps_its_place_between_milliseconds_until_released() {
        let dir = tempfile::tempdir().unwrap();
        let lib = LocalLibrary::open(&dir.path().join("library.db")).unwrap();
        let id = lib.add_track(&make_track("Exact", "/music/x.mp3")).unwrap();
        lib.set_analysis(id, &banded_analysis()).unwrap();
        let before = lib.get_analysis(id).unwrap().unwrap();
        lib.set_beat_grid(id, &shifted(&before, 3), Some(-0.37), None).unwrap();
        let edits = lib.user_edits(id);
        assert!(edits.grid_locked);
        assert_eq!(edits.grid_offset_ms, Some(-0.37));
        let stored = lib.get_analysis(id).unwrap().unwrap();
        assert_eq!(stored.beat_grid.beats[0].time_ms, before.beat_grid.beats[0].time_ms + 3);
        lib.set_beat_grid(id, &shifted(&before, 3), None, None).unwrap();
        assert_eq!(lib.user_edits(id).grid_offset_ms, None);
        lib.set_beat_grid(id, &shifted(&before, 3), Some(0.25), None).unwrap();
        lib.release_user_edits(id).unwrap();
        assert_eq!(lib.user_edits(id).grid_offset_ms, None);
        assert!(lib.set_beat_grid(id, &shifted(&before, 3), Some(0.8), None).is_err());
        assert!(lib.set_beat_grid(id, &shifted(&before, 3), Some(f64::NAN), None).is_err());
    }

    fn shifted(analysis: &AnalysisResult, delta_ms: u32) -> Vec<BeatPosition> {
        analysis
            .beat_grid
            .beats
            .iter()
            .map(|b| BeatPosition { time_ms: b.time_ms + delta_ms, bar_position: b.bar_position })
            .collect()
    }

    #[test]
    fn edited_beat_grid_is_stored_and_the_rest_of_the_analysis_is_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let lib = LocalLibrary::open(&dir.path().join("library.db")).unwrap();
        let id = lib.add_track(&make_track("Edit", "/music/e.mp3")).unwrap();
        lib.set_analysis(id, &banded_analysis()).unwrap();
        let before = lib.get_analysis(id).unwrap().unwrap();

        lib.set_beat_grid(id, &shifted(&before, 37), None, None).unwrap();

        let after = lib.get_analysis(id).unwrap().unwrap();
        let times: Vec<u32> = after.beat_grid.beats.iter().map(|b| b.time_ms).collect();
        assert_eq!(times, vec![37, 506, 975, 1443]);
        assert!(after.beat_grid.beats.iter().all(|b| b.tempo == 12800));
        let bars: Vec<u8> = after.beat_grid.beats.iter().map(|b| b.bar_position).collect();
        assert_eq!(bars, vec![1, 2, 3, 4]);
        assert_eq!(after.bpm, before.bpm);
        assert_eq!(after.key, before.key);
        assert_eq!(after.waveform.data, before.waveform.data);
        assert_eq!(after.cue_points.len(), before.cue_points.len());
        let (a, b) = (after.color_waveform.unwrap(), before.color_waveform.unwrap());
        assert_eq!(a.detail, b.detail);
        assert_eq!(a.overview, b.overview);
        assert_eq!(lib.analysis_stages(id).unwrap(), Some(analysis_stage::ALL));
    }

    #[test]
    fn a_one_millisecond_grid_nudge_is_stored_exactly_through_a_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("library.db");
        let (id, before) = {
            let lib = LocalLibrary::open(&db_path).unwrap();
            let id = lib.add_track(&make_track("Fine", "/music/f.mp3")).unwrap();
            lib.set_analysis(id, &banded_analysis()).unwrap();
            let before = lib.get_analysis(id).unwrap().unwrap();
            lib.set_beat_grid(id, &shifted(&before, 1), None, None).unwrap();
            (id, before)
        };
        let lib = LocalLibrary::open(&db_path).unwrap();
        let after = lib.get_analysis(id).unwrap().unwrap();
        let expected: Vec<u32> = before.beat_grid.beats.iter().map(|b| b.time_ms + 1).collect();
        let times: Vec<u32> = after.beat_grid.beats.iter().map(|b| b.time_ms).collect();
        assert_eq!(times, expected);
    }

    #[test]
    fn edited_beat_grid_keeps_pending_stages_and_survives_a_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("library.db");
        let id = {
            let lib = LocalLibrary::open(&db_path).unwrap();
            let id = lib.add_track(&make_track("Edit", "/music/e.mp3")).unwrap();
            lib.set_analysis_stages(id, &make_analysis(), analysis_stage::WAVEFORM | analysis_stage::KEY).unwrap();
            lib.set_beat_grid(id, &[BeatPosition { time_ms: 120, bar_position: 1 }, BeatPosition { time_ms: 589, bar_position: 2 }], None, None).unwrap();
            id
        };
        let lib = LocalLibrary::open(&db_path).unwrap();
        let stored = lib.get_analysis(id).unwrap().unwrap();
        assert_eq!(stored.beat_grid.beats.len(), 2);
        assert_eq!(stored.beat_grid.beats[0].time_ms, 120);
        assert_eq!(lib.analysis_stages(id).unwrap(), Some(analysis_stage::WAVEFORM | analysis_stage::KEY));
    }

    #[test]
    fn edited_beat_grid_replaces_a_locked_imported_grid_and_stays_through_reanalysis() {
        let dir = tempfile::tempdir().unwrap();
        let lib = LocalLibrary::open(&dir.path().join("library.db")).unwrap();
        let id = lib.add_track(&make_track("Locked", "/music/l.mp3")).unwrap();
        lib.set_analysis(id, &make_analysis()).unwrap();
        rekordbox::ImportedMeta {
            source: rekordbox::SourceKind::Xml,
            source_id: "1".into(),
            grid_locked: true,
            cues_locked: false,
            rating: None,
            color: None,
            cues: Vec::new(),
            grid: Vec::new(),
        }
        .write(&lib.track_anlz_dir(id).unwrap())
        .unwrap();

        let current = lib.get_analysis(id).unwrap().unwrap();
        lib.set_beat_grid(id, &shifted(&current, 50), None, None).unwrap();
        assert_eq!(lib.get_analysis(id).unwrap().unwrap().beat_grid.beats[0].time_ms, 50);

        let mut reanalysis = make_analysis();
        reanalysis.beat_grid.beats[0].time_ms = 9;
        lib.set_analysis(id, &reanalysis).unwrap();
        assert_eq!(lib.get_analysis(id).unwrap().unwrap().beat_grid.beats[0].time_ms, 50);
    }

    fn cue(slot: u32, time_ms: u32, name: &str, color: Option<&str>) -> ImportedCue {
        ImportedCue { hot_cue_number: slot, time_ms, loop_time_ms: None, color: color.map(String::from), name: name.into() }
    }

    fn analyzed_file_library(dir: &Path) -> (LocalLibrary, i64) {
        let lib = LocalLibrary::open(&dir.join("library.db")).unwrap();
        let id = lib.add_track(&make_track("Cues", "/music/c.mp3")).unwrap();
        lib.set_analysis(id, &make_analysis()).unwrap();
        (lib, id)
    }

    fn locks(lib: &LocalLibrary, id: i64) -> (bool, bool) {
        let edits = lib.user_edits(id);
        (edits.bpm_locked, edits.key_locked)
    }

    #[test]
    fn user_bpm_and_key_are_stored_and_survive_reanalysis() {
        let dir = tempfile::tempdir().unwrap();
        let (lib, id) = analyzed_file_library(dir.path());
        assert_eq!(locks(&lib, id), (false, false));

        lib.set_user_bpm(id, 127.5).unwrap();
        lib.set_user_key(id, "8A").unwrap();
        let stored = lib.get_analysis(id).unwrap().unwrap();
        assert_eq!((stored.bpm, stored.key.as_str()), (127.5, "8A"));
        assert_eq!(locks(&lib, id), (true, true));
        assert_eq!(stored.beat_grid.beats[0].tempo, 12800);

        let mut reanalysis = make_analysis();
        reanalysis.bpm = 90.0;
        reanalysis.key = "3B".into();
        lib.set_analysis(id, &reanalysis).unwrap();
        let after = lib.get_analysis(id).unwrap().unwrap();
        assert_eq!((after.bpm, after.key.as_str()), (127.5, "8A"));

        let reopened = LocalLibrary::open(&dir.path().join("library.db")).unwrap();
        assert_eq!(locks(&reopened, id), (true, true));
    }

    #[test]
    fn only_the_edited_field_is_locked_and_release_lets_reanalysis_replace_it() {
        let dir = tempfile::tempdir().unwrap();
        let (lib, id) = analyzed_file_library(dir.path());
        lib.set_user_key(id, "5B").unwrap();
        assert_eq!(locks(&lib, id), (false, true));

        let mut reanalysis = make_analysis();
        reanalysis.bpm = 90.0;
        reanalysis.key = "3B".into();
        lib.set_analysis(id, &reanalysis).unwrap();
        let kept = lib.get_analysis(id).unwrap().unwrap();
        assert_eq!((kept.bpm, kept.key.as_str()), (90.0, "5B"));

        lib.release_user_edits(id).unwrap();
        assert_eq!(locks(&lib, id), (false, false));
        lib.set_analysis(id, &reanalysis).unwrap();
        assert_eq!(lib.get_analysis(id).unwrap().unwrap().key, "3B");
    }

    #[test]
    fn releasing_the_bpm_leaves_the_key_locked_and_the_other_way_round() {
        let dir = tempfile::tempdir().unwrap();
        let (lib, id) = analyzed_file_library(dir.path());
        lib.set_user_key(id, "5B").unwrap();
        lib.rescale_beat_grid(id, 64.0).unwrap();
        assert_eq!(locks(&lib, id), (true, true));

        lib.release_user_bpm(id).unwrap();
        assert_eq!(locks(&lib, id), (false, true));
        let edits = lib.user_edits(id);
        assert!(!edits.grid_locked && edits.grid_anchor_ms.is_none());

        lib.rescale_beat_grid(id, 64.0).unwrap();
        lib.release_user_key(id).unwrap();
        assert_eq!(locks(&lib, id), (true, false));
    }

    #[test]
    fn track_list_rows_and_labels_match_the_per_track_reads() {
        let dir = tempfile::tempdir().unwrap();
        let lib = LocalLibrary::open(&dir.path().join("library.db")).unwrap();
        let plain = lib.add_track(&make_track("Plain", "/music/p.mp3")).unwrap();
        let rated = lib.add_track(&make_track("Rated", "/music/r.mp3")).unwrap();
        let imported = lib.add_track(&make_track("Imported", "/music/i.mp3")).unwrap();
        for id in [rated, imported] {
            lib.set_analysis(id, &make_analysis()).unwrap();
        }
        lib.write_analysis(imported, &make_analysis(), analysis_stage::WAVEFORM).unwrap();
        let meta = |rating, color: Option<&str>| rekordbox::ImportedMeta {
            source: rekordbox::SourceKind::Xml,
            source_id: "1".into(),
            grid_locked: false,
            cues_locked: false,
            rating,
            color: color.map(String::from),
            cues: Vec::new(),
            grid: Vec::new(),
        };
        meta(Some(2), None).write(&lib.track_anlz_dir(rated).unwrap()).unwrap();
        lib.set_user_rating(rated, 4).unwrap();
        meta(Some(3), Some("#ff0000")).write(&lib.track_anlz_dir(imported).unwrap()).unwrap();
        lib.set_user_rating(imported, 0).unwrap();

        let labels = lib.track_labels_reader().read_all();
        assert_eq!(labels.len(), 2);
        assert_eq!(labels[&rated], TrackLabels { rating: Some(4), color: None });
        assert_eq!(labels[&imported], TrackLabels { rating: None, color: Some("#ff0000".into()) });

        let rows = lib.get_track_list_rows().unwrap();
        let bare = lib.get_track_list_rows_without_previews().unwrap();
        assert!(bare.iter().all(|row| row.row_preview.is_none()));
        assert_eq!(
            rows.iter().map(|row| TrackListRow { row_preview: None, ..row.clone() }).collect::<Vec<_>>(),
            bare
        );
        let flags = lib.get_all_tracks_with_flags().unwrap();
        let previews = lib.get_row_previews().unwrap();
        assert_eq!(rows.len(), flags.len());
        for (row, (track, has_artwork, has_analysis, _)) in rows.iter().zip(&flags) {
            let id = i64::from(row.id);
            assert_eq!(row.id, track.id);
            assert_eq!(row.source_path, track.source_path.to_string_lossy());
            assert_eq!((&row.title, &row.artist, &row.album, &row.genre), (&track.title, &track.artist, &track.album, &track.genre));
            assert_eq!((row.tempo, &row.key, row.bitrate, row.file_size), (track.tempo, &track.key, track.bitrate, track.file_size));
            assert_eq!(row.duration_secs, track.duration_secs);
            assert_eq!(row.has_artwork, *has_artwork);
            assert_eq!(row.analysis_stages.is_some(), *has_analysis);
            assert_eq!(row.analysis_stages, lib.analysis_stages(id).unwrap());
            assert_eq!(row.row_preview.as_ref(), previews.get(&id));
        }
        assert_eq!(rows.iter().find(|r| i64::from(r.id) == plain).unwrap().analysis_stages, None);
        assert_eq!(lib.get_incomplete_analysis_ids().unwrap(), vec![imported]);

        let ids: Vec<i64> = rows.iter().map(|r| i64::from(r.id)).chain([999_999]).collect();
        let some: Vec<(i64, RowPreview)> = lib.get_row_previews_of(&ids).unwrap();
        assert!(!some.is_empty());
        assert!(some.iter().all(|(id, preview)| previews.get(id) == Some(preview)));
        assert_eq!(some.len(), previews.len());
        assert!(lib.get_row_previews_of(&[plain, 999_999]).unwrap().is_empty());
    }

    fn rated_library(ratings: &[u8]) -> (tempfile::TempDir, LocalLibrary, Vec<i64>) {
        let dir = tempfile::tempdir().unwrap();
        let lib = LocalLibrary::open(&dir.path().join("library.db")).unwrap();
        let ids: Vec<i64> = ratings
            .iter()
            .enumerate()
            .map(|(n, &rating)| {
                let id = lib.add_track(&make_track(&format!("T{n}"), &format!("/music/{n}.mp3"))).unwrap();
                lib.set_analysis(id, &make_analysis()).unwrap();
                if rating > 0 {
                    lib.set_user_rating(id, rating).unwrap();
                }
                id
            })
            .collect();
        (dir, lib, ids)
    }

    fn write_rating_outside_the_app(lib: &LocalLibrary, id: i64, rating: u8) {
        let dir = lib.track_anlz_dir(id).unwrap();
        let mut edits = UserEdits::read(&dir).unwrap_or_default();
        edits.rating = Some(rating);
        edits.write(&dir).unwrap();
    }

    fn rating(labels: &HashMap<i64, TrackLabels>, id: i64) -> Option<u8> {
        labels.get(&id).and_then(|l| l.rating)
    }

    #[test]
    fn labels_cache_serves_the_last_scan_and_refresh_reports_outside_changes() {
        let (dir, lib, ids) = rated_library(&[3, 0, 5]);
        let reader = lib.track_labels_reader();
        let scanned = reader.read_cached();
        assert_eq!(scanned, reader.read_all());
        assert!(dir.path().join(LABELS_CACHE_FILE).exists());

        write_rating_outside_the_app(&lib, ids[1], 2);
        write_rating_outside_the_app(&lib, ids[2], 0);
        assert_eq!(reader.read_cached(), scanned, "a sidecar change alone does not touch the anlz folder");

        let mut changes = reader.refresh();
        changes.sort_by_key(|(id, _)| *id);
        assert_eq!(changes, vec![(ids[1], Some(TrackLabels { rating: Some(2), color: None })), (ids[2], None)]);
        let cached = reader.read_cached();
        assert_eq!(cached, reader.read_all());
        assert_eq!((rating(&cached, ids[0]), rating(&cached, ids[1]), rating(&cached, ids[2])), (Some(3), Some(2), None));
        assert!(reader.refresh().is_empty());
    }

    #[test]
    fn labels_cache_follows_ratings_set_in_the_app() {
        let (_dir, lib, ids) = rated_library(&[3, 0]);
        lib.track_labels_reader().read_cached();
        lib.set_user_rating(ids[0], 0).unwrap();
        lib.set_user_rating(ids[1], 4).unwrap();
        let cached = lib.track_labels_reader().read_cached();
        assert_eq!((rating(&cached, ids[0]), rating(&cached, ids[1])), (None, Some(4)));
        assert!(lib.track_labels_reader().refresh().is_empty());
    }

    #[test]
    fn labels_cache_is_rescanned_when_track_folders_come_or_go() {
        let (_dir, lib, ids) = rated_library(&[3, 4]);
        let reader = lib.track_labels_reader();
        reader.read_cached();
        std::thread::sleep(std::time::Duration::from_millis(20));
        write_rating_outside_the_app(&lib, ids[0], 1);
        lib.remove_tracks(&[ids[1]]).unwrap();
        let cached = reader.read_cached();
        assert_eq!(cached, reader.read_all());
        assert_eq!((rating(&cached, ids[0]), rating(&cached, ids[1])), (Some(1), None));

        reader.invalidate();
        assert!(!lib.db_path.as_ref().unwrap().parent().unwrap().join(LABELS_CACHE_FILE).exists());
        assert_eq!(reader.read_cached(), cached);
    }

    #[test]
    fn user_rating_shows_over_the_imported_one_and_survives_reanalysis() {
        let dir = tempfile::tempdir().unwrap();
        let (lib, id) = analyzed_file_library(dir.path());
        assert_eq!(lib.effective_rating(id, None), None);
        assert_eq!(lib.effective_rating(id, Some(2)), Some(2));

        lib.set_user_rating(id, 4).unwrap();
        assert_eq!(lib.effective_rating(id, Some(2)), Some(4));
        lib.set_user_rating(id, 0).unwrap();
        assert_eq!(lib.effective_rating(id, Some(2)), None);
        lib.set_user_rating(id, 5).unwrap();

        lib.set_analysis(id, &make_analysis()).unwrap();
        lib.release_user_edits(id).unwrap();
        let reopened = LocalLibrary::open(&dir.path().join("library.db")).unwrap();
        assert_eq!(reopened.effective_rating(id, None), Some(5));
        assert!(lib.set_user_rating(id, 6).is_err());
    }

    #[test]
    fn user_bpm_and_key_reject_bad_values_and_unanalyzed_tracks() {
        let dir = tempfile::tempdir().unwrap();
        let (lib, id) = analyzed_file_library(dir.path());
        assert!(lib.set_user_bpm(id, 39.9).is_err());
        assert!(lib.set_user_bpm(id, 250.1).is_err());
        assert!(lib.set_user_bpm(id, f64::NAN).is_err());
        assert!(lib.set_user_key(id, "13A").is_err());
        assert!(lib.set_user_key(id, "0B").is_err());
        assert!(lib.set_user_key(id, "8C").is_err());
        assert!(lib.set_user_key(id, "Am").is_err());
        assert_eq!(locks(&lib, id), (false, false));

        let bare = lib.add_track(&make_track("Bare", "/music/bare.mp3")).unwrap();
        assert!(lib.set_user_bpm(bare, 128.0).is_err());
        assert!(lib.set_user_key(bare, "8A").is_err());
    }

    #[test]
    fn changing_the_bpm_leaves_the_beat_grid_alone_until_it_is_rescaled() {
        let dir = tempfile::tempdir().unwrap();
        let (lib, id) = analyzed_file_library(dir.path());
        let grid_times = |lib: &LocalLibrary| -> Vec<(u32, u32)> {
            lib.get_analysis(id).unwrap().unwrap().beat_grid.beats.iter().map(|b| (b.time_ms, b.tempo)).collect()
        };
        let before = grid_times(&lib);
        lib.set_user_bpm(id, 64.0).unwrap();
        assert_eq!(grid_times(&lib), before);

        lib.rescale_beat_grid(id, 64.0).unwrap();
        let scaled = lib.get_analysis(id).unwrap().unwrap();
        assert_eq!(scaled.bpm, 64.0);
        let times: Vec<u32> = scaled.beat_grid.beats.iter().map(|b| b.time_ms).collect();
        assert_eq!(&times[..4], &[0, 938, 1875, 2813]);
        assert_eq!(times.len(), 321);
        assert!(scaled.beat_grid.beats.iter().all(|b| b.tempo == 6400));
        assert_eq!(scaled.cue_points.len(), 3);
        assert!(lib.user_edits(id).bpm_locked);
    }

    #[test]
    fn rescaling_keeps_the_downbeat_and_its_sub_millisecond_place_and_survives_reanalysis_and_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let (lib, id) = analyzed_file_library(dir.path());
        let before = lib.get_analysis(id).unwrap().unwrap();
        lib.set_beat_grid(id, &shifted(&before, 1000), Some(-0.37), None).unwrap();

        lib.rescale_beat_grid(id, 100.0).unwrap();
        let reopened = LocalLibrary::open(&dir.path().join("library.db")).unwrap();
        let scaled = reopened.get_analysis(id).unwrap().unwrap();
        assert_eq!(reopened.user_edits(id).grid_offset_ms, Some(-0.37));
        let anchor = scaled.beat_grid.beats.iter().find(|b| b.time_ms == 1000).unwrap();
        assert_eq!(anchor.bar_position, 1);
        assert_eq!(scaled.beat_grid.beats.first().unwrap().time_ms, 1000 % 600);
        assert_eq!(scaled.beat_grid.beats.first().unwrap().bar_position, 1 + ((1000 / 600) as u8 * 3) % 4);
        assert_eq!(scaled.bpm, 100.0);

        reopened.set_analysis(id, &make_analysis()).unwrap();
        let kept = reopened.get_analysis(id).unwrap().unwrap();
        assert_eq!(kept.bpm, 100.0);
        assert_eq!(kept.beat_grid.beats.iter().find(|b| b.time_ms == 1000).unwrap().tempo, 10000);
    }

    #[test]
    fn halving_and_doubling_the_bpm_space_the_beats_twice_as_far_or_close() {
        let dir = tempfile::tempdir().unwrap();
        let (lib, id) = analyzed_file_library(dir.path());
        lib.rescale_beat_grid(id, 120.0).unwrap();
        let spacing = |lib: &LocalLibrary| {
            let beats = lib.get_analysis(id).unwrap().unwrap().beat_grid.beats;
            beats[1].time_ms - beats[0].time_ms
        };
        assert_eq!(spacing(&lib), 500);
        lib.rescale_beat_grid(id, 60.0).unwrap();
        assert_eq!(spacing(&lib), 1000);
        lib.rescale_beat_grid(id, 120.0).unwrap();
        assert_eq!(spacing(&lib), 500);
    }

    fn stored_grid(lib: &LocalLibrary, id: i64) -> Vec<(u32, u8)> {
        lib.get_analysis(id).unwrap().unwrap().beat_grid.beats.iter().map(|b| (b.time_ms, b.bar_position)).collect()
    }

    #[test]
    fn doubling_then_halving_the_bpm_across_saves_returns_the_exact_grid() {
        let dir = tempfile::tempdir().unwrap();
        let (lib, id) = analyzed_file_library(dir.path());
        let before = lib.get_analysis(id).unwrap().unwrap();
        lib.set_beat_grid(id, &shifted(&before, 1000), None, None).unwrap();
        lib.rescale_beat_grid(id, 125.0).unwrap();
        let original = stored_grid(&lib, id);
        assert!(original.contains(&(1000, 1)));

        lib.rescale_beat_grid(id, 250.0).unwrap();
        assert!(stored_grid(&lib, id).contains(&(1000, 1)));
        let reopened = LocalLibrary::open(&dir.path().join("library.db")).unwrap();
        reopened.rescale_beat_grid(id, 125.0).unwrap();

        assert_eq!(stored_grid(&reopened, id), original);
        assert_eq!(reopened.user_edits(id).grid_anchor_ms, Some(1000.0));
    }

    #[test]
    fn a_downbeat_set_mid_track_stays_fixed_under_rescale_and_release_forgets_it() {
        let dir = tempfile::tempdir().unwrap();
        let (lib, id) = analyzed_file_library(dir.path());
        let before = lib.get_analysis(id).unwrap().unwrap();
        lib.set_beat_grid(id, &shifted(&before, 1000), Some(0.25), Some(60_000.25)).unwrap();

        for bpm in [128.0, 90.0, 174.0, 70.0] {
            lib.rescale_beat_grid(id, bpm).unwrap();
            let grid = lib.get_analysis(id).unwrap().unwrap().beat_grid.beats;
            let at_anchor = grid.iter().find(|b| b.time_ms == 60_000).unwrap();
            assert_eq!(at_anchor.bar_position, 1, "{bpm}");
            assert_eq!(lib.user_edits(id).grid_anchor_ms, Some(60_000.25));
        }

        lib.release_user_edits(id).unwrap();
        assert_eq!(lib.user_edits(id).grid_anchor_ms, None);
    }

    #[test]
    fn rescale_matches_the_app_for_the_shared_cases() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!("../tests/fixtures/rescale-parity.json")).unwrap();
        let duration_ms = fixture["durationSecs"].as_f64().unwrap() * 1000.0;
        for case in fixture["cases"].as_array().unwrap() {
            let dir = tempfile::tempdir().unwrap();
            let (lib, id) = analyzed_file_library(dir.path());
            assert_eq!(lib.get_track(id).unwrap().unwrap().duration_secs * 1000.0, duration_ms);
            let beats: Vec<BeatPosition> = case["beats"]
                .as_array()
                .unwrap()
                .iter()
                .map(|b| BeatPosition { time_ms: b["time_ms"].as_u64().unwrap() as u32, bar_position: b["bar_position"].as_u64().unwrap() as u8 })
                .collect();
            let offset = case["offsetMs"].as_f64().unwrap();
            lib.set_beat_grid(id, &beats, Some(offset), case["anchorMs"].as_f64()).unwrap();
            let bpm = case["bpm"].as_f64().unwrap();
            lib.rescale_beat_grid(id, bpm).unwrap();

            let grid = lib.get_analysis(id).unwrap().unwrap().beat_grid.beats;
            let label = format!("{bpm} BPM, anchor {:?}", case["anchorMs"]);
            assert_eq!(grid.len(), case["count"].as_u64().unwrap() as usize, "{label}");
            let expected = |key: &str| -> Vec<(f64, u8)> {
                case[key].as_array().unwrap().iter().map(|b| (b[0].as_f64().unwrap(), b[1].as_u64().unwrap() as u8)).collect()
            };
            let actual = |range: std::ops::Range<usize>| -> Vec<(f64, u8)> {
                grid[range].iter().map(|b| (b.time_ms as f64 + offset, b.bar_position)).collect()
            };
            for (key, got) in [("head", actual(0..8)), ("tail", actual(grid.len() - 8..grid.len()))] {
                for (want, have) in expected(key).iter().zip(&got) {
                    assert!((want.0 - have.0).abs() < 1e-9 && want.1 == have.1, "{label} {key}: {want:?} vs {have:?}");
                }
            }
        }
    }

    #[test]
    fn edited_cues_keep_name_colour_and_loop_and_survive_a_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let (lib, id) = analyzed_file_library(dir.path());
        let mut looped = cue(3, 40_000, "Drop", Some("#ff00aa"));
        looped.loop_time_ms = Some(48_000);
        lib.set_cues(id, &[cue(0, 2_000, "", None), looped.clone(), cue(1, 10_000, "Intro", Some("#00ff00"))]).unwrap();
        drop(lib);

        let lib = LocalLibrary::open(&dir.path().join("library.db")).unwrap();
        let cues = lib.get_cues(id).unwrap();
        let slots: Vec<u32> = cues.iter().map(|c| c.hot_cue_number).collect();
        assert_eq!(slots, vec![1, 3, 0]);
        assert_eq!(cues[0].name, "Intro");
        assert_eq!(cues[1], looped);
        let stored = lib.get_analysis(id).unwrap().unwrap().cue_points;
        assert_eq!(stored.len(), 3);
        let stored_loop = stored.iter().find(|c| c.hot_cue_number == 3).unwrap();
        assert_eq!((stored_loop.time_ms, stored_loop.loop_time_ms), (40_000, Some(48_000)));
        assert_eq!(stored_loop.name, "Drop");
        assert!(stored_loop.color.as_deref().is_some_and(|c| c.starts_with("table:")));
        let stored_intro = stored.iter().find(|c| c.hot_cue_number == 1).unwrap();
        assert_eq!(stored_intro.name, "Intro");

        lib.set_cues(id, &[cue(1, 10_000, "Intro", None)]).unwrap();
        assert_eq!(lib.get_cues(id).unwrap().len(), 1);
        lib.set_cues(id, &[]).unwrap();
        assert!(lib.get_cues(id).unwrap().is_empty());
        assert!(lib.get_analysis(id).unwrap().unwrap().cue_points.is_empty());
    }

    #[test]
    fn cues_of_a_track_nobody_edited_come_from_the_analysis_without_names() {
        let dir = tempfile::tempdir().unwrap();
        let (lib, id) = analyzed_file_library(dir.path());
        let cues = lib.get_cues(id).unwrap();
        assert_eq!(cues.len(), 3);
        assert!(cues.iter().all(|c| c.name.is_empty() && c.color.is_none()));
    }

    #[test]
    fn invalid_cue_lists_are_rejected_and_change_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let (lib, id) = analyzed_file_library(dir.path());
        assert!(lib.set_cues(id, &[cue(9, 1, "", None)]).is_err());
        assert!(lib.set_cues(id, &[cue(2, 1, "", None), cue(2, 5, "", None)]).is_err());
        assert!(lib.set_cues(id, &[cue(0, 7, "", None), cue(0, 7, "", None)]).is_err());
        let mut backwards = cue(1, 5_000, "", None);
        backwards.loop_time_ms = Some(4_000);
        assert!(lib.set_cues(id, &[backwards]).is_err());
        assert_eq!(lib.get_cues(id).unwrap().len(), 3);
    }

    #[test]
    fn edited_cues_and_grid_survive_reanalysis_until_released() {
        let dir = tempfile::tempdir().unwrap();
        let (lib, id) = analyzed_file_library(dir.path());
        let current = lib.get_analysis(id).unwrap().unwrap();
        lib.set_beat_grid(id, &shifted(&current, 50), None, None).unwrap();
        lib.set_cues(id, &[cue(2, 33_000, "Mine", Some("#123456"))]).unwrap();

        let mut reanalysis = make_analysis();
        reanalysis.beat_grid.beats[0].time_ms = 9;
        reanalysis.cue_points = vec![CuePoint { hot_cue_number: 1, time_ms: 5, loop_time_ms: None, ..Default::default() }];
        lib.set_analysis(id, &reanalysis).unwrap();
        let kept = lib.get_analysis(id).unwrap().unwrap();
        assert_eq!(kept.beat_grid.beats[0].time_ms, 50);
        assert_eq!(kept.cue_points.len(), 1);
        assert_eq!(kept.cue_points[0].time_ms, 33_000);
        assert_eq!(lib.get_cues(id).unwrap()[0].name, "Mine");

        lib.release_user_edits(id).unwrap();
        lib.set_analysis(id, &reanalysis).unwrap();
        let replaced = lib.get_analysis(id).unwrap().unwrap();
        assert_eq!(replaced.beat_grid.beats[0].time_ms, 9);
        assert_eq!(replaced.cue_points[0].time_ms, 5);
    }

    #[test]
    fn a_track_nobody_edited_is_replaced_by_reanalysis() {
        let dir = tempfile::tempdir().unwrap();
        let (lib, id) = analyzed_file_library(dir.path());
        let mut reanalysis = make_analysis();
        reanalysis.beat_grid.beats[0].time_ms = 9;
        reanalysis.cue_points.clear();
        lib.set_analysis(id, &reanalysis).unwrap();
        let replaced = lib.get_analysis(id).unwrap().unwrap();
        assert_eq!(replaced.beat_grid.beats[0].time_ms, 9);
        assert!(replaced.cue_points.is_empty());
    }

    #[test]
    fn imported_cues_show_their_names_until_the_user_edits_them() {
        let dir = tempfile::tempdir().unwrap();
        let (lib, id) = analyzed_file_library(dir.path());
        rekordbox::ImportedMeta {
            source: rekordbox::SourceKind::Xml,
            source_id: "1".into(),
            grid_locked: false,
            cues_locked: true,
            rating: None,
            color: None,
            cues: vec![cue(1, 1_000, "Vocal in", Some("table:3")), cue(0, 2_000, "Break", None)],
            grid: Vec::new(),
        }
        .write(&lib.track_anlz_dir(id).unwrap())
        .unwrap();

        let imported = lib.get_cues(id).unwrap();
        assert_eq!((imported[0].name.as_str(), imported[0].color.as_deref()), ("Vocal in", Some("table:3")));

        let mut renamed = imported.clone();
        renamed[0].name = "Vocal".into();
        renamed[0].color = Some("#ffffff".into());
        lib.set_cues(id, &renamed).unwrap();
        lib.set_analysis(id, &make_analysis()).unwrap();
        let after = lib.get_cues(id).unwrap();
        assert_eq!((after[0].name.as_str(), after[0].color.as_deref()), ("Vocal", Some("#ffffff")));
        assert_eq!(after[1].name, "Break");
        assert_eq!(lib.get_analysis(id).unwrap().unwrap().cue_points.len(), 2);
    }

    #[test]
    fn cues_need_a_library_with_files() {
        let lib = LocalLibrary::open_in_memory().unwrap();
        let id = lib.add_track(&make_track("Mem", "/music/m.mp3")).unwrap();
        assert!(lib.set_cues(id, &[cue(1, 1, "", None)]).is_err());
    }

    #[test]
    fn beat_grid_of_an_unanalyzed_track_is_an_error() {
        let lib = LocalLibrary::open_in_memory().unwrap();
        let id = lib.add_track(&make_track("None", "/music/n.mp3")).unwrap();
        assert!(lib.set_beat_grid(id, &[BeatPosition { time_ms: 0, bar_position: 1 }], None, None).is_err());
    }

    #[test]
    fn row_preview_is_stored_with_the_analysis_and_matches_the_files() {
        let dir = tempfile::tempdir().unwrap();
        let lib = LocalLibrary::open(&dir.path().join("library.db")).unwrap();
        let banded = lib.add_track(&make_track("Banded", "/music/b.mp3")).unwrap();
        lib.set_analysis(banded, &banded_analysis()).unwrap();
        let plain = lib.add_track(&make_track("Plain", "/music/p.mp3")).unwrap();
        lib.set_analysis(plain, &make_analysis()).unwrap();
        lib.add_track(&make_track("None", "/music/n.mp3")).unwrap();

        assert_eq!(lib.backfill_row_previews().unwrap(), 0);
        let previews = lib.get_row_previews().unwrap();
        assert_eq!(previews.len(), 2);
        let from_files = lib.get_waveform_analyses().unwrap();
        for id in [banded, plain] {
            assert_eq!(previews[&id], RowPreview::from_analysis(&from_files[&id]));
        }
        assert!(matches!(previews[&banded], RowPreview::Bands(_)));
        assert!(matches!(previews[&plain], RowPreview::Mono(_)));
    }

    #[test]
    fn v3_library_migrates_and_backfills_row_previews() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("library.db");
        let (banded, plain) = {
            let lib = LocalLibrary::open(&db_path).unwrap();
            let banded = lib.add_track(&make_track("Banded", "/music/b.mp3")).unwrap();
            lib.set_analysis(banded, &banded_analysis()).unwrap();
            let plain = lib.add_track(&make_track("Plain", "/music/p.mp3")).unwrap();
            lib.set_analysis(plain, &make_analysis()).unwrap();
            (banded, plain)
        };
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.execute_batch("ALTER TABLE analyses DROP COLUMN row_preview; UPDATE schema_version SET version = 3;")
                .unwrap();
        }

        let lib = LocalLibrary::open(&db_path).unwrap();
        assert!(lib.get_row_previews().unwrap().is_empty());
        assert_eq!(lib.backfill_row_previews().unwrap(), 2);
        let previews = lib.get_row_previews().unwrap();
        assert!(matches!(previews[&banded], RowPreview::Bands(_)));
        assert!(matches!(previews[&plain], RowPreview::Mono(_)));
        assert_eq!(lib.backfill_row_previews().unwrap(), 0);
        assert_eq!(lib.get_analysis(banded).unwrap().unwrap().bpm, make_analysis().bpm);
    }

    /// Verify ANLZ files are cleaned up when a track is removed.
    #[test]
    fn remove_track_cleans_up_anlz() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("library.db");
        let lib = LocalLibrary::open(&db_path).unwrap();

        let id = lib.add_track(&make_track("T", "/music/t.mp3")).unwrap();
        lib.set_analysis(id, &make_analysis()).unwrap();

        let anlz_dir = dir.path().join("anlz").join(id.to_string());
        assert!(anlz_dir.exists());

        lib.remove_track(id).unwrap();
        assert!(!anlz_dir.exists());
    }

    #[test]
    fn get_analyzed_tracks_returns_only_analyzed() {
        let lib = LocalLibrary::open_in_memory().unwrap();
        let id1 = lib.add_track(&make_track("Analyzed", "/music/a.mp3")).unwrap();
        let _id2 = lib.add_track(&make_track("Not Analyzed", "/music/b.mp3")).unwrap();

        lib.set_analysis(id1, &make_analysis()).unwrap();

        let (tracks, analyses) = lib.get_analyzed_tracks().unwrap();
        assert_eq!(tracks.len(), 1);
        assert_eq!(analyses.len(), 1);
        assert_eq!(tracks[0].title, "Analyzed");
    }

    #[test]
    fn partial_analysis_stays_pending_and_out_of_export() {
        let lib = LocalLibrary::open_in_memory().unwrap();
        let id = lib.add_track(&make_track("A", "/music/a.mp3")).unwrap();
        let done = lib.add_track(&make_track("B", "/music/b.mp3")).unwrap();
        lib.set_analysis(done, &make_analysis()).unwrap();

        lib.set_analysis_stages(id, &make_analysis(), analysis_stage::WAVEFORM | analysis_stage::KEY).unwrap();

        assert_eq!(lib.analysis_stages(id).unwrap(), Some(3));
        assert_eq!(lib.get_unanalyzed_track_ids().unwrap(), vec![id]);
        assert_eq!(lib.get_incomplete_analysis_ids().unwrap(), vec![id]);
        assert_eq!(lib.get_analyzed_tracks().unwrap().0.len(), 1);

        lib.set_analysis(id, &make_analysis()).unwrap();
        assert_eq!(lib.analysis_stages(id).unwrap(), Some(analysis_stage::ALL));
        assert!(lib.get_unanalyzed_track_ids().unwrap().is_empty());
        assert_eq!(lib.get_analyzed_tracks().unwrap().0.len(), 2);
    }

    #[test]
    fn v4_library_migrates_with_every_analysis_complete() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("library.db");
        let id = {
            let lib = LocalLibrary::open(&db_path).unwrap();
            let id = lib.add_track(&make_track("A", "/music/a.mp3")).unwrap();
            lib.set_analysis(id, &make_analysis()).unwrap();
            id
        };
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.execute_batch("ALTER TABLE analyses DROP COLUMN stages; UPDATE schema_version SET version = 4;").unwrap();
        }
        let lib = LocalLibrary::open(&db_path).unwrap();
        assert_eq!(lib.analysis_stages(id).unwrap(), Some(analysis_stage::ALL));
    }

    #[test]
    fn v5_library_migrates_to_stick_tables_and_keeps_its_data() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("library.db");
        let (track, playlist) = {
            let lib = LocalLibrary::open(&db_path).unwrap();
            let track = lib.add_track(&make_track("A", "/music/a.mp3")).unwrap();
            lib.set_analysis(track, &make_analysis()).unwrap();
            let playlist = lib.create_playlist("Warmup").unwrap();
            lib.set_playlist_tracks(playlist, &[track]).unwrap();
            (track, playlist)
        };
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.execute_batch(
                "DROP TABLE stick_playlists; DROP TABLE stick_tracks; DROP TABLE sticks;
                 UPDATE schema_version SET version = 5;",
            )
            .unwrap();
        }
        let lib = LocalLibrary::open(&db_path).unwrap();
        let version: u32 = lib
            .conn
            .query_row("SELECT version FROM schema_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, schema::SCHEMA_VERSION);
        assert_eq!(lib.get_playlist(playlist).unwrap().unwrap().track_ids, vec![track as u32]);
        assert_eq!(lib.analysis_stages(track).unwrap(), Some(analysis_stage::ALL));
        assert_eq!(lib.add_stick_tracks("stick-1", &[track]).unwrap(), 1);
        assert_eq!(lib.add_stick_playlists("stick-1", &[playlist]).unwrap(), 1);
        drop(lib);
        let reopened = LocalLibrary::open(&db_path).unwrap();
        let contents = reopened.stick_contents("stick-1").unwrap();
        assert_eq!(contents.track_ids, vec![track]);
        assert_eq!(contents.playlist_ids, vec![playlist]);
    }

    #[test]
    fn get_unanalyzed_track_ids() {
        let lib = LocalLibrary::open_in_memory().unwrap();
        let id1 = lib.add_track(&make_track("A", "/music/a.mp3")).unwrap();
        let id2 = lib.add_track(&make_track("B", "/music/b.mp3")).unwrap();

        lib.set_analysis(id1, &make_analysis()).unwrap();

        let unanalyzed = lib.get_unanalyzed_track_ids().unwrap();
        assert_eq!(unanalyzed, vec![id2]);
    }

    #[test]
    fn v6_library_migrates_to_playlist_folders_and_keeps_its_playlists() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("library.db");
        let (track, playlist) = {
            let lib = LocalLibrary::open(&db_path).unwrap();
            let track = lib.add_track(&make_track("A", "/music/a.mp3")).unwrap();
            let playlist = lib.create_playlist("Warmup").unwrap();
            lib.set_playlist_tracks(playlist, &[track]).unwrap();
            (track, playlist)
        };
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.execute_batch(
                "ALTER TABLE playlists DROP COLUMN parent_id;
                 ALTER TABLE playlists DROP COLUMN is_folder;
                 UPDATE schema_version SET version = 6;",
            )
            .unwrap();
        }
        let lib = LocalLibrary::open(&db_path).unwrap();
        let version: u32 = lib
            .conn
            .query_row("SELECT version FROM schema_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, schema::SCHEMA_VERSION);
        let migrated = lib.get_playlist(playlist).unwrap().unwrap();
        assert_eq!((migrated.parent_id, migrated.is_folder), (0, false));
        assert_eq!(migrated.track_ids, vec![track as u32]);
        let folder = lib.create_folder("Sets", 0).unwrap();
        lib.create_playlist_in("Friday", folder as u32).unwrap();
        assert_eq!(lib.get_all_playlists().unwrap().len(), 3);
    }

    fn tree_playlist(id: u32, name: &str, parent_id: u32, is_folder: bool, track_ids: &[u32]) -> Playlist {
        Playlist { id, name: name.to_string(), track_ids: track_ids.to_vec(), parent_id, is_folder }
    }

    #[test]
    fn playlist_tree_round_trips_in_any_row_order() {
        let lib = LocalLibrary::open_in_memory().unwrap();
        let t1 = lib.add_track(&make_track("A", "/music/a.mp3")).unwrap() as u32;
        let tree = vec![
            tree_playlist(3, "Friday", 2, false, &[t1]),
            tree_playlist(2, "Inner", 1, true, &[]),
            tree_playlist(1, "Sets", 0, true, &[]),
            tree_playlist(4, "Loose", 0, false, &[]),
        ];
        lib.replace_playlists(&tree).unwrap();

        let stored = lib.get_all_playlists().unwrap();
        let shape: Vec<_> = stored.iter().map(|p| (p.id, p.parent_id, p.is_folder)).collect();
        assert_eq!(shape, vec![(3, 2, false), (2, 1, true), (1, 0, true), (4, 0, false)]);
        assert_eq!(stored[0].track_ids, vec![t1]);
    }

    #[test]
    fn replace_playlists_applies_reorders_removals_and_repeat_saves() {
        let lib = LocalLibrary::open_in_memory().unwrap();
        let ids: Vec<u32> = (0..4)
            .map(|n| lib.add_track(&make_track(&format!("T{n}"), &format!("/music/{n}.mp3"))).unwrap() as u32)
            .collect();
        let tracks_of = |lib: &LocalLibrary| lib.get_playlist(1).unwrap().unwrap().track_ids;

        let initial = [tree_playlist(1, "Set", 0, false, &ids[..3])];
        lib.replace_playlists(&initial).unwrap();
        lib.replace_playlists(&initial).unwrap();
        assert_eq!(tracks_of(&lib), ids[..3].to_vec());

        let reordered = [tree_playlist(1, "Set", 0, false, &[ids[2], ids[0], ids[1]])];
        lib.replace_playlists(&reordered).unwrap();
        assert_eq!(tracks_of(&lib), vec![ids[2], ids[0], ids[1]]);

        let appended = [tree_playlist(1, "Set", 0, false, &[ids[2], ids[0], ids[1], ids[3]])];
        lib.replace_playlists(&appended).unwrap();
        assert_eq!(tracks_of(&lib), vec![ids[2], ids[0], ids[1], ids[3]]);

        let shrunk = [tree_playlist(1, "Set", 0, false, &[ids[2], ids[0]])];
        lib.replace_playlists(&shrunk).unwrap();
        assert_eq!(tracks_of(&lib), vec![ids[2], ids[0]]);

        let emptied = [tree_playlist(1, "Set", 0, false, &[])];
        lib.replace_playlists(&emptied).unwrap();
        assert!(tracks_of(&lib).is_empty());
    }

    #[test]
    fn replace_playlists_rejects_cycles_and_bad_parents() {
        let lib = LocalLibrary::open_in_memory().unwrap();
        lib.replace_playlists(&[tree_playlist(1, "A", 0, true, &[])]).unwrap();

        let cycle = [tree_playlist(1, "A", 2, true, &[]), tree_playlist(2, "B", 1, true, &[])];
        assert!(lib.replace_playlists(&cycle).is_err());
        let own_parent = [tree_playlist(1, "A", 1, true, &[])];
        assert!(lib.replace_playlists(&own_parent).is_err());
        let missing_parent = [tree_playlist(1, "A", 9, false, &[])];
        assert!(lib.replace_playlists(&missing_parent).is_err());
        let playlist_parent = [tree_playlist(1, "A", 0, false, &[]), tree_playlist(2, "B", 1, false, &[])];
        assert!(lib.replace_playlists(&playlist_parent).is_err());
        let folder_with_tracks = [tree_playlist(1, "A", 0, true, &[1])];
        assert!(lib.replace_playlists(&folder_with_tracks).is_err());

        let stored = lib.get_all_playlists().unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!((stored[0].parent_id, stored[0].is_folder), (0, true));
    }

    #[test]
    fn deleting_a_folder_deletes_its_contents() {
        let lib = LocalLibrary::open_in_memory().unwrap();
        let t1 = lib.add_track(&make_track("A", "/music/a.mp3")).unwrap();
        let folder = lib.create_folder("Sets", 0).unwrap();
        let inner = lib.create_folder("Inner", folder as u32).unwrap();
        let playlist = lib.create_playlist_in("Friday", inner as u32).unwrap();
        lib.set_playlist_tracks(playlist, &[t1]).unwrap();
        let keep = lib.create_playlist("Keep").unwrap();

        lib.delete_playlist(folder).unwrap();

        let ids: Vec<u32> = lib.get_all_playlists().unwrap().iter().map(|p| p.id).collect();
        assert_eq!(ids, vec![keep as u32]);
        assert!(lib.get_track(t1).unwrap().is_some());
    }

    #[test]
    fn replace_playlists_dropping_a_folder_removes_its_children() {
        let lib = LocalLibrary::open_in_memory().unwrap();
        lib.replace_playlists(&[
            tree_playlist(1, "Sets", 0, true, &[]),
            tree_playlist(2, "Friday", 1, false, &[]),
            tree_playlist(3, "Keep", 0, false, &[]),
        ])
        .unwrap();
        lib.replace_playlists(&[tree_playlist(3, "Keep", 0, false, &[])]).unwrap();
        let ids: Vec<u32> = lib.get_all_playlists().unwrap().iter().map(|p| p.id).collect();
        assert_eq!(ids, vec![3]);
    }

    #[test]
    fn stick_selection_includes_folder_contents_and_enclosing_folders() {
        let playlists = vec![
            tree_playlist(1, "Sets", 0, true, &[]),
            tree_playlist(2, "Friday", 1, false, &[5]),
            tree_playlist(3, "Archive", 0, true, &[]),
            tree_playlist(4, "Old", 3, false, &[6]),
            tree_playlist(5, "Loose", 0, false, &[7]),
        ];
        let ids = |chosen: &[i64]| -> Vec<u32> {
            selected_playlist_tree(&playlists, chosen).iter().map(|p| p.id).collect()
        };
        assert_eq!(ids(&[2]), vec![1, 2]);
        assert_eq!(ids(&[3]), vec![3, 4]);
        assert_eq!(ids(&[5]), vec![5]);
    }

    #[test]
    fn prepare_for_usb_remaps_playlist_parents() {
        let playlists = vec![
            tree_playlist(40, "Sets", 0, true, &[]),
            tree_playlist(70, "Friday", 40, false, &[]),
        ];
        let (_, _, usb) = prepare_for_usb(Vec::new(), Vec::new(), playlists);
        assert_eq!(usb.iter().map(|p| (p.id, p.parent_id)).collect::<Vec<_>>(), vec![(1, 0), (2, 1)]);
    }

    #[test]
    fn playlist_crud() {
        let lib = LocalLibrary::open_in_memory().unwrap();
        let t1 = lib.add_track(&make_track("A", "/music/a.mp3")).unwrap();
        let t2 = lib.add_track(&make_track("B", "/music/b.mp3")).unwrap();

        let pl_id = lib.create_playlist("My Playlist").unwrap();
        lib.set_playlist_tracks(pl_id, &[t2, t1]).unwrap();

        let pl = lib.get_playlist(pl_id).unwrap().unwrap();
        assert_eq!(pl.name, "My Playlist");
        assert_eq!(pl.track_ids, vec![t2 as u32, t1 as u32]);

        lib.rename_playlist(pl_id, "Renamed").unwrap();
        let pl = lib.get_playlist(pl_id).unwrap().unwrap();
        assert_eq!(pl.name, "Renamed");

        let all = lib.get_all_playlists().unwrap();
        assert_eq!(all.len(), 1);

        lib.delete_playlist(pl_id).unwrap();
        assert!(lib.get_playlist(pl_id).unwrap().is_none());
        assert_eq!(lib.track_count().unwrap(), 2);
    }

    #[test]
    fn prepare_for_usb_remaps_ids() {
        let tracks = vec![
            { let mut t = make_track("A", "/a.mp3"); t.id = 10; t },
            { let mut t = make_track("B", "/b.mp3"); t.id = 25; t },
        ];
        let analyses = vec![make_analysis(), make_analysis()];
        let playlists = vec![Playlist {
            id: 99,
            name: "PL".to_string(),
            track_ids: vec![25, 10],
            ..Default::default()
        }];

        let (usb_tracks, _usb_analyses, usb_playlists) =
            prepare_for_usb(tracks, analyses, playlists);

        assert_eq!(usb_tracks[0].id, 1);
        assert_eq!(usb_tracks[1].id, 2);
        assert_eq!(usb_playlists[0].id, 1);
        assert_eq!(usb_playlists[0].track_ids, vec![2, 1]);
    }

    #[test]
    fn persistence_across_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("library.db");

        {
            let lib = LocalLibrary::open(&db_path).unwrap();
            let id = lib.add_track(&make_track("Persisted", "/music/p.mp3")).unwrap();
            lib.set_analysis(id, &make_analysis()).unwrap();
            let pl = lib.create_playlist("PL").unwrap();
            lib.set_playlist_tracks(pl, &[id]).unwrap();
        }

        {
            let lib = LocalLibrary::open(&db_path).unwrap();
            assert_eq!(lib.track_count().unwrap(), 1);
            let tracks = lib.get_all_tracks().unwrap();
            assert_eq!(tracks[0].title, "Persisted");
            let analysis = lib.get_analysis(tracks[0].id as i64).unwrap();
            assert!(analysis.is_some());
            // Verify full ANLZ data persists across reopen.
            let a = analysis.unwrap();
            assert_eq!(a.bpm, 128.0);
            assert_eq!(a.beat_grid.beats.len(), 4);
            let playlists = lib.get_all_playlists().unwrap();
            assert_eq!(playlists.len(), 1);
            assert_eq!(playlists[0].track_ids.len(), 1);
        }
    }

    #[test]
    fn add_track_with_artwork() {
        let lib = LocalLibrary::open_in_memory().unwrap();
        let mut track = make_track("Art Track", "/music/art.mp3");
        track.artwork = Some(vec![0xFF, 0xD8, 0xFF, 0xE0]);
        let id = lib.add_track(&track).unwrap();

        let art = lib.get_artwork(id).unwrap().unwrap();
        assert_eq!(art, vec![0xFF, 0xD8, 0xFF, 0xE0]);
    }

    #[test]
    fn covers_added_later_are_queued_then_stored_without_overwriting_or_reviving() {
        let lib = LocalLibrary::open_in_memory().unwrap();
        let mut tracks: Vec<Track> =
            ["a", "b", "c", "d"].iter().map(|n| make_track(n, &format!("/music/{n}.mp3"))).collect();
        tracks[0].artwork = Some(vec![9]);
        let ids = lib.add_tracks_covers_later(&tracks).unwrap();
        assert!(lib.get_artwork(ids[0]).unwrap().is_none());
        assert!(lib.get_track_list_rows().unwrap().iter().all(|r| !r.has_artwork));
        let queued: Vec<i64> = lib.pending_artwork(0, 10).unwrap().into_iter().map(|(id, _)| id).collect();
        assert_eq!(queued, ids);
        assert_eq!(lib.pending_artwork(ids[1], 1).unwrap(), vec![(ids[2], PathBuf::from("/music/c.mp3"))]);

        lib.set_artwork(ids[1], &[7]).unwrap();
        lib.remove_track(ids[2]).unwrap();
        let path = |n: &str| PathBuf::from(format!("/music/{n}.mp3"));
        lib.store_pending_artwork(&[
            (ids[0], path("a"), Some(vec![1])),
            (ids[1], path("b"), Some(vec![2])),
            (ids[2], path("c"), Some(vec![3])),
            (ids[3], path("other"), Some(vec![4])),
        ])
        .unwrap();
        assert_eq!(lib.pending_artwork(0, 10).unwrap(), vec![(ids[3], path("d"))]);
        assert!(lib.get_artwork(ids[3]).unwrap().is_none());
        lib.store_pending_artwork(&[(ids[3], path("d"), None)]).unwrap();

        assert_eq!(lib.get_artwork(ids[0]).unwrap(), Some(vec![1]));
        assert_eq!(lib.get_artwork(ids[1]).unwrap(), Some(vec![7]));
        assert!(lib.get_artwork(ids[2]).unwrap().is_none());
        assert!(lib.get_artwork(ids[3]).unwrap().is_none());
        assert!(lib.pending_artwork(0, 10).unwrap().is_empty());
        assert_eq!(lib.track_paths().unwrap().len(), 3);
    }

    struct Xorshift(u64);

    impl Xorshift {
        fn below(&mut self, n: usize) -> usize {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            (self.0 % n as u64) as usize
        }
    }

    fn random_track(rng: &mut Xorshift) -> Track {
        const ARTISTS: [&str; 6] = ["Hale", "HAL\u{c9}", "Ode & Ash", "ode ash", "Unknown Artist", ""];
        const TITLES: [&str; 5] = ["Copper Lines", "copper lines (Original Mix)", "Glass Harbor", "Intro", ""];
        const DURATIONS: [f64; 5] = [0.0, 100.0, 101.5, 103.0, 300.0];
        const SIZES: [u64; 3] = [0, 1000, 2000];
        const FOLDERS: [&str; 3] = ["/music", "/Music", "/other"];
        let path = format!("{}/{}.mp3", FOLDERS[rng.below(3)], rng.below(25));
        Track {
            artist: ARTISTS[rng.below(ARTISTS.len())].into(),
            duration_secs: DURATIONS[rng.below(DURATIONS.len())],
            file_size: SIZES[rng.below(SIZES.len())],
            ..make_track(TITLES[rng.below(TITLES.len())], &path)
        }
    }

    fn assert_cached_duplicates_equal_a_fresh_scan(lib: &LocalLibrary, context: &str) {
        let fresh = lib.duplicate_index().unwrap();
        assert!(lib.lock_duplicates().as_ref() == Some(&fresh), "{context}");
    }

    #[test]
    fn the_cached_duplicate_index_equals_a_fresh_scan_after_random_writes() {
        let lib = LocalLibrary::open_in_memory().unwrap();
        let mut rng = Xorshift(0x2545_F491_4F6C_DD1D);
        for _ in 0..30 {
            let _ = lib.add_track(&random_track(&mut rng));
        }
        lib.with_duplicate_index(|_| ()).unwrap();
        assert_cached_duplicates_equal_a_fresh_scan(&lib, "built");
        for step in 0..600 {
            let ids: Vec<i64> = lib.get_all_tracks().unwrap().iter().map(|t| t.id as i64).collect();
            let some_id = |rng: &mut Xorshift| (!ids.is_empty()).then(|| ids[rng.below(ids.len())]);
            let op = rng.below(7);
            match (op, some_id(&mut rng)) {
                (0, _) => {
                    let _ = lib.add_track(&random_track(&mut rng));
                }
                (1, _) => {
                    let batch: Vec<Track> = (0..1 + rng.below(4)).map(|_| random_track(&mut rng)).collect();
                    let _ = lib.add_tracks(&batch);
                }
                (2, Some(id)) | (3, Some(id)) => {
                    let _ = lib.update_track(id, &random_track(&mut rng));
                }
                (4, Some(id)) => lib.remove_track(id).unwrap(),
                (5, Some(id)) => {
                    let other = some_id(&mut rng).unwrap();
                    lib.remove_tracks(&[id, other]).unwrap();
                }
                (6, Some(id)) => {
                    let staged = lib.begin_analysis(id, &make_analysis(), analysis_stage::ALL).unwrap().stage().unwrap();
                    let _ = lib.commit_analysis(staged, Some(&random_track(&mut rng)));
                }
                _ => {}
            }
            assert_cached_duplicates_equal_a_fresh_scan(&lib, &format!("after step {step} (op {op})"));
        }
        assert!(lib.track_count().unwrap() > 0);
    }

    #[test]
    fn the_cached_duplicate_index_follows_relinks_and_relocations() {
        let dir = tempfile::tempdir().unwrap();
        let (found, there) = (dir.path().join("found"), dir.path().join("there"));
        std::fs::create_dir_all(&found).unwrap();
        std::fs::create_dir_all(&there).unwrap();
        for name in ["a.mp3", "b.mp3", "c.mp3"] {
            std::fs::write(found.join(name), [0u8; 10]).unwrap();
        }
        std::fs::write(there.join("d.mp3"), [0u8; 10]).unwrap();
        let lib = LocalLibrary::open_in_memory().unwrap();
        let track = |name: &str, folder: &str| Track { file_size: 10, ..make_track(name, &format!("{folder}/{name}.mp3")) };
        let ids = lib.add_tracks(&[track("a", "/gone"), track("b", "/gone"), track("c", "/gone"), track("d", "/gone")]).unwrap();
        lib.with_duplicate_index(|_| ()).unwrap();

        let report = lib.relocate_in_folder(&ids[..3], &found).unwrap();
        assert_eq!(report.matched.len(), 3);
        assert_cached_duplicates_equal_a_fresh_scan(&lib, "relocated");
        lib.relink_track(ids[3], &there.join("d.mp3")).unwrap();
        assert_cached_duplicates_equal_a_fresh_scan(&lib, "relinked");
        let moved = lib.get_track(ids[3]).unwrap().unwrap();
        let hit = lib.with_duplicate_index(|index| index.find(&moved)).unwrap();
        assert_eq!(hit.map(|h| (h.existing_id, h.reason)), Some((ids[3], DuplicateReason::SamePath)));
    }

    #[test]
    fn the_cached_duplicate_index_is_rebuilt_when_it_no_longer_matches_the_library() {
        let lib = LocalLibrary::open_in_memory().unwrap();
        let ids = lib.add_tracks(&[make_track("A", "/music/a.mp3"), make_track("B", "/music/b.mp3")]).unwrap();
        lib.with_duplicate_index(|index| assert_eq!(index.len(), 2)).unwrap();
        lib.conn.execute("DELETE FROM tracks WHERE id = ?1", [ids[0]]).unwrap();
        lib.with_duplicate_index(|index| assert_eq!(index.len(), 1)).unwrap();
        assert_cached_duplicates_equal_a_fresh_scan(&lib, "rebuilt");
    }
}
