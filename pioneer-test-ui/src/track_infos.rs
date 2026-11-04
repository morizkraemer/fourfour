use pioneer_library::{LocalLibrary, RowPreview, TrackLabelsReader, analysis_stage};

pub use pioneer_library::{effective_preview, row_bands};

use crate::dto::{LoadedState, PlaylistInput, RowPreviewInfo, TrackInfo, TrackLabelInfo, TrackRow, TrackTableBuilder};

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

pub fn loaded_state(lib: &LocalLibrary) -> Result<LoadedState, String> {
    let labels_reader = lib.track_labels_reader();
    let (playlists, mut labels) = std::thread::scope(|scope| {
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
    let mut table = TrackTableBuilder::default();
    let missing_previews = lib.for_each_track_list_row(|row| {
        let labels = labels.remove(&i64::from(row.id));
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
            rating: labels.as_ref().and_then(|l| l.rating),
            color: labels.as_ref().and_then(|l| l.color.as_deref()),
        });
    })
    .map_err(|e| e.to_string())?;
    if missing_previews > 0 {
        lib.backfill_row_previews().map_err(|e| e.to_string())?;
    }
    Ok(LoadedState { tracks: table.finish(), playlists })
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
    fn split_preview_sorts_band_and_mono_previews() {
        assert_eq!(split_preview(RowPreview::Bands(vec![1, 2])), (Some(vec![1, 2]), None));
        assert_eq!(split_preview(RowPreview::Mono(vec![3])), (None, Some(vec![3])));
        assert_eq!(split_preview(RowPreview::None), (None, None));
    }
}
