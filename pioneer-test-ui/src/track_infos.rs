use pioneer_library::{LocalLibrary, RowPreview};

pub use pioneer_library::{ROW_BAND_COLUMNS, effective_preview, row_bands};

use crate::dto::TrackInfo;

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
    let rows = lib.get_all_tracks_with_flags().map_err(|e| e.to_string())?;
    lib.backfill_row_previews().map_err(|e| e.to_string())?;
    let mut previews = lib.get_row_previews().map_err(|e| e.to_string())?;
    Ok(rows
        .into_iter()
        .map(|(track, has_artwork, has_analysis, has_cues)| {
            let (waveform_bands, waveform_preview) = match has_analysis
                .then(|| previews.remove(&i64::from(track.id)))
                .flatten()
            {
                Some(RowPreview::Bands(bands)) => (Some(bands), None),
                Some(RowPreview::Mono(mono)) => (None, Some(mono)),
                Some(RowPreview::None) | None => (None, None),
            };
            TrackInfo {
                id: track.id,
                source_path: track.source_path.to_string_lossy().to_string(),
                title: track.title,
                artist: track.artist,
                album: track.album,
                genre: track.genre,
                tempo: track.tempo,
                key: track.key,
                duration_secs: track.duration_secs,
                bitrate: track.bitrate,
                file_size: track.file_size,
                has_artwork,
                has_cues,
                has_analysis,
                waveform_preview,
                waveform_bands,
            }
        })
        .collect())
}

