use std::collections::HashMap;
use std::ops::RangeInclusive;

use pioneer_library::{LocalLibrary, RowPreview, TrackLabels, TrackLabelsReader, TrackListRowRef, analysis_stage};

pub use pioneer_library::{effective_preview, row_bands};

use crate::dto::{LoadedState, PlaylistInput, RowPreviewInfo, TrackInfo, TrackLabelInfo, TrackRow, TrackTable, TrackTableBuilder};

/// Build TrackInfo list from the library's tracks + flags.
///
/// Embeds the row preview so the list paints mini waveforms without an N+1
/// fetch. The preview comes from the `analyses.row_preview` column (no ANLZ file
/// reads); tracks stored before that column existed are filled once, here.
/// Tracks without a real preview (the all-zero placeholder, missing ANLZ
/// files) get none, otherwise the UI would cache a flat waveform as if it were
/// real. When the DAT's PWAV section is degenerate but color data exists, the
/// preview is synthesized from the color overview so the list mini-waveform
/// stays consistent with the player strip — see #waveform-bug.
pub fn build_track_infos(lib: &LocalLibrary) -> Result<Vec<TrackInfo>, String> {
    lib.backfill_row_previews().map_err(|e| e.to_string())?;
    track_infos(lib, true)
}

fn track_infos(lib: &LocalLibrary, with_previews: bool) -> Result<Vec<TrackInfo>, String> {
    let labels_reader = lib.track_labels_reader();
    let (rows, mut labels) = std::thread::scope(|scope| {
        let labels = scope.spawn(|| labels_reader.read_cached());
        let rows = if with_previews { lib.get_track_list_rows() } else { lib.get_track_list_rows_without_previews() };
        (rows, labels.join().expect("track label reader panicked"))
    });
    let rows = rows.map_err(|e| e.to_string())?;
    Ok(rows
        .into_iter()
        .map(|row| {
            let (waveform_bands, waveform_preview) = row.row_preview.map_or((None, None), split_preview);
            let labels = labels.remove(&i64::from(row.id));
            TrackInfo {
                id: row.id,
                source_path: row.source_path,
                title: row.title,
                artist: row.artist,
                album: row.album,
                genre: row.genre,
                tempo: row.tempo,
                key: row.key,
                duration_secs: row.duration_secs,
                bitrate: row.bitrate,
                file_size: row.file_size,
                has_artwork: row.has_artwork,
                has_cues: false,
                has_analysis: row.analysis_stages.is_some(),
                analysis_pending: row.analysis_stages.is_some_and(|stages| stages != analysis_stage::ALL),
                waveform_preview,
                waveform_bands,
                rating: labels.as_ref().and_then(|l| l.rating),
                color: labels.and_then(|l| l.color),
            }
        })
        .collect())
}

fn split_preview(preview: RowPreview) -> (Option<Vec<u8>>, Option<Vec<u8>>) {
    match preview {
        RowPreview::Bands(bands) => (Some(bands), None),
        RowPreview::Mono(mono) => (None, Some(mono)),
        RowPreview::None => (None, None),
    }
}

/// The track list without row previews: the list fetches them for the rows on screen with [`row_preview_infos`].
pub fn list_track_infos(lib: &LocalLibrary) -> Result<Vec<TrackInfo>, String> {
    lib.backfill_row_previews().map_err(|e| e.to_string())?;
    track_infos(lib, false)
}

pub fn row_preview_infos(lib: &LocalLibrary, track_ids: &[u32]) -> Result<Vec<RowPreviewInfo>, String> {
    let ids: Vec<i64> = track_ids.iter().map(|&id| i64::from(id)).collect();
    Ok(lib
        .get_row_previews_of(&ids)
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter_map(|(id, preview)| {
            let (waveform_bands, waveform_preview) = split_preview(preview);
            (waveform_bands.is_some() || waveform_preview.is_some()).then(|| RowPreviewInfo {
                id: id as u32,
                waveform_bands,
                waveform_preview,
            })
        })
        .collect())
}

