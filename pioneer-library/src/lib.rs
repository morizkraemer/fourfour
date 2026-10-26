//! Persistent local music library for Pioneer CDJ workflows.
//!
//! Backed by a plain SQLite database. Provides CRUD for tracks, analyses,
//! artwork, and playlists, plus convenience methods to export or sync to a
//! Pioneer-formatted USB drive via [`pioneer_usb_writer`].
//!
//! Analysis data (beat grids, waveforms, cue points) is stored as ANLZ binary
//! files on disk alongside the database. The SQLite `analyses` table keeps only
//! a lightweight index (bpm + key) for fast queries.

mod multi_sync;
mod queries;
pub mod rekordbox;
mod row_preview;
mod schema;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use pioneer_usb_writer::models::{
    AnalysisResult, Beat, BeatGrid, ColorWaveform, Playlist, SyncReport, Track, WaveformPreview,
};
use pioneer_usb_writer::{reader, writer};

/// One beat of an edited grid: where it is and its place in the bar (1 to 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BeatPosition {
    pub time_ms: u32,
    pub bar_position: u8,
}

pub use multi_sync::{
    NoProgress, PreparedMultiSync, ProgressSink, StickProgress, StickReport, StickStatus,
    SyncPhase, SyncSelection, SyncTarget, WriterFn, statvfs_available_bytes,
};
pub use row_preview::{ROW_BAND_COLUMNS, RowPreview, effective_preview, row_bands};

