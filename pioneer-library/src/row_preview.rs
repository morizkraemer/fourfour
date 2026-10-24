use pioneer_usb_writer::models::{AnalysisResult, ColorWaveform};

/// Columns in the 3-band row preview that `load_state` and `get_analysis_data` send.
pub const ROW_BAND_COLUMNS: usize = 128;

pub const MONO_PREVIEW_BYTES: usize = 400;

/// The row preview: the `PWV6` overview (else the detail) averaged down to
/// `ROW_BAND_COLUMNS` columns of interleaved low, mid, high bytes. `None` without band data.
pub fn row_bands(cw: &ColorWaveform) -> Option<Vec<u8>> {
    let source = if cw.overview.is_empty() { &cw.detail } else { &cw.overview };
    if source.is_empty() {
        return None;
    }
    let n = source.len();
    let mut out = Vec::with_capacity(ROW_BAND_COLUMNS * 3);
    for i in 0..ROW_BAND_COLUMNS {
        let start = i * n / ROW_BAND_COLUMNS;
        let end = ((i + 1) * n / ROW_BAND_COLUMNS).max(start + 1).min(n);
        let window = &source[start.min(n - 1)..end];
        for band in 0..3 {
            let sum: u32 = window.iter().map(|c| u32::from(c[band])).sum();
            out.push(((sum + window.len() as u32 / 2) / window.len() as u32) as u8);
        }
    }
    Some(out)
}

/// Build a 400-byte mono PWAV preview from a color waveform's per-sample band
/// peaks. Used when a track's stored PWAV section is degenerate but its color
/// data is intact, so the list mini-waveform matches what the player renders.
/// Byte layout matches PWAV: bits 0-4 height (0-31), bits 5-7 whiteness (0-7).
fn mono_preview_from_color(cw: &ColorWaveform) -> Option<[u8; MONO_PREVIEW_BYTES]> {
    let src = if !cw.overview.is_empty() {
        &cw.overview
    } else if !cw.detail.is_empty() {
        &cw.detail
    } else {
        return None;
    };

    let amps: Vec<u8> = src.iter().map(|[l, m, h]| (*l).max(*m).max(*h)).collect();
    let max = amps.iter().copied().max().unwrap_or(0);
    if max == 0 {
        return None;
    }

    let n = amps.len();
    let mut out = [0u8; MONO_PREVIEW_BYTES];
    for (i, byte) in out.iter_mut().enumerate() {
        let start = i * n / MONO_PREVIEW_BYTES;
        let end = ((i + 1) * n / MONO_PREVIEW_BYTES).max(start + 1).min(n);
        let bucket = amps[start..end].iter().copied().max().unwrap_or(0) as u32;
        let height = ((bucket * 31) / 127).min(31) as u8;
        let whiteness = ((bucket * 7) / 127).min(7) as u8;
        *byte = (whiteness << 5) | (height & 0x1F);
    }
    Some(out)
}

/// The mono PWAV preview to show, shared by the list and the player so both
/// surfaces stay in sync: the real DAT preview when it carries signal, else a
/// preview synthesized from the color waveform, else `None` (no waveform data).
pub fn effective_preview(a: &AnalysisResult) -> Option<Vec<u8>> {
    if a.waveform.data.iter().any(|&b| b & 0x1f != 0) {
        Some(a.waveform.data.to_vec())
    } else {
        a.color_waveform
            .as_ref()
            .and_then(mono_preview_from_color)
            .map(|d| d.to_vec())
    }
}

/// What the list shows for one track, as stored in `analyses.row_preview`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowPreview {
    Bands(Vec<u8>),
    Mono(Vec<u8>),
    None,
}

impl RowPreview {
    pub fn from_analysis(a: &AnalysisResult) -> Self {
        if let Some(bands) = a.color_waveform.as_ref().and_then(row_bands) {
            return RowPreview::Bands(bands);
        }
        match effective_preview(a) {
            Some(mono) => RowPreview::Mono(mono),
            None => RowPreview::None,
        }
    }

    /// Bands are `ROW_BAND_COLUMNS * 3` bytes, mono is 400, an empty blob means
    /// "checked, nothing to show" so it is not read from the ANLZ files again.
    pub fn to_blob(&self) -> Vec<u8> {
        match self {
            RowPreview::Bands(b) | RowPreview::Mono(b) => b.clone(),
            RowPreview::None => Vec::new(),
        }
    }

    pub fn from_blob(blob: Vec<u8>) -> Self {
        match blob.len() {
            n if n == ROW_BAND_COLUMNS * 3 => RowPreview::Bands(blob),
            n if n == MONO_PREVIEW_BYTES => RowPreview::Mono(blob),
            _ => RowPreview::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blob_round_trip() {
        let bands = RowPreview::Bands(vec![7; ROW_BAND_COLUMNS * 3]);
        assert_eq!(RowPreview::from_blob(bands.to_blob()), bands);
        let mono = RowPreview::Mono(vec![9; MONO_PREVIEW_BYTES]);
        assert_eq!(RowPreview::from_blob(mono.to_blob()), mono);
        assert_eq!(RowPreview::from_blob(RowPreview::None.to_blob()), RowPreview::None);
    }
}
