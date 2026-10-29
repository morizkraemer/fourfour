use pioneer_library::{LocalLibrary, RowPreview, analysis_stage};

pub use pioneer_library::{effective_preview, row_bands};

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
            let (waveform_bands, waveform_preview) = match row.row_preview {
                Some(RowPreview::Bands(bands)) => (Some(bands), None),
                Some(RowPreview::Mono(mono)) => (None, Some(mono)),
                Some(RowPreview::None) | None => (None, None),
            };
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
