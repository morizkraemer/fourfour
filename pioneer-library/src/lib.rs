//! Persistent local music library for Pioneer CDJ workflows.
//!
//! Backed by a plain SQLite database. Provides CRUD for tracks, analyses,
//! artwork, and playlists, plus convenience methods to export or sync to a
//! Pioneer-formatted USB drive via [`pioneer_usb_writer`].
//!
//! Analysis data (beat grids, waveforms, cue points) is stored as ANLZ binary
//! files on disk alongside the database. The SQLite `analyses` table keeps only
//! a lightweight index (bpm + key) for fast queries.

mod queries;
mod schema;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use pioneer_usb_writer::models::{
    AnalysisResult, BeatGrid, Playlist, SyncReport, Track, WaveformPreview,
};
use pioneer_usb_writer::{reader, writer};

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
        }

        // Update lightweight DB index.
        queries::upsert_analysis_index(&self.conn, track_id, analysis.bpm, &analysis.key)?;

        Ok(())
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
        let Some((bpm, key)) = queries::select_analysis_index(&self.conn, track_id)? else {
            return Ok(None);
        };

        // Try reading full data from ANLZ files.
        if let Some(dir) = self.track_anlz_dir(track_id) {
            let dat_path = dir.join("ANLZ0000.DAT");
            if dat_path.exists() {
                let mut result = reader::anlz::read_anlz(&dat_path)?;
                // Override bpm/key from the DB index (authoritative source).
                result.bpm = bpm;
                result.key = key;
                return Ok(Some(result));
            }
        }

        // Fallback for in-memory or missing files: return minimal result.
        Ok(Some(AnalysisResult {
            beat_grid: BeatGrid { beats: Vec::new() },
            waveform: WaveformPreview { data: [0u8; 400] },
            bpm,
            key,
            cue_points: Vec::new(),
            color_waveform: None,
        }))
    }

    /// Retrieve all tracks that have analysis data, with artwork included.
    /// Returns parallel vecs suitable for passing to the writer.
    pub fn get_analyzed_tracks(&self) -> Result<(Vec<Track>, Vec<AnalysisResult>)> {
        let track_ids = queries::select_analyzed_track_ids(&self.conn)?;

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

    /// Return IDs of tracks that have no analysis data yet.
    pub fn get_unanalyzed_track_ids(&self) -> Result<Vec<i64>> {
        queries::select_unanalyzed_track_ids(&self.conn)
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