/// Ratings and colours that changed in the sidecar files since the labels `load_state` served were cached.
pub fn changed_track_labels(reader: &TrackLabelsReader) -> Vec<TrackLabelInfo> {
    reader
        .refresh()
        .into_iter()
        .map(|(id, labels)| TrackLabelInfo {
            id: id as u32,
            rating: labels.as_ref().and_then(|l| l.rating),
            color: labels.and_then(|l| l.color),
        })
        .collect()
}

const PARALLEL_SCAN_MIN_IDS: i64 = 4_000;
const MAX_SCAN_THREADS: usize = 4;

pub fn loaded_state(lib: &LocalLibrary) -> Result<LoadedState, String> {
    let labels_reader = lib.track_labels_reader();
    let (playlists, labels) = std::thread::scope(|scope| {
        let labels = scope.spawn(|| labels_reader.read_cached());
        let playlists = lib.get_all_playlists();
        (playlists, labels.join().expect("track label reader panicked"))
    });
    let playlists = playlists
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|pl| PlaylistInput {
            id: pl.id,
            name: pl.name,
            track_ids: pl.track_ids,
            parent_id: pl.parent_id,
            is_folder: pl.is_folder,
        })
        .collect();
    let (tracks, missing_previews) = track_table(lib, &labels)?;
    if missing_previews > 0 {
        lib.backfill_row_previews().map_err(|e| e.to_string())?;
    }
    Ok(LoadedState { tracks, playlists })
}

/// The rows of the whole library as a table, and how many tracks still lack a row preview. A large library is
/// scanned in id ranges on several threads (the first on `lib`'s connection, the others on their own).
fn track_table(lib: &LocalLibrary, labels: &HashMap<i64, TrackLabels>) -> Result<(TrackTable, usize), String> {
    let Some((first_id, last_id)) = lib.track_id_bounds().map_err(|e| e.to_string())? else {
        return Ok((TrackTable::default(), 0));
    };
    let threads = if lib.track_list_scanner().is_some() && last_id - first_id >= PARALLEL_SCAN_MIN_IDS {
        std::thread::available_parallelism().map_or(1, usize::from).min(MAX_SCAN_THREADS)
    } else {
        1
    };
    track_table_in_ranges(lib, labels, id_ranges(first_id, last_id, threads))
}

/// Ranges of equal width that cover every id: the first starts at the lowest and the last ends at the highest
/// possible id, so that tracks added since `first_id` and `last_id` were read are not lost.
fn id_ranges(first_id: i64, last_id: i64, count: usize) -> Vec<RangeInclusive<i64>> {
    let count = count as i64;
    let width = (last_id - first_id) / count + 1;
    (0..count)
        .map(|part| {
            let start = if part == 0 { i64::MIN } else { first_id + part * width };
            let end = if part + 1 == count { i64::MAX } else { first_id + (part + 1) * width - 1 };
            start..=end
        })
        .collect()
}

/// The tables of the ranges are joined in id order, which gives the table that one scan would.
fn track_table_in_ranges(
    lib: &LocalLibrary,
    labels: &HashMap<i64, TrackLabels>,
    ranges: Vec<RangeInclusive<i64>>,
) -> Result<(TrackTable, usize), String> {
    let scanner = lib.track_list_scanner();
    std::thread::scope(|scope| {
        let others: Vec<_> = ranges[1..]
            .iter()
            .map(|ids| {
                let scanner = scanner.as_ref().expect("several ranges need a scanner");
                scope.spawn(move || {
                    let (builder, missing) = scan_rows(labels, |visit| scanner.for_each_track_list_row(ids.clone(), visit))?;
                    Ok::<_, String>((builder.finish(), missing))
                })
            })
            .collect();
        let (mut table, mut missing) = scan_rows(labels, |visit| lib.for_each_track_list_row(ranges[0].clone(), visit))?;
        for other in others {
            let (part, part_missing) = other.join().expect("track scan thread panicked")?;
            table.append(part);
            missing += part_missing;
        }
        Ok((table.finish(), missing))
    })
}