/// Bits of `analyses.stages`: which parts of a track's analysis are stored.
pub mod analysis_stage {
    pub const WAVEFORM: u8 = 1;
    pub const KEY: u8 = 2;
    pub const GRID: u8 = 4;
    pub const ALL: u8 = WAVEFORM | KEY | GRID;
}

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
        })
    }

    /// Open an in-memory library (for testing).
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        schema::initialize(&conn, None)?;
        Ok(Self {
            conn,
            db_path: None,
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
        if let Some(ref art) = track.artwork {
            queries::upsert_artwork(&self.conn, id, art)?;
        }
        Ok(id)
    }

    /// Add multiple tracks in a single transaction. Returns assigned IDs.
    pub fn add_tracks(&self, tracks: &[Track]) -> Result<Vec<i64>> {
        let tx = self.conn.unchecked_transaction()?;
        let mut ids = Vec::with_capacity(tracks.len());
        for track in tracks {
            let id = queries::insert_track(&tx, track)?;
            if let Some(ref art) = track.artwork {
                queries::upsert_artwork(&tx, id, art)?;
            }
            ids.push(id);
        }
        tx.commit()?;
        Ok(ids)
    }

    /// Update metadata for an existing track (identified by library ID).
    pub fn update_track(&self, id: i64, track: &Track) -> Result<()> {
        queries::update_track(&self.conn, id, track)
    }

    /// Remove a track (and its analysis + artwork via CASCADE), including ANLZ files.
    pub fn remove_track(&self, id: i64) -> Result<()> {
        queries::delete_tracks(&self.conn, &[id])?;
        self.remove_track_anlz(id);
        Ok(())
    }

    /// Remove multiple tracks in a single transaction, including ANLZ files.
    pub fn remove_tracks(&self, ids: &[i64]) -> Result<()> {
        queries::delete_tracks(&self.conn, ids)?;
        for id in ids {
            self.remove_track_anlz(*id);
        }
        Ok(())
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

    /// Return the number of tracks in the library.
    pub fn track_count(&self) -> Result<u32> {
        queries::track_count(&self.conn)
    }

    // -----------------------------------------------------------------------
    // Artwork
    // -----------------------------------------------------------------------

    /// Store (or replace) artwork for a track.
    pub fn set_artwork(&self, track_id: i64, image_data: &[u8]) -> Result<()> {
        queries::upsert_artwork(&self.conn, track_id, image_data)
    }

    /// Retrieve artwork for a track. Returns `None` if no artwork stored.
    pub fn get_artwork(&self, track_id: i64) -> Result<Option<Vec<u8>>> {
        queries::select_artwork(&self.conn, track_id)
    }

    /// Remove artwork for a track.
    pub fn remove_artwork(&self, track_id: i64) -> Result<()> {
        queries::delete_artwork(&self.conn, track_id)
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
        let analysis = &self.keep_imported_data(track_id, analysis)?;
        self.write_analysis(track_id, analysis, stages)
    }

    /// Replace the beat grid of an analyzed track with an edited one (nudged, or set from the playhead).
    /// The tempo of every beat stays the stored one. An edit is user data, so it is written as it is (not
    /// replaced by a locked imported grid) and the analysis stages stay as they were.
    pub fn set_beat_grid(&self, track_id: i64, positions: &[BeatPosition]) -> Result<()> {
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
        self.write_analysis(track_id, &analysis, stages)
    }

    fn write_analysis(&self, track_id: i64, analysis: &AnalysisResult, stages: u8) -> Result<()> {
        // Write ANLZ files if we have a disk path.
        if let Some(dir) = self.track_anlz_dir(track_id) {
            std::fs::create_dir_all(&dir)
                .with_context(|| format!("Failed to create ANLZ dir: {}", dir.display()))?;

            let track = self
                .get_track(track_id)?
                .context("Track not found for analysis storage")?;

            // For local library ANLZ files, use source_path in the PPTH section
            // (CDJs never read these files — they're only for local round-tripping).
            let mut track_for_anlz = track.clone();
            track_for_anlz.usb_path = track.source_path.to_string_lossy().to_string();

            let dat_path = dir.join("ANLZ0000.DAT");
            let ext_path = dir.join("ANLZ0000.EXT");

            writer::anlz::write_anlz_dat(&dat_path, &track_for_anlz, analysis)?;
            writer::anlz::write_anlz_ext(&ext_path, &track_for_anlz, analysis)?;
            writer::anlz::write_anlz_2ex(&dir.join("ANLZ0000.2EX"), &track_for_anlz, analysis)?;
        }

        let row_preview = match self.track_anlz_dir(track_id) {
            Some(dir) => read_waveform_analysis(&dir.join("ANLZ0000.DAT")).map_or(RowPreview::None, |a| RowPreview::from_analysis(&a)),
            None => RowPreview::from_analysis(analysis),
        };

        // Update lightweight DB index.
        queries::upsert_analysis_index(&self.conn, track_id, analysis.bpm, &analysis.key, analysis.energy, &row_preview.to_blob(), stages)?;

        Ok(())
    }

    /// Re-analysis must not replace what was imported from Rekordbox: a locked
    /// beat grid (and its tempo) stays as stored, locked cues come from the
    /// import metadata.
    fn keep_imported_data(&self, track_id: i64, analysis: &AnalysisResult) -> Result<AnalysisResult> {
        let mut kept = analysis.clone();
        let Some(meta) = self.imported_meta(track_id) else {
            return Ok(kept);
        };
        if meta.grid_locked {
            match self.get_analysis(track_id)? {
                Some(current) if !current.beat_grid.beats.is_empty() => {
                    kept.beat_grid = current.beat_grid;
                    kept.bpm = current.bpm;
                }
                _ if !meta.grid.is_empty() => {
                    kept.bpm = meta.grid[0].tempo as f64 / 100.0;
                    kept.beat_grid = BeatGrid { beats: meta.grid.clone() };
                }
                _ => {}
            }
        }
        if meta.cues_locked {
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
        let workers = std::thread::available_parallelism().map_or(4, |n| n.get());
        let chunk_len = ids.len().div_ceil(workers).max(1);

        let mut out = HashMap::with_capacity(ids.len());
        std::thread::scope(|scope| {
            let handles: Vec<_> = ids
                .chunks(chunk_len)
                .map(|chunk| {
                    let anlz_dir = &anlz_dir;
                    scope.spawn(move || {
                        chunk
                            .iter()
                            .filter_map(|id| {
                                let dat = anlz_dir.join(id.to_string()).join("ANLZ0000.DAT");
                                read_waveform_analysis(&dat).map(|a| (*id, a))
                            })
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            for handle in handles {
                out.extend(handle.join().expect("waveform reader thread panicked"));
            }
        });
        out
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

    /// Retrieve all tracks that have analysis data, with artwork included.
    /// Returns parallel vecs suitable for passing to the writer.
    pub fn get_analyzed_tracks(&self) -> Result<(Vec<Track>, Vec<AnalysisResult>)> {
        let track_ids = queries::select_complete_analysis_track_ids(&self.conn, analysis_stage::ALL)?;

        let mut tracks = Vec::new();
        let mut analyses = Vec::new();

        for id in track_ids {
            if let (Some(mut track), Some(analysis)) = (self.get_track(id)?, self.get_analysis(id)?) {
                // Load artwork for USB export.
                track.artwork = queries::select_artwork(&self.conn, id)?;
                tracks.push(track);
                analyses.push(analysis);
            }
        }

        Ok((tracks, analyses))
    }

    /// Return IDs of tracks that have no analysis data yet or only some of its stages.
    pub fn get_unanalyzed_track_ids(&self) -> Result<Vec<i64>> {
        queries::select_unanalyzed_track_ids(&self.conn, analysis_stage::ALL)
    }

    /// Stage bits stored for a track, `None` when it has no analysis row.
    pub fn analysis_stages(&self, track_id: i64) -> Result<Option<u8>> {
        queries::select_analysis_stages(&self.conn, track_id)
    }

    /// Tracks that have an analysis row with stages still missing.
    pub fn get_incomplete_analysis_ids(&self) -> Result<Vec<i64>> {
        queries::select_incomplete_analysis_ids(&self.conn, analysis_stage::ALL)
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
                    let _ = queries::upsert_artwork(&tx, library_id, &art_data);
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

        Ok(ImportReport {
            tracks_imported,
            tracks_skipped,
            playlists_imported,
        })
    }
}

/// Remap sparse library IDs to dense 1-based USB IDs.
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

    for (i, playlist) in playlists.iter_mut().enumerate() {
        playlist.id = (i + 1) as u32;
    }

    (tracks, analyses, playlists)
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

    fn make_track(title: &str, source: &str) -> Track {
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

    fn make_analysis() -> AnalysisResult {
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
                CuePoint { hot_cue_number: 0, time_ms: 1000, loop_time_ms: None },
                CuePoint { hot_cue_number: 1, time_ms: 60_000, loop_time_ms: None },
                CuePoint { hot_cue_number: 2, time_ms: 90_000, loop_time_ms: Some(100_000) },
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

    fn banded_analysis() -> AnalysisResult {
        use pioneer_usb_writer::models::ColorWaveform;
        let mut a = make_analysis();
        a.color_waveform = Some(ColorWaveform {
            detail: (0..900).map(|n| [(n % 100) as u8, 40, (n % 60) as u8]).collect(),
            overview: (0..1200).map(|n| [(n % 100) as u8, 40, (n % 60) as u8]).collect(),
            exact: None,
        });
        a
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

        lib.set_beat_grid(id, &shifted(&before, 37)).unwrap();

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
    fn edited_beat_grid_keeps_pending_stages_and_survives_a_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("library.db");
        let id = {
            let lib = LocalLibrary::open(&db_path).unwrap();
            let id = lib.add_track(&make_track("Edit", "/music/e.mp3")).unwrap();
            lib.set_analysis_stages(id, &make_analysis(), analysis_stage::WAVEFORM | analysis_stage::KEY).unwrap();
            lib.set_beat_grid(id, &[BeatPosition { time_ms: 120, bar_position: 1 }, BeatPosition { time_ms: 589, bar_position: 2 }]).unwrap();
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
        lib.set_beat_grid(id, &shifted(&current, 50)).unwrap();
        assert_eq!(lib.get_analysis(id).unwrap().unwrap().beat_grid.beats[0].time_ms, 50);

        let mut reanalysis = make_analysis();
        reanalysis.beat_grid.beats[0].time_ms = 9;
        lib.set_analysis(id, &reanalysis).unwrap();
        assert_eq!(lib.get_analysis(id).unwrap().unwrap().beat_grid.beats[0].time_ms, 50);
    }

    #[test]
    fn beat_grid_of_an_unanalyzed_track_is_an_error() {
        let lib = LocalLibrary::open_in_memory().unwrap();
        let id = lib.add_track(&make_track("None", "/music/n.mp3")).unwrap();
        assert!(lib.set_beat_grid(id, &[BeatPosition { time_ms: 0, bar_position: 1 }]).is_err());
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
    fn get_unanalyzed_track_ids() {
        let lib = LocalLibrary::open_in_memory().unwrap();
        let id1 = lib.add_track(&make_track("A", "/music/a.mp3")).unwrap();
        let id2 = lib.add_track(&make_track("B", "/music/b.mp3")).unwrap();

        lib.set_analysis(id1, &make_analysis()).unwrap();

        let unanalyzed = lib.get_unanalyzed_track_ids().unwrap();
        assert_eq!(unanalyzed, vec![id2]);
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
}
