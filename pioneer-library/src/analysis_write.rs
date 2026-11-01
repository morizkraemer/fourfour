//! Storing an analysis in three steps, so a caller that shares the library behind a lock holds it only for the short
//! parts: [`LocalLibrary::begin_analysis`] reads what the stored analysis depends on (lock held),
//! [`AnalysisWrite::stage`] builds the ANLZ files in a staging folder next to the track's files (no lock), and
//! [`LocalLibrary::commit_analysis`] moves them in and writes the row (lock held). The result is the same as
//! [`LocalLibrary::set_analysis_stages`]: commit refuses when anything the files were built from changed in between,
//! and the caller then stores the analysis the old way.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result};
use pioneer_usb_writer::models::{AnalysisResult, Track};
use pioneer_usb_writer::writer;

use crate::edits::EDITS_FILE;
use crate::rekordbox::SIDECAR_FILE;
use crate::{LocalLibrary, RowPreview, queries, read_waveform_analysis};

const DAT: &str = "ANLZ0000.DAT";
const EXT: &str = "ANLZ0000.EXT";
const TWO_EX: &str = "ANLZ0000.2EX";
pub(crate) const STAGING_PREFIX: &str = ".staging-";

static NEXT_STAGING: AtomicU64 = AtomicU64::new(0);

/// Everything a stored analysis is built from besides the new result: the track row, the user's edits, the imported
/// Rekordbox data, the stored index row and the stored beat grid file. Equal inputs give equal files and rows.
#[derive(PartialEq)]
struct Inputs {
    track: Option<Vec<u8>>,
    edits: Option<Vec<u8>>,
    imported: Option<Vec<u8>>,
    index: Option<(u64, String, Option<u8>)>,
    stored_dat: Option<Vec<u8>>,
}

/// An analysis merged with what the user edited or imported, not yet written.
pub struct AnalysisWrite {
    track_id: i64,
    stages: u8,
    analysis: AnalysisResult,
    track: Option<Track>,
    dir: Option<PathBuf>,
    inputs: Inputs,
}

/// An analysis whose files are written to a staging folder, waiting for [`LocalLibrary::commit_analysis`].
pub struct StagedAnalysis {
    track_id: i64,
    stages: u8,
    analysis: AnalysisResult,
    files: Option<StagedFiles>,
    row_preview: RowPreview,
    inputs: Inputs,
}

struct StagedFiles {
    dir: PathBuf,
    staging: PathBuf,
    has_2ex: bool,
}