fn scan_rows(
    labels: &HashMap<i64, TrackLabels>,
    scan: impl FnOnce(&mut dyn FnMut(TrackListRowRef<'_>)) -> anyhow::Result<usize>,
) -> Result<(TrackTableBuilder, usize), String> {
    let mut table = TrackTableBuilder::default();
    let missing_previews = scan(&mut |row| {
        let labels = labels.get(&i64::from(row.id));
        table.push(&TrackRow {
            id: row.id,
            source_path: row.source_path,
            title: row.title,
            artist: row.artist,
            album: row.album,
            genre: row.genre,
            key: row.key,
            tempo: row.tempo,
            duration_secs: row.duration_secs,
            bitrate: row.bitrate,
            file_size: row.file_size,
            has_artwork: row.has_artwork,
            has_cues: false,
            has_analysis: row.analysis_stages.is_some(),
            analysis_pending: row.analysis_stages.is_some_and(|stages| stages != analysis_stage::ALL),
            rating: labels.and_then(|l| l.rating),
            color: labels.and_then(|l| l.color.as_deref()),
        });
    })
    .map_err(|e| e.to_string())?;
    Ok((table, missing_previews))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pioneer_library::analysis_stage;
    use pioneer_usb_writer::models::{AnalysisResult, Beat, BeatGrid, Track, WaveformPreview};
    use std::path::PathBuf;

    fn track(path: &str, title: &str) -> Track {
        Track {
            id: 0,
            source_path: PathBuf::from(path),
            usb_path: String::new(),
            title: title.into(),
            artist: "Artist".into(),
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
            duration_secs: 200.0,
            sample_rate: 44100,
            bitrate: 320,
            file_size: 1,
            artwork: None,
        }
    }

    fn analysis() -> AnalysisResult {
        AnalysisResult {
            beat_grid: BeatGrid { beats: vec![Beat { bar_position: 1, time_ms: 0, tempo: 12800 }] },
            waveform: WaveformPreview { data: [9u8; 400] },
            bpm: 128.0,
            key: "8A".into(),
            energy: None,
            cue_points: Vec::new(),
            color_waveform: None,
        }
    }

    fn library() -> (LocalLibrary, [i64; 3]) {
        let lib = LocalLibrary::open_in_memory().unwrap();
        let fresh = lib.add_track(&track("/m/fresh.mp3", "Fresh")).unwrap();
        let done = lib.add_track(&track("/m/done.mp3", "Done")).unwrap();
        let partial = lib.add_track(&track("/m/partial.mp3", "Partial")).unwrap();
        lib.set_analysis(done, &analysis()).unwrap();
        lib.set_analysis_stages(partial, &analysis(), analysis_stage::WAVEFORM).unwrap();
        (lib, [fresh, done, partial])
    }

    #[test]
    fn rows_tell_unanalyzed_pending_and_finished_tracks_apart() {
        let (lib, [fresh, done, partial]) = library();
        let infos = list_track_infos(&lib).unwrap();
        let by_id = |id: i64| infos.iter().find(|t| i64::from(t.id) == id).unwrap();

        assert_eq!(infos.len(), 3);
        assert_eq!((by_id(fresh).has_analysis, by_id(fresh).analysis_pending), (false, false));
        assert_eq!((by_id(done).has_analysis, by_id(done).analysis_pending), (true, false));
        assert_eq!((by_id(partial).has_analysis, by_id(partial).analysis_pending), (true, true));
        assert_eq!(by_id(done).title, "Done");
        assert_eq!(by_id(done).source_path, PathBuf::from("/m/done.mp3"));
    }

    #[test]
    fn the_list_and_the_full_build_hold_the_same_tracks() {
        let (lib, _) = library();
        let listed = list_track_infos(&lib).unwrap();
        let built: Vec<u32> = build_track_infos(&lib).unwrap().iter().map(|t| t.id).collect();
        assert_eq!(listed.iter().map(|t| t.id).collect::<Vec<_>>(), built);
        assert!(listed.iter().all(|t| t.waveform_bands.is_none() && t.waveform_preview.is_none()));
    }

    #[test]
    fn row_previews_come_back_only_for_tracks_that_have_one() {
        let (lib, [fresh, ..]) = library();
        let asked: Vec<u32> = list_track_infos(&lib).unwrap().iter().map(|t| t.id).collect();
        let previews = row_preview_infos(&lib, &asked).unwrap();
        assert!(previews.iter().all(|p| p.waveform_bands.is_some() || p.waveform_preview.is_some()));
        assert!(previews.iter().all(|p| i64::from(p.id) != fresh));
        assert!(row_preview_infos(&lib, &[]).unwrap().is_empty());
    }

    #[test]
    fn loaded_state_carries_playlists_with_their_tracks_and_folders() {
        let (lib, [fresh, done, _]) = library();
        let folder = lib.create_folder("Sets", 0).unwrap();
        let list = lib.create_playlist_in("Friday", folder as u32).unwrap();
        lib.set_playlist_tracks(list, &[done, fresh]).unwrap();

        let state = loaded_state(&lib).unwrap();
        let friday = state.playlists.iter().find(|p| p.name == "Friday").unwrap();
        let sets = state.playlists.iter().find(|p| p.name == "Sets").unwrap();

        assert_eq!(state.tracks.len(), 3);
        assert_eq!(friday.track_ids, vec![done as u32, fresh as u32]);
        assert_eq!((friday.parent_id, friday.is_folder), (folder as u32, false));
        assert!(sets.is_folder);
    }

    #[test]
    fn id_ranges_cover_every_id_once() {
        assert_eq!(id_ranges(5, 5, 1), vec![i64::MIN..=i64::MAX]);
        assert_eq!(id_ranges(1, 10, 3), vec![i64::MIN..=4, 5..=8, 9..=i64::MAX]);
        assert_eq!(id_ranges(100, 103, 4), vec![i64::MIN..=100, 101..=101, 102..=102, 103..=i64::MAX]);
    }

    #[test]
    fn scanning_in_ranges_gives_the_table_of_one_scan() {
        let dir = std::env::temp_dir().join(format!("ff-track-ranges-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let lib = LocalLibrary::open(&dir.join("library.db")).unwrap();
        let mut ids = Vec::new();
        for n in 0..40 {
            let mut t = track(&format!("/m/artist {}/album {}/{n}.mp3", n % 7, n % 3), &format!("Title {n}"));
            t.artist = format!("Artist {}", n % 7);
            t.album = format!("Album {}", n % 3);
            t.genre = ["House", "", "Techno"][n % 3].into();
            t.tempo = 9000 + n as u32;
            t.key = if n % 4 == 0 { "5A".into() } else { String::new() };
            ids.push(lib.add_track(&t).unwrap());
        }
        for &id in ids.iter().step_by(3) {
            lib.set_analysis(id, &analysis()).unwrap();
        }
        lib.set_analysis_stages(ids[1], &analysis(), analysis_stage::WAVEFORM).unwrap();
        let labels: HashMap<i64, TrackLabels> = ids
            .iter()
            .enumerate()
            .filter(|(n, _)| n % 5 != 2)
            .map(|(n, &id)| {
                let color = (n % 2 == 0).then(|| ["#ff0000", "#00ff00"][n % 4 / 2].to_string());
                (id, TrackLabels { rating: (n % 3 != 0).then_some((n % 6) as u8), color })
            })
            .collect();
        let (first, last) = lib.track_id_bounds().unwrap().unwrap();

        let json = |ranges| {
            let (table, missing) = track_table_in_ranges(&lib, &labels, ranges).unwrap();
            (serde_json::to_string(&table).unwrap(), missing)
        };
        let whole = json(id_ranges(first, last, 1));
        assert!(whole.0.contains("#00ff00") && whole.0.contains("\"ratings\":[1,"));
        for threads in [2, 3, 7] {
            assert_eq!(json(id_ranges(first, last, threads)), whole, "{threads} ranges");
        }
        assert_eq!(whole.1, 0);
        drop(lib);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn split_preview_sorts_band_and_mono_previews() {
        assert_eq!(split_preview(RowPreview::Bands(vec![1, 2])), (Some(vec![1, 2]), None));
        assert_eq!(split_preview(RowPreview::Mono(vec![3])), (None, Some(vec![3])));
        assert_eq!(split_preview(RowPreview::None), (None, None));
    }
}
