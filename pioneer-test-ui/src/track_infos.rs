use pioneer_library::{LocalLibrary, RowPreview, analysis_stage};

pub use pioneer_library::{effective_preview, row_bands};

use crate::dto::{LoadedState, PlaylistInput, RowPreviewInfo, TrackInfo};

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
    let labels_reader = lib.track_labels_reader();
    let (rows, mut labels) = std::thread::scope(|scope| {
        let labels = scope.spawn(|| labels_reader.read_all());
        let rows = lib.get_track_list_rows();
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
    let mut tracks = build_track_infos(lib)?;
    for track in &mut tracks {
        track.waveform_bands = None;
        track.waveform_preview = None;
    }
    Ok(tracks)
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

pub fn loaded_state(lib: &LocalLibrary) -> Result<LoadedState, String> {
    let tracks = list_track_infos(lib)?;
    let playlists = lib
        .get_all_playlists()
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
    Ok(LoadedState { tracks, playlists })
}