impl StagedFiles {
    fn create(dir: &Path) -> Result<Self> {
        let staging = dir.join(format!("{STAGING_PREFIX}{}-{}", std::process::id(), NEXT_STAGING.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir_all(&staging).with_context(|| format!("Failed to create ANLZ dir: {}", staging.display()))?;
        Ok(StagedFiles { dir: dir.to_path_buf(), staging, has_2ex: false })
    }

    fn discard(&self) {
        let _ = std::fs::remove_dir_all(&self.staging);
    }
}

impl AnalysisWrite {
    /// The track row the analysis was merged against.
    pub fn track(&self) -> Option<&Track> {
        self.track.as_ref()
    }

    /// Writes the ANLZ files to a staging folder inside the track's folder and reads the row preview back from them.
    /// Needs no access to the database.
    pub fn stage(self) -> Result<StagedAnalysis> {
        let (files, row_preview) = match &self.dir {
            Some(dir) => {
                let track = self.track.as_ref().context("Track not found for analysis storage")?;
                let files = StagedFiles::create(dir)?;
                match write_files(&files.staging, track, &self.analysis) {
                    Ok(has_2ex) => {
                        let row_preview = read_waveform_analysis(&files.staging.join(DAT))
                            .map_or(RowPreview::None, |a| RowPreview::from_analysis(&a));
                        (Some(StagedFiles { has_2ex, ..files }), row_preview)
                    }
                    Err(e) => {
                        files.discard();
                        return Err(e);
                    }
                }
            }
            None => (None, RowPreview::from_analysis(&self.analysis)),
        };
        Ok(StagedAnalysis {
            track_id: self.track_id,
            stages: self.stages,
            analysis: self.analysis,
            files,
            row_preview,
            inputs: self.inputs,
        })
    }
}

fn write_files(staging: &Path, track: &Track, analysis: &AnalysisResult) -> Result<bool> {
    let mut track_for_anlz = track.clone();
    track_for_anlz.usb_path = track.source_path.to_string_lossy().to_string();
    writer::anlz::write_anlz_dat(&staging.join(DAT), &track_for_anlz, analysis)?;
    writer::anlz::write_anlz_ext(&staging.join(EXT), &track_for_anlz, analysis)?;
    writer::anlz::write_anlz_2ex(&staging.join(TWO_EX), &track_for_anlz, analysis)
}

impl LocalLibrary {
    /// First step of storing an analysis (see the module docs): merges it with the user's edits and imported data
    /// like [`set_analysis_stages`](Self::set_analysis_stages) and notes what that depended on.
    pub fn begin_analysis(&self, track_id: i64, analysis: &AnalysisResult, stages: u8) -> Result<AnalysisWrite> {
        let inputs = self.analysis_inputs(track_id)?;
        let analysis = self.keep_user_edits(track_id, &self.keep_imported_data(track_id, analysis)?)?;
        Ok(AnalysisWrite {
            track_id,
            stages,
            analysis,
            track: self.get_track(track_id)?,
            dir: self.track_anlz_dir(track_id),
            inputs,
        })
    }

    /// Last step of storing an analysis: moves the staged files in and writes the index row, and `track` (when given)
    /// in the same transaction. Returns `false` and changes nothing when the track, its edits, its imported data or
    /// its stored analysis changed since [`begin_analysis`](Self::begin_analysis); the caller then stores it again.
    pub fn commit_analysis(&self, staged: StagedAnalysis, track: Option<&Track>) -> Result<bool> {
        if self.analysis_inputs(staged.track_id)? != staged.inputs {
            if let Some(files) = &staged.files {
                files.discard();
                if staged.inputs.track.is_none() || self.get_track(staged.track_id)?.is_none() {
                    let _ = std::fs::remove_dir(&files.dir);
                }
            }
            return Ok(false);
        }
        if let Some(files) = &staged.files {
            let moved = move_into_place(files);
            files.discard();
            moved?;
        }
        let tx = self.conn.unchecked_transaction()?;
        if let Some(track) = track {
            queries::update_track(&tx, staged.track_id, track)?;
        }
        queries::upsert_analysis_index(
            &tx,
            staged.track_id,
            staged.analysis.bpm,
            &staged.analysis.key,
            staged.analysis.energy,
            &staged.row_preview.to_blob(),
            staged.stages,
        )?;
        tx.commit()?;
        Ok(true)
    }

    /// Replaces the ANLZ files of a track with ones built from `analysis`, each file moving in whole (never
    /// truncated and rewritten in place), so a crash or a reader in between sees an old or a new file.
    pub(crate) fn write_anlz_files(&self, track_id: i64, analysis: &AnalysisResult) -> Result<()> {
        let Some(dir) = self.track_anlz_dir(track_id) else { return Ok(()) };
        let track = self.get_track(track_id)?.context("Track not found for analysis storage")?;
        let mut files = StagedFiles::create(&dir)?;
        let moved = write_files(&files.staging, &track, analysis).and_then(|has_2ex| {
            files.has_2ex = has_2ex;
            move_into_place(&files)
        });
        files.discard();
        moved
    }

    fn analysis_inputs(&self, track_id: i64) -> Result<Inputs> {
        let read = |dir: &Option<PathBuf>, name: &str| dir.as_ref().and_then(|d| std::fs::read(d.join(name)).ok());
        let dir = self.track_anlz_dir(track_id);
        Ok(Inputs {
            track: self.get_track(track_id)?.map(|t| serde_json::to_vec(&t)).transpose()?,
            edits: read(&dir, EDITS_FILE),
            imported: read(&dir, SIDECAR_FILE),
            index: queries::select_analysis_index(&self.conn, track_id)?.map(|(bpm, key, energy)| (bpm.to_bits(), key, energy)),
            stored_dat: read(&dir, DAT),
        })
    }
}

/// Removes the staging folders that a process other than this one left inside the track folders of the library at
/// `db_path` (a crash between staging and commit). Touches nothing else. Returns how many it removed.
pub fn remove_stale_staging(db_path: &Path) -> usize {
    let Some(root) = db_path.parent().map(|p| p.join("anlz")) else { return 0 };
    let Ok(track_dirs) = std::fs::read_dir(&root) else { return 0 };
    let own_pid = std::process::id();
    let mut removed = 0;
    for track_dir in track_dirs.flatten() {
        let Ok(entries) = std::fs::read_dir(track_dir.path()) else { continue };
        for entry in entries.flatten() {
            let is_real_dir = entry.file_type().is_ok_and(|t| t.is_dir());
            let stale = is_real_dir && staging_pid(&entry.file_name().to_string_lossy()).is_some_and(|pid| pid != own_pid);
            if stale && std::fs::remove_dir_all(entry.path()).is_ok() {
                removed += 1;
            }
        }
    }
    removed
}

fn staging_pid(folder_name: &str) -> Option<u32> {
    let (pid, counter) = folder_name.strip_prefix(STAGING_PREFIX)?.split_once('-')?;
    counter.parse::<u64>().ok()?;
    pid.parse().ok()
}

fn move_into_place(files: &StagedFiles) -> Result<()> {
    let rename = |name: &str| {
        std::fs::rename(files.staging.join(name), files.dir.join(name))
            .with_context(|| format!("Failed to store {}", files.dir.join(name).display()))
    };
    rename(DAT)?;
    rename(EXT)?;
    if files.has_2ex {
        rename(TWO_EX)
    } else {
        match std::fs::remove_file(files.dir.join(TWO_EX)) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis_stage;
    use crate::rekordbox::ImportedCue;
    use crate::tests::{banded_analysis, make_analysis, make_track};

    type Row = (f64, String, Option<u8>, Option<Vec<u8>>, u8);

    fn stored(lib: &LocalLibrary, id: i64) -> (Vec<Option<Vec<u8>>>, Option<Row>) {
        let dir = lib.track_anlz_dir(id).unwrap();
        let files = [DAT, EXT, TWO_EX].iter().map(|f| std::fs::read(dir.join(f)).ok()).collect();
        let row = lib
            .conn
            .query_row("SELECT bpm, key, energy, row_preview, stages FROM analyses WHERE track_id = ?1", [id], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
            })
            .ok();
        (files, row)
    }

    #[test]
    fn stale_staging_of_other_processes_is_removed_and_nothing_else() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("library.db");
        let lib = LocalLibrary::open(&db).unwrap();
        let id = lib.add_track(&make_track("Stale", "/music/stale.mp3")).unwrap();
        lib.set_analysis(id, &make_analysis()).unwrap();
        let track_dir = lib.track_anlz_dir(id).unwrap();
        let own = track_dir.join(format!("{STAGING_PREFIX}{}-0", std::process::id()));
        let other = track_dir.join(format!("{STAGING_PREFIX}{}-7", std::process::id() + 1));
        let odd_names = [".staging-abc-1", ".staging-12", "staging-1-1", ".staging-1-x"].map(|n| track_dir.join(n));
        for folder in odd_names.iter().chain([&own, &other]) {
            std::fs::create_dir_all(folder).unwrap();
            std::fs::write(folder.join(DAT), b"x").unwrap();
        }
        let before = stored(&lib, id);

        assert_eq!(remove_stale_staging(&db), 1);

        assert!(!other.exists());
        assert!(own.exists());
        assert!(odd_names.iter().all(|f| f.join(DAT).exists()));
        assert_eq!(stored(&lib, id), before);
    }

    fn no_staging_left(lib: &LocalLibrary, id: i64) {
        let names: Vec<_> =
            std::fs::read_dir(lib.track_anlz_dir(id).unwrap()).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert!(names.iter().all(|n| !n.to_string_lossy().starts_with(STAGING_PREFIX)), "{names:?}");
    }

    fn staged_store(lib: &LocalLibrary, id: i64, analysis: &AnalysisResult, stages: u8) -> bool {
        let staged = lib.begin_analysis(id, analysis, stages).unwrap().stage().unwrap();
        lib.commit_analysis(staged, None).unwrap()
    }

    fn two_libraries() -> (tempfile::TempDir, LocalLibrary, LocalLibrary) {
        let dir = tempfile::tempdir().unwrap();
        let open = |name: &str| {
            std::fs::create_dir_all(dir.path().join(name)).unwrap();
            let lib = LocalLibrary::open(&dir.path().join(name).join("library.db")).unwrap();
            lib.add_track(&make_track("Same", "/music/same.mp3")).unwrap();
            lib
        };
        let (old, new) = (open("old"), open("new"));
        (dir, old, new)
    }

    #[test]
    fn staged_store_writes_the_same_files_and_row_as_set_analysis() {
        let (_dir, old, new) = two_libraries();
        let mut plain = make_analysis();
        plain.energy = Some(7);
        for (analysis, stages) in [
            (plain, analysis_stage::WAVEFORM),
            (banded_analysis(), analysis_stage::ALL),
            (make_analysis(), analysis_stage::ALL),
        ] {
            old.set_analysis_stages(1, &analysis, stages).unwrap();
            assert!(staged_store(&new, 1, &analysis, stages));
            assert_eq!(stored(&old, 1), stored(&new, 1));
        }
        no_staging_left(&new, 1);
    }

    #[test]
    fn staged_store_keeps_edited_grid_cues_bpm_and_key_like_set_analysis() {
        let (_dir, old, new) = two_libraries();
        let cues = [ImportedCue { hot_cue_number: 2, time_ms: 1500, loop_time_ms: None, color: None, name: "drop".into() }];
        for lib in [&old, &new] {
            lib.set_analysis(1, &banded_analysis()).unwrap();
            let grid = lib.get_analysis(1).unwrap().unwrap();
            let positions: Vec<_> = grid
                .beat_grid
                .beats
                .iter()
                .map(|b| crate::BeatPosition { time_ms: b.time_ms + 11, bar_position: b.bar_position })
                .collect();
            lib.set_beat_grid(1, &positions, None, None).unwrap();
            lib.set_cues(1, &cues).unwrap();
            lib.set_user_bpm(1, 131.5).unwrap();
            lib.set_user_key(1, "5B").unwrap();
        }
        let mut reanalysis = banded_analysis();
        reanalysis.bpm = 90.0;
        reanalysis.key = "2A".into();
        old.set_analysis(1, &reanalysis).unwrap();
        assert!(staged_store(&new, 1, &reanalysis, analysis_stage::ALL));
        assert_eq!(stored(&old, 1), stored(&new, 1));
    }

    #[test]
    fn commit_refuses_when_the_user_edits_the_track_in_between() {
        let dir = tempfile::tempdir().unwrap();
        let lib = LocalLibrary::open(&dir.path().join("library.db")).unwrap();
        let id = lib.add_track(&make_track("Race", "/music/race.mp3")).unwrap();
        lib.set_analysis(id, &banded_analysis()).unwrap();
        let staged = lib.begin_analysis(id, &make_analysis(), analysis_stage::ALL).unwrap().stage().unwrap();
        lib.set_user_key(id, "8A").unwrap();
        let before = stored(&lib, id);
        assert!(!lib.commit_analysis(staged, None).unwrap());
        assert_eq!(stored(&lib, id), before);
        no_staging_left(&lib, id);
    }

    #[test]
    fn commit_of_a_removed_track_leaves_no_folder_behind() {
        let dir = tempfile::tempdir().unwrap();
        let lib = LocalLibrary::open(&dir.path().join("library.db")).unwrap();
        let id = lib.add_track(&make_track("Gone", "/music/gone.mp3")).unwrap();
        let write = lib.begin_analysis(id, &make_analysis(), analysis_stage::ALL).unwrap();
        lib.remove_track(id).unwrap();
        let staged = write.stage().unwrap();
        assert!(!lib.commit_analysis(staged, None).unwrap());
        assert!(!lib.track_anlz_dir(id).unwrap().exists());
    }

    #[test]
    fn storing_an_analysis_the_plain_way_never_shows_a_half_written_file() {
        let dir = tempfile::tempdir().unwrap();
        let lib = LocalLibrary::open(&dir.path().join("library.db")).unwrap();
        let id = lib.add_track(&make_track("Plain", "/music/plain.mp3")).unwrap();
        lib.set_analysis(id, &banded_analysis()).unwrap();
        let files: Vec<PathBuf> = [DAT, EXT, TWO_EX].iter().map(|name| lib.track_anlz_dir(id).unwrap().join(name)).collect();
        let sizes: Vec<u64> = files.iter().map(|f| std::fs::metadata(f).unwrap().len()).collect();
        let done = std::sync::atomic::AtomicBool::new(false);

        let torn = std::thread::scope(|scope| {
            let reader = scope.spawn(|| {
                let mut torn = Vec::new();
                while !done.load(Ordering::Relaxed) {
                    for (file, size) in files.iter().zip(&sizes) {
                        match std::fs::metadata(file) {
                            Ok(meta) if meta.len() == *size => {}
                            other => torn.push(format!("{}: {other:?}", file.display())),
                        }
                    }
                }
                torn
            });
            for _ in 0..150 {
                lib.set_analysis(id, &banded_analysis()).unwrap();
            }
            done.store(true, Ordering::Relaxed);
            reader.join().unwrap()
        });

        assert!(torn.is_empty(), "{} reads saw a missing or short file, first: {:?}", torn.len(), torn.first());
        no_staging_left(&lib, id);
    }

    #[test]
    fn commit_writes_the_track_row_with_the_analysis() {
        let dir = tempfile::tempdir().unwrap();
        let lib = LocalLibrary::open(&dir.path().join("library.db")).unwrap();
        let id = lib.add_track(&make_track("Row", "/music/row.mp3")).unwrap();
        let write = lib.begin_analysis(id, &make_analysis(), analysis_stage::ALL).unwrap();
        let mut track = write.track().unwrap().clone();
        track.key = "9B".into();
        assert!(lib.commit_analysis(write.stage().unwrap(), Some(&track)).unwrap());
        assert_eq!(lib.get_track(id).unwrap().unwrap().key, "9B");
        assert!(lib.get_analysis(id).unwrap().is_some());
        no_staging_left(&lib, id);
    }

    #[test]
    #[ignore = "needs the fixture library: FF_FIXTURE_LIBRARY=<dir with library.db and anlz/>"]
    fn fixture_library_files_are_byte_identical_both_ways() {
        let source = PathBuf::from(std::env::var("FF_FIXTURE_LIBRARY").expect("FF_FIXTURE_LIBRARY"));
        let limit: usize = std::env::var("FF_FIXTURE_TRACKS").ok().and_then(|n| n.parse().ok()).unwrap_or(300);
        let dir = tempfile::tempdir().unwrap();
        let copy = |name: &str| {
            let root = dir.path().join(name);
            std::fs::create_dir_all(root.join("anlz")).unwrap();
            for file in ["library.db", "library.db-wal", "library.db-shm"] {
                if source.join(file).exists() {
                    std::fs::copy(source.join(file), root.join(file)).unwrap();
                }
            }
            LocalLibrary::open(&root.join("library.db")).unwrap()
        };
        let (old, new) = (copy("old"), copy("new"));
        let ids: Vec<i64> = queries::select_analyzed_track_ids(&old.conn).unwrap().into_iter().take(limit).collect();
        for root in ["old", "new"] {
            for id in &ids {
                let from = source.join("anlz").join(id.to_string());
                let to = dir.path().join(root).join("anlz").join(id.to_string());
                std::fs::create_dir_all(&to).unwrap();
                for entry in std::fs::read_dir(&from).unwrap() {
                    let entry = entry.unwrap();
                    std::fs::copy(entry.path(), to.join(entry.file_name())).unwrap();
                }
            }
        }
        let (mut banded, mut edited) = (0, 0);
        for id in &ids {
            let analysis = old.get_analysis(*id).unwrap().unwrap();
            banded += usize::from(analysis.color_waveform.is_some());
            edited += usize::from(old.track_anlz_dir(*id).unwrap().join(EDITS_FILE).exists());
            let stages = old.analysis_stages(*id).unwrap().unwrap_or(analysis_stage::ALL);
            old.set_analysis_stages(*id, &analysis, stages).unwrap();
            assert!(staged_store(&new, *id, &analysis, stages));
            assert_eq!(stored(&old, *id), stored(&new, *id), "track {id}");
        }
        eprintln!("byte identical: {} tracks ({banded} with colour waveform, {edited} with edits.json)", ids.len());
    }
}
